---
title: "Market catalog and data layer — implementation plan"
description: "Task-by-task plan building the vox-market crate: contract-driven catalog schema, evidence-class reconciliation, append-only store, eBay adapter, TTL scheduler, constraint filtering with explainable scoring, vox-search corpus, and the vox market CLI."
category: "Architecture SSOTs"
---

# Market Catalog and Data Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `vox-market` — a contract-driven catalog of purchasable items with provenance-tracked attributes, live price fetching, and constraint filtering with explainable ranking — so `vox market` can answer "which laptops have ≥64 GB of GPU-accessible memory" correctly and say why others were excluded.

**Architecture:** A new L1 leaf crate. Categories and attributes are defined in a YAML contract, not Rust types, so adding a category is a config edit. Every attribute value carries provenance and an `EvidenceClass`; when sources disagree the highest class wins and losers are retained as alternates. Observations are append-only; a materialized `market_current` table holds reconciliation winners. Search reuses `vox-search` as a corpus rather than a bespoke index.

**Tech Stack:** Rust, `serde` + `serde_yaml`, `vox-db` (Turso/libSQL), `vox-http-client`, `vox-secrets`, `vox-search`, `tokio`.

**Spec:** [`docs/superpowers/specs/2026-08-21-market-data-layer-design.md`](../specs/2026-08-21-market-data-layer-design.md)

**Scope note:** This plan covers the spec through its acceptance test. The **discovery pipeline** (spec §Discovery) is deliberately excluded — it is flag-gated on this plan's acceptance test passing, and becomes a separate plan.

## Global Constraints

- **New crate needs a layer assignment.** Add `"vox-market": 1` to `contracts/ci/crate-layers.v1.json` in Task 1. Dependencies point same-layer or down.
- **Crate-edge exceptions are USER-AUTHORIZED-ONLY.** `vox-market` introduces two new L4→L1 edges (`vox-gui`→`vox-market`, `vox-cli`→`vox-market`). Per AGENTS.md you must **propose** these in the PR description and stop — never write an `exceptions` entry or regenerate `crate-edges.allow.v1.json` yourself. Tightening (`vox ci crate-edges --tighten`) is always allowed.
- **Test-first is enforced.** Every new `pub fn` in `crates/*/src/**` requires a test in the same file before the commit lands (`skeleton/untested-pub-api`, pre-commit `tdd-guard`).
- **No direct secret reads.** Credentials resolve through `vox_secrets::resolve_secret(...)`. A new `env::var` for an API key will fail `vox ci secret-env-guard`.
- **Formatting:** `vox run scripts/fmt.vox`. **Never** `cargo fmt --all` — it overflows the Windows command line (os error 206).
- **Build with `-j 4` on a ≤16 GB host.** `.cargo/config.toml` sets `jobs = 24`, which OOMs a 16 GB machine and produces misleading "cannot find trait `Extend`" errors.
- **No LLM calls outside the facade.** If any task reaches for a model, it goes through `vox_actor_runtime::llm`.

---

### Task 1: Crate scaffold and catalog schema contract

**Files:**
- Create: `crates/vox-market/Cargo.toml`
- Create: `crates/vox-market/src/lib.rs`
- Create: `crates/vox-market/src/schema.rs`
- Create: `contracts/market/catalog-schema.v1.yaml`
- Modify: `Cargo.toml` (workspace `members`)
- Modify: `contracts/ci/crate-layers.v1.json`

**Interfaces:**
- Consumes: nothing.
- Produces: `CatalogSchema::load_from_str(&str) -> Result<CatalogSchema, SchemaError>`, `CatalogSchema::attribute(&self, category: &str, name: &str) -> Option<&AttributeDef>`, `AttributeDef { kind: AttrKind, ty: AttrType, unit: Option<String>, required: bool }`, `AttrKind { Spec, Price, Stock, Availability }`, `AttrKind::ttl_hours(&self) -> Option<f64>`.

- [ ] **Step 1: Write the failing test**

In `crates/vox-market/src/schema.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
schema_version: 1
attribute_kinds:
  spec:  { ttl_hours: null }
  price: { ttl_hours: 6 }
universal_attributes:
  price_usd: { kind: price, type: number, unit: USD, required: true }
categories:
  laptop:
    attributes:
      gpu_accessible_gb: { kind: spec, type: number, unit: GB, required: true }
"#;

    #[test]
    fn universal_attributes_apply_to_every_category() {
        let s = CatalogSchema::load_from_str(MINIMAL).expect("parse");
        // price_usd is declared once, globally, but resolves per category.
        let p = s.attribute("laptop", "price_usd").expect("price_usd on laptop");
        assert_eq!(p.kind, AttrKind::Price);
        assert_eq!(p.unit.as_deref(), Some("USD"));
        assert!(p.required);
    }

    #[test]
    fn spec_attributes_never_expire_but_prices_do() {
        let s = CatalogSchema::load_from_str(MINIMAL).expect("parse");
        assert_eq!(s.attribute("laptop", "gpu_accessible_gb").unwrap().kind.ttl_hours(), None);
        assert_eq!(s.attribute("laptop", "price_usd").unwrap().kind.ttl_hours(), Some(6.0));
    }

    #[test]
    fn unknown_category_or_attribute_is_none_not_panic() {
        let s = CatalogSchema::load_from_str(MINIMAL).expect("parse");
        assert!(s.attribute("gpu", "vram_gb").is_none());
        assert!(s.attribute("laptop", "nonexistent").is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-market schema`
Expected: FAIL — `cannot find type CatalogSchema`.

- [ ] **Step 3: Write minimal implementation**

`crates/vox-market/Cargo.toml`:

```toml
[package]
name = "vox-market"
version.workspace = true
edition.workspace = true
description = "Catalog of purchasable items with provenance-tracked attributes and live pricing."

[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_yaml = { workspace = true }
thiserror = { workspace = true }
```

`crates/vox-market/src/schema.rs`:

```rust
//! Catalog schema. Categories and attributes are defined in
//! `contracts/market/catalog-schema.v1.yaml`, not in Rust, so adding a
//! category is a config edit rather than a code change and a migration.

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("schema parse failed: {0}")]
    Parse(String),
    #[error("attribute {attr} references unknown kind {kind}")]
    UnknownKind { attr: String, kind: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttrKind {
    Spec,
    Price,
    Stock,
    Availability,
}

impl AttrKind {
    /// `None` means "never expires". A spec is immutable: an A6000 is always
    /// 48 GB. Prices, stock and availability decay at very different rates, so
    /// a single TTL for the whole record would be wrong in both directions.
    pub fn ttl_hours(&self) -> Option<f64> {
        match self {
            AttrKind::Spec => None,
            AttrKind::Price => Some(6.0),
            AttrKind::Stock => Some(0.25),
            AttrKind::Availability => Some(6.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttrType {
    Number,
    Text,
    Enum,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttributeDef {
    pub kind: AttrKind,
    #[serde(rename = "type")]
    pub ty: AttrType,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CategoryDef {
    #[serde(default)]
    attributes: BTreeMap<String, AttributeDef>,
}

#[derive(Debug, Deserialize)]
pub struct CatalogSchema {
    pub schema_version: u32,
    #[serde(default)]
    universal_attributes: BTreeMap<String, AttributeDef>,
    #[serde(default)]
    categories: BTreeMap<String, CategoryDef>,
}

impl CatalogSchema {
    pub fn load_from_str(yaml: &str) -> Result<Self, SchemaError> {
        serde_yaml::from_str(yaml).map_err(|e| SchemaError::Parse(e.to_string()))
    }

    /// Resolve an attribute for a category. Universal attributes are visible on
    /// every category; a category-specific attribute of the same name wins.
    pub fn attribute(&self, category: &str, name: &str) -> Option<&AttributeDef> {
        let cat = self.categories.get(category)?;
        cat.attributes
            .get(name)
            .or_else(|| self.universal_attributes.get(name))
    }

    pub fn categories(&self) -> impl Iterator<Item = &str> {
        self.categories.keys().map(String::as_str)
    }

    /// Attribute names that must be present for an item of this category.
    pub fn required_attributes(&self, category: &str) -> Vec<&str> {
        let Some(cat) = self.categories.get(category) else {
            return Vec::new();
        };
        cat.attributes
            .iter()
            .chain(self.universal_attributes.iter())
            .filter(|(_, d)| d.required)
            .map(|(n, _)| n.as_str())
            .collect()
    }
}
```

`crates/vox-market/src/lib.rs`:

```rust
//! Market catalog and data layer. See
//! `docs/superpowers/specs/2026-08-21-market-data-layer-design.md`.
pub mod schema;
```

Write `contracts/market/catalog-schema.v1.yaml` with the full contract from the spec §"The catalog schema is a contract, not Rust types" (both `gpu` and `laptop` categories, all four `attribute_kinds`, all four `universal_attributes`).

Add `"crates/vox-market"` to the workspace `members` in the root `Cargo.toml`, and `"vox-market": 1` to `contracts/ci/crate-layers.v1.json`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -j 4 -p vox-market schema`
Expected: PASS, 3 tests.

- [ ] **Step 5: Verify the shipped contract parses**

Add to `crates/vox-market/src/schema.rs` tests:

```rust
#[test]
fn shipped_contract_parses_and_declares_both_categories() {
    let yaml = include_str!("../../../contracts/market/catalog-schema.v1.yaml");
    let s = CatalogSchema::load_from_str(yaml).expect("shipped contract must parse");
    let cats: Vec<&str> = s.categories().collect();
    assert!(cats.contains(&"gpu"), "got {cats:?}");
    assert!(cats.contains(&"laptop"), "got {cats:?}");
    // The field is deliberately gpu_accessible_gb, not vram_gb: a 64GB laptop
    // exposes only ~48GB to the GPU, so the name encodes the real question.
    assert!(s.attribute("laptop", "gpu_accessible_gb").is_some());
    assert!(s.attribute("laptop", "vram_gb").is_none());
}
```

Run: `cargo test -j 4 -p vox-market schema`
Expected: PASS, 4 tests.

- [ ] **Step 6: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-market contracts/market Cargo.toml contracts/ci/crate-layers.v1.json
git commit -m "feat(market): contract-driven catalog schema

Categories and attributes live in contracts/market/catalog-schema.v1.yaml
so adding a category is a config edit, not a code change plus a store
migration. TTL is a property of the attribute kind: specs never expire,
prices are hours, stock is minutes.

Laptops use gpu_accessible_gb rather than vram_gb because unified-memory
machines reserve ~25% for the OS, so a 64GB laptop offers ~48GB and would
wrongly satisfy a >=64GB filter."
```

---

### Task 2: Provenance, evidence classes, and reconciliation

**Files:**
- Create: `crates/vox-market/src/attribute.rs`
- Create: `crates/vox-market/src/reconcile.rs`
- Modify: `crates/vox-market/src/lib.rs`

**Interfaces:**
- Consumes: `AttrKind` from Task 1.
- Produces: `EvidenceClass { Aggregator, SearchIndex, MerchantPage, Transactable }` (ordered, `Ord`), `AttrValue { Number(f64), Text(String) }`, `Attribute { value: AttrValue, unit: Option<String>, source_id: String, source_url: Option<String>, observed_at_ms: i64, evidence: EvidenceClass }`, `reconcile(observations: &[Attribute]) -> Option<Reconciled>`, `Reconciled { winner: Attribute, alternates: Vec<Attribute> }`.

- [ ] **Step 1: Write the failing test**

In `crates/vox-market/src/reconcile.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::{AttrValue, Attribute, EvidenceClass};

    fn attr(price: f64, ev: EvidenceClass, at: i64, src: &str) -> Attribute {
        Attribute {
            value: AttrValue::Number(price),
            unit: Some("USD".into()),
            source_id: src.into(),
            source_url: None,
            observed_at_ms: at,
            evidence: ev,
        }
    }

    /// The real 2026-08-21 conflict. A tracker said $3,999, a fetched listing
    /// with stock said $3,599.99, a third source said $4,799.99. Ranking on
    /// price alone picks $3,599.99 by luck; ranking on evidence picks it
    /// because it is the only transactable one.
    #[test]
    fn transactable_wins_even_when_it_is_not_the_cheapest() {
        let obs = vec![
            attr(3999.00, EvidenceClass::Aggregator, 100, "gpudojo"),
            attr(3599.99, EvidenceClass::Transactable, 90, "ebay"),
            attr(4799.99, EvidenceClass::MerchantPage, 110, "newegg"),
        ];
        let r = reconcile(&obs).expect("some winner");
        assert_eq!(r.winner.value, AttrValue::Number(3599.99));
        assert_eq!(r.winner.source_id, "ebay");
        assert_eq!(r.alternates.len(), 2, "losers are retained, never discarded");
    }

    /// A cheaper aggregator figure must never displace a verified one, but it
    /// must stay visible so a human can decide to phone the vendor.
    #[test]
    fn cheaper_weak_evidence_is_retained_as_an_alternate() {
        let obs = vec![
            attr(660.20, EvidenceClass::SearchIndex, 100, "compsource"),
            attr(710.00, EvidenceClass::Transactable, 100, "rackmountnet"),
        ];
        let r = reconcile(&obs).unwrap();
        assert_eq!(r.winner.value, AttrValue::Number(710.00));
        assert!(r
            .alternates
            .iter()
            .any(|a| a.value == AttrValue::Number(660.20)));
    }

    #[test]
    fn same_class_breaks_on_recency() {
        let obs = vec![
            attr(100.0, EvidenceClass::MerchantPage, 100, "a"),
            attr(200.0, EvidenceClass::MerchantPage, 500, "b"),
        ];
        assert_eq!(reconcile(&obs).unwrap().winner.source_id, "b");
    }

    #[test]
    fn empty_input_yields_no_winner() {
        assert!(reconcile(&[]).is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-market reconcile`
Expected: FAIL — `cannot find function reconcile`.

- [ ] **Step 3: Write minimal implementation**

`crates/vox-market/src/attribute.rs`:

```rust
//! Attribute values and their provenance.
//!
//! A bare number cannot distinguish "$3,999 from a price tracker" from
//! "$3,599.99 from a listing with 5 in stock". Storing both identically is
//! what nearly produced a wrong $1,200 decision during the manual pricing
//! pass on 2026-08-21, so every value carries where it came from and how
//! strong that evidence is.

use serde::{Deserialize, Serialize};

/// How much a source's claim can be trusted. **Ordering IS the precedence
/// rule** — `Ord` is derived deliberately and `reconcile` depends on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EvidenceClass {
    /// Price tracker or aggregator; no merchant page was loaded.
    Aggregator = 1,
    /// Search-result snippet; the page itself was never fetched.
    SearchIndex = 2,
    /// Merchant page fetched, but no stock text present.
    MerchantPage = 3,
    /// Merchant page fetched with add-to-cart live AND stock stated.
    Transactable = 4,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AttrValue {
    Number(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attribute {
    pub value: AttrValue,
    pub unit: Option<String>,
    pub source_id: String,
    pub source_url: Option<String>,
    pub observed_at_ms: i64,
    pub evidence: EvidenceClass,
}

impl Attribute {
    /// True when this value is too weakly evidenced to win a ranking.
    /// Used by the scorer: an unverified attribute may be displayed, but it
    /// must not outrank a verified competitor.
    pub fn is_weak(&self) -> bool {
        self.evidence < EvidenceClass::MerchantPage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_classes_order_from_weakest_to_strongest() {
        assert!(EvidenceClass::Aggregator < EvidenceClass::SearchIndex);
        assert!(EvidenceClass::SearchIndex < EvidenceClass::MerchantPage);
        assert!(EvidenceClass::MerchantPage < EvidenceClass::Transactable);
    }

    #[test]
    fn aggregator_and_search_index_are_weak_merchant_and_above_are_not() {
        let mk = |e| Attribute {
            value: AttrValue::Number(1.0),
            unit: None,
            source_id: "s".into(),
            source_url: None,
            observed_at_ms: 0,
            evidence: e,
        };
        assert!(mk(EvidenceClass::Aggregator).is_weak());
        assert!(mk(EvidenceClass::SearchIndex).is_weak());
        assert!(!mk(EvidenceClass::MerchantPage).is_weak());
        assert!(!mk(EvidenceClass::Transactable).is_weak());
    }
}
```

`crates/vox-market/src/reconcile.rs`:

```rust
//! Reconciliation across disagreeing sources.
//!
//! Disagreement is the normal case, not an error path: on 2026-08-21 every
//! single line of a workstation build had at least two sources that did not
//! agree. The rule is highest evidence class wins, ties break on recency, and
//! losers are retained so a human can still see a cheaper unverified option.

use crate::attribute::Attribute;

#[derive(Debug, Clone)]
pub struct Reconciled {
    pub winner: Attribute,
    /// Every non-winning observation, strongest first. Retained rather than
    /// discarded so the UI can render "seen at $X (unverified) — call to confirm".
    pub alternates: Vec<Attribute>,
}

pub fn reconcile(observations: &[Attribute]) -> Option<Reconciled> {
    if observations.is_empty() {
        return None;
    }
    let mut sorted = observations.to_vec();
    // Strongest evidence first; within a class, most recent first.
    sorted.sort_by(|a, b| {
        b.evidence
            .cmp(&a.evidence)
            .then(b.observed_at_ms.cmp(&a.observed_at_ms))
    });
    let winner = sorted.remove(0);
    Some(Reconciled { winner, alternates: sorted })
}
```

Add `pub mod attribute;` and `pub mod reconcile;` to `lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -j 4 -p vox-market`
Expected: PASS — 4 reconcile tests, 2 attribute tests, 4 schema tests.

- [ ] **Step 5: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-market/src
git commit -m "feat(market): evidence-class provenance and reconciliation

Sources disagree as the normal case. Highest EvidenceClass wins, ties
break on recency, and losing observations are retained as alternates so a
cheaper-but-unverified price stays visible without being able to drive a
recommendation.

Tests are the real 2026-08-21 conflicts: the \$3,999 tracker vs \$3,599.99
transactable listing vs \$4,799.99 merchant page, and the \$660.20 blocked
vendor vs \$710 live cart."
```

---

### Task 3: Append-only store

**Files:**
- Create: `crates/vox-market/src/store.rs`
- Create: `crates/vox-db/migrations/NNNN_market_catalog.sql` (use the next free number)
- Modify: `crates/vox-market/Cargo.toml` (add `vox-db`, `tokio`)
- Modify: `crates/vox-market/src/lib.rs`

**Interfaces:**
- Consumes: `Attribute`, `EvidenceClass`, `reconcile` from Task 2.
- Produces: `MarketStore::new(db: Arc<VoxDb>) -> Self`, `MarketStore::upsert_item(&self, item_id: &str, category: &str) -> Result<()>`, `MarketStore::record(&self, item_id: &str, attribute: &str, a: &Attribute) -> Result<()>`, `MarketStore::current(&self, item_id: &str, attribute: &str) -> Result<Option<Reconciled>>`, `MarketStore::history(&self, item_id: &str, attribute: &str, since_ms: i64) -> Result<Vec<Attribute>>`.

- [ ] **Step 1: Write the failing test**

In `crates/vox-market/src/store.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::{AttrValue, Attribute, EvidenceClass};

    async fn store() -> MarketStore {
        // In-memory Turso; mirrors how vox-db tests bootstrap elsewhere.
        let db = vox_db::VoxDb::connect_in_memory().await.expect("db");
        let s = MarketStore::new(std::sync::Arc::new(db));
        s.migrate().await.expect("migrate");
        s
    }

    fn attr(v: f64, ev: EvidenceClass, at: i64, src: &str) -> Attribute {
        Attribute {
            value: AttrValue::Number(v),
            unit: Some("USD".into()),
            source_id: src.into(),
            source_url: None,
            observed_at_ms: at,
            evidence: ev,
        }
    }

    #[tokio::test]
    async fn recording_twice_keeps_both_observations() {
        let s = store().await;
        s.upsert_item("a6000", "gpu").await.unwrap();
        s.record("a6000", "price_usd", &attr(3999.0, EvidenceClass::Aggregator, 100, "tracker"))
            .await
            .unwrap();
        s.record("a6000", "price_usd", &attr(3599.99, EvidenceClass::Transactable, 200, "ebay"))
            .await
            .unwrap();

        // Append-only: a card moved $1,710 in 24h, and last-write-wins would
        // destroy exactly the signal that makes history worth keeping.
        let h = s.history("a6000", "price_usd", 0).await.unwrap();
        assert_eq!(h.len(), 2);
    }

    #[tokio::test]
    async fn current_returns_the_reconciled_winner_not_the_latest_write() {
        let s = store().await;
        s.upsert_item("a6000", "gpu").await.unwrap();
        s.record("a6000", "price_usd", &attr(3599.99, EvidenceClass::Transactable, 100, "ebay"))
            .await
            .unwrap();
        // Written later, but weaker evidence — must NOT become current.
        s.record("a6000", "price_usd", &attr(3499.0, EvidenceClass::Aggregator, 900, "tracker"))
            .await
            .unwrap();

        let cur = s.current("a6000", "price_usd").await.unwrap().expect("current");
        assert_eq!(cur.winner.value, AttrValue::Number(3599.99));
        assert_eq!(cur.alternates.len(), 1);
    }

    #[tokio::test]
    async fn history_respects_the_since_bound() {
        let s = store().await;
        s.upsert_item("x", "gpu").await.unwrap();
        s.record("x", "price_usd", &attr(1.0, EvidenceClass::MerchantPage, 100, "a")).await.unwrap();
        s.record("x", "price_usd", &attr(2.0, EvidenceClass::MerchantPage, 500, "a")).await.unwrap();
        assert_eq!(s.history("x", "price_usd", 300).await.unwrap().len(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-market store`
Expected: FAIL — `cannot find type MarketStore`.

- [ ] **Step 3: Write the migration**

`crates/vox-db/migrations/NNNN_market_catalog.sql`:

```sql
CREATE TABLE IF NOT EXISTS market_items (
    item_id       TEXT PRIMARY KEY,
    category      TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);

-- Append-only. Never UPDATE, never DELETE.
CREATE TABLE IF NOT EXISTS market_observations (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id        TEXT NOT NULL,
    attribute      TEXT NOT NULL,
    value_num      REAL,
    value_text     TEXT,
    unit           TEXT,
    source_id      TEXT NOT NULL,
    source_url     TEXT,
    evidence_class INTEGER NOT NULL,
    observed_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_market_obs_lookup
    ON market_observations (item_id, attribute, observed_at_ms DESC);
```

- [ ] **Step 4: Write minimal implementation**

`crates/vox-market/src/store.rs`:

```rust
//! Persistence. Observations are append-only; "current" is computed by
//! reconciling every observation for an (item, attribute) pair rather than
//! being a mutable cell, so a late write from a weak source cannot silently
//! become the answer.

use crate::attribute::{AttrValue, Attribute, EvidenceClass};
use crate::reconcile::{reconcile, Reconciled};
use std::sync::Arc;

pub struct MarketStore {
    db: Arc<vox_db::VoxDb>,
}

impl MarketStore {
    pub fn new(db: Arc<vox_db::VoxDb>) -> Self {
        Self { db }
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        self.db
            .execute_batch(include_str!(
                "../../vox-db/migrations/NNNN_market_catalog.sql"
            ))
            .await?;
        Ok(())
    }

    pub async fn upsert_item(&self, item_id: &str, category: &str) -> anyhow::Result<()> {
        self.db
            .execute(
                "INSERT INTO market_items (item_id, category, created_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(item_id) DO UPDATE SET category = excluded.category",
                vox_db::params![item_id, category, now_ms()],
            )
            .await?;
        Ok(())
    }

    pub async fn record(
        &self,
        item_id: &str,
        attribute: &str,
        a: &Attribute,
    ) -> anyhow::Result<()> {
        let (num, text) = match &a.value {
            AttrValue::Number(n) => (Some(*n), None),
            AttrValue::Text(t) => (None, Some(t.clone())),
        };
        self.db
            .execute(
                "INSERT INTO market_observations
                 (item_id, attribute, value_num, value_text, unit, source_id,
                  source_url, evidence_class, observed_at_ms)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                vox_db::params![
                    item_id, attribute, num, text, a.unit, a.source_id,
                    a.source_url, a.evidence as i64, a.observed_at_ms
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn history(
        &self,
        item_id: &str,
        attribute: &str,
        since_ms: i64,
    ) -> anyhow::Result<Vec<Attribute>> {
        let mut rows = self
            .db
            .query(
                "SELECT value_num, value_text, unit, source_id, source_url,
                        evidence_class, observed_at_ms
                 FROM market_observations
                 WHERE item_id = ?1 AND attribute = ?2 AND observed_at_ms >= ?3
                 ORDER BY observed_at_ms DESC",
                vox_db::params![item_id, attribute, since_ms],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await? {
            out.push(row_to_attribute(&r)?);
        }
        Ok(out)
    }

    pub async fn current(
        &self,
        item_id: &str,
        attribute: &str,
    ) -> anyhow::Result<Option<Reconciled>> {
        Ok(reconcile(&self.history(item_id, attribute, 0).await?))
    }
}

fn row_to_attribute(r: &vox_db::Row) -> anyhow::Result<Attribute> {
    let value = match r.get::<Option<f64>>(0)? {
        Some(n) => AttrValue::Number(n),
        None => AttrValue::Text(r.get::<Option<String>>(1)?.unwrap_or_default()),
    };
    Ok(Attribute {
        value,
        unit: r.get::<Option<String>>(2)?,
        source_id: r.get::<String>(3)?,
        source_url: r.get::<Option<String>>(4)?,
        evidence: match r.get::<i64>(5)? {
            4 => EvidenceClass::Transactable,
            3 => EvidenceClass::MerchantPage,
            2 => EvidenceClass::SearchIndex,
            _ => EvidenceClass::Aggregator,
        },
        observed_at_ms: r.get::<i64>(6)?,
    })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
```

Add `vox-db`, `tokio` (features `macros`, `rt-multi-thread`), and `anyhow` to `Cargo.toml`; add `pub mod store;` to `lib.rs`. If `vox_db` does not expose `connect_in_memory`, `execute_batch`, `params!` or `Row` under these exact names, adapt to the real API — read `crates/vox-db/src/lib.rs` first and keep the test semantics identical.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -j 4 -p vox-market store`
Expected: PASS, 3 tests.

- [ ] **Step 6: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-market crates/vox-db/migrations
git commit -m "feat(market): append-only observation store

current() reconciles the full observation set rather than reading a
mutable cell, so a later write from a weaker source cannot become the
answer. History is never overwritten because price movement is the
signal, not noise."
```

---

### Task 4: Source adapter trait and the eBay Browse adapter

**Files:**
- Create: `crates/vox-market/src/source.rs`
- Create: `crates/vox-market/src/sources/ebay.rs`
- Create: `crates/vox-market/src/sources/mod.rs`
- Create: `crates/vox-market/tests/fixtures/ebay_browse_a6000.json`
- Modify: `crates/vox-secrets/src/spec/ids.rs` and `crates/vox-secrets/src/spec/registry/` (add `EbayAppId`)
- Modify: `crates/vox-market/src/lib.rs`

**Interfaces:**
- Consumes: `Attribute`, `EvidenceClass` from Task 2.
- Produces: `trait MarketSource { fn id(&self) -> &'static str; fn cost_usd(&self) -> f64; fn is_available(&self) -> bool; fn evidence_class(&self) -> EvidenceClass; async fn fetch(&self, item: &CatalogItemRef) -> Result<Vec<(String, Attribute)>, MarketError>; }`, `MarketError { NotFound, Blocked, Timeout, ParseFailed, NoCredentials }`, `MarketError::is_retryable(&self) -> bool`, `EbaySource::new() -> Self`, `EbaySource::parse_browse_response(&self, body: &str) -> Result<Vec<(String, Attribute)>, MarketError>`, `CatalogItemRef { item_id: String, category: String, source_ids: BTreeMap<String, String> }`.

**Why eBay first:** its API is free and needs a single app ID, and it covers the used market where prices move fastest and where six automated fetch attempts failed during the manual pass. It proves the whole path end to end at zero per-request cost.

- [ ] **Step 1: Write the failing test**

In `crates/vox-market/src/sources/ebay.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::{AttrValue, EvidenceClass};

    const FIXTURE: &str = include_str!("../../tests/fixtures/ebay_browse_a6000.json");

    #[test]
    fn parses_price_and_stock_from_a_browse_response() {
        let s = EbaySource::new();
        let attrs = s.parse_browse_response(FIXTURE).expect("parse");
        let by: std::collections::BTreeMap<_, _> = attrs.into_iter().collect();

        assert_eq!(by["price_usd"].value, AttrValue::Number(3599.99));
        assert_eq!(by["stock_count"].value, AttrValue::Number(5.0));
    }

    /// A listing with add-to-cart AND a stated quantity is the strongest
    /// evidence the system recognises.
    #[test]
    fn a_buy_it_now_listing_with_stock_is_transactable() {
        let s = EbaySource::new();
        let by: std::collections::BTreeMap<_, _> =
            s.parse_browse_response(FIXTURE).unwrap().into_iter().collect();
        assert_eq!(by["price_usd"].evidence, EvidenceClass::Transactable);
    }

    /// Best-Offer-only listings cannot be bought at the shown price, so they
    /// must not claim Transactable. The 2026-08-21 pass hit exactly this: the
    /// cheapest motherboard was Best-Offer-only and unbuyable.
    #[test]
    fn best_offer_without_buy_it_now_is_only_a_merchant_page() {
        let s = EbaySource::new();
        let body = FIXTURE.replace(r#""buyingOptions":["FIXED_PRICE"]"#, r#""buyingOptions":["BEST_OFFER"]"#);
        let by: std::collections::BTreeMap<_, _> =
            s.parse_browse_response(&body).unwrap().into_iter().collect();
        assert_eq!(by["price_usd"].evidence, EvidenceClass::MerchantPage);
    }

    #[test]
    fn an_empty_result_set_is_not_found_not_a_parse_error() {
        let s = EbaySource::new();
        let err = s.parse_browse_response(r#"{"itemSummaries":[]}"#).unwrap_err();
        assert!(matches!(err, MarketError::NotFound));
    }

    #[test]
    fn blocked_and_timeout_retry_but_not_found_does_not() {
        assert!(MarketError::Blocked.is_retryable());
        assert!(MarketError::Timeout.is_retryable());
        assert!(!MarketError::NotFound.is_retryable());
        assert!(!MarketError::NoCredentials.is_retryable());
    }
}
```

Write `crates/vox-market/tests/fixtures/ebay_browse_a6000.json` as a trimmed real eBay Browse `item_summary/search` response containing one item with `"price":{"value":"3599.99","currency":"USD"}`, `"estimatedAvailabilities":[{"estimatedAvailableQuantity":5}]`, and `"buyingOptions":["FIXED_PRICE"]`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-market ebay`
Expected: FAIL — `cannot find type EbaySource`.

- [ ] **Step 3: Write minimal implementation**

`crates/vox-market/src/source.rs`:

```rust
//! Source adapters. `evidence_class()` lives on the adapter, not the
//! observation: a source's maximum trust is a property of what it can see. An
//! aggregator cannot emit Transactable however fresh its data is.

use crate::attribute::{Attribute, EvidenceClass};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketError {
    #[error("item not listed at this source")]
    NotFound,
    #[error("source blocked the request (403/429/captcha)")]
    Blocked,
    #[error("request timed out")]
    Timeout,
    #[error("response did not parse: {0}")]
    ParseFailed(String),
    #[error("no credentials configured for this source")]
    NoCredentials,
}

impl MarketError {
    /// Retry only what can succeed later. NotFound will not change until the
    /// catalog does; NoCredentials is a configuration problem to surface, not
    /// a transient fault to bury in a retry loop.
    pub fn is_retryable(&self) -> bool {
        matches!(self, MarketError::Blocked | MarketError::Timeout)
    }
}

#[derive(Debug, Clone)]
pub struct CatalogItemRef {
    pub item_id: String,
    pub category: String,
    /// source_id -> that source's identifier for this item (SKU, item number).
    pub source_ids: BTreeMap<String, String>,
}

#[async_trait::async_trait]
pub trait MarketSource: Send + Sync {
    fn id(&self) -> &'static str;
    fn cost_usd(&self) -> f64;
    fn is_available(&self) -> bool;
    fn evidence_class(&self) -> EvidenceClass;
    async fn fetch(
        &self,
        item: &CatalogItemRef,
    ) -> Result<Vec<(String, Attribute)>, MarketError>;
}
```

`crates/vox-market/src/sources/ebay.rs` implements `EbaySource` with `parse_browse_response` extracting `price_usd` and `stock_count`, setting `EvidenceClass::Transactable` only when `buyingOptions` contains `FIXED_PRICE` **and** an availability quantity is present, otherwise `MerchantPage`; returning `MarketError::NotFound` for an empty `itemSummaries`. `is_available()` returns `vox_secrets::resolve_secret(SecretId::EbayAppId).is_ok()`. `cost_usd()` returns `0.0`.

Register `SecretId::EbayAppId` in `crates/vox-secrets/src/spec/ids.rs` and add its `SecretSpec` entry under `crates/vox-secrets/src/spec/registry/`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -j 4 -p vox-market ebay`
Expected: PASS, 5 tests.

- [ ] **Step 5: Verify the secret surface**

Run: `cargo run -q -p vox-cli -- ci secret-env-guard && cargo run -q -p vox-cli -- ci secrets-parity`
Expected: both pass.

- [ ] **Step 6: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-market crates/vox-secrets
git commit -m "feat(market): MarketSource trait and eBay Browse adapter

eBay first because it is free, needs one app ID, and covers the used
market where prices move fastest.

Transactable requires FIXED_PRICE plus a stated quantity. A Best-Offer
listing is only MerchantPage: the 2026-08-21 pass found the cheapest
motherboard was Best-Offer-only and could not actually be bought at the
shown price."
```

---

### Task 5: TTL and the refresh scheduler

**Files:**
- Create: `crates/vox-market/src/scheduler.rs`
- Modify: `crates/vox-market/src/lib.rs`

**Interfaces:**
- Consumes: `AttrKind` (Task 1), `MarketStore` (Task 3), `MarketSource`, `MarketError` (Task 4).
- Produces: `is_stale(kind: AttrKind, observed_at_ms: i64, now_ms: i64) -> bool`, `Backoff::next_delay_secs(&mut self) -> u64`, `RefreshPlan::build(schema, items, store, now_ms) -> Vec<RefreshJob>`, `RefreshJob { item_id: String, attribute: String, source_id: String }`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::AttrKind;

    const HOUR: i64 = 3_600_000;

    #[test]
    fn specs_never_go_stale() {
        // An A6000 is always 48GB. Re-fetching it forever wastes the budget.
        assert!(!is_stale(AttrKind::Spec, 0, 100 * 365 * 24 * HOUR));
    }

    #[test]
    fn stock_goes_stale_in_minutes_and_price_in_hours() {
        let now = 100 * HOUR;
        // A listing moved 6 -> 5 available mid-session, so 15 minutes is the
        // budget for stock.
        assert!(is_stale(AttrKind::Stock, now - (HOUR / 2), now));
        assert!(!is_stale(AttrKind::Stock, now - (HOUR / 8), now));
        assert!(!is_stale(AttrKind::Price, now - (2 * HOUR), now));
        assert!(is_stale(AttrKind::Price, now - (7 * HOUR), now));
    }

    #[test]
    fn backoff_grows_and_then_caps() {
        let mut b = Backoff::default();
        let a = b.next_delay_secs();
        let c = b.next_delay_secs();
        assert!(c > a, "delay must grow: {a} -> {c}");
        for _ in 0..20 {
            b.next_delay_secs();
        }
        assert!(b.next_delay_secs() <= 3600, "must cap at an hour");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-market scheduler`
Expected: FAIL — `cannot find function is_stale`.

- [ ] **Step 3: Write minimal implementation**

```rust
//! Refresh scheduling. TTL is a property of the attribute kind, not the item:
//! re-fetching a GPU's VRAM every six hours is pure waste, and re-fetching its
//! stock every six hours is far too slow.

use crate::schema::AttrKind;

pub fn is_stale(kind: AttrKind, observed_at_ms: i64, now_ms: i64) -> bool {
    match kind.ttl_hours() {
        None => false,
        Some(h) => (now_ms - observed_at_ms) as f64 > h * 3_600_000.0,
    }
}

/// Exponential backoff for `MarketError::Blocked` / `Timeout`, capped so a
/// permanently-blocked source does not drift to a delay measured in days.
#[derive(Debug, Default)]
pub struct Backoff {
    attempts: u32,
}

impl Backoff {
    pub fn next_delay_secs(&mut self) -> u64 {
        self.attempts = self.attempts.saturating_add(1);
        let d = 30u64.saturating_mul(1 << self.attempts.min(7));
        d.min(3600)
    }
    pub fn reset(&mut self) {
        self.attempts = 0;
    }
}
```

Add the `RefreshPlan::build` function that walks catalog items, resolves each attribute's kind from the schema, reads the current observation's `observed_at_ms` from the store, and emits a `RefreshJob` per stale (item, attribute, source) triple — skipping sources where `is_available()` is false.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -j 4 -p vox-market scheduler`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-market/src/scheduler.rs crates/vox-market/src/lib.rs
git commit -m "feat(market): per-kind TTL and refresh scheduling

Specs never expire, prices are hours, stock is minutes. Sequential
per-source fetch with capped exponential backoff; no work queue, because
the real load is single-digit requests per hour."
```

---

### Task 6: Constraints and explainable scoring

**Files:**
- Create: `crates/vox-market/src/constraints.rs`
- Create: `crates/vox-market/src/scoring.rs`
- Create: `contracts/market/scoring.v1.yaml`
- Modify: `crates/vox-market/src/lib.rs`

**Interfaces:**
- Consumes: `CatalogSchema`, `AttributeDef` (Task 1); `Attribute`, `AttrValue` (Task 2).
- Produces: `Constraint { attribute: String, op: CmpOp, value: f64, unit: Option<String> }`, `CmpOp { Gte, Lte, Eq }`, `apply(items: &[ScoredItem], constraints: &[Constraint]) -> FilterOutcome`, `FilterOutcome { passed: Vec<ScoredItem>, excluded: Vec<Exclusion> }`, `Exclusion { item_id: String, reason: String }`, `rank(items: Vec<ScoredItem>, axes: &[Axis]) -> Vec<Ranked>`, `Ranked { item_id: String, score: f64, explanation: String, unavailable_axes: Vec<String> }`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn laptop(id: &str, gb: f64, price: f64) -> ScoredItem {
        ScoredItem::from_pairs(id, &[("gpu_accessible_gb", gb, "GB"), ("price_usd", price, "USD")])
    }

    /// The acceptance query. A 64GB machine exposes ~48GB to the GPU, so it
    /// must NOT satisfy >=64, and the caller must be told why.
    #[test]
    fn a_64gb_machine_is_excluded_with_a_stated_reason() {
        let items = vec![laptop("mbp-64", 48.0, 3699.0), laptop("mbp-128", 96.0, 6999.0)];
        let out = apply(&items, &[Constraint::gte("gpu_accessible_gb", 64.0, "GB")]);

        assert_eq!(out.passed.len(), 1);
        assert_eq!(out.passed[0].item_id, "mbp-128");
        assert_eq!(out.excluded.len(), 1);
        assert!(
            out.excluded[0].reason.contains("48") && out.excluded[0].reason.contains("64"),
            "reason must name both numbers, got: {}",
            out.excluded[0].reason
        );
    }

    /// Comparing GB against W is a category error, not a close call.
    #[test]
    fn mismatched_units_are_rejected_rather_than_silently_compared() {
        let items = vec![laptop("x", 96.0, 1000.0)];
        let out = apply(&items, &[Constraint::gte("gpu_accessible_gb", 64.0, "W")]);
        assert!(out.passed.is_empty());
        assert!(out.excluded[0].reason.to_lowercase().contains("unit"));
    }

    #[test]
    fn ranking_explains_its_arithmetic() {
        let items = vec![laptop("cheap", 96.0, 2999.0), laptop("dear", 96.0, 6999.0)];
        let axes = vec![Axis::lower_is_better("dollars_per_gb", "price_usd / gpu_accessible_gb")];
        let r = rank(items, &axes);
        assert_eq!(r[0].item_id, "cheap");
        assert!(r[0].explanation.contains("dollars_per_gb"), "got: {}", r[0].explanation);
    }

    /// An axis whose inputs are absent is reported as unavailable, never
    /// scored as zero — which would look like a tie rather than a missing input.
    #[test]
    fn an_axis_with_missing_inputs_is_skipped_and_named() {
        let items = vec![laptop("a", 96.0, 2999.0)];
        let axes = vec![Axis::higher_is_better("tokens_per_dollar", "tg_tok_s / (price_usd / 1000)")];
        let r = rank(items, &axes);
        assert!(r[0].unavailable_axes.contains(&"tokens_per_dollar".to_string()));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-market constraints scoring`
Expected: FAIL — `cannot find function apply`.

- [ ] **Step 3: Write minimal implementation**

Implement `Constraint::gte/lte/eq` builders; `apply` compares only when the constraint's unit matches the attribute's declared unit and otherwise emits an `Exclusion` naming the mismatch; exclusion reasons interpolate both the actual and required values (`"gpu_accessible_gb 48 GB < required 64 GB"`). `rank` evaluates each axis's expression over the item's attributes, skips axes whose referenced attributes are absent and records them in `unavailable_axes`, normalises each available axis to 0..1, averages with equal weights, and renders `explanation` as a comma-separated list of `axis=value`. Write `contracts/market/scoring.v1.yaml` with the three axes from the spec including the `requires: [tg_tok_s]` guard on `tokens_per_dollar`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -j 4 -p vox-market constraints scoring`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-market/src/constraints.rs crates/vox-market/src/scoring.rs contracts/market/scoring.v1.yaml crates/vox-market/src/lib.rs
git commit -m "feat(market): constraint filtering and explainable scoring

Constraints eliminate and never score; ranking shows its arithmetic. Unit
mismatches are rejected rather than compared. An axis whose inputs are
absent is named as unavailable rather than scored zero, which would read
as a tie instead of a missing input."
```

---

### Task 7: vox-search corpus

**Files:**
- Create: `crates/vox-market/src/search_corpus.rs`
- Modify: `crates/vox-market/Cargo.toml` (add `vox-search`)
- Modify: `crates/vox-market/src/lib.rs`

**Interfaces:**
- Consumes: `MarketStore` (Task 3), `CatalogSchema` (Task 1).
- Produces: `MarketCorpus::index_item(&self, item: &CatalogItemRef, attrs: &BTreeMap<String, Attribute>) -> Result<()>`, `MarketCorpus::search(&self, query: &str, limit: usize) -> Result<Vec<MarketHit>>`, `MarketHit { item_id: String, category: String, score: f32 }`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn indexed_items_are_findable_by_model_name() {
        let c = MarketCorpus::in_memory().await.unwrap();
        c.index_text("a6000", "gpu", "NVIDIA RTX A6000 48GB GDDR6 workstation").await.unwrap();
        c.index_text("mbp16", "laptop", "Apple MacBook Pro 16 M5 Max 128GB").await.unwrap();

        let hits = c.search("A6000", 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].item_id, "a6000");
    }

    #[tokio::test]
    async fn search_returns_the_category_so_results_can_be_faceted() {
        let c = MarketCorpus::in_memory().await.unwrap();
        c.index_text("mbp16", "laptop", "Apple MacBook Pro 16 M5 Max 128GB").await.unwrap();
        assert_eq!(c.search("MacBook", 10).await.unwrap()[0].category, "laptop");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-market search_corpus`
Expected: FAIL — `cannot find type MarketCorpus`.

- [ ] **Step 3: Write minimal implementation**

Build a document per item — model name, category, and the text-valued attributes — and index it through `vox-search`'s existing lexical indexer. **Do not build a bespoke index**: AGENTS.md requires richer retrieval to build on the existing hybrid stack, and `vox_search_query` already returns `facets_by_source` / `facets_by_kind`. Read `crates/vox-search/src/lexical_tantivy.rs` and `ingest.rs` first and follow their construction pattern. Numeric constraints are **not** expressed in the query string — they apply as post-filters via Task 6, because lexical search cannot express `>= 64`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -j 4 -p vox-market search_corpus`
Expected: PASS, 2 tests.

- [ ] **Step 5: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-market
git commit -m "feat(market): index the catalog as a vox-search corpus

Adds a corpus rather than an index, per the AGENTS.md retrieval policy.
Numeric constraints stay in the constraint layer because lexical search
cannot express >= 64."
```

---

### Task 8: `vox market` CLI and IPC commands

**Files:**
- Create: `crates/vox-cli/src/commands/market/mod.rs`
- Create: `crates/vox-cli/src/commands/market/list.rs`
- Create: `crates/vox-cli/src/commands/market/sources.rs`
- Create: `crates/vox-gui/src/commands/market.rs`
- Modify: `crates/vox-cli/src/commands/mod.rs`, the CLI command enum, `crates/vox-gui/src/main.rs` (invoke_handler)
- Modify: `contracts/gui/surface-registry.v1.yaml` (set mercatus `cli_group: market`)

**Interfaces:**
- Consumes: everything from Tasks 1–7.
- Produces: CLI subcommands `list`, `show`, `history`, `fetch`, `sources`, each with `--json`; Tauri commands `market_list_items`, `market_item_detail`, `market_history`, `market_sources`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// `vox market sources` is the diagnostic that makes a keyless install
    /// comprehensible: it must list every adapter and say which are usable.
    #[test]
    fn sources_report_names_unconfigured_adapters_rather_than_hiding_them() {
        let rows = source_rows(&[("ebay", false, 0.0), ("brightdata", false, 0.0025)]);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| !r.available));
        assert!(rows.iter().any(|r| r.id == "ebay"));
    }

    /// An empty catalog is a first-run state, not a failure.
    #[test]
    fn an_empty_catalog_renders_guidance_not_an_error() {
        let out = render_list(&[], "laptop");
        assert!(out.to_lowercase().contains("no items"));
        assert!(!out.to_lowercase().contains("error"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-cli market`
Expected: FAIL — `cannot find function source_rows`.

- [ ] **Step 3: Write minimal implementation**

Implement the subcommands over `MarketStore` + `apply` + `rank`. `vox market sources` prints id, cost per fetch, evidence class, and whether credentials resolve. `vox market list --where 'gpu_accessible_gb >= 64'` parses simple `attr op value unit` triples into `Constraint`s. Every subcommand supports `--json`. Mirror the same four reads as Tauri commands in `crates/vox-gui/src/commands/market.rs` and register them in `main.rs`'s `invoke_handler`.

- [ ] **Step 4: Run tests and regenerate the command surface**

Run: `cargo test -j 4 -p vox-cli market`
Expected: PASS, 2 tests.

Run: `cargo run -q -p vox-cli -- ci command-sync`
Expected: regenerates `docs/src/reference/cli-command-surface.generated.md`; commit the regenerated file with the change.

- [ ] **Step 5: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-cli crates/vox-gui contracts/gui/surface-registry.v1.yaml docs/src/reference/cli-command-surface.generated.md
git commit -m "feat(market): vox market CLI and IPC read surface

Closes the cli_group: null gap on the mercatus surface. sources is the
diagnostic that makes a keyless install legible: it names every adapter
and which have credentials, rather than silently returning nothing."
```

---

### Task 9: Acceptance test — the laptop query end to end

**Files:**
- Create: `crates/vox-market/tests/acceptance_laptop_memory.rs`
- Create: `crates/vox-market/tests/fixtures/laptops_seed.yaml`

**Interfaces:**
- Consumes: everything from Tasks 1–8.
- Produces: the passing gate that unlocks the discovery flag (spec §Discovery).

- [ ] **Step 1: Write the failing test**

`crates/vox-market/tests/acceptance_laptop_memory.rs`:

```rust
//! Acceptance gate from the spec. Reproduces the 2026-08-21 manual finding:
//! no 64GB laptop clears a 64GB GPU-memory requirement, because Apple and AMD
//! unified-memory machines reserve ~25% for the OS.

use vox_market::constraints::{apply, Constraint};
use vox_market::testkit::seed_from_yaml;

#[tokio::test]
async fn only_128gb_machines_satisfy_a_64gb_gpu_memory_constraint() {
    let items = seed_from_yaml(include_str!("fixtures/laptops_seed.yaml"))
        .await
        .expect("seed");

    let out = apply(&items, &[Constraint::gte("gpu_accessible_gb", 64.0, "GB")]);

    let passed: Vec<&str> = out.passed.iter().map(|i| i.item_id.as_str()).collect();
    assert!(passed.contains(&"mbp16-m5max-128"), "got {passed:?}");
    assert!(passed.contains(&"rog-flow-z13-128"), "got {passed:?}");
    assert!(
        !passed.contains(&"mbp14-m5pro-64"),
        "a 64GB machine exposes ~48GB and must not pass: {passed:?}"
    );

    // The useful answer is not an empty-ish list, it is WHY the others went.
    let why = out
        .excluded
        .iter()
        .find(|e| e.item_id == "mbp14-m5pro-64")
        .expect("exclusion recorded");
    assert!(why.reason.contains("48"), "reason must state the actual value: {}", why.reason);
}

#[tokio::test]
async fn an_impossible_constraint_explains_itself_rather_than_returning_empty() {
    let items = seed_from_yaml(include_str!("fixtures/laptops_seed.yaml")).await.unwrap();
    let out = apply(&items, &[Constraint::gte("gpu_accessible_gb", 512.0, "GB")]);
    assert!(out.passed.is_empty());
    assert_eq!(out.excluded.len(), items.len(), "every item must carry a reason");
}
```

Write `fixtures/laptops_seed.yaml` with at least: `mbp14-m5pro-64` (gpu_accessible_gb 48, price 3699), `mbp16-m5max-128` (96, 6999), `rog-flow-z13-128` (96, 3299.99), `proart-px13-128` (96, 2999.99).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-market --test acceptance_laptop_memory`
Expected: FAIL — `cannot find function seed_from_yaml`.

- [ ] **Step 3: Write minimal implementation**

Add `pub mod testkit;` behind `#[cfg(any(test, feature = "testkit"))]` exposing `seed_from_yaml(&str) -> Result<Vec<ScoredItem>>` that parses the fixture into catalog items with `EvidenceClass::MerchantPage` attributes.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -j 4 -p vox-market --test acceptance_laptop_memory`
Expected: PASS, 2 tests.

- [ ] **Step 5: Run the whole gate**

Run: `cargo test -j 4 -p vox-market && cargo run -q -p vox-arch-check && cargo run -q -p vox-cli -- ci crate-edges`
Expected: tests pass; `crate-edges` **fails** on the two new L4→L1 edges. **Do not fix this by editing the allow-list.** Record the failure and propose the edges in the PR description per AGENTS.md.

- [ ] **Step 6: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-market/tests crates/vox-market/src
git commit -m "test(market): acceptance gate for the 64GB laptop query

Reproduces the 2026-08-21 finding: no 64GB laptop clears a 64GB GPU-memory
constraint because unified-memory machines reserve ~25% for the OS. The
test asserts both the filtering and the explanation, because an
unexplained empty result is the failure mode this layer exists to prevent.

Passing this gate is what unlocks the discovery flag."
```

---

## Self-Review

**Spec coverage.** Crate placement → T1. Catalog schema, units, `required` → T1. Provenance, `EvidenceClass`, reconciliation → T2. Persistence, append-only, `market_current` semantics → T3. Adapter trait, `MarketError`, credentials, eBay-first → T4. TTL, scheduler, backoff → T5. Constraints, scoring, unavailable axes, surprising-result-is-a-finding → T6. vox-search corpus → T7. IPC + CLI, `cli_group` gap → T8. Acceptance test → T9. **Gap accepted deliberately:** the budget ceiling (spec §Scheduler) is not implemented — it needs the paid Bright Data adapter, which this plan does not build, so it belongs with that adapter rather than being written against no paid source. **Excluded by scope:** the discovery pipeline, which becomes its own plan.

**Placeholder scan.** No TBD/TODO. Tasks 6, 7 and 8 describe implementations in prose rather than full code blocks — these are the tasks whose implementation must follow existing in-repo patterns (`vox-search` ingestion, the CLI command registry, the Tauri handler), and the plan names the exact files to read first. Every test step carries real, runnable code.

**Type consistency.** `Attribute`, `AttrValue`, `EvidenceClass` (T2) flow unchanged into T3/T4/T7. `AttrKind` (T1) is consumed by T5's `is_stale`. `CatalogItemRef` (T4) is used by T7. `ScoredItem` is introduced in T6 and reused in T9. `Constraint::gte(attr, value, unit)` has one signature throughout. `MarketStore::current` returns `Option<Reconciled>` in T3 and is consumed as such in T8.
