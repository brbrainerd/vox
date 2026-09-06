---
title: "Free-Tier Model Selection & Onboarding — Audit, Research, and Risk-Reviewed Design (2026-08-01)"
description: "Live-code audit of Vox's model-selection engine and free-tier handling, external research on public model-rating registries, coding-specific benchmarks, model-router prior art, and competitor onboarding UX, plus an adversarial pre-mortem and risk register for the resulting design recommendations."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Free-Tier Model Selection & Onboarding — Audit, Research, and Risk-Reviewed Design (2026-08-01)

> **Provenance.** Two rounds of parallel research. Round 1 (4 agents): synthesis of ~10 existing
> repo docs, a live-code audit of the selection engine and GUI, external research on public
> model-rating registries, external research on competitor onboarding UX. Round 2 (4 agents), run
> specifically to critique round 1's output before it hardens into a plan: an adversarial pre-mortem
> of every recommendation (with code citations), research on coding-specific benchmarks and desktop
> OAuth security, research on dedicated model-router products and additional competitor tools, and
> a direct attempt to resolve the one discrepancy round 1 left open. Round 2's findings are folded
> in throughout rather than appended, since several of them change round 1's conclusions materially
> (§1.6, §1.7). Companions: [`model-selection-2026-q2.md`](model-selection-2026-q2.md),
> [`free-by-default-and-residual-work-plan-2026.md`](free-by-default-and-residual-work-plan-2026.md),
> [`vox-harness-parity-plan-2026-07-30.md`](vox-harness-parity-plan-2026-07-30.md),
> [`coding-agent-local-model-ux-comparison-2026-07-30.md`](coding-agent-local-model-ux-comparison-2026-07-30.md),
> [`telemetry-trust-ssot.md`](telemetry-trust-ssot.md), [`vox-gui-design-review-2026.md`](vox-gui-design-review-2026.md).

## 0. Bottom line

Vox already has a ratified, wired, unit-tested selection engine (`vox_orchestrator::models::decide()`,
a three-axis `SelectionAxes{cost, responsiveness, intelligence}` scorer) and a shipped
free-by-default cost policy. The two dead routing engines a 2026-07-16 audit flagged for deletion
were in fact deleted and have not regressed. The scorer's apparent "inertness" (§1.6) turned out to
be a real, diagnosable, and much smaller bug than originally feared — not a blocker. What's left is
narrower and more concrete than "vastly improve free-tier handling" suggests, but each item below
now carries a *specific*, previously-hidden failure mode that must be designed for, not just a
feature to build:

1. **A brand-new user with zero API keys and no Ollama gets a hard error.** Fixing this with an
   inline OAuth key-provisioning flow is sound in principle, but Vox's own codebase shows the
   *reason* this hasn't been done already: there's no existing desktop-OAuth-redirect
   infrastructure to build it on, and the one thing Vox already knows how to do (device-flow OAuth)
   doesn't map onto OpenRouter's PKCE flow. §5.A and §1.6-adjacent findings resolve this with a
   concrete mechanism (RFC 8252 loopback server), but it's genuinely new infrastructure, not a
   thin wrapper.
2. **No onboarding wizard exists anywhere in the GUI** — and no reusable "first-run" infrastructure
   exists to build it on either, though a prior *design-only* doc already specifies a replay
   pattern worth adopting verbatim (§1.4).
3. **No external model-rating registry is ingested**, and the two most-cited general-purpose
   sources (Chatbot Arena-style) are the wrong ones to lean on for a *coding* agent — coding-specific
   benchmarks exist and are a better fit, but none of them offer a clean live API either (§2).
4. **The scorer discrepancy is resolved, and reveals a real (smaller) bug**: complexity-based
   scoring is correctly wired and unit-tested, but a flat telemetry-history boost can swamp it for
   any model that already has scoreboard data, making the ranking look complexity-blind in exactly
   the case someone tested it (§1.6).
5. **New, and more consequential than it first looked: Vox's per-user budget caps
   (`daily_budget_usd`, `per_session_budget_usd`) are defined but completely unenforced** — no code
   path reads them to block, warn, or downgrade anything (§1.7). This matters directly for the
   "free" onboarding flow: there is currently no safety net if a wizard-provisioned key later starts
   incurring real cost.

Section 1 is the audit (now including the resolved scorer question and the budget-cap finding).
Section 2 is registry/benchmark research (now including coding-specific benchmarks and router prior
art). Section 3 is competitor UX research (now including OAuth desktop-security specifics and
cautionary tales). Section 4 is a risk register consolidating every failure mode found across both
research rounds. Section 5 is the design recommendations, revised to bake in fixes for what the
adversarial pass found. Section 6 is success metrics/rollout/testing notes that were missing
entirely from the first draft. Section 7 is open questions, split into "must resolve before
spec'ing" and "sequencing/scope" tiers.

---

## 1. Current-state audit: the selection engine (verified 2026-08-01, live code)

### 1.1 The engine is real and wired, not dead code

`crates/vox-orchestrator/src/models/` is a substantial module: `registry.rs` (1145 lines),
`scoring.rs` (1041 lines), `select.rs` (1399 lines), plus `admission.rs`, `autonomic.rs`,
`discovery_pipeline.rs`, `policy.rs`, `prompt_profiles.rs`, `routing_table.rs`, `vram.rs`.

Scoring axes actually implemented: `SelectionAxes { cost, responsiveness, intelligence }`
(`models/select.rs:284-320`), a VRAM-fit penalty (`models/scoring.rs:38-56`, explicitly documented
as *advisory only* — "the underlying VRAM estimate can be wrong," which matters for §1.7/§5.D), and
a telemetry-feedback blend, `scoreboard_feedback_boost` (`models/scoring.rs:58-108`), that folds
measured latency/quality history back into ranking. There is no explicit "locality" scoring axis —
local-vs-cloud preference is enforced as a hard filter elsewhere, not a soft-scored dimension.

Real callers of `decide()` / `select_with_default_registry()`:

- `crates/vox-orchestrator/src/registry_model_resolve.rs:50`
- `crates/vox-orchestrator/src/orchestrator/task_dispatch/research_dispatch.rs:200,313`
- `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs:368` — **this is the
  live path GUI/CLI chat actually resolves through**
- `crates/vox-gui/src/commands/models.rs:361`

### 1.2 The two "dead" engines: deletion confirmed, not regressed

A 2026-07-16 audit found two complete routing engines — a "7-way" resolver in
`vox-actor-runtime` and a `ModelPool` engine in `vox-config` — had zero callers, and recommended
deleting both (fork "F3") rather than finishing the wiring.

Verified via git log: both were deleted 2026-07-16/17 (`8fe5925c15`/`7ed997ae5d` for `ModelPool`,
`b0fd1e40b8`/`829710ea63` for the 7-way resolver). `crates/vox-actor-runtime/src/model_resolution.rs:1-27`
documents the deletion inline: *"The former 7-way provider-route resolver ... was deleted
2026-07-16 (Axis GUI remediation F3); the single exercised selection path is
`vox_orchestrator::models::decide()`."* No commits have touched either path since — **the F3
deletion executed cleanly and stuck.** Any memory or doc still describing `ModelPool` as
in-progress is stale; treat it as gone.

### 1.3 Free-tier detection: dynamic, not hardcoded — but the zero-key path still fails

`OPENROUTER_FREE_MODELS` still exists in `crates/vox-gamify/src/ai/constants.rs:16-19`, but it is
no longer an independent hardcoded list — it now aliases a single SSOT in
`crates/vox-config/src/bootstrap_inference.rs:24-30` (`OPENROUTER_FREE_FALLBACK_MODELS`), with a
drift test enforcing they never diverge (`constants.rs:21-31`).

More importantly, **real free-model detection is dynamic**: `crates/vox-orchestrator/src/catalog.rs:136-148`
computes `is_free` live from the OpenRouter API's pricing response (prompt and completion price
both zero or negative). The static list is only an emergency fallback when the live catalog is
unreachable.

**The actual gap**: dispatching *any* OpenRouter model — including `:free`-tier ones — still
requires `OPENROUTER_API_KEY` to be set (`crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs:26-30`
hard-fails via `required_secret` on an empty key). So a genuinely new user, with zero keys and no
Ollama installed, does not get routed to a working free model — they hit a terminal error
(`resolve.rs:392-397`): *"No LLM model available — set OPENROUTER_API_KEY or GEMINI_API_KEY,
install Ollama ... or add models.toml."* `vox doctor`'s LLM routing check
(`crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs:56-62`) reports
this as a **FAIL**, not as "running in free/local mode" — there is no passing zero-key state at
all today.

This is the real headline finding for "improve free-tier handling": **the free-by-default *cost
policy* is fully shipped, but there is no free-by-default *zero-key onboarding path*.** Those are
different problems, and only the first one is solved. Note also (new in this revision, §1.7): even
once a key is provisioned, nothing currently caps what it can spend.

### 1.4 GUI: no onboarding wizard exists — but a matching design already does, unbuilt

- `crates/vox-gui/ui/src/components/surfaces/Models/ModelsView.tsx` — a "Model Registry" grid
  (`ModelCard`) rendering `cost_per_1k`, context/`max_tokens`, `latency_p50_ms`, and a green
  **"free" badge** when `is_free` is true (line 156). `quality_score` is defined on the `ModelCard`
  interface (line 18) but **is never rendered** — a dead field. There is **no free-tier filter
  toggle** — the badge exists, but you can't filter the grid down to free-only.
- `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` (1546 lines) — a real
  `KeysSecretsSection` (~line 398, Tauri `set_secret`/`remove_secret`/`list_secret_status`, plus
  `.env` import) and `LlmSettingsSection` (line 887) that shows a "no OpenRouter key configured —
  add one under Keys & Secrets" banner **with no jump link to that tab.**
- `crates/vox-gui/ui/src/components/surfaces/Models/BackendAvailability.tsx` — a per-provider
  status strip (key configured / no key; online/offline for local) — informational only, no
  inline "add key" action.
- **Zero onboarding/wizard/first-run component exists anywhere** in `crates/vox-gui/ui/src` —
  confirmed by an exhaustive grep for `onboard|Onboard|Wizard|firstRun|first.run` across the whole
  tree, cross-checked twice across both research rounds. A setup wizard needs to be built new.
- **But a matching design already exists, unbuilt**: `docs/src/architecture/vox-gui-design-review-2026.md`
  §9.5 (self-labeled *"a design artifact for review... no code, no commits"*) specifies a first-run
  tour pattern: welcome modal → highlight key surfaces → sandboxed sample task → mark done via a
  completion flag → **"replayable from Settings → Onboarding."** Nothing implements this today
  (confirmed via grep for `vox_first_run_done`/`firstRun`/`OnboardingSection` — zero matches), but
  the design is ready-made for exactly this purpose. Reuse it rather than inventing a second
  first-run pattern (see §5.C).
- Two existing components already implement the "seen/dismissed" persistence a wizard needs:
  `VersionMismatchBanner.tsx` and `BackendBanner.tsx`. Reuse their dismiss-persistence pattern
  rather than writing a third one.
- **"Vox Axis"** is confirmed as the product brand name for the Vox GUI (`vox axis`, alias of
  `vox gui`; crate remains `vox-gui` per `docs/src/contributors/axis-brand.md`). Any wizard "as
  part of Vox Axis" is Rust/TS work under `crates/vox-gui/{ui/src,src/commands}`, plus registration
  in the sidebar/surface system (`crates/vox-gui/ui/src/components/layout/Sidebar.tsx`).

### 1.5 Secrets/API-key flow today: functional, entirely manual — but OAuth-shaped storage already exists

CLI: `vox secrets login`, `vox secrets set <registry> <token>`, `vox secrets import-env`, `vox
secrets sync --mesh`, `vox secrets status`/`doctor`. GUI: the `KeysSecretsSection` above. Backing
store: Vox Secrets / Clavis vault. There is no guided "you have zero keys, here's how to get free
access" flow in either surface.

New in this revision: `crates/vox-secrets/src/spec/types.rs` already defines `SecretKind::OAuthRefreshToken`
and `SecretKind::OAuthClientCredential` — the storage layer already anticipates OAuth-obtained
credentials. Any new onboarding flow **must** persist through this existing `vox_secrets`/Clavis API
(so `vox doctor`/`vox secrets status` see a wizard-provisioned key identically to a manually-entered
one), not a separate, easier-to-reach-for local-storage shortcut. See §4 risk register and §5.A.

### 1.6 Scorer discrepancy — RESOLVED (was §1.6 "unresolved" in the prior draft)

The prior draft of this doc left open whether the live scorer is "empirically inert" (a prior doc's
claim: identical rankings across task complexity). Round 2 resolved this directly against a live
debug binary plus a full code trace, rather than leaving it as a flagged unknown.

**Empirical result, confirmed live**: `vox model explain` does return a byte-identical top-5 ranking
across four complexity probes (1, 3, 8, 10) on the current default registry — the underlying claim
is real, *in this specific configuration*.

**But the scorer itself is not broken.** `crates/vox-orchestrator/src/models/registry.rs:823-896`
(`explain_selection`) documents and fixes a real prior bug ("F3a": explain used to sort by raw
`cost_per_1k`, ignoring complexity) — it now calls `scoring::auto_score_model`, which genuinely
shifts weights with complexity (`scoring.rs:321-349`: `w.precision += 10 / w.efficiency -= 10` above
a high-complexity cutoff, and the inverse below a low one). `cargo test -p vox-orchestrator --lib
explain_selection_complexity_tests` passes, including an assertion against the real builtin catalog
that top-5 rankings *do* differ across complexity levels (`models/tests.rs:399-508`).

**Root cause of the observed inertness**: `scoring.rs:482` adds a flat, complexity-independent
`telemetry_boost` from `scoreboard_feedback_boost` (up to ~0.15). The live database
(`model_scoreboard` table) shows `ibm-granite/granite-4.0-h-micro` — the model that won every probe
— already has real usage history (`success_rate=1.0, quality_score=1.0`), giving it a flat edge that
swamps the ±10-weight complexity shift among the other, historyless candidates in that registry.

**This is a real, smaller bug worth its own follow-up, separate from anything in §5**: the moment
any one model accumulates scoreboard history, it can dominate every ranking regardless of task
complexity, for exactly as long as it's the only scored candidate. Whether `scoreboard_feedback_boost`'s
magnitude should itself scale down at high complexity (so a "proven but maybe-not-suited-to-this-task"
model doesn't out-rank an appropriately-capable but historyless one) is a genuine open design
question — flag it to the harness-parity plan's owner as a scoped, well-understood follow-up, not
as a P0 blocker on §5's recommendations. Recommendations B/D below do not depend on resolving it.

### 1.7 New finding: budget caps exist in config but are completely unenforced

`crates/vox-config/src/config/vox_config.rs:19-20,52-53` defines `daily_budget_usd` (default $5) and
`per_session_budget_usd` (default $1) on `VoxConfig`, surfaced read/write in the GUI
(`crates/vox-gui/src/commands/user_config.rs:47-48,258-262`) and explicitly labeled **"soft cap on
spend"** in the key manifest (`crates/vox-llm-config/src/keys.rs:138-139`). A repo-wide grep for
both field names turns up only definition/storage/docs — **no orchestrator, routing, or dispatch
code path reads either field to block, warn, or downgrade anything.** This matches a gap already
named in `docs/superpowers/specs/2026-06-15-llm-ssot-remaining-roadmap-and-cost.md:25`.

There *is* a real, enforced hard-budget mechanism in the codebase — `crates/vox-scaling-policy/src/cost_defense.rs:29-30,197-202`
("Layer 3: Hard daily budget ceiling... tasks are rejected once exceeded") — but it's a *different*
system, scoped to multi-agent mesh/fleet economics, not the per-user `VoxConfig` fields a desktop
user would set.

**Why this matters for this doc specifically**: any design that provisions a "free" API key for a
new user (§5.A) is implicitly relying on the *provider's* rate limit as the only spend control,
because Vox's own per-user budget enforcement doesn't exist yet. If a user later attaches a paid key
(e.g., after outgrowing OpenRouter's free tier) without separately learning that the budget fields
in Settings do nothing, there is currently no safety net against runaway spend. This is a
pre-existing gap, not something §5 introduces — but §5.A's "free onboarding" framing makes it more
likely a first-time, inexperienced user hits it, so it belongs in this doc's risk register (§4) even
though fixing it is arguably its own separate piece of work.

---

## 2. External research: registries, coding-specific benchmarks, and router prior art

No prior Vox doc proposes ingesting an external model-rating registry — the existing design stance
is deliberately telemetry-first (`model_scoreboard`), reasoned as "a better signal than crowd spend
because it's measured on *your* workload." That's right for an established install. It has nothing
to offer a brand-new model or user — the exact free-tier/first-run scenario this doc is about — so
treating an external registry as a **cold-start prior only** is the right frame (§5.B). Two
additions from round 2 change what that prior should actually be built from.

### 2.1 General-purpose registries: what's usable

| Source | Auth | Free tier | Axes covered | Verdict |
|---|---|---|---|---|
| **OpenRouter `/api/v1/models`** | None | Unlimited, public | catalog, pricing, context, modality | Spine — join key for everything else (`canonical_slug`/`hugging_face_id`) |
| **OpenRouter `/api/v1/benchmarks`** | Free API key | 30 req/min, 500/day | intelligence (re-exports Artificial Analysis intelligence/coding/agentic index), speed (Design Arena elo/win-rate/avg-generation-time), OpenRouter's own task accuracy | Best single general-purpose source — all 3 axes in one authenticated-but-free call |
| **Artificial Analysis API** | Free key | 100 req/day | intelligence index, output tokens/sec, TTFT, price | Best dedicated *speed* source (TTFT/tok-s); also how Groq/Cerebras/SambaNova get independently benchmarked |
| **LiteLLM `model_prices_and_context_window.json`** | None | Unlimited, raw GitHub JSON | pricing, context, capability flags | Free no-auth backup/cross-check for pricing+context only — no quality or speed |
| **models.dev `api.json`** | None | Unlimited | catalog, capability flags, pricing | Same category as LiteLLM; used by OpenCode (see §2.4 for why that's a cautionary tale, not just a precedent) |
| **LMArena HF dataset** | None (HF download) | Unlimited | human-preference Elo, general chat | Periodic bulk pull; weak signal for coding specifically (see §2.3) |

**Dead ends** (don't build sync jobs against these): Vellum AI leaderboard (web-only, no API);
Hugging Face Open LLM Leaderboard (officially retired mid-2025, fragmented into 200+ inconsistent
community Spaces, no successor); Ollama's central library (no catalog API, only per-instance
`/api/tags` for already-pulled models); OpenAI/Anthropic `GET /v1/models` (thin metadata, no
pricing/quality/speed, needs its own key each); Groq/Cerebras/SambaNova direct (no public
speed-benchmark API of their own — already covered via Artificial Analysis).

**Warp.dev**: confirmed opaque. Their picker surfaces Intelligence/Speed/Cost per model plus four
Auto sub-modes — strong evidence of demand for exactly this feature — but no blog post, changelog,
or repo discloses the scoring methodology; the app is closed-source. This validates the need, it
doesn't give us a source to integrate.

### 2.2 New: coding-specific benchmarks should be weighted above general-purpose ones

Chatbot Arena and similar general-preference leaderboards measure open-ended chat quality, which
correlates weakly with an agent's actual job: terminal/agentic code editing. For a coding agent,
these are the relevant benchmarks — none has a clean first-party live API, but several are more
programmatically usable than expected:

| Benchmark | What it measures | Access |
|---|---|---|
| **Epoch AI** (`pip install epochai`) | Aggregates SWE-bench Verified, Aider Polyglot, METR time-horizons, and more into one queryable client, plus their own Epoch Capabilities Index | **Best single programmatic option** — official open-source client, one integration point instead of five |
| **Aider polyglot** | 225 Exercism exercises across 6 languages, two-attempt pass rate, cost, edit-format fidelity | No API, but harness + dataset are fully open (`Aider-AI/polyglot-benchmark`) — Vox could run it itself, or parse the community-submitted `_data/` files directly from GitHub |
| **METR time-horizon** | Task length (human-hours-equivalent) a model completes autonomously at 50% success | Nonprofit-run (not vendor-self-reported), code+data on GitHub, mirrored into Epoch |
| **LiveBench** | Contamination-resistant, monthly-refreshed, includes a coding category, objective scoring (no LLM judge) | Fully open (code + HF datasets), supports self-run evaluation |
| **Terminal-Bench** | Real terminal/agentic tasks — closest to what Vox's agent loop actually does | Official leaderboard is a webpage; a community GitHub mirror (`RDI-Foundation/terminal-bench-leaderboard`) is more reliably scriptable than scraping |
| **SWE-bench / SWE-bench Verified** | Real GitHub issue → passing-patch generation | Dataset on Hugging Face; **leaderboard scores are ~99% self-reported by submitters, not independently run** — use with that caveat attached, not as a trusted-as-is number |
| **BigCodeBench** | Function-level generation with real-world library/API usage | Hugging Face Space + code-execution API; community-submitted results |

**Recommendation**: weight Aider polyglot (edit-format fidelity matters directly for a
file-patching agent) and Terminal-Bench (closest analog to Vox's own loop) highest, METR
time-horizon as a proxy for safe unsupervised session length, LiveBench's coding category as a
contamination check, SWE-bench Verified as a secondary signal *with the self-reported caveat always
displayed if surfaced to users*, and general Chatbot-Arena-style signals only as tie-breakers.
Practically, treat all of this as a **periodically-refreshed cached table** (pulled via a scheduled
job, most easily through the Epoch AI client, which already aggregates most of these) — not a
live per-request call, same caching posture as the general registries above (see §4 risk register
for why per-client polling doesn't scale here).

### 2.3 New: dedicated model-router prior art — one piece is genuinely adoptable as algorithm, not just data

Several products exist whose entire business is "automatically pick the best model for this
request" — directly relevant prior art:

- **RouteLLM** (`lm-sys/RouteLLM`, open source, ICLR 2025) — ships four concrete routing algorithms
  (matrix-factorization, similarity-weighted-ranking, a BERT classifier, a causal-LLM classifier),
  trained on preference data, that **generalize to new strong/weak model pairs without retraining**
  per their own published claim. This is the one piece of prior art here that's adoptable as
  *algorithm*, not just as a data source — the lighter-weight variants (matrix-factorization,
  similarity-weighted-ranking) don't need Arena-scale training data and could plausibly be retrained
  on even weak implicit Vox signal (a user retrying a task on a stronger model, an aborted run) to
  build a *learned* router layered on top of the static registry, rather than a router that only
  ever consults a static leaderboard.
- **OpenRouter's own `openrouter/auto`** dropped its earlier NotDiamond dependency for an in-house,
  disclosed methodology: classify the prompt into ~30 task categories, rank candidates by **real
  community spend in that category over a trailing 7-day window**, filter by a cost-quality slider.
  This "usage-as-revealed-preference" approach requires no training pipeline and sidesteps the
  cold-start/benchmark-staleness problem — worth considering as a cheaper alternative or complement
  to static external ratings, if Vox ever has enough aggregate (opt-in, anonymized) usage data of
  its own to make it meaningful.
- **Martian, NotDiamond, Unify.ai, Portkey, Requesty, Eden AI** — commercial gateways, all with
  either fully opaque ("proprietary Model Mapping," patent-pending) or only high-level-disclosed
  methodology. Useful only as evidence of market demand, not as sources to integrate or emulate
  directly.
- **RouterArena** (academic benchmark of 12 routers, arXiv 2510.00202) found **no router — commercial
  or open-source — reliably recognizes when a cheap/small model already suffices**; commercial
  routers buy accuracy by restricting to curated pools, open-source ones plateau. This is directly
  relevant design guidance: Vox should not attempt upfront perfect task-difficulty classification
  (nobody has solved that well), but should instead design for **escalation** — start on a
  cheap/local/free candidate, detect struggle signals (retries, error loops, explicit "try a
  smarter model"), and escalate only then. This is a genuine differentiation opportunity precisely
  because the field hasn't solved the alternative.

### 2.4 New: OpenCode's cautionary tale — a registry without a quality signal degrades the picker, not just the ranking

OpenCode (a coding-agent CLI, uses `models.dev` as its registry — same source table entry as §2.1)
is the clearest cautionary precedent available: its own docs state it "doesn't select a model for
you, automatically route between models, or rank models as best," because models.dev has no quality
axis to rank on. Its actual fallback is a **hand-curated, explicitly-unordered, admittedly-stale
shortlist** maintained by editorial judgment ("not exhaustive nor necessarily up to date," per their
own docs), plus a separate usage-ranking dashboard that isn't wired into the picker at all. Default
model selection with nothing configured is purely mechanical (CLI flag → config → last-used → first
available) — zero intelligence.

**The lesson for Vox**: if the eventual registry (§2.1/§2.2) doesn't get a genuine quality signal
wired all the way into whatever counts as the "Auto" default, the honest outcome is the same
disclaimer OpenCode had to write into its own docs — and shipping an "Auto" picker that quietly
degrades to that without saying so is worse than admitting it's curated/manual. §5.B and §5.C should
be judged against this bar specifically: does the wizard's default path actually rank by something,
or does it just alphabetize/first-available and call it Auto?

---

## 3. External research: competitor onboarding UX for zero-key users

### 3.1 Onboarding UX patterns (round 1, load-bearing findings)

- **Windsurf — the strongest "zero-decision first screen" precedent.** Sign in, optional
  settings-import, then straight into a working state: *"no API key to configure, no model to
  select, no billing to set up — we manage it."* Free tier = 25 credits/month + unlimited access to
  their own base model; hitting the cap degrades gracefully to base-level autocomplete rather than
  locking out.
- **Cline — inline account creation + explicit `:free` filter.** "Install → create a Cline account
  *inside the extension* → pick `grok-code-fast-1`" is a real no-external-browser signup flow. For
  OpenRouter specifically, Cline's picker filters to the literal `:free` model-ID suffix, and
  OpenRouter's own free-tier ladder (50 req/day → 1,000/day after $10 lifetime spend) is a
  graduated-trust pattern, not a hard wall.
- **GitHub Copilot — the most consequential recent industry signal.** As of 2026-06-24, GitHub
  removed manual model selection *entirely* for Free/Student plans, replacing it with Auto-only. A
  major vendor concluded free-tier users are better served by no picker at all than by a picker with
  confusing cost-multiplier badges.
- **LM Studio and Jan — hardware-fit badges, the single most portable/differentiating pattern
  found.** Both compute a live, per-model fit indicator against detected RAM/VRAM: LM Studio's
  green/yellow/red, Jan's near-identical Fits/May-be-slow/Won't-fit. No cloud-first tool in this
  survey does this at all. Vox already has `vox-plugin-nvml-probe` in-tree, unwired — but see §3.2
  for a hard constraint on what "wiring it up" actually covers.

### 3.2 New: hardware-fit badge — confirmed NVIDIA-only, fails clean but needs an explicit "Unknown" state

Direct code check confirms `crates/vox-plugin-nvml-probe/Cargo.toml:16`'s only dependency is
`nvml-wrapper` — **no AMD/ROCm, Intel, or Apple Silicon path exists.** On a machine with no NVIDIA
driver, `Nvml::init()` fails cleanly (`probe.rs:53-56,137`, mapped to `ProbeError::LibraryUnavailable`,
propagated as a typed error through the plugin ABI — no crash, no false-positive success). But this
is a plain error today, not an explicit fit signal: whatever UI consumes it must map that error to a
neutral **"hardware detection unavailable on this platform"** state, not silently omit the badge
(reads as a missing feature) and not default to treating "no data" as "no penalty" (a false green on
non-NVIDIA hardware that then OOMs is worse for trust than no badge at all — LM Studio/Jan's own
badges are vendor-agnostic, which Vox's v1 explicitly would not be). This mapping doesn't exist yet
and is real, scoped work — see §5.D.

### 3.3 New: OAuth desktop-security specifics — the mechanism recommendation A needs

Vox is a Tauri desktop app, not a web app, so "redirect back with a code" (§3.4) isn't as simple as
it sounds — this was flagged as unspecified in round 1 and round 2 resolved it:

- **RFC 8252 (OAuth 2.0 for Native Apps)** recommends a **loopback HTTP server**
  (`http://127.0.0.1:<random-port>/callback`) as the preferred pattern for *desktop* OSes
  specifically (custom URI schemes are the mobile-oriented fallback). Plain HTTP is acceptable here
  because the request never leaves the device.
- A **custom URI scheme** (e.g. `vox://oauth-callback`) carries a real, named risk on desktop:
  multiple installed apps can register the same scheme, and the OS may hand the redirect to the
  wrong (or a malicious, scheme-squatting) app — part of why RFC 8252 mandates **PKCE** regardless
  of transport: even an intercepted `code` is useless without the original `code_verifier`, which
  never leaves the legitimate process.
- **`state` parameter** is required either way, to bind the callback to a specific flow instance and
  block CSRF-style account-mixup attacks.
- **OpenRouter's own docs explicitly support this**: their PKCE flow's `callback_url` accepts
  `localhost`/`127.0.0.1` on any port, called out for "local CLI tools" — i.e., they built for
  exactly the loopback pattern RFC 8252 recommends.
- Vox would be an early adopter of this specific combination on desktop: `opencode` (a closely
  analogous CLI coding agent) has an open design discussion proposing the same loopback-server
  approach for this exact OpenRouter flow, but the issue was closed "not planned" — evidence of the
  right pattern, not a working reference implementation to copy. Vox's *own* existing OAuth-shaped
  precedent (`vox-populi`'s GitHub device flow, `vox-cli`'s Ludus auth) deliberately avoids the
  redirect problem entirely via Device Authorization Grant (show a code, user visits a URL, app
  polls) — worth asking whether OpenRouter supports (or would consider) a device-flow equivalent
  before committing to building loopback-server infrastructure from scratch.

**Recommendation**: build the loopback-HTTP-server pattern (RFC 8252's desktop-preferred choice),
with PKCE + `state` mandatory, and treat the custom-URI-scheme path as, at most, a documented
fallback — not the primary mechanism.

### 3.4 New: free-tier abuse reality — a real friction point, not evidence of an exploitable gap

OpenRouter's 50 req/day + 20 req/min free-tier ladder (rising to 1,000/day after $10 lifetime spend)
is confirmed still current. OpenRouter's own docs state limits are enforced **globally**, not
per-account, specifically to blunt multi-accounting — no independent audit confirms this is
airtight, but no evidence of active farming/exploitation surfaced either; the visible developer
complaints are about the limits being **too tight for legitimate agentic (multi-call) workloads**,
not about the gate being gamed. **Design implication**: this is real, expected friction to message
honestly in onboarding (§5.A/§5.C — "50 free requests/day until you've spent $10 elsewhere," not a
vague "free forever"), and Vox should not build UX that nudges users toward multi-accounting to
stretch it.

### 3.5 New: additional tools — mostly cautionary notes about vendor dependency, not new UX patterns to steal

- **OpenCode** — see §2.4; the cautionary tale on ranking without a quality signal.
- **Gemini Code Assist** — the standalone free individual tier was **discontinued** in June 2026,
  users pushed to migrate to a rebranded "Antigravity" family. A live example of a competitor's free
  tier disappearing entirely, not just changing terms.
- **Amazon Q Developer** — **new signups (free and paid) closed May 2026.** Another live example of
  a free tier being pulled outright.
- **JetBrains AI Assistant** — a clean, worth-copying pattern distinct from anything above: local
  completions (their own small model) are unlimited/free on every tier and don't consume credits at
  all, while cloud/frontier-model chat draws from a tight credit cap. This local-unlimited /
  cloud-budgeted split is a good model for how Vox could frame local-Ollama vs. cloud-key routing in
  the wizard, independent of the OpenRouter-specific work in §5.A.
- **Void editor** — was open-source, MIT, no disclosed auto-ranking; **archived June 2026**, now
  dead prior art, not a live UX to reference.

**Combined implication of the last three**: don't design Vox's free-tier story as a single point of
dependency on one vendor's goodwill (echoing the open question already flagged in the prior draft —
this round found two concrete, recent examples of exactly that risk materializing for competitors).

---

## 4. Risk register

Consolidates every failure mode surfaced by the adversarial pre-mortem (round 2) and the resolved
findings above, so severity can be triaged at a glance instead of reconstructed from prose.

| # | Risk | Recommendation affected | Severity | Status / mitigation |
|---|---|---|---|---|
| 1 | No existing desktop OAuth-redirect infrastructure; custom-scheme approach is genuinely riskier than it looks | A | Critical | Resolved by §3.3 — use loopback HTTP server per RFC 8252, not `vox://` scheme, as primary path |
| 2 | OAuth-provisioned key could land in a second, weaker secret store instead of Clavis | A | Critical | Must persist via existing `vox_secrets`/`SecretKind::OAuthRefreshToken` API (§1.5) — visible to `vox doctor`/`vox secrets status` identically to a manual key. State as an explicit acceptance criterion in any spec. |
| 3 | Free-tier rate-limit exhaustion *after* successful onboarding recreates the exact opaque-error bug this doc exists to fix | A | Critical | Must distinguish "no credential" vs. "credential valid but quota-exceeded" at the `provider_auth.rs`/`resolve.rs` level, with its own clear message and a path to add a personal key. Not yet designed — needs its own scoped follow-up. |
| 4 | Per-user budget caps (`daily_budget_usd`/`per_session_budget_usd`) are unenforced — no safety net if a provisioned key starts costing money | A | Critical (newly discovered, §1.7) | Pre-existing gap, not introduced by this doc, but the "free" framing in A makes it more likely a first-time user is affected. Flag as a dependency, likely its own separate piece of work, not silently assumed solved. |
| 5 | External registry's decay-into-telemetry blend has no specified function; a naive binary switch would cause a visible ranking jump the instant one telemetry data point exists | B | High | Specify precisely (e.g., weight = `max(0, 1 - n_calls/N)` blended additively with the existing `scoreboard_feedback_boost` pattern) or explicitly scope as its own design pass — don't leave "decays" as unspecified prose. |
| 6 | Model-ID mismatches across OpenRouter `canonical_slug`, Artificial Analysis IDs, and Vox's internal registry keys could silently misattribute or drop external scores | B | High | Require a concrete reconciliation test before any external score is blended in; log/metric unmatched IDs rather than silently dropping them. |
| 7 | Per-client polling of rate-limited registry endpoints (500/day, 100/day) doesn't scale across many desktop installs | B | High | Centralize: a Vox-operated cache/proxy refreshed on a schedule and redistributed, not each install calling the APIs directly (mirrors the existing centralized-telemetry precedent). |
| 8 | No staleness/deprecation handling — Vox could keep recommending a model a provider has retired | B | High | Add a "prune models absent from the latest OpenRouter `/models` pull" rule to the refresh job. |
| 9 | Hardware-fit badge is NVIDIA-only; a false "green" on unsupported hardware is worse than no badge | D | High | Confirmed via code (§3.2) — require an explicit third "Unknown" state, not silent omission or a default-green. |
| 10 | Registry with no genuine quality signal degrades the "Auto" default to an unordered/first-available list while still calling itself intelligent | B, C | High | OpenCode's precedent (§2.4) — audit whatever ships as "Auto" against this bar explicitly before calling it done. |
| 11 | Wizard could re-show itself to existing users who already have keys configured, or fail to detect a genuinely fresh install | C | Medium | Gate explicitly: zero configured provider secrets AND zero local models detected AND a dismissal flag unset. Reuse `VersionMismatchBanner.tsx`/`BackendBanner.tsx`'s existing dismiss-persistence pattern and the already-designed §9.5 replay-from-Settings pattern (§1.4) rather than inventing new infra. |
| 12 | No accessibility, i18n, or shared-machine/multi-profile scoping stated for a new top-level GUI surface | C | Medium | State explicit v1 scope exclusions (e.g., English-only first pass) rather than leaving them silently undecided; confirm whether Clavis secret storage is already per-OS-profile scoped. |
| 13 | Vox's UI becomes a low-friction vector for scripted mass OpenRouter account/key creation | A | Medium | Abuse prevention is OpenRouter's responsibility at their own authorize screen — state this explicitly rather than leaving it unconsidered; don't add anything on Vox's side that makes farming easier (e.g., no bulk/headless invocation of the flow). |
| 14 | Single point of failure: both A and B depend on OpenRouter specifically; two competitors' free tiers vanished outright in the last few months (§3.5) | A, B | Medium | Treat as a named dependency risk, not just a discussion bullet — see §7 open question on whether a second provider (e.g., Gemini free tier) should be a co-equal option, not a fallback nobody built. |
| 15 | No telemetry-trust compliance statement for either the registry pull (B) or wizard funnel analytics (C) | B, C | Medium | Resolved for B — `telemetry-trust-ssot.md` covers *outbound* data only; a one-way catalog GET with no user data leaving the device is out of scope, confirmed by direct read (§1.7-adjacent finding). Wizard analytics, if added, is a new outbound data class and **would** need ADR-023's opt-in path — state this explicitly if KPIs (§6) are implemented. |
| 16 | `vox model explain`'s complexity-blindness (§1.6) blocks other work if misdiagnosed as a dead scorer | B, C, D | Resolved | Root-caused to a telemetry-boost-dominance artifact, not a wiring bug (§1.6). Does not block B/C/D. The narrower follow-up (should telemetry boost decay at high complexity) is separate, smaller, and not gating. |

---

## 5. Design recommendations

Scoped to plug into the existing `SelectionAxes`/`decide()`/`model_scoreboard` architecture (§1.1),
not build a parallel system beside it. Each now incorporates its round-2 fixes directly rather than
leaving them as separate critique notes.

**A. Fix the zero-key path — via a loopback-HTTP-server OAuth PKCE flow, not a bare "add the
button."** When no provider key is resolvable and no local model is available, offer inline
provisioning of an OpenRouter key using RFC 8252's desktop-preferred loopback pattern (§3.3), PKCE +
`state` mandatory. The provisioned token must be written through the existing `vox_secrets`/Clavis
API (`SecretKind::OAuthRefreshToken`, §1.5) so it's indistinguishable from a manually-entered key to
every other Vox surface. This alone does not fully solve "free tier handling" — it must ship
alongside distinct, non-generic messaging for the *quota-exceeded* state (risk #3), not just the
*no-credential* state, or the same class of opaque failure reappears after a few hours of use. Treat
the current absence of enforced budget caps (§1.7, risk #4) as an explicit dependency to flag, not
something this recommendation can silently lean on.

**B. Cold-start-only ingestion of an external rating registry, weighted toward coding-specific
benchmarks.** Build the registry from Epoch AI's client (aggregating Aider polyglot, METR
time-horizon, SWE-bench Verified) as the primary coding-relevant signal, OpenRouter `/benchmarks` +
Artificial Analysis as the general-purpose/speed backbone (§2.1/§2.2), used as a *prior* for
models/users with no `model_scoreboard` history yet. Specify the decay-into-telemetry function
concretely rather than leaving it as prose (risk #5); require an ID-reconciliation test across
sources before blending (risk #6); centralize fetching behind a scheduled, cached refresh rather
than per-install polling (risk #7); prune retired models on refresh (risk #8). Judge the result
against OpenCode's cautionary bar (§2.4, risk #10): if "Auto" can't actually be ranked by something
real, say so rather than implying intelligence that isn't there. Consider RouteLLM's lightweight
routing algorithms (§2.3) as a longer-horizon upgrade path once Vox has any implicit preference
signal (retries, escalations) to train on — a learned router beats a static leaderboard lookup, but
isn't a v1 requirement.

**C. Build the onboarding wizard as a new Axis surface, reusing existing (if unbuilt) design and
dismiss-persistence patterns rather than inventing new ones.** Plug points: `ModelsView.tsx`/
`BackendAvailability.tsx` (add the missing free-tier filter; finally render the dead `quality_score`
field), `SettingsView.tsx` (link `LlmSettingsSection`'s banner to `KeysSecretsSection` at minimum),
registered as a new top-level surface in `Sidebar.tsx`. Default a brand-new user straight into a
working Auto-only state (Windsurf/GitHub Copilot pattern, §3.1), manual picking as an opt-in
"customize" step. Gate wizard visibility explicitly: zero configured secrets AND zero local models
AND dismissal flag unset (risk #11) — reuse `VersionMismatchBanner.tsx`/`BackendBanner.tsx`'s
dismiss pattern and the already-designed §9.5 first-run-tour spec (replayable from Settings →
Onboarding) instead of building a third pattern from scratch. State explicit v1 scope exclusions for
a11y/i18n/shared-machine handling rather than leaving them silently undecided (risk #12).

**D. Wire the hardware-fit badge, with an explicit third "Unknown" state and NVIDIA-only scoping
stated up front.** `vox-plugin-nvml-probe` fails clean on non-NVIDIA hardware (§3.2) but the
caller-side mapping from that error to a neutral badge state doesn't exist yet — build it as
Fit/Won't-fit/**Unknown**, never defaulting unknown to a false green. Scope the recommendation text
itself (not just an audit footnote) as "NVIDIA GPUs only in v1" so nobody mistakes this for
vendor-agnostic parity with LM Studio/Jan.

**E. Resolved — no longer a blocker.** The scorer's apparent complexity-blindness (§1.6) is
root-caused to a telemetry-boost-dominance artifact on registries where one candidate already has
scoreboard history, not a wiring bug. B/C/D do not depend on further resolution. The narrower
follow-up — whether `scoreboard_feedback_boost` should decay at high task complexity — is real but
separate, smaller-scoped work; flag it to the harness-parity plan's owner rather than folding it
into this doc's recommendations.

---

## 6. Success metrics, rollout, and testing (new — absent from the prior draft)

**Metrics**, if C ships: completion rate through the wizard, time-to-first-successful-inference,
per-step drop-off, and — the metric that actually answers "did this fix the bug" — % of new installs
that reach a working chat state without hitting a terminal error. Any of these that leave the device
are new outbound telemetry and must go through the opt-in path per `telemetry-trust-ssot.md`/ADR-023
(risk #15), not a bespoke reporting call bolted onto the wizard.

**Rollout**: no feature-flag strategy is specified yet for A–D. Given OAuth redirect handling
(§3.3) is genuinely new infrastructure with real platform-specific failure modes (e.g., a broken
loopback bind on one OS), ship behind a flag with a fast rollback path rather than a single release
cutover — consistent with the repo's existing feature-flag conventions elsewhere.

**Testing**: the OAuth flow (A) should be testable without a live OpenRouter round-trip (mock/stub
the token exchange); the zero-key/zero-Ollama state should be simulable in CI so `vox doctor`'s LLM
routing check (§1.3) can be updated in lockstep rather than drifting from A's behavior change;
registry ingestion (B) needs coverage for API downtime and malformed responses, not just the happy
path.

## 7. Open questions

**Must resolve before spec'ing** (technical feasibility, not just scope):

1. Does OpenRouter's PKCE flow work cleanly with a loopback redirect on all three desktop OSes in
   practice, or does it need a fallback path — and would OpenRouter support a device-flow variant
   instead, matching Vox's existing OAuth precedent (§3.3)?
2. What's the actual decay function for B's external-prior-to-telemetry blend (risk #5) — needs a
   concrete design pass, not just "decays."
3. Who owns closing the unenforced-budget-caps gap (§1.7, risk #4), and does A ship before or after
   that's fixed, given A increases the odds a first-time user is affected?

**Sequencing/scope** (once feasibility is settled):

4. Scope for a first pass — all of A–D, or A alone (arguably the sharpest, most self-contained bug)
   as a fast follow, with B/C as a separate, larger effort?
5. Is OpenRouter an acceptable single point of dependency for both the free-key flow (A) and the
   rating registry (B), given two competitors' free tiers vanished outright in the last few months
   (§3.5, risk #14) — or should a second provider (Gemini's free tier, or a bundled
   local-Ollama-first path) be a co-equal option in the wizard from day one, not a fallback added
   later?
6. Should the narrower scoreboard-feedback-decay follow-up from §1.6 be handed to the harness-parity
   plan's owner as separate work, or folded into whichever spec touches `scoring.rs` next?

## 8. Index

Browse **Architecture SSOTs** in the Starlight sidebar, or start at [contributor-hub](../contributors/contributor-hub.md).
