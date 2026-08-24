---
title: "Market catalog and constraint layer — implementation plan"
description: "Three-task plan reaching the laptop acceptance query: a contract-driven catalog schema, three-valued constraint evaluation with explained exclusions, and a derived unified-memory capacity. The fetch layer is deferred with its blocking findings attached."
category: "architecture"
---

# Market Catalog and Constraint Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Answer `gpu_accessible_gb >= 64` over a laptop catalog with an explanation that names *why* a 64 GB machine only exposes 48 GB — the 2026-08-21 query, reproduced without hand-holding.

**Architecture:** A contract-defined attribute schema plus a three-valued constraint evaluator, as one module in an existing L1 crate. No new crate, no database, no network, no scheduler.

**Tech Stack:** Rust, `serde_yaml` (aliased `serde_yaml_ng`), `contracts/market/*.v1.yaml`.

**Spec:** [`docs/superpowers/specs/2026-08-21-market-data-layer-design.md`](../specs/2026-08-21-market-data-layer-design.md)

---

## Revision note — 2026-08-24

This plan previously had nine tasks building a `vox-market` crate with a
database, an eBay adapter, a refresh scheduler, and a vox-search corpus. A
seven-track parallel critique against the codebase found that version
unbuildable and, where buildable, wrong. It is replaced rather than patched.

| Finding | Confirmed by | Change |
|---|---|---|
| `vox-market` at L1 is arithmetically impossible (`vox-db: 2`, `vox-search: 3`, downward-only rule) | Tracks 1, 2, 5 + verified directly | No crate. Module in `vox-cli-core` (L1). |
| The acceptance test touches no store, adapter, scheduler, search, or CLI | Track 5 | Nine tasks to three. |
| **The plan never builds a writer.** No fetch loop, no `vox market add` — a store with nothing to put in it | Track 7 | Deferred section states this as the entry-point blocker. |
| 3,746 lines already solve this shape in `vox-populi/src/mens/cloud/` | Track 5 + verified directly | Prior-art section added to the spec. |
| `crates/vox-db/migrations/` does not exist, and the drafted instruction (a `Migration` at a non-baseline version) makes the next `connect` fail `LegacySchemaChain` | Tracks 1, 2, 7 + verified directly | Store deferred; DDL goes in `vox-db/src/schema/domains/`. |
| Raw SQL from a leaf crate fails `sql-surface-guard` **and** `turso-import-guard` | Track 1 | Deferred with the store. |
| Stale `Transactable` outranks fresh `MerchantPage` forever | Track 3 | Spec §Reconciliation rewritten: freshness gates precedence. |
| `ScoredItem::from_pairs` carries no `EvidenceClass`, so "an unverified attribute cannot win" is unimplementable in the plan's own types | Track 4 | `Value` carries `Evidence` from Task 2. |
| The acceptance test asserts `48 < 64` on hand-typed literals and never asserts the stated criterion | Tracks 3, 4 | Task 3 asserts the reason text; the 48 is derived. |
| Axis skipping was per-item, so the least-documented item wins | Track 3 | Set-scoped in the spec. |
| `market_current` specced, never built; `current()` scans all history | Tracks 3, 5, 7 | Table deleted from the spec. |
| `attribute_kinds` discarded by serde while Rust hardcodes the same TTLs | Tracks 1, 4, 5, 7 | Task 1 deserializes it and tests for drift. |
| Free-tier quota, not disk, is the binding limit: ~52 items saturates eBay's 5,000/day | Track 7 | Recorded in the deferred scheduler section. |
| Edge count was 2; the ratchet is an exact set, so 6, plus 4 fan-in bumps | Track 2 | Moot — no crate. |

**One critique finding not adopted.** Track 7 reported that
`model_route_policy::budget_guard` does not exist in this workspace. It does —
`crates/vox-gui/src/commands/user_config.rs` and three files under
`crates/vox-orchestrator-mcp/` reference it. The spec's citation stands.

**Net effect:** the acceptance gate is reached by ~200 lines with no I/O, no new
crate edges, and no user-authorized baseline changes.

---

## Global Constraints

- **No new crate.** Code lands in `crates/vox-cli-core/src/market.rs`, already
  L1 and already the shared-logic home. New dependency: `serde_yaml` only
  (already in `[workspace.dependencies]`, aliased to `serde_yaml_ng` 0.10 — the
  plain name is the correct spelling).
- **Do not create `crates/vox-market/`.** If a later piece needs this from the
  GUI process, promote then — a file move, with the shape known rather than
  guessed. Promotion lands at **L3**.
- **Test-first, per AGENTS.md.** Every new `pub fn` needs a test in the same
  file before the commit lands; the lefthook `tdd-guard` hook blocks otherwise.
  `market.rs` exceeds 30 non-blank lines, so it needs `#[cfg(test)] mod tests`
  from its first commit.
- **New contract files** need an `x-vox-version: 1` first line (house style, see
  `contracts/gui/hud-tiles.v1.yaml`) and a `contracts/index.yaml` row.
- **Formatting:** `vox run scripts/fmt.vox` — never `cargo fmt --all` (Windows
  `CreateProcess` limit, os error 206). Single crate: `cargo fmt -p vox-cli-core`.
- **Builds:** `-j 4`. This host OOMs at the repo default of 24 jobs.
- **Before pushing:** `vox ci pre-push --complete`. The default fast tier skips
  clippy, which is the largest single source of post-merge fixups in this repo.

---

### Task 1: Catalog schema contract and loader

**Files:**
- Create: `contracts/market/catalog-schema.v1.yaml`
- Create: `crates/vox-cli-core/src/market.rs`
- Modify: `crates/vox-cli-core/src/lib.rs` (add `pub mod market;`)
- Modify: `contracts/index.yaml`
- Modify: `crates/vox-cli-core/Cargo.toml` (add `serde_yaml`)

**Interfaces:**
- Produces: `CatalogSchema::load_from_str(&str) -> Result<CatalogSchema, SchemaError>`;
  `CatalogSchema::attribute(&self, category: &str, name: &str) -> Option<&AttributeDef>`;
  `CatalogSchema::declared_ttl_hours(&self, kind: &str) -> Option<Option<f64>>`;
  `CatalogSchema::required_attributes(&self, category: &str) -> Vec<&str>`;
  `AttributeDef { kind, ty, unit, required, note }`;
  `AttrKind::{Spec, Price, Stock, Availability}` with `ttl_hours()`.

**Why `note` exists:** Task 3 must assert that the exclusion explains *why* a
64 GB machine exposes 48 GB. Without a per-attribute note the reason is a
generic `"48 GB < required 64 GB"`, which states the numbers and not the cause —
and the spec's acceptance criterion is the cause.

**On `include_str!`:** the tests read the shipped contract at compile time. That
is correct for a test asserting the *shipped* file, but it does not make the
schema runtime-loadable, and the spec's claim "adding a category is a config
edit, no Rust change" is therefore not yet true — it is a recompile. Resolving
that needs a runtime load path, which arrives with the CLI in the deferred work.
Recorded rather than papered over.

- [ ] **Step 1: Write the failing test**

In `crates/vox-cli-core/src/market.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_contract_parses_and_exposes_its_categories() {
        let yaml = include_str!("../../../contracts/market/catalog-schema.v1.yaml");
        let s = CatalogSchema::load_from_str(yaml).expect("shipped contract must parse");
        assert!(s.attribute("laptop", "gpu_accessible_gb").is_some());
        // Universal attributes apply to every category without being restated.
        assert!(s.attribute("laptop", "price_usd").is_some());
        assert!(s.attribute("gpu", "price_usd").is_some());
    }

    #[test]
    fn an_unknown_category_or_attribute_is_none_rather_than_a_panic() {
        let yaml = include_str!("../../../contracts/market/catalog-schema.v1.yaml");
        let s = CatalogSchema::load_from_str(yaml).unwrap();
        assert!(s.attribute("spaceship", "warp_factor").is_none());
        assert!(s.attribute("laptop", "warp_factor").is_none());
    }

    /// The contract declares TTLs in YAML and `AttrKind::ttl_hours()` declares
    /// them in Rust. Asserting the Rust constant against a Rust literal — which
    /// the first draft did — tests nothing that can break. What CAN break is the
    /// two drifting: as originally drafted `CatalogSchema` had no
    /// `attribute_kinds` field at all, so serde discarded the block and the
    /// contract could say 24 while the code used 6, forever, silently. That also
    /// made the operator's only lever over fetch volume inert.
    #[test]
    fn declared_ttls_in_the_contract_match_the_compiled_ttls() {
        let yaml = include_str!("../../../contracts/market/catalog-schema.v1.yaml");
        let s = CatalogSchema::load_from_str(yaml).unwrap();
        for (name, kind) in [
            ("spec", AttrKind::Spec),
            ("price", AttrKind::Price),
            ("stock", AttrKind::Stock),
            ("availability", AttrKind::Availability),
        ] {
            let declared = s
                .declared_ttl_hours(name)
                .unwrap_or_else(|| panic!("contract declares no attribute_kind `{name}`"));
            assert_eq!(
                declared,
                kind.ttl_hours(),
                "drift: contract says {name}={declared:?}, code says {:?}",
                kind.ttl_hours()
            );
        }
        assert!(AttrKind::Spec.ttl_hours().is_none(), "an A6000 is always 48GB");
        assert!(
            AttrKind::Stock.ttl_hours().unwrap() < AttrKind::Price.ttl_hours().unwrap(),
            "stock moves faster than price"
        );
    }

    /// `attribute()` resolves category-specific over universal. This rule had no
    /// coverage in the first draft, and the drafted `required_attributes` broke
    /// it: chaining the category map with the universal map let an overridden
    /// name leak back in from the universal side and report as required.
    #[test]
    fn a_category_attribute_overrides_the_universal_one_of_the_same_name() {
        const OVERRIDE: &str = r#"
x-vox-version: 1
schema_version: 1
attribute_kinds:
  spec:  { ttl_hours: null }
  price: { ttl_hours: 6 }
universal_attributes:
  price_usd: { kind: price, type: number, unit: USD, required: true }
categories:
  bundle:
    attributes:
      price_usd: { kind: price, type: number, unit: EUR, required: false }
  laptop:
    attributes:
      gpu_accessible_gb: { kind: spec, type: number, unit: GB, required: true }
"#;
        let s = CatalogSchema::load_from_str(OVERRIDE).unwrap();
        assert_eq!(s.attribute("bundle", "price_usd").unwrap().unit.as_deref(), Some("EUR"));
        assert_eq!(s.attribute("laptop", "price_usd").unwrap().unit.as_deref(), Some("USD"));

        let req = s.required_attributes("bundle");
        assert_eq!(
            req.iter().filter(|n| **n == "price_usd").count(),
            0,
            "the override is not required; the universal one must not leak back in: {req:?}"
        );
    }

    #[test]
    fn a_malformed_contract_names_what_it_could_not_read() {
        let e = CatalogSchema::load_from_str("categories: [not, a, map]")
            .unwrap_err()
            .to_string();
        assert!(e.to_lowercase().contains("categories"), "got: {e}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-cli-core market:: 2>&1 | tail -20`
Expected: FAIL — `cannot find type CatalogSchema`.

- [ ] **Step 3: Write the contract**

`contracts/market/catalog-schema.v1.yaml`:

```yaml
x-vox-version: 1
schema_version: 1
description: >
  Categories and typed attributes for the hardware market catalog. Attribute
  kinds carry TTL and volatility class; vox-cli-core::market asserts these TTLs
  against its compiled constants so the two cannot drift.

attribute_kinds:
  spec:         { ttl_hours: null }   # immutable: an A6000 is always 48 GB
  price:        { ttl_hours: 6 }
  stock:        { ttl_hours: 0.25 }
  availability: { ttl_hours: 6 }

# Applied to every category. Price and stock are not category-specific, and
# leaving them implicit is how a schema ends up with two spellings of "price".
# `required: false` throughout — a price exists only after a successful fetch,
# so requiring it would bar exactly the items whose vendors block automation.
universal_attributes:
  price_usd:    { kind: price,        type: number, unit: USD,   required: false }
  stock_count:  { kind: stock,        type: number, unit: count, required: false }
  availability:
    kind: availability
    type: enum
    required: false
    values: [in_stock, out_of_stock, quote_only, backorder, unknown]

categories:
  laptop:
    attributes:
      total_memory_gb:
        kind: spec
        type: number
        unit: GB
        required: true
        note: "as advertised by the vendor"
      gpu_accessible_gb:
        kind: spec
        type: number
        unit: GB
        required: true
        note: "unified memory reserves ~25% for the OS, so a 64GB machine exposes ~48GB"
      tdp_w:     { kind: spec, type: number, unit: W,  required: false }
      weight_kg: { kind: spec, type: number, unit: kg, required: false }
  gpu:
    attributes:
      vram_gb: { kind: spec, type: number, unit: GB, required: true }
      tdp_w:   { kind: spec, type: number, unit: W,  required: false }
```

`in_stock` is deliberately absent: it restates `stock_count > 0`, and three
spellings of one fact give three chances to disagree — in a system whose
motivating failure *was* a disagreement about stock.

Add to `contracts/index.yaml`:

```yaml
  - id: market-catalog-schema
    path: contracts/market/catalog-schema.v1.yaml
    owner: vox-cli-core
    kind: yaml
    description: Categories and typed attributes for the hardware market catalog.
    enforced_by: [vox ci contracts-index]
```

- [ ] **Step 4: Write the loader**

```rust
//! Hardware market catalog: schema and constraint evaluation.
//!
//! The schema is a contract (`contracts/market/catalog-schema.v1.yaml`), not
//! Rust types. TTLs are declared there and asserted against the compiled
//! constants by the tests below, rather than restated in two places that drift.

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("catalog schema: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("`{0}` is a derived attribute and cannot be asserted by a source")]
    AssertedDerived(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttrKind { Spec, Price, Stock, Availability }

impl AttrKind {
    /// `None` = never expires. Asserted against the contract in tests.
    pub fn ttl_hours(&self) -> Option<f64> {
        match self {
            AttrKind::Spec => None,
            AttrKind::Price | AttrKind::Availability => Some(6.0),
            AttrKind::Stock => Some(0.25),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttrType { Number, Text, Enum }

#[derive(Debug, Clone, Deserialize)]
pub struct AttributeDef {
    pub kind: AttrKind,
    #[serde(rename = "type")]
    pub ty: AttrType,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub required: bool,
    /// Human-readable cause, surfaced in exclusion reasons. This is what turns
    /// "48 GB < 64 GB" into "…because unified memory reserves ~25%".
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct KindDef {
    #[serde(default)]
    ttl_hours: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct Category {
    #[serde(default)]
    attributes: BTreeMap<String, AttributeDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogSchema {
    #[serde(default)]
    attribute_kinds: BTreeMap<String, KindDef>,
    #[serde(default)]
    universal_attributes: BTreeMap<String, AttributeDef>,
    #[serde(default)]
    categories: BTreeMap<String, Category>,
}

impl CatalogSchema {
    pub fn load_from_str(yaml: &str) -> Result<Self, SchemaError> {
        Ok(serde_yaml::from_str(yaml)?)
    }

    /// A category-specific attribute of the same name wins over the universal one.
    pub fn attribute(&self, category: &str, name: &str) -> Option<&AttributeDef> {
        self.categories
            .get(category)
            .and_then(|c| c.attributes.get(name))
            .or_else(|| self.universal_attributes.get(name))
    }

    /// Outer `None` = the contract does not declare this kind.
    /// Inner `None` = declared `ttl_hours: null`, i.e. never expires.
    pub fn declared_ttl_hours(&self, kind: &str) -> Option<Option<f64>> {
        self.attribute_kinds.get(kind).map(|k| k.ttl_hours)
    }

    /// Required names for a category, deduplicated. Resolve-then-filter rather
    /// than chaining two maps, so a category override with `required: false`
    /// cannot have the universal `required: true` leak back in.
    pub fn required_attributes(&self, category: &str) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .universal_attributes
            .keys()
            .map(String::as_str)
            .chain(
                self.categories
                    .get(category)
                    .into_iter()
                    .flat_map(|c| c.attributes.keys().map(String::as_str)),
            )
            .collect();
        names.sort_unstable();
        names.dedup();
        names.retain(|n| self.attribute(category, n).is_some_and(|d| d.required));
        names
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -j 4 -p vox-cli-core market::`
Expected: 5 passed.

- [ ] **Step 6: Verify the contract gate, then format**

```bash
cargo run -q -j 4 -p vox-cli -- ci contracts-index
```

Then `vox run scripts/fmt.vox`.

- [ ] **Step 7: Commit**

```bash
git add contracts/market/catalog-schema.v1.yaml contracts/index.yaml crates/vox-cli-core/src/market.rs crates/vox-cli-core/src/lib.rs crates/vox-cli-core/Cargo.toml
git commit -m "feat(market): catalog schema contract and loader"
```

---

### Task 2: Three-valued constraints with explained exclusions

**Files:**
- Modify: `crates/vox-cli-core/src/market.rs`

**Interfaces:**
- Consumes: `CatalogSchema`, `AttributeDef` from Task 1.
- Produces: `Evidence::{Aggregator, SearchIndex, MerchantPage, Transactable, Derived}`;
  `Value { number: f64, unit: Option<String>, evidence: Evidence }`;
  `CatalogItem { item_id, category, attributes: BTreeMap<String, Value> }`;
  `CmpOp::{Gte, Lte, Eq}`; `Constraint::{gte, lte, eq}(attribute, value, unit)`;
  `apply(&CatalogSchema, &[CatalogItem], &[Constraint]) -> Outcome`;
  `Outcome { passed, excluded, indeterminate }`; `Excluded { item_id, reason }`.

**Why three-valued:** the spec names the failure — *"a constraint filter over a
null field silently drops candidates rather than erroring, the failure where a
qualifying machine never appears and nobody notices."* Both binary defaults are
wrong: silently excluded is that failure; silently included fails at checkout.

**Why `Value` carries `Evidence`:** the spec's rule *"an unverified attribute
cannot win a comparison"* is not expressible over a bare `f64`. The first draft
built `Attribute::is_weak()` and then consumed it nowhere, because
`ScoredItem::from_pairs` had no field to put it in. Carrying it from the start
costs one field; retrofitting it costs every call site.

- [ ] **Step 1: Write the failing test**

Add inside `mod tests`:

```rust
    fn v(n: f64, unit: &str) -> Value {
        Value { number: n, unit: Some(unit.into()), evidence: Evidence::MerchantPage }
    }

    fn item(id: &str, attrs: &[(&str, Value)]) -> CatalogItem {
        CatalogItem {
            item_id: id.into(),
            category: "laptop".into(),
            attributes: attrs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
        }
    }

    fn schema() -> CatalogSchema {
        CatalogSchema::load_from_str(include_str!(
            "../../../contracts/market/catalog-schema.v1.yaml"
        ))
        .unwrap()
    }

    #[test]
    fn a_measured_miss_is_excluded_with_both_numbers_and_the_cause() {
        let items = vec![item("mbp14", &[("gpu_accessible_gb", v(48.0, "GB"))])];
        let out = apply(&schema(), &items, &[Constraint::gte("gpu_accessible_gb", 64.0, "GB")]);

        assert!(out.passed.is_empty());
        assert_eq!(out.excluded.len(), 1);
        let r = &out.excluded[0].reason;
        // Both numbers in the right roles. `contains("48") && contains("64")`
        // alone also passes if they are swapped into "64 < required 48".
        assert!(r.contains("48"), "must state the actual value: {r}");
        assert!(r.contains("64"), "must state the requirement: {r}");
        // The schema `note` is what turns a comparison into an explanation.
        assert!(
            r.contains("unified") || r.contains("reserves"),
            "must explain WHY 64GB exposes only 48GB: {r}"
        );
    }

    /// The spec's named silent-drop failure. An item that simply lacks the
    /// attribute must be distinguishable from one measured and found short: the
    /// two demand different action — close a data gap, or move on.
    #[test]
    fn an_absent_attribute_is_indeterminate_not_a_measured_failure() {
        let items = vec![
            item("roomy", &[("gpu_accessible_gb", v(96.0, "GB"))]),
            item("cramped", &[("gpu_accessible_gb", v(48.0, "GB"))]),
            item("unknown", &[("price_usd", v(4200.0, "USD"))]),
        ];
        let out = apply(&schema(), &items, &[Constraint::gte("gpu_accessible_gb", 64.0, "GB")]);

        assert_eq!(out.passed.len(), 1);
        assert_eq!(out.passed[0].item_id, "roomy");
        assert_eq!(out.excluded.len(), 1, "only the measured miss");
        assert_eq!(out.excluded[0].item_id, "cramped");

        assert_eq!(out.indeterminate.len(), 1);
        let r = out.indeterminate[0].reason.to_lowercase();
        assert!(r.contains("unknown") || r.contains("not recorded"), "got: {r}");
        assert!(!r.contains('<'), "must not claim a comparison it never made: {r}");
    }

    /// 96 >= 64 numerically, so only unit-checking produces this exclusion.
    #[test]
    fn a_value_in_the_wrong_unit_is_indeterminate_not_silently_compared() {
        let items = vec![item("euro-spec", &[("gpu_accessible_gb", v(96.0, "GiB"))])];
        let out = apply(&schema(), &items, &[Constraint::gte("gpu_accessible_gb", 64.0, "GB")]);
        assert!(out.passed.is_empty(), "GiB and GB differ by 7.4%");
        assert_eq!(out.indeterminate.len(), 1);
        assert!(out.indeterminate[0].reason.contains("GiB"), "{:?}", out.indeterminate[0]);
    }

    #[test]
    fn every_comparison_operator_works_at_its_boundary() {
        let items = vec![item("edge", &[("gpu_accessible_gb", v(64.0, "GB"))])];
        let s = schema();
        for (c, should_pass) in [
            (Constraint::gte("gpu_accessible_gb", 64.0, "GB"), true),
            (Constraint::lte("gpu_accessible_gb", 64.0, "GB"), true),
            (Constraint::eq("gpu_accessible_gb", 64.0, "GB"), true),
            (Constraint::gte("gpu_accessible_gb", 65.0, "GB"), false),
            (Constraint::lte("gpu_accessible_gb", 63.0, "GB"), false),
        ] {
            let out = apply(&s, &items, &[c]);
            assert_eq!(out.passed.len(), usize::from(should_pass), "at the boundary");
        }
    }

    /// Constraints eliminate in conjunction; the reason must name the one that
    /// actually did it, not the first one checked.
    #[test]
    fn multiple_constraints_report_the_one_that_eliminated_the_item() {
        let items = vec![item(
            "heavy",
            &[("gpu_accessible_gb", v(96.0, "GB")), ("tdp_w", v(2000.0, "W"))],
        )];
        let out = apply(
            &schema(),
            &items,
            &[
                Constraint::gte("gpu_accessible_gb", 64.0, "GB"),
                Constraint::lte("tdp_w", 1600.0, "W"),
            ],
        );
        assert_eq!(out.excluded.len(), 1);
        let r = &out.excluded[0].reason;
        assert!(r.contains("tdp_w"), "must name the failing constraint: {r}");
        assert!(!r.contains("gpu_accessible_gb"), "must not blame a passing one: {r}");
    }

    #[test]
    fn a_constraint_on_an_attribute_the_schema_does_not_know_is_loud() {
        let items = vec![item("x", &[("gpu_accessible_gb", v(96.0, "GB"))])];
        let out = apply(&schema(), &items, &[Constraint::gte("warp_factor", 9.0, "c")]);
        assert!(out.passed.is_empty());
        assert_eq!(out.indeterminate.len(), 1);
        assert!(
            out.indeterminate[0].reason.contains("warp_factor"),
            "a typo'd attribute must be loud, not a silent empty set: {:?}",
            out.indeterminate[0]
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-cli-core market::`
Expected: FAIL — `cannot find function apply`.

- [ ] **Step 3: Write the implementation**

```rust
/// Trust tier of an observed value. Ordering IS the precedence rule.
///
/// Scoped to market attributes (price, stock, availability) when it gates a
/// comparison: an A6000's 48 GB is 48 GB whether a search snippet or a checkout
/// page reported it. Applying an evidence ladder to a physical spec is a
/// category error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    Aggregator = 1,   // price tracker, no merchant page
    SearchIndex = 2,  // search snippet, page never loaded
    MerchantPage = 3, // page fetched, no stock text
    Transactable = 4, // page fetched, cart live, stock stated
    Derived = 5,      // computed from an observation by a named, versioned rule
}

#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    pub number: f64,
    pub unit: Option<String>,
    pub evidence: Evidence,
}

#[derive(Debug, Clone)]
pub struct CatalogItem {
    pub item_id: String,
    pub category: String,
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp { Gte, Lte, Eq }

#[derive(Debug, Clone)]
pub struct Constraint {
    pub attribute: String,
    pub op: CmpOp,
    pub value: f64,
    pub unit: String,
}

impl Constraint {
    pub fn gte(attribute: &str, value: f64, unit: &str) -> Self {
        Self::new(attribute, CmpOp::Gte, value, unit)
    }
    pub fn lte(attribute: &str, value: f64, unit: &str) -> Self {
        Self::new(attribute, CmpOp::Lte, value, unit)
    }
    pub fn eq(attribute: &str, value: f64, unit: &str) -> Self {
        Self::new(attribute, CmpOp::Eq, value, unit)
    }

    fn new(attribute: &str, op: CmpOp, value: f64, unit: &str) -> Self {
        Self { attribute: attribute.into(), op, value, unit: unit.into() }
    }

    fn satisfied_by(&self, v: f64) -> bool {
        match self.op {
            CmpOp::Gte => v >= self.value,
            CmpOp::Lte => v <= self.value,
            // ponytail: exact f64 equality. Both sides are literals — one from a
            // contract, one from a caller — never arithmetic results. Switch to
            // an epsilon if a derivation ever feeds this path.
            CmpOp::Eq => v == self.value,
        }
    }

    fn describe(&self) -> &'static str {
        match self.op { CmpOp::Gte => ">=", CmpOp::Lte => "<=", CmpOp::Eq => "==" }
    }
}

#[derive(Debug, Clone)]
pub struct Excluded {
    pub item_id: String,
    pub reason: String,
}

/// `indeterminate` is deliberately separate from `excluded`. Folding them loses
/// the distinction between "measured, falls short" and "never measured" — and
/// only the second is a data gap someone can close.
#[derive(Debug, Default)]
pub struct Outcome {
    pub passed: Vec<CatalogItem>,
    pub excluded: Vec<Excluded>,
    pub indeterminate: Vec<Excluded>,
}

enum Verdict { Passed, Excluded(String), Indeterminate(String) }

fn evaluate(schema: &CatalogSchema, item: &CatalogItem, c: &Constraint) -> Verdict {
    let Some(def) = schema.attribute(&item.category, &c.attribute) else {
        return Verdict::Indeterminate(format!(
            "`{}` is not an attribute of category `{}` in the catalog schema",
            c.attribute, item.category
        ));
    };
    let Some(v) = item.attributes.get(&c.attribute) else {
        return Verdict::Indeterminate(format!(
            "`{}` is unknown for this item — not recorded by any source",
            c.attribute
        ));
    };
    // String equality, not conversion. GiB vs GB is a real 7.4% difference, and
    // guessing which was meant is how a wrong answer looks confident.
    if v.unit.as_deref() != Some(c.unit.as_str()) {
        return Verdict::Indeterminate(format!(
            "`{}` is recorded in {} but the constraint is in {} — no conversion is defined",
            c.attribute,
            v.unit.as_deref().unwrap_or("no unit"),
            c.unit
        ));
    }
    if c.satisfied_by(v.number) {
        return Verdict::Passed;
    }
    let mut reason = format!(
        "{} is {} {}, which fails {} {} {}",
        c.attribute, v.number, c.unit, c.attribute, c.describe(), c.value
    );
    if let Some(note) = def.note.as_deref() {
        reason.push_str(&format!(" ({note})"));
    }
    Verdict::Excluded(reason)
}

/// Hard filters. Constraints eliminate; they never score.
pub fn apply(
    schema: &CatalogSchema,
    items: &[CatalogItem],
    constraints: &[Constraint],
) -> Outcome {
    let mut out = Outcome::default();
    'items: for item in items {
        for c in constraints {
            match evaluate(schema, item, c) {
                Verdict::Passed => {}
                Verdict::Excluded(r) => {
                    out.excluded.push(Excluded { item_id: item.item_id.clone(), reason: r });
                    continue 'items;
                }
                Verdict::Indeterminate(r) => {
                    out.indeterminate.push(Excluded { item_id: item.item_id.clone(), reason: r });
                    continue 'items;
                }
            }
        }
        out.passed.push(item.clone());
    }
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -j 4 -p vox-cli-core market::`
Expected: 11 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli-core/src/market.rs
git commit -m "feat(market): three-valued constraints with explained exclusions"
```

---

### Task 3: Acceptance test — the laptop query, with a derived value

**Files:**
- Create: `crates/vox-cli-core/tests/fixtures/laptops_seed.yaml`
- Create: `crates/vox-cli-core/tests/market_acceptance.rs`
- Modify: `crates/vox-cli-core/src/market.rs`

**Interfaces:**
- Consumes: everything from Tasks 1 and 2.
- Produces: `Arch::{AppleUnified, StrixHalo, Discrete}`;
  `derive_gpu_accessible_gb(total_gb: f64, arch: Arch) -> Value`;
  `seed_from_yaml(&CatalogSchema, &str) -> Result<Vec<CatalogItem>, SchemaError>`.

**Why a derivation and not a literal:** the first draft hand-seeded
`gpu_accessible_gb: 48` and asserted `48 < 64` — Rust's `<` operator on two
numbers a human typed. It also stamped that number `MerchantPage`, though no
merchant page states it. The figure is a rule of thumb, so it is stored as one:
`Derived`, from an observed `total_memory_gb`, by a named rule. The reserve is
per-architecture because it is not a universal 25% — Apple's is adjustable via
`iogpu.wired_limit_percent`, and Strix Halo's is a UEFI setting.

- [ ] **Step 1: Write the failing test**

`crates/vox-cli-core/tests/market_acceptance.rs`:

```rust
//! The 2026-08-21 laptop query, end to end. This is the gate that unlocks the
//! discovery pipeline, so it asserts the criterion the spec actually states —
//! that the answer EXPLAINS the unified-memory reservation — not merely 48 < 64.

use vox_cli_core::market::*;

fn schema() -> CatalogSchema {
    CatalogSchema::load_from_str(include_str!(
        "../../../contracts/market/catalog-schema.v1.yaml"
    ))
    .expect("shipped contract parses")
}

fn seeded() -> Vec<CatalogItem> {
    seed_from_yaml(&schema(), include_str!("fixtures/laptops_seed.yaml")).expect("seed")
}

#[test]
fn only_128gb_machines_satisfy_a_64gb_gpu_memory_constraint() {
    let out = apply(&schema(), &seeded(), &[Constraint::gte("gpu_accessible_gb", 64.0, "GB")]);

    let passed: Vec<&str> = out.passed.iter().map(|i| i.item_id.as_str()).collect();
    assert!(passed.contains(&"mbp16-m5max-128"), "got {passed:?}");
    assert!(passed.contains(&"rog-flow-z13-128"), "got {passed:?}");
    assert!(
        !passed.contains(&"mbp14-m5pro-64"),
        "a 64GB machine exposes ~48GB and must not pass: {passed:?}"
    );

    let why = out
        .excluded
        .iter()
        .find(|e| e.item_id == "mbp14-m5pro-64")
        .expect("a measured miss, not an indeterminate");

    let r = why.reason.to_lowercase();
    assert!(r.contains("48"), "must state the derived value: {}", why.reason);
    assert!(r.contains("64"), "must state the requirement: {}", why.reason);
    // The criterion the spec states. A generic
    // "gpu_accessible_gb 48 GB < required 64 GB" satisfies the two lines above
    // and fails this one — deliberately.
    assert!(
        r.contains("unified") || r.contains("reserves"),
        "must explain WHY a 64GB machine offers 48GB: {}",
        why.reason
    );
}

/// The seed states `total_memory_gb`, which a vendor page really does say. The
/// 48 must come from the derivation, so a source claiming
/// `gpu_accessible_gb: 64` outright cannot quietly become the catalog's answer.
#[test]
fn gpu_accessible_memory_is_derived_from_total_memory_not_asserted() {
    let items = seeded();
    let mbp = items.iter().find(|i| i.item_id == "mbp14-m5pro-64").unwrap();

    let total = mbp.attributes.get("total_memory_gb").expect("observed");
    assert_eq!(total.number, 64.0);
    assert_eq!(total.evidence, Evidence::MerchantPage, "a page really does state 64GB");

    let derived = mbp.attributes.get("gpu_accessible_gb").expect("derived");
    assert_eq!(derived.number, 48.0);
    assert_eq!(
        derived.evidence,
        Evidence::Derived,
        "no merchant page states 48GB — it must not borrow a page's provenance"
    );
}

#[test]
fn a_source_may_not_assert_a_derived_attribute() {
    const CHEATING: &str = r#"
items:
  - item_id: liar
    category: laptop
    arch: apple_unified
    attributes:
      total_memory_gb:   { number: 64, unit: GB, evidence: merchant_page }
      gpu_accessible_gb: { number: 64, unit: GB, evidence: merchant_page }
"#;
    let e = seed_from_yaml(&schema(), CHEATING).unwrap_err().to_string();
    assert!(e.contains("gpu_accessible_gb"), "got: {e}");
}

#[test]
fn the_reserve_is_per_architecture_not_a_flat_quarter() {
    // Apple's reserve and Strix Halo's are set by different mechanisms
    // (iogpu.wired_limit_percent vs a UEFI setting), so one shared constant
    // would be a coincidence rather than a rule. Asserted separately so a
    // future per-architecture correction touches one line and one assertion.
    assert_eq!(derive_gpu_accessible_gb(64.0, Arch::AppleUnified).number, 48.0);
    assert_eq!(derive_gpu_accessible_gb(128.0, Arch::AppleUnified).number, 96.0);
    assert_eq!(derive_gpu_accessible_gb(128.0, Arch::StrixHalo).number, 96.0);
    // A discrete GPU's VRAM is not carved out of system memory at all.
    assert_eq!(derive_gpu_accessible_gb(64.0, Arch::Discrete).number, 64.0);
}

#[test]
fn an_impossible_constraint_explains_itself_rather_than_returning_empty() {
    let items = seeded();
    let out = apply(&schema(), &items, &[Constraint::gte("gpu_accessible_gb", 512.0, "GB")]);

    assert!(out.passed.is_empty());
    assert_eq!(out.excluded.len(), items.len(), "every item carries a reason");
    for e in &out.excluded {
        // "Explains itself" was previously unasserted: the first draft checked
        // only that the exclusion list was the right LENGTH, which an
        // implementation emitting empty strings satisfies.
        assert!(!e.reason.trim().is_empty(), "empty reason for {}", e.item_id);
        assert!(e.reason.contains("512"), "must name the requirement: {}", e.reason);
    }
    // The closest candidate must be legible, so the reader learns how far off
    // the requirement is rather than only that nothing matched.
    assert!(
        out.excluded.iter().any(|e| e.reason.contains("96")),
        "the best machine in the catalog must be visible: {:?}",
        out.excluded
    );
}
```

- [ ] **Step 2: Write the fixture**

`crates/vox-cli-core/tests/fixtures/laptops_seed.yaml`:

```yaml
# Observed values only. `gpu_accessible_gb` is absent by design — it is derived
# at load time from `total_memory_gb` and `arch`, and carries Evidence::Derived.
items:
  - item_id: mbp14-m5pro-64
    category: laptop
    arch: apple_unified
    attributes:
      total_memory_gb: { number: 64,   unit: GB,  evidence: merchant_page }
      price_usd:       { number: 3699, unit: USD, evidence: merchant_page }
      weight_kg:       { number: 1.55, unit: kg,  evidence: merchant_page }
  - item_id: mbp16-m5max-128
    category: laptop
    arch: apple_unified
    attributes:
      total_memory_gb: { number: 128,  unit: GB,  evidence: merchant_page }
      price_usd:       { number: 5999, unit: USD, evidence: merchant_page }
      weight_kg:       { number: 2.15, unit: kg,  evidence: merchant_page }
  - item_id: rog-flow-z13-128
    category: laptop
    arch: strix_halo
    attributes:
      total_memory_gb: { number: 128,  unit: GB,  evidence: merchant_page }
      price_usd:       { number: 2799, unit: USD, evidence: merchant_page }
      weight_kg:       { number: 1.20, unit: kg,  evidence: merchant_page }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -j 4 -p vox-cli-core --test market_acceptance`
Expected: FAIL — `cannot find function seed_from_yaml`.

- [ ] **Step 4: Write the derivation and loader**

Append to `crates/vox-cli-core/src/market.rs`:

```rust
/// Memory architecture, which decides how much of installed RAM the GPU can
/// actually address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arch { AppleUnified, StrixHalo, Discrete }

/// `unified_memory_reserve_v1`.
///
/// Neither reserve is a hardware constant: Apple's is adjustable via
/// `iogpu.wired_limit_percent`, Strix Halo's is a UEFI/driver setting. These
/// are the shipped defaults, which is what a buyer gets out of the box — and
/// why the output is `Evidence::Derived` with the rule named and versioned,
/// rather than a figure wearing a merchant page's provenance.
pub fn derive_gpu_accessible_gb(total_gb: f64, arch: Arch) -> Value {
    let usable = match arch {
        Arch::AppleUnified | Arch::StrixHalo => (total_gb * 0.75).floor(),
        // A discrete GPU's VRAM is not carved out of system memory.
        Arch::Discrete => total_gb,
    };
    Value { number: usable, unit: Some("GB".into()), evidence: Evidence::Derived }
}

#[derive(Deserialize)]
struct SeedValue {
    number: f64,
    #[serde(default)]
    unit: Option<String>,
    evidence: Evidence,
}

#[derive(Deserialize)]
struct SeedItem {
    item_id: String,
    category: String,
    #[serde(default = "discrete")]
    arch: Arch,
    #[serde(default)]
    attributes: BTreeMap<String, SeedValue>,
}

fn discrete() -> Arch { Arch::Discrete }

#[derive(Deserialize)]
struct Seed { items: Vec<SeedItem> }

/// Loads observed values and computes derived ones.
///
/// A seed that asserts a derived attribute directly is rejected: letting a
/// source hand-write `gpu_accessible_gb` is precisely the unprovenanced number
/// this layer exists to stop.
pub fn seed_from_yaml(
    schema: &CatalogSchema,
    yaml: &str,
) -> Result<Vec<CatalogItem>, SchemaError> {
    let seed: Seed = serde_yaml::from_str(yaml)?;
    let mut items = Vec::with_capacity(seed.items.len());
    for s in seed.items {
        let mut attributes: BTreeMap<String, Value> = s
            .attributes
            .into_iter()
            .map(|(k, v)| (k, Value { number: v.number, unit: v.unit, evidence: v.evidence }))
            .collect();

        if attributes.contains_key("gpu_accessible_gb") {
            return Err(SchemaError::AssertedDerived("gpu_accessible_gb".into()));
        }
        if let Some(total) = attributes.get("total_memory_gb") {
            let derived = derive_gpu_accessible_gb(total.number, s.arch);
            attributes.insert("gpu_accessible_gb".into(), derived);
        }
        // Reserved: required-attribute checking belongs at promotion time,
        // which arrives with the store. Named here so the parameter is not
        // mistaken for an oversight.
        let _ = schema;
        items.push(CatalogItem { item_id: s.item_id, category: s.category, attributes });
    }
    Ok(items)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -j 4 -p vox-cli-core`
Expected: all pass — 11 unit, 5 acceptance.

- [ ] **Step 6: Run the gates**

```bash
vox run scripts/fmt.vox && cargo run -q -j 4 -p vox-cli -- ci pre-push --complete
```

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli-core/src/market.rs crates/vox-cli-core/tests/
git commit -m "feat(market): acceptance test with derived unified-memory capacity"
```

---

## Deferred: the fetch layer

Everything below was Tasks 2–5, 7, and 8 of the first draft. It is real work and
it is not on the path to the acceptance gate. Each item carries the critique
findings that must be addressed *before* it is written, so they are not
rediscovered.

**Two things to settle before any of it starts:**

1. **Read `crates/vox-populi/src/mens/cloud/`** and decide whether this belongs
   alongside `vast.rs` / `runpod_provider.rs` rather than in a parallel adapter
   registry. That decision changes everything below it.
2. **Decide who fetches.** The spec's open question ("recommend daemon-only,
   decided before implementation") was never decided, and the first draft built
   both the CLI and the GUI IPC surface with no owner. Two schedulers means
   duplicate fetches, which halves the quota ceiling below, and two budget
   counters means no enforcement at all. One paragraph, not work.

### No writer exists — the entry-point blocker
The first draft had no fetch loop, no `vox market add`, and no catalog file
format. Nothing could put a row in `market_items` except a test seed. Whatever
lands first must include a way to populate the catalog, or the feature has no
entry point.

### Quota, not disk, is the binding limit
One eBay fetch returns price + stock + availability together, so the 0.25 h
stock TTL sets the cadence at 96 fetches/item/day. Against the free tier's
5,000 calls/day that is **52 items** — 26 if two processes fetch. Nothing in the
first draft mentioned a rate limit, a daily counter, or a cadence backstop.
Derive max catalog size from the quota, or refuse to build an oversized
`RefreshPlan`.

Disk is the slower problem but still real: 73,000 observations/item/year/source
at ~175 B/row is 128 MB/yr at 10 items, 1.28 GB/yr at 100 — landing in the
shared `.vox/store.db`. `contracts/db/retention-policy.yaml` already exists and
supports `kind: ms_days`; one entry fixes it.

### Security preconditions

These are not hardening passes to schedule later; each one is cheaper to build
in than to retrofit, and the first must land in the same commit as the column
it protects.

- **Strip query and fragment from `source_url` at write time.** Response URLs
  carry affiliate params, session ids, and sometimes the API key itself. Rows
  land in `.vox/store.db`, which `DbConfig::from_env`
  (`crates/vox-db/src/config.rs:115`) replicates to a remote libSQL primary
  whenever `VOX_DB_URL` + `VOX_DB_TOKEN` resolve — so append-only means a leaked
  token is retained forever *and* pushed off-box. `url` 2 is already a workspace
  dep (`Cargo.toml:400`); this is ~6 lines. Keep any parameter needed for
  re-fetch in its own typed column.
- **Keep `MarketError` payload-free.** Five variants, no body. Adding
  `ParseFailed(String)` holding the raw response is what turns an error row into
  a credential row, and it looks like an obvious debugging improvement.
- **Add a runtime host allowlist before the first adapter fetches.** There is no
  outbound host policy anywhere in the repo: `resolve_egress` is a
  provider-to-URL resolver honouring `base_url_override` unconditionally, and
  `vox-http-client` carries no host checks. Reconciliation follows URLs that came
  from search results, so the target is partly attacker-influenced — SSRF-shaped.
  A const list plus `allowed_host(&str) -> bool` beside the adapter registry is
  ~10 lines. Set `reqwest::redirect::Policy::none()` or re-check the host after
  redirects, or a 302 defeats it. Do not try to do this with a `vox-code-audit`
  detector: the risk is a runtime-constructed URL, which source-grepping cannot
  see.
- **eBay is two secrets, not one.** `VoxMarketEbayClientId` +
  `VoxMarketEbayClientSecret` as sibling `SecretSpec` entries in one `SPECS_*`
  const — the Reddit pattern at
  `crates/vox-secrets/src/spec/registry/social.rs:50-73`. There is no pair type
  and none is needed. Remember the taxonomy match arms in `spec/mod.rs`, or
  `secrets-parity` fails.
- **The 2-hour access token is memory-only, not a `SecretId`.** A `SecretSpec`
  is a user-configured input; a minted token is derived state. Use a
  `SnapshotCache`-style static with an expiry (`resolve_egress.rs:35-37`), which
  also removes the per-attribute-per-tick Credential Manager round-trip noted
  under the adapter above.
- **Budget: copy the check, not the type.** `BudgetLedger::check_capacity` is
  six lines of arithmetic wrapped in cloud-GPU specifics — constructed from
  `CloudProviderConfig.max_budget_usd`, accruing from `cloud_dispatch_log`,
  taking a `JobHandle`/`gpu_name`/`vram_mb`, pro-rating *running* jobs by
  elapsed time, and hardcoding `--max-budget=N` in its error text. A fetch
  budget is a discrete per-call debit against a daily reset — a different
  accrual model — and reusing the type would take a `vox-populi` crate edge this
  plan exists to avoid. Defactor per AGENTS.md
  (`// vox:defactored-from vox-populi <date>`, under 50 lines, no edge). Lazier
  still and the right first move: make `is_available()` false for any source
  with `cost_usd() > 0.0` until a real ledger exists.

**Checked and fine, no work needed:** no PII is captured anywhere in the specced
field set, and no new third-party crate is required — `reqwest`,
`reqwest-middleware`, `reqwest-retry`, `governor`, `scraper`, `quick-xml`, and
`url` are all already in the workspace root `Cargo.toml`.

### Store
- No `crates/vox-db/migrations/` exists. DDL is a `SchemaFragment` const in
  `crates/vox-db/src/schema/domains/market.rs`, registered in `manifest.rs`,
  with `BASELINE_VERSION` 90 → 91 and a changelog comment.
- **The drafted instruction actively bricks the DB.** A `Migration` at any
  version other than `BASELINE_VERSION` makes the next ordinary `VoxDb::connect`
  return `StoreError::LegacySchemaChain`. "Use the next free number" is the one
  instruction that must not be followed.
- `VoxDb::connect(DbConfig::Memory)`, not `connect_in_memory()`. Needs
  `features = ["local"]`. `turso::params!`, not `vox_db::params!`.
- Raw SQL outside `vox-db` fails `sql-surface-guard` and `turso-import-guard`.
  Expose typed `impl VoxDb` accessors in `store/ops_market.rs`.
- `current()` must not scan full history: `ORDER BY observed_at_ms DESC LIMIT 50`
  turns O(all rows) into O(sources). Without it the UI is unusable at ~2 months
  with 10 items, and it holds the process-wide `GuardedConnection` mutex while
  doing it — starving every other DB consumer, chat writes included.
- Cap `alternates` at one per source, or `market_item_detail` returns ~20 MB of
  JSON per IPC call at one year.
- Write only on change, so `observed_at_ms` means "last changed" rather than
  "last polled" — which the recency tie-break already assumes.
- `record()` must retry on cross-process `Busy`; today the observation is lost
  after the HTTP call was already spent.
- Wrap item + attributes in one transaction and validate `required_attributes()`
  before committing. That method exists and is called by nobody.
- Test append-only as **immutability**, not a row count — a count check passes
  an implementation that appends *and* rewrites.
- `since_ms` inclusivity needs a row exactly on the bound to be tested at all.
- Attribute renames silently orphan history (`WHERE attribute = ?` on the new
  name returns nothing). `vram_gb` → `gpu_accessible_gb` already happened once.

### Offers, not prices
`market_offers(item_id, source_id, vendor, condition, currency, price, shipping, ...)`,
reconciled within `(item, source, condition)`. See spec §Price is a property of
an offer. A schema change now that costs a rewrite later.

### eBay adapter
- `resolve_secret` returns `ResolvedSecret`; use `.is_present()`, not `.is_ok()`.
- Append the `SecretSpec` to an existing `SPECS_*` const; a new file also needs
  `ALL_REGISTRIES` wiring in `spec/mod.rs`.
- Run `vox ci secrets-contracts` and commit the regenerated
  `contracts/secrets/managed-env-names.v1.json` **before** `secrets-parity` runs.
  Update `docs/src/reference/secrets-ssot.md`.
- `is_available()` calls `resolve_secret` per (item, attribute, source) — a
  Credential Manager round-trip per attribute per tick on Windows. Resolve once
  at construction.
- Item identity is a keyword search: a "$59 A6000 cooling shroud" becomes the
  item's `Transactable` price and wins reconciliation outright. Require an exact
  per-source identifier and record the match rule as provenance.
- `estimatedAvailableQuantity` is documented as an *estimate*, often bucketed.
  Map it to `in_stock: yes`, never a numeric `stock_count` — this is the
  "99,999 vs 4" failure with a different label.
- Test FIXED_PRICE-*without*-quantity: the spec says a cart button alone is not
  `Transactable`, and nothing tested it.
- Guard fixture `.replace()` calls with `assert_ne!(body, FIXTURE)`, or a
  pretty-printed fixture makes the substitution a silent no-op.

### Scheduler
- `Backoff` duplicates `vox_foundation::primitives::backoff` and ignores
  `vox_http_client::parse_retry_after` — the documented SSOT, and the only thing
  that matters for a 429.
- `RefreshPlan::build` had zero tests. Needs: keyless source skipped not failed,
  cold-start (never-observed) attributes scheduled, specs never scheduled.
- Staleness must be computed from last **attempt**, not last **success**, or a
  `NotFound` item is re-fetched every tick forever — the spec's own failure
  table says "do not retry until config changes", and nothing implements it.
- `now_ms().unwrap_or(0)` records 1970 on a clock error: stale forever, and
  loses every recency tie-break. Refuse to write instead.
- `is_stale` goes negative on a backwards clock step and the attribute **freezes**.
  Treat a negative delta as stale and self-heal.
- No jitter anywhere. Seven days asleep means every attribute is stale on wake
  and the plan emits them all in one pass.
- The budget ceiling was deferred while stock TTL is 0.25 h. Ship the guard with
  the first metered adapter, or make `is_available()` false for any
  `cost_usd() > 0.0` source until it exists. `BudgetLedger::check_capacity`
  already exists in `vox-populi`.
- `cost_usd()` is a trait method with no consumer, though Decision 1 claims it
  drives source ordering. Implement it or drop the claim.
- No health output at all: "no credentials", "blocked since Tuesday",
  "scheduler panicked", and "working fine" all render identically.
  `SELECT source_id, MAX(observed_at_ms) ... GROUP BY source_id` is one indexed
  query and the highest value-per-line item in the whole critique.

### Search corpus
- `SearchCorpus` is a **closed enum** in L0 `vox-db-types`. There is no
  registration API. Use `persist_text_document_chunk` with a `market:` URI
  prefix over `DocumentChunks`, or accept an L0 change.
- `lexical_tantivy` exposes only `rebuild(dir, &[docs])` — full rebuild, no
  incremental add. Per-observation rebuild is ~5,000 full rebuilds/day.
- Index staleness is bounded by catalog *membership* churn, not observation
  churn, because the document holds no price or stock. Rebuild on membership
  change and say so in the spec.
- Defer regardless: lexical search over ~10 hand-entered items is `.contains()`.

### CLI and IPC
- `crates/vox-gui/src/commands/mercatus.rs` already exists and is already
  registered — do not create a parallel `market.rs`.
- The gate chain is five commands, not one: `operations-sync --target cli
  --write` → `command-sync --write` → `UPDATE_CLI_CATALOG_BASELINE=1 cargo test
  -p vox-cli --test command_catalog_paths_baseline` → `gui-surface-registry
  --write` + `gui-surface-coverage --write` → `ssot-drift`.
- PowerShell has no inline env-var prefix:
  `$env:UPDATE_CLI_CATALOG_BASELINE = '1'; cargo test …`.
- Dispatch needs arms in **three** places in `cli_dispatch/mod.rs`, not one.
- The `--where` parser is the surface a user types into and had no tests. A
  unitless expression must be rejected, not guessed — otherwise `tdp_w <= 1600`
  and `vram_gb >= 1600` become the same comparison.
- `--json` is a contract; assert its shape, including that `evidence` and
  `observed_at_ms` reach consumers.
- This is where the runtime schema load path belongs, which is what makes the
  spec's "adding a category is a config edit" true rather than a recompile.

### If the crate is eventually created
- Layer **3**, not 1, in `contracts/ci/crate-layers.v1.json`.
- Also needs: `layers.toml` row, `where-things-live.md` row, `workspace-hack`
  dep, `cargo hakari manage-deps && cargo hakari generate`, and
  `vox ci affected-crates --regen --out contracts/ci/crate-graph.v1.json` —
  this last one is in the *fast* pre-push tier, so it fails first.
- Six crate-edge exceptions and four `fan-in-snapshot.v1.json` bumps. Both
  baselines are **USER-AUTHORIZED-ONLY**: propose in the PR description and stop.
- `vox-arch-check`'s `orphan = "error"` fires from crate creation until the
  first consumer is wired. Do not paper over it with a fake consumer.

---

## Self-Review

**Spec coverage.** Tasks 1–3 cover the schema contract, three-valued
constraints, unit checking, the derivation, and the acceptance gate. Everything
else in the spec is explicitly deferred above with its blocking findings
attached — not silently dropped.

**Placeholder scan.** No TBDs. Every step has runnable code or a runnable
command. The `let _ = schema;` in `seed_from_yaml` is a commented parameter
reservation, not a stub.

**Type consistency.** `Evidence`, `Value`, `CatalogItem`, `Arch`, `CmpOp`,
`Constraint`, `Outcome`, `Excluded` are each defined once and used consistently.
`Evidence` carries `Deserialize` + `rename_all = "snake_case"` from Task 2 so
the Task 3 fixture parses. `SchemaError::AssertedDerived` is declared in Task 1
and used in Task 3.

**Known weak point.** `derive_gpu_accessible_gb` uses 0.75 for both Apple and
Strix Halo. Those are shipped defaults from two different mechanisms, and the
test asserts them separately so a future per-architecture correction changes one
line and one assertion rather than a shared constant. The rule is versioned
(`_v1`) so that if a measured figure contradicts either, the old values stay
attributable rather than being silently restated.
