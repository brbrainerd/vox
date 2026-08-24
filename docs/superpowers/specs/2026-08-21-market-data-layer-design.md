---
title: "Market data layer and catalog (Mercatus piece A) — design"
description: "Typed catalog, source adapters, freshness policy, evidence-class reconciliation, constraint filtering with explainable scoring, and a staged discovery pipeline — turning Mercatus from a config viewer into a live comparison tool."
category: "Architecture SSOTs"
---

# Market data layer and catalog — piece A design

Brainstormed 2026-08-21. Foundation piece of a four-part program
(A data layer + catalog → B surface + CLI → C benchmark layer → D build optimizer).
Each later piece gets its own spec; nothing below assumes they exist.

## The problem, from evidence

The Mercatus surface is a **read-only viewer for a price-watch system that was
never built**. Established by direct inspection on 2026-08-21:

- `crates/vox-gui/src/commands/mercatus.rs` is ~80 lines of `fs::read_to_string`
  plus `serde_json::from_str`. No fetch, no scheduler, no cache.
- The config it reads lives at `<config_dir>/storage-tier/price-watch/price-watch.config.json`.
  Nothing in the repository writes that file.
- `_meta.sources[].costUsd` / `.cadenceHours` / `.tier` are consumed by nothing.
  They describe a scheduler's inputs; the scheduler is absent.
- The surface renders a **coverage matrix** — which source IDs are pinned per
  part — not prices. There is no price anywhere in the system.
- `contracts/gui/surface-registry.v1.yaml` gives mercatus `cli_group: null`.
  No `vox market`, no MCP tool, no `.vox` script.

## Why this spec is larger than "fetch some prices"

A full manual pass on 2026-08-21 — researching and pricing a workstation build —
produced the requirements below. Every one is a failure that actually occurred:

| What happened | Requirement it creates |
|---|---|
| A price tracker said $3,999; the real listing said $3,599.99; a third source said $4,799.99. The tracker figure nearly drove the recommendation. | Provenance per value, and precedence by evidence class |
| "Max Quantity: 99,999 per customer" was read as availability. Actual stock: 4. | Stock is its own attribute, distinct from purchase limits |
| A motherboard went quote-only → out-of-stock mid-session. | Availability tier is a tracked, refreshable attribute |
| One GPU listing moved 6 → 5 available while work was in progress. | Stock TTL measured in minutes, not hours |
| A card moved $1,710 in 24 hours. | Price history is append-only; last-write-wins destroys the signal |
| "≥64 GB GPU memory in a laptop" excludes every discrete-GPU machine — the useful answer is *why*. | An empty or surprising result set is a finding, not an error |
| Laptop comparison needed battery Wh, weight, measured dBA — fields a GPU-shaped record cannot hold. | Categories are data, not Rust types |

## Scope

**In:** typed catalog schema, source-adapter trait and registry, credential
wiring, freshness/TTL policy, refresh scheduler, evidence-class reconciliation,
price and attribute history, constraint filtering with explainable scoring,
vox-search integration, and the IPC/CLI read surface. Discovery pipeline is
**specified here and built behind a flag** (see §Discovery).

**Out:** the GUI comparison view (piece B), benchmark ingestion (C), the
constraint solver for whole builds (D). Also out: editing the watchlist from the
GUI — the catalog stays file-and-CLI-driven for now.

## Decisions taken during brainstorming

1. **Per-source adapters.** Free official APIs where they exist (eBay Browse,
   Best Buy), Bright Data for retailers with none. This makes `cost_usd`
   meaningful: the scheduler prefers free sources and spends the paid tier only
   where coverage requires it.
2. **Data-driven catalog now, discovery later.** Adding a category must not
   require a code change. The schema is designed so a discovery pipeline can
   write into it without a rewrite.
3. **Constraints plus explicit scoring.** Hard filters eliminate; a visible,
   tunable formula ranks survivors. No opaque or model-judged ranking.
4. **Evidence class wins over price.** A verified, in-stock, add-to-cart price
   outranks a cheaper unverified one. The cheaper figure stays visible as a
   footnote.
5. **Search goes through `vox-search`.** Per AGENTS.md, richer retrieval must
   build on the existing hybrid stack. `vox_search_query` already returns
   `facets_by_source` / `facets_by_kind`, so this adds a corpus, not an engine.
6. **Sequence A → B → C → D**, foundation first.

## Architecture

### Crate placement

A new leaf crate, `vox-market`, at **L1** in `contracts/ci/crate-layers.v1.json`.
Depends on `vox-http-client`, `vox-secrets`, `vox-db`, and `vox-search` — all
existing, all same-layer or lower. Only `vox-gui` and `vox-cli` (L4) depend on
it, so the crate-edges ratchet needs two L4→L1 edges.

Rejected: putting it in `vox-cli-*`. Price fetching is not a CLI concern, and
piece B needs it from the GUI process.

### The catalog schema is a contract, not Rust types

`contracts/market/catalog-schema.v1.yaml` defines categories and their attributes:

```yaml
schema_version: 1
attribute_kinds:            # TTL and volatility class, inherited by attributes
  spec:         { ttl_hours: null }      # immutable: an A6000 is always 48 GB
  price:        { ttl_hours: 6 }
  stock:        { ttl_hours: 0.25 }
  availability: { ttl_hours: 6 }

# Applied to every category. Price and stock are not category-specific, and
# leaving them implicit is how a schema ends up with two spellings of "price".
universal_attributes:
  price_usd:     { kind: price,        type: number, unit: USD, required: true }
  in_stock:      { kind: stock,        type: enum, values: [yes, no, unknown] }
  stock_count:   { kind: stock,        type: number, unit: count }
  availability:  { kind: availability, type: enum, values: [transactable, quote_only, backorder, out_of_stock] }

categories:
  gpu:
    attributes:
      vram_gb:           { kind: spec,  type: number, unit: GB,   required: true }
      memory_bandwidth:  { kind: spec,  type: number, unit: GB_per_s }
      tdp_w:             { kind: spec,  type: number, unit: W,    required: true }
      power_connector:   { kind: spec,  type: enum, values: [pcie_8pin, eps_8pin, conn_12v_2x6] }
  laptop:
    attributes:
      gpu_accessible_gb: { kind: spec,  type: number, unit: GB,   required: true }
      memory_bandwidth:  { kind: spec,  type: number, unit: GB_per_s }
      battery_wh:        { kind: spec,  type: number, unit: Wh }
      weight_kg:         { kind: spec,  type: number, unit: kg }
      noise_db:          { kind: spec,  type: number, unit: dBA }
```

Adding a category is a config edit. No Rust change, no migration.

Three properties are load-bearing:

**Units are part of the type.** `vram_gb` and `tdp_w` are both numbers.
Comparing 128 against 1600 without units is how a scorer silently ranks a power
supply above a GPU. The scorer refuses to combine mismatched units.

**`required` is enforced at write time.** A laptop with no `gpu_accessible_gb`
cannot enter the catalog. A constraint filter over a null field silently drops
candidates rather than erroring — the failure where a qualifying machine never
appears and nobody notices.

**`gpu_accessible_gb`, not `vram_gb`, for laptops.** This is deliberate. Apple
and AMD unified-memory machines reserve ~25% for the OS, so a 64 GB laptop
offers ~48 GB to the GPU. A schema that stored "64" under a field named `vram_gb`
would return machines that do not meet a 64 GB constraint. The field name
encodes the question the user is actually asking.

### Every value carries its provenance

```rust
pub struct Attribute {
    pub value: AttrValue,          // typed per the schema, unit-tagged
    pub source_url: Option<String>,
    pub observed_at_ms: i64,
    pub evidence: EvidenceClass,
}

/// Ordering IS the precedence rule. Higher wins.
#[derive(PartialOrd, Ord)]
pub enum EvidenceClass {
    Aggregator = 1,    // price tracker, no merchant page
    SearchIndex = 2,   // search snippet, page never loaded
    MerchantPage = 3,  // page fetched, no stock text
    Transactable = 4,  // page fetched, add-to-cart live, stock stated
}
```

A bare number cannot distinguish "$3,999 from a tracker" from "$3,599.99 from a
listing with 5 in stock". Storing both identically is what nearly produced a
wrong $1,200 decision.

### Reconciliation: highest evidence class wins, ties break on recency

When sources disagree — which was the normal case, not the exception:

1. Highest `EvidenceClass` wins.
2. Tie → most recent `observed_at_ms`.
3. Losing values are **retained and surfaced** as alternates, never discarded.

A cheaper price from a weaker class never displaces a verified one. It renders
as *"seen at $X (unverified) — call to confirm"*. This costs real money when a
genuinely cheaper vendor blocks automation (three did on 2026-08-21), which is
exactly why the alternate stays visible rather than being dropped.

### Source adapters

```rust
#[async_trait]
pub trait MarketSource: Send + Sync {
    fn id(&self) -> &'static str;
    fn cost_usd(&self) -> f64;
    fn is_available(&self) -> bool;          // credentials resolve?
    fn evidence_class(&self) -> EvidenceClass;
    async fn fetch(&self, item: &CatalogItem) -> Result<Observation, MarketError>;
}
```

`evidence_class()` on the adapter, not the observation: a source's *maximum*
trust is a property of what it can see. An aggregator cannot emit
`Transactable` however fresh its data.

`MarketError` distinguishes `NotFound`, `Blocked` (403/429/CAPTCHA), `Timeout`,
`ParseFailed`, and `NoCredentials`. These are not interchangeable: `Blocked`
backs off and retries, `NotFound` does not retry until the config changes, and
`NoCredentials` is a configuration problem to surface rather than bury.

### Credentials

New `SecretId` entries in `crates/vox-secrets/src/spec/registry/`, resolved via
`vox_secrets::resolve_secret(...)`. No direct `env::var` in adapters.

- `BRIGHTDATA_API_KEY` — Bright Data (Amazon, Newegg, Micro Center)
- `EBAY_APP_ID` — eBay Browse API
- `BESTBUY_API_KEY` — Best Buy API

Adapters whose credentials do not resolve report `is_available() == false` and
are **skipped, not failed**. A keyless install is legitimate: the surface shows
an empty catalog and names which sources are unconfigured.

### Scheduler

One async task per enabled source, each on its own timer. Per tick it refreshes
attributes whose TTL has expired for items listing that source, sequentially,
with a small delay between requests.

Deliberately simple: **no work queue, no concurrency pool, no priority
scheduling.** Single-digit requests per hour. A queue would be infrastructure
serving a load that does not exist.

`ponytail: sequential per-source fetch; add a bounded concurrency pool if a
catalog ever exceeds ~50 items.`

Budget enforcement reuses the existing shape rather than inventing one: a
per-day spend ceiling checked before each paid fetch, mirroring
`model_route_policy::budget_guard`'s warn-then-block. Free sources are never gated.

### Persistence

Three tables in `vox-db`:

- `market_items` — catalog membership: `(item_id, category, created_at_ms)`.
- `market_observations` — **append-only**. `(item_id, attribute, value, unit,
  source_id, evidence_class, observed_at_ms, source_url)`. This is the history
  that answers "is this price unusual?" and it is why last-write-wins is wrong:
  a card moved $1,710 in 24 hours, and overwriting destroys exactly the signal
  that makes the feature worth having.
- `market_current` — materialized winner per `(item_id, attribute)` after
  reconciliation, so the common read is one indexed query.

### Constraints and scoring

Constraints are hard filters over typed, unit-checked attributes:
`gpu_accessible_gb >= 64`, `price_usd <= 6000`, `tdp_w <= 1600`. They eliminate;
they never score.

Survivors rank on named axes with visible weights, defined in
`contracts/market/scoring.v1.yaml`:

```yaml
axes:
  dollars_per_gb:    { expr: "price_usd / gpu_accessible_gb", lower_is_better: true }
  dollars_per_watt:  { expr: "price_usd / tdp_w",             lower_is_better: true }
  # Axes referencing attributes the catalog does not yet hold are SKIPPED with a
  # named reason, never silently zeroed. `tg_tok_s` arrives with piece C
  # (benchmarks); until then a throughput axis is reported as unavailable rather
  # than scoring every candidate identically — which would look like a tie
  # rather than a missing input.
  tokens_per_dollar: { expr: "tg_tok_s / (price_usd / 1000)", lower_is_better: false, requires: [tg_tok_s] }
```

An axis whose `requires` are unmet is omitted from the ranking and listed in the
explanation as *"throughput axis unavailable: tg_tok_s not in catalog"*. Piece A
therefore ranks on price, capacity and power alone — which is sufficient for the
acceptance test below, and honest about what it cannot yet weigh.

Every ranking renders its arithmetic: *"2nd: $/GB 14% better, tg/s per $1k 30%
worse."* Two rules:

**An unverified attribute cannot win a comparison.** If a ranking depends on a
value at `EvidenceClass::Aggregator` or `SearchIndex`, the item is shown, flagged,
and cannot outrank a fully-verified competitor.

**A surprising result set is a finding.** When constraints eliminate everything,
or eliminate a whole class, the response says *why* — "no discrete-GPU laptop
qualifies; only unified-memory architectures do" — rather than returning empty.
This is a required output, not a nicety: it is the answer the 2026-08-21 laptop
query actually needed.

### Search: a vox-search corpus

The catalog is indexed as a `market` corpus. `vox_search_query(query, limit,
scope)` already returns `hits`, `facets_by_source`, `facets_by_kind`, `total`,
`next_cursor`, and `corpora` — the shape a filter UI needs. Facets map to
category and attribute buckets; constraints apply as post-filters over typed
values, because lexical search cannot express `>= 64`.

No bespoke index. Per AGENTS.md, richer retrieval builds on the existing hybrid
stack.

### Discovery — specified, built behind a flag

A pipeline that **writes into the same catalog** rather than a parallel system:

**query plan** (capability spec → per-source queries) → **fetch** (reuses
`MarketSource`) → **extract** (page → typed attributes) → **reconcile** (dedup
against existing items, apply precedence) → **stage**.

Two properties make this safe:

**Discovered items land in a staging state.** They carry `SearchIndex` or
`MerchantPage` evidence by construction, so they appear flagged and cannot
outrank verified entries. Promotion to the live catalog is a review action.

**The flag is gated on validation, testably.** Discovery stays dark until the
catalog has been populated by hand and its scoring has reproduced a
recommendation the operator agrees with. "Once the data model is validated
working" becomes a checklist, not an aspiration.

### Read surface — both axes

The `cli_group: null` gap closes here, not in piece B.

- **IPC:** `market_list_items(category, constraints)`,
  `market_item_detail(item_id)` (current + alternates + provenance),
  `market_history(item_id, attribute, since_ms)`, `market_sources()`.
- **CLI:** `vox market list [--category C] [--where 'expr']`,
  `vox market show <id>`, `vox market history <id> <attr>`,
  `vox market fetch [--source S] [--dry-run]`, `vox market sources`.
- `--json` on every subcommand, per workspace convention.

`vox market sources` is the diagnostic that makes a keyless install
comprehensible: which adapters exist, which have credentials, what each costs.

## Failure behaviour

Every mode below was observed on 2026-08-21:

| Condition | Behaviour |
|---|---|
| Source blocked (403/429/CAPTCHA) | Record failure, exponential backoff, keep last good value and mark it stale |
| Item not found at source | Record `NotFound`; do not retry until config changes |
| No credentials | `is_available() == false`; skipped and reported by `vox market sources` |
| Budget ceiling hit | Paid sources pause; free sources continue |
| Sources disagree | Precedence rule; losers retained as alternates |
| Catalog absent | Empty state naming the config path — not an error |

**Stale data is labelled, never served as current.** Every value carries
`observed_at_ms`; anything past twice its TTL renders as stale. A price with no
visible age is indistinguishable from a fabricated one.

## Testing

- **Adapter contract:** one shared suite each adapter satisfies, run against
  recorded HTTP fixtures. No network in unit tests.
- **Reconciliation:** table-driven over the exact 2026-08-21 conflicts — the
  $3,999/$3,599.99/$4,799.99 GPU case, the 99,999-vs-4 stock case, the
  quote-only→out-of-stock transition. These are regression tests for real
  mistakes, not synthetic cases.
- **Scheduler:** injected clock; assert per-kind TTL, backoff, budget gating.
- **Store:** round-trip observations; assert `market_current` tracks the
  reconciliation winner, and that history is never overwritten.
- **Scoring:** assert unit mismatch is rejected; assert an unverified attribute
  cannot rank first; assert the explanation string matches the arithmetic.
- Per workspace policy every new `pub fn` lands with its test first.

## Acceptance test

The catalog is validated when this query, run through `vox market`, reproduces
the 2026-08-21 manual result without hand-holding:

> `gpu_accessible_gb >= 64` over `category: laptop`

It must return **only 128 GB machines**, and it must explain that 64 GB
configurations were excluded because ~25% of unified memory is reserved for the
OS. Returning 64 GB machines, or returning an unexplained empty set, is a
failure. This is the gate that unlocks the discovery flag.

## What this does not solve

**Without credentials, nothing fetches.** The layer is correctly inert on a
machine with no keys — reporting exactly which sources are unconfigured — but
inert. Bright Data was unauthenticated throughout the 2026-08-21 research, which
is why several prices in that work carry UNVERIFIED markers.

The first adapter to build is therefore **eBay Browse**: free API, a single app
ID, and it covers the used market where volatility is highest and where the
manual pass repeatedly failed to fetch. It proves the whole path end to end at
zero per-request cost.

**Automated fetching systematically over-reports availability.** A plain HTTP
fetch read a live cart button on a page that returns 403 to a real browser, and
cannot distinguish "Add to Cart" from "Request a quote". `EvidenceClass` mitigates
this but does not eliminate it; `Transactable` requires stock text, not just a
button.

## Open question for the next session

Whether `vox market fetch` may run from the GUI process, or whether fetching
belongs exclusively to the daemon with the GUI reading the store. Daemon-only is
cleaner and avoids two processes racing the same budget ceiling; it also means
prices do not update when only the GUI is running. **Recommend daemon-only**,
decided before implementation.
