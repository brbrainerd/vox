---
title: "Market data layer and catalog (Mercatus piece A) — design"
description: "Typed catalog, source adapters, freshness policy, evidence-class reconciliation, constraint filtering with explainable scoring, and a staged discovery pipeline — turning Mercatus from a config viewer into a live comparison tool."
category: "architecture"
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

## Prior art in this repo, which this spec originally ignored

**Added 2026-08-24.** Before writing a line of `vox-market`, read
`crates/vox-populi/src/mens/cloud/` — 3,746 lines that already solve this
problem shape against two live paid APIs:

| This spec's concept | Already shipping |
|---|---|
| Normalized cross-source record | `GpuOffer { provider, offer_id, gpu_name, vram_mb, price_per_hour_usd, reliability_pct, fetched_at }` |
| TTL staleness | `GpuOffer::is_stale(max_age)` |
| Adapters behind a trait | `CloudProvider` + `vast.rs`, `runpod_provider.rs`, `local_provider.rs` |
| Parallel fetch, filter, rank by cost | `CloudResolver::resolve` |
| Spend ceiling with a hard gate | `BudgetLedger::check_capacity` (DB-backed, and it is the budget guard this spec deferred) |
| Polling loop | `CloudWatchdog` |
| TTL + budget cap as config | `SecretId::VoxCloudPriceTtl`, `SecretId::VoxCloudMaxBudget` |
| Runtime-loaded hardware attribute catalog | `mens/config/gpu-specs.yaml` — 300 lines of `gpu -> {fp16_tflops, vram_mb}`, whose header already says "add new GPUs without recompiling" |

That last row is a direct collision: a `gpu` category in
`catalog-schema.v1.yaml` would be a second spelling of "how much VRAM does an
A6000 have", which is exactly the split-brain this spec warns against. One of
the two files must become the source and the other a view.

Two more reimplementations to drop: `Backoff` duplicates
`vox_foundation::primitives::backoff` and ignores
`vox_http_client::parse_retry_after` — documented as the SSOT for rate-limit
backoff extraction, and the only thing that matters for a 429.

None of this appeared in the original spec because I did not look. The
architecture below is not wrong for having been derived independently, but it
must be reconciled with what ships before it is built.

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

**Corrected 2026-08-24.** The original text placed `vox-market` at L1. That is
arithmetically impossible and was a factual error, not a judgement call.
`contracts/ci/crate-layers.v1.json` has `vox-db: 2` and `vox-search: 3`, and
`crate_edges::check` emits `UpwardEdge` whenever `from_layer < to_layer`. An L1
crate depending on both produces two guaranteed violations. The floor set by its
own dependencies is **L3**.

The layer arithmetic is a symptom. The real finding is that **piece A does not
need a crate at all.**

The acceptance test — `apply(&items, &[Constraint::gte("gpu_accessible_gb", 64.0)])`
over a seeded fixture — touches no store, no adapter, no scheduler, no search,
no CLI. It is delivered by the schema loader plus the constraint evaluator:
roughly 200 lines with no I/O. Under the repo's own defactor policy that is far
below the bar for a crate edge, let alone a crate.

A new crate costs a workspace member, a layer assignment, **six** ratcheted
crate edges (the ratchet is an exact set — outbound edges count, so the original
"two edges" was wrong by four), four `fan-in-snapshot.v1.json` bumps, a
`contracts/index.yaml` registration, a `layers.toml` row, a
`where-things-live.md` row, and a `crate-graph.v1.json` regeneration. Two of
those baselines are USER-AUTHORIZED-ONLY and must be *proposed*, not written.

**Decision: piece A lands as `crates/vox-cli-core/src/market.rs`** — already L1,
already the shared-logic home, no new edges, no new baselines. Promote to
`vox-market` at L3 if and when piece B demonstrates a real need from the GUI
process and the module has outgrown a file. Promotion is a file move; guessing
the shape now is not recoverable.

Rejected: `vox-market` at L1 (impossible). Deferred: `vox-market` at L3 (real,
but unearned until there is code to move).

### The catalog schema is a contract, not Rust types

`contracts/market/catalog-schema.v1.yaml` defines categories and their attributes:

Note (2026-08-24): `CatalogSchema` as drafted has no `attribute_kinds` field, so
serde silently discards this block while `AttrKind::ttl_hours()` hardcodes the
same numbers in Rust. Two SSOTs, one of which loses silently — the contract could
say 24 and the scheduler use 6 forever with no signal. Either deserialize the
block and drive TTLs from it, or delete it and state that TTLs are Rust-owned.
Shipping both is how the "config edit, not a code change" claim becomes false.

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

### Reconciliation: freshness gates precedence, then evidence, then recency

**Corrected 2026-08-24.** The original rule was "highest evidence class wins,
ties break on recency" — freshness appeared nowhere in the sort key, and
staleness was presentational only. That is a real bug, and it reintroduces the
exact failure this spec exists to prevent.

The trace: eBay records `$3,599.99 / in_stock / Transactable`. Two days later
the listing sells out and delists, so the adapter returns `NotFound` — which
"does not retry until the config changes", so the scheduler stops asking. Three
weeks later Newegg returns `$5,299 / MerchantPage`, fetched one minute ago.
Under the original rule `current()` returns **$3,599.99 from a listing that no
longer exists**, permanently, because nothing can ever displace it. The spec's
own sentence "stale data is labelled, never served as current" was false as
designed: it *was* served as current, wearing a label.

The corrected sort key, all descending:

```
(!is_stale(kind, observed_at, now), evidence, observed_at_ms)
```

1. **Fresh beats stale**, regardless of class. A fresh `MerchantPage` outranks a
   stale `Transactable`.
2. Within the same freshness bucket, highest `EvidenceClass` wins.
3. Tie → most recent `observed_at_ms`. An exact tie resolves deterministically
   on `source_id`, because a "current price" that flaps between runs is
   indistinguishable from a real price change to anyone reading the history.
4. If **every** observation is stale, return the reconciled winner explicitly
   flagged `stale` and say so — never silently.
5. Losing values are **retained and surfaced** as alternates, never discarded.

A cheaper price from a weaker class never displaces a verified one *of the same
freshness*. It renders as *"seen at $X (unverified) — call to confirm"*. This
costs real money when a genuinely cheaper vendor blocks automation (three did on
2026-08-21), which is exactly why the alternate stays visible rather than being
dropped.

**Reconciliation is unit-aware.** The original `reconcile` ignored `unit`
entirely, so a €3,299 observation could outrank a $3,599 one and render as
dollars — a ~10% silent error in the number every recommendation is built on,
with no visible symptom. The signature takes a target unit
(`reconcile_in_unit(&[Attribute], "USD")`) and observations in another unit are
retained as alternates but cannot win. If no observation is in the target unit,
the result is `None` — no USD price, rather than a EUR figure wearing a `$`.

### Staleness is a display rule, not only a refresh trigger

The original spec labelled a value stale only past **2×** TTL while refreshing
at 1×, with no rationale for the gap. In between, a value is known-expired,
possibly unrefreshable, and rendered as fresh. At a 6h price TTL an 11.9h-old
price displayed unlabelled — and against this spec's own motivating datum (a
card that moved $1,710 in 24h) that figure can be ~$850 wrong while presenting
as current.

Decouple the two thresholds: refresh at 1× TTL, **label at 1× TTL**, and always
render a price with its age ("as of 4h ago") regardless of threshold. A price
with no visible age is indistinguishable from a fabricated one — the spec
already said so, then exempted its own display rule from it.

### Price is a property of an offer, not of an item

**Added 2026-08-24.** The original schema made `price_usd` a universal
*item* attribute, so every source's price for `a6000` collapsed into one series
with one winner. Price is per **vendor × condition × currency × shipping**, and
the schema had none of those.

The worst case is the first adapter. eBay was chosen precisely because "it
covers the used market": a used A6000 at $2,400 with `FIXED_PRICE` + quantity
earns `Transactable` and beats Newegg's new, sealed $4,799 `MerchantPage`.
`dollars_per_gb` becomes $50/GB and a new-workstation quote comes out $2,399
under reality — sourced from a used card, at the highest trust level in the
system. There is no `condition` attribute anywhere in the original schema.

Three further failures share this root:

- **Two valid offers.** eBay $3,599 fetched at 14:00:03 beats Newegg $3,499
  fetched at 14:00:01 on a two-second scheduler phase difference, and flips next
  tick. The append-only history for `price_usd` becomes a mixed-vendor series
  oscillating $100 with no underlying price movement — destroying the exact
  "is this price unusual?" signal the append-only store exists to provide.
- **Currency.** eBay Browse returns GBP for UK listings; a parser that reads
  `price.value` and stamps `USD` from the schema admits a 27% error tagged
  `Transactable`.
- **Shipping and tax** are excluded, so `$/GB` comparisons between a
  free-shipping vendor and a $180-freight vendor are wrong by more than the
  margins the ranking discriminates on.

**The split:** specs stay item-level attributes (they genuinely are item
properties). Offers get their own table —
`market_offers(item_id, source_id, vendor, condition, currency, price, shipping, stock, evidence, observed_at_ms)`
— reconciled **within** `(item, source, condition)`, never across vendors.
Scoring references an explicit selector, `best_transactable_offer(condition: new)`,
not a magic `price_usd`. Multi-vendor becomes alternates-of-equal-standing by
construction, and history becomes per-vendor and therefore meaningful.

This one change dissolves the multi-vendor tie, the condition conflation, the
polluted history, and the stock/availability half of the freshness bug. It is a
schema change now that costs a rewrite later.

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

Two tables in `vox-db`:

- `market_items` — catalog membership: `(item_id, category, created_at_ms)`.
- `market_observations` — **append-only**. `(item_id, attribute, value, unit,
  source_id, evidence_class, observed_at_ms, source_url)`. This is the history
  that answers "is this price unusual?" and it is why last-write-wins is wrong:
  a card moved $1,710 in 24 hours, and overwriting destroys exactly the signal
  that makes the feature worth having.
**`market_current` is deleted (2026-08-24).** The original spec promised a
materialized winner table "so the common read is one indexed query". No task
ever wrote it: `current()` reconciles full history on every read. The read-time
version is correct *and* lazier, so the table goes rather than the code.

But the read-time version as drafted is `reconcile(history(item, attr, 0))` — a
scan of every row ever recorded, deserialized and sorted in Rust, to return one
value. Two bounds make that safe, in order of laziness:

1. **Write only on change.** If the newest observation for
   `(item, attribute, source)` has an identical value and evidence, update a
   `last_seen_ms` column and write no row. At a 0.25h stock TTL this collapses
   ~35,000 rows/item/source/year to the number of *real* moves. It also makes
   `observed_at_ms` mean "last changed" rather than "last polled" — which the
   recency tie-break already assumes and the original design quietly violated.
2. **Bound the reconciliation window** to `now - 4x ttl`, falling back to a full
   scan only when that window is empty.

Retention: full-resolution stock for 48h, then transitions only. The original
spec had no retention policy at all.

**Storage placement.** These tables cannot live in `crates/vox-market/`. `vox ci
sql-surface-guard` bans `.connection().query(...)` outside `vox-db`/`vox-compiler`,
`vox ci turso-import-guard` bans the `turso::` prefix outside allowlisted crates,
and `contracts/db/data-storage-policy.v1.yaml` names `vox-db`/`vox-secrets` as
the only tier-A owners. There is also no `crates/vox-db/migrations/` directory —
schema is Rust `SchemaFragment` consts under `crates/vox-db/src/schema/domains/`,
aggregated by `manifest.rs` with a `BASELINE_VERSION` that must be bumped. DDL
goes there; the market module calls typed `impl VoxDb` accessors only.

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

**Axis availability is a property of the candidate set, not of an item
(corrected 2026-08-24).** The prose above says "omitted from the ranking",
which is set-scoped and correct. The plan implemented it per-item — skipping
axes an *individual* item lacks, then averaging over a per-item denominator.
That makes ignorance a competitive advantage:

| item | $/GB norm | $/W norm | axes used | score |
|---|---|---|---|---|
| A (fully documented) | 0.40 | 0.20 | 2 | **0.30** |
| B (`tdp_w` never fetched) | 0.40 | — | 1 | **0.40** |

B ranks first while being identical on the only shared axis and unmeasured on
the other. The item we know least about wins, and the incentive gradient points
at "block our scraper". If any survivor lacks an axis input, either drop the
axis for **everyone** and name it, or score the missing item at the **worst**
observed value flagged `imputed worst-case`. Never a smaller denominator.

**Scores normalize against a fixed reference, not the result set.** Min-max over
the filtered survivors makes every score relative to whatever else the query
returned — adding a candidate that *loses* can invert the winners above it, so
the published score is a statement about the query rather than about the world.
Normalize against each axis's declared plausible range. A zero-range axis (one
survivor, or all survivors equal — common with identical prices) must not divide
by zero: `NaN` poisons the average and `partial_cmp().unwrap()` **panics**.
Zero range means every candidate scores 0.5 on that axis, with a stated reason.
Likewise a zero denominator inside an axis expression (`price / 0`) yields
`inf`; that axis is unavailable for that item, not a score.

**Weights are declared.** The prose promises "visible weights" and
`scoring.v1.yaml` has no `weight:` key — so weights are both invisible and
hardcoded equal. Treating $/GB and $/W as equally important is a value
judgement (someone on a 15 A circuit treats watts as a hard constraint; someone
on cheap hydro treats them as noise) presented as objective. Add the key, print
the defaults in every explanation.

Every ranking renders its arithmetic: *"2nd: $/GB 14% better, tg/s per $1k 30%
worse."* Note that cross-axis aggregation is **dimensionless by normalization** —
the unit system does not guard it, and the spec should stop implying it does.

Three rules:

**An unverified attribute cannot win a comparison — and this must be
expressible.** `ScoredItem::from_pairs(id, &[(name, f64, unit)])` carries no
`EvidenceClass`, so this rule is not implementable in the plan's own types;
`Attribute::is_weak()` was built and consumed by nothing. `ScoredItem` is
constructed `from_attributes(&BTreeMap<String, Attribute>)` — which is what the
store returns anyway — so evidence travels with the value.

The rule is also scoped: `is_weak()` takes the `AttrKind` and applies only to
`price`, `stock`, and `availability`. An A6000's 48 GB is 48 GB whether a search
snippet or a checkout page reported it; physical specs do not become truer by
being seen on a cart page. Applying an evidence ladder to `AttrKind::Spec` is a
category error.

And "cannot win" means "cannot win *silently*". A rule that demotes a $2,999
`SearchIndex` price below a $6,999 `Transactable` one steers the user to a
$4,000-worse purchase in the name of protecting them — the spec already
understands this for alternates and then re-creates the harm in ranking. When
the top item depends on weak market evidence, emit a **two-item tie** with the
action spelled out: *"A is cheaper by $4,000 but unverified — call the vendor;
B is confirmed buyable at $6,999."* That is the true state of knowledge; a
forced ordering is not.

**An absent attribute is `Indeterminate`, not `false`.** A constraint over an
item lacking the attribute has no defined semantics in the original spec and no
test. Both defaults are wrong: silently excluded is precisely the "a qualifying
machine never appears and nobody notices" failure this spec names; silently
included fails at checkout. Constraint results are three-valued — `Passed`,
`Excluded` (known, falls short), `Indeterminate` (input unknown) — with
indeterminates surfaced as their own bucket: *"3 machines could not be
evaluated: `gpu_accessible_gb` unknown (vendor blocked, last attempt 2h ago)."*

This also resolves a contradiction in the original `required` defence.
`price_usd` was `required: true`, but price only exists after a successful
fetch — so an item could not be created before its first fetch, and an item
whose vendor *blocks automation* (three did) could never enter the catalog at
all. `required` is scoped to **spec** attributes at promotion time; market
attributes are legitimately absent and render `unknown`.

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

**Corrected 2026-08-24 — there is no corpus registration API.** `SearchCorpus`
is a *closed enum* in L0 `vox-db-types` (`Memory`, `KnowledgeGraph`,
`DocumentChunks`, `RepoInventory`, `WebResearch`, `SymbolProximity`,
`GraphifyStructural`). Adding a `Market` variant is an L0 change plus edits to
`vox-search`'s dispatch and the GUI's `scope_to_corpus` — not the leaf-crate
change the spec assumed. `MarketCorpus::in_memory()` / `index_text()` do not
exist anywhere.

The lazy path that needs no enum change: index via
`vox_search::ingest::persist_text_document_chunk(&db, "market:<item_id>", ...)`
and query the existing `DocumentChunks` corpus filtered by the `market:` URI
prefix.

**And piece A should not do even that.** Lexical search over ~10 hand-entered
items is `.contains()`. The corpus earns its keep once the catalog is large
enough that finding things in it is hard — which is the discovery pipeline,
gated behind this very acceptance test. Deferred to discovery.

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

**Corrected 2026-08-24 — as drafted, this gate certified nothing.** The plan's
test hand-seeds `gpu_accessible_gb: 48` into the fixture and then asserts
`48 < 64`. It tests that Rust's `<` operator works on two numbers a human typed.
Nothing derives 48 from "64 GB unified memory", nothing would catch a source
writing `gpu_accessible_gb: 64`, and the assertion
`why.reason.contains("48")` is satisfied by a generic
`"gpu_accessible_gb 48 GB < required 64 GB"` — which mentions neither unified
memory nor the reservation, i.e. never asserts the criterion the paragraph above
actually states. A gate whose stated purpose is to prove the data model works
before unlocking discovery must exercise the thing it gates.

Worse, the fixture seeds those numbers as `EvidenceClass::MerchantPage`. No
merchant page states "96 GB GPU-accessible". That figure is a 0.75x rule of
thumb wearing borrowed provenance, inside a system whose entire thesis is that
provenance is load-bearing — and the rule is wrong in detail: Apple's reserve is
not a flat 25% and is adjustable via `iogpu.wired_limit_percent`, and the ROG
Flow Z13's Strix Halo allocation is a UEFI setting, not a fixed fraction.

The corrected gate:

1. `gpu_accessible_gb` is a **derived** attribute. Store the observed
   `total_memory_gb` with real provenance from a real page, plus a versioned,
   per-architecture derivation (`unified_memory_reserve_v1`) whose output carries
   `evidence: Derived` and pointers to both the input observation and the rule.
2. The test feeds a **recorded source payload** stating "64GB unified memory"
   through extraction, and asserts both that the catalog lands at 48 *and* that
   the exclusion reason names the derivation — `reason` must contain "unified"
   or "reserved", not merely the digits.
3. A negative case: a source asserting `vram_gb: 64` on a unified-memory laptop
   is rejected or flagged, never stored.

Without these three, the gate proves nothing and unlocks discovery on a green
light with no bulb in it.

## Stored URLs are a credential-leak vector

**Added 2026-08-24.** Every observation stores `source_url` verbatim in an
append-only table. Merchant and API response URLs routinely carry affiliate
parameters, session identifiers, and signed query tokens — eBay Browse URLs
carry `_trkparms` / `campid`, and some Best Buy and Bright Data call shapes put
the API key in the query string.

Those rows land in `.vox/store.db`
(`crates/vox-db/src/store/mod.rs:13`), and that file is not necessarily local.
`DbConfig::from_env` (`crates/vox-db/src/config.rs:115`) returns `Remote` when
`VOX_DB_URL` + `VOX_DB_TOKEN` resolve, or `EmbeddedReplica` when a path is set
as well. In either case every row is replicated to a remote libSQL primary. So
the design's own append-only guarantee — never overwrite, retain forever — is
also a guarantee that a leaked token is retained forever and pushed off-box.

**Store the identity, not the session.** Strip query and fragment at write time.
`url` 2 is already a workspace dependency (`Cargo.toml:400`), so this is one
function and about six lines. Provenance keeps everything it was for: the value
is *which listing*, never *which session*. Where a parameter is genuinely needed
to re-fetch (an eBay item id), it belongs in its own typed column, not smuggled
inside a URL blob. This must land in the same commit as the column, or it never
lands.

Telemetry is **not** the vector here — `vox-telemetry` never opens the store,
and its remote upload is opt-in and off by default
(`crates/vox-telemetry/src/config.rs:40`). Replication and backup are.

**Keep `MarketError` payload-free.** Its five variants carry no response body
today, which is why no merchant `Set-Cookie` or session echo can reach a stored
row. If someone later adds `ParseFailed(String)` holding the raw body, that is
the change that turns an error row into a credential row. Worth saying out loud
because it looks like an obvious debugging improvement.

## No outbound host allowlist exists

**Added 2026-08-24.** `vox_config::resolve_egress`
(`crates/vox-config/src/resolve_egress.rs:146`) is a resolver, not an allowlist:
it maps a provider name to a key and base URL, and honours `base_url_override`
unconditionally. `vox-http-client` is ~115 lines of `reqwest::ClientBuilder`
presets with no host policy, and no CI gate requires adapters to route through
it. A new adapter calling `reqwest::get(url)` on an arbitrary host compiles,
passes clippy, and passes CI.

This matters more here than in most features because **the fetch target is
partly attacker-influenced**: reconciliation follows URLs that arrived from
search results. That is SSRF-shaped — `http://169.254.169.254/` and localhost
are both reachable.

The `llm_provider_call` detector is the wrong tool to copy. It flags hostname
*literals*, and the risk here is a URL constructed at runtime, which no
source-level grep sees. The guard is a runtime `allowed_host(&str) -> bool`
against a const list living beside the adapter registry, checked before every
fetch — about ten lines. It must also set `reqwest::redirect::Policy::none()`
or re-check the final host after redirects, or a 302 walks straight through it.

Incidental, found while confirming this: `crates/vox-http-client/src/lib.rs:3`
cites `docs/src/architecture/outbound-http-policy.md`, which does not exist —
the file is tombstoned under `docs/src/archive/research-2026-q1/`. The crate's
only stated policy is a dead link, and that document is where a host allowlist
would belong.

## Vendor terms constrain the product, not just the implementation

**Added 2026-08-24, from a ToS review of every named source.** This section is
not a compliance footnote. Two clauses bear directly on whether the headline
feature — a side-by-side multi-retailer comparison — can be built as described.

**eBay forbids co-mingled display.** The API License Agreement, §8: eBay Content
in a public display "may not be co-mingled or combined with non-eBay Content...
all eBay Content in a Public Display must be visually isolated from third-party
listings or other non-eBay information." A single table with an eBay price in
one row and a Newegg price in the next is exactly the display this prohibits.
The same section also bars using eBay Content "either alone or in combination
with third-party information, to suggest or model prices", and — a clause
worth reading twice in *this* repository — bars using it "to train algorithms,
conduct machine learning... and/or train artificial intelligence systems."

**Best Buy forbids the analysis itself.** Its terms bar using Content "on behalf
of or for the benefit of any third party (such as other retailers) for the
purposes of analyzing, receiving or reviewing information regarding Best Buy
pricing", cap caching at **72 hours**, require conspicuous attribution plus the
Best Buy logo on every surface where the API has a presence, and reserve
revocation at Best Buy's sole discretion.

Three consequences for the design above:

1. **The comparison surface needs a per-source display mode**, not one uniform
   table. Sources carry display constraints — visual isolation, attribution,
   logo, maximum content age — and those are source metadata the adapter trait
   must expose, exactly as it exposes `evidence_class()`. A `DisplayPolicy` next
   to `cost_usd()` is the cheap version.
2. **Cache TTLs are contractual ceilings, not tuning knobs.** eBay: listing data
   no more than 6 h older than the site, other content 24 h, and *disclose the
   age if it lags*. Best Buy: 72 h hard cap. The spec's 6 h price TTL happens to
   land inside eBay's limit by coincidence, not design. Note also eBay's "when
   the eBay Content is no longer publicly available, you must delete it" — which
   is the same requirement the freshness-gating correction above arrived at from
   a correctness argument, now also a contractual one. Append-only storage and
   "must delete" are in direct tension; resolve it before the store is built.
3. **`EbayAppId` as a single secret is wrong.** Browse requires an OAuth2
   client-credentials grant: `client_id` + `client_secret`, POSTed to
   `identity/v1/oauth2/token`, yielding a 2-hour token. Two secrets and a token
   refresh path, not one static key. The Buy APIs additionally carry an
   "additional license" footnote pointing at eBay Partner Network approval.

   There is no "credential pair" type in `vox-secrets`, and none is needed: the
   established pattern is N independent `SecretId`s sharing a name prefix. Reddit
   does exactly this at `crates/vox-secrets/src/spec/registry/social.rs:50-73`
   (`VoxSocialRedditClientId` / `...ClientSecret` / `...RefreshToken`, three
   sibling `SecretSpec` entries in one const, each `optional_skip`). YouTube
   repeats it; ORCID does the client-id/secret pair without a token. Copy that
   shape, and remember the taxonomy match arms in `spec/mod.rs` — missing them
   fails `secrets-parity`.

   **The minted access token is not a `SecretId`.** A `SecretSpec` describes a
   user-configured *input*; a machine-minted 2-hour token is derived state. The
   repo already reflects this — it registers Reddit's long-lived *refresh* token
   and derives the access token at runtime — and the caching mechanism exists:
   the `SnapshotCache` statics in `resolve_egress.rs:35-37`. Holding the token
   in one of those with an expiry also removes the per-attribute-per-tick
   Credential Manager round-trip that `is_available()` would otherwise incur.

**Amazon PA-API is retired** (May 2026, now HTTP 403). Its successor gates on
active affiliate sales. Newegg has no public product API; Micro Center has no
API at all and sits behind a Cloudflare challenge — and defeating a technical
access control is a materially worse legal posture than ignoring a ToS line.

**On scraping the rest.** Post-*Meta v. Bright Data* (N.D. Cal. 2024), scraping
public pages while **logged out** is defensible: the court held terms could not
bind a logged-off scraper because it is not a "user". The conditions that keep
it that way are specific — never create or use an account, take only facts
(prices, SKUs, availability are uncopyrightable under *Feist*; product photos
and review prose are not), and never defeat a technical barrier. Note that
Bright Data's own MSA shifts liability *toward* the customer: you agree to
defend Bright Data against third-party claims arising from your use, and to
stay within the use case you declared at KYC. It lists price comparison as an
approved use case, which helps, but it is not a shield.

**What this does not do:** none of the above is legal advice, and the spec
should not pretend to settle it. What it does is move these from unknowns to
recorded constraints, so the display design and the adapter trait account for
them before code exists rather than after a takedown.

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
