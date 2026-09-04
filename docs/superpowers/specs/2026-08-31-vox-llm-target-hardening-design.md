---
title: "Vox as an LLM Target — Actuation, Measurement, and the Cost-Budgeted Enforcement Plane"
description: "Revision 2, rewritten after a seven-track adversarial audit retracted its central conclusion. Design for closing the gap between Vox's built guardrails and its armed ones: restoring an amputated measurement spine, arming gates whose failure paths are unreachable, extending the canonical-map registry instead of inventing a second one, and paying for all of it on a fast tier that is already 1.3-1.5x over its own budget."
category: "architecture"
status: "design"
date: 2026-08-31
---

# Vox as an LLM Target — Actuation, Measurement, and the Cost-Budgeted Enforcement Plane

> **Revision 2 (2026-08-31).** Revision 1 was audited along seven independent tracks against
> the source tree. It did not survive. Its central empirical claim was a misread exit code, six
> of its thirteen proposals rebuilt things that already ship, two rested on false premises about
> Rust and about the Vox compiler, and three would have caused incidents. Every claim below now
> carries `file:line` or a measurement. **§10 records what Revision 1 got wrong**, per this
> repo's provenance convention — the failed claims are kept, not deleted, because two of them
> were wrong in instructive ways.

> **Scope.** Continues, and does not restate:
> [`vox-as-llm-target-audit-and-plan-2026.md`](../../src/architecture/vox-as-llm-target-audit-and-plan-2026.md)
> (CR-L1..CR-L8 criteria) and
> [`llm-target-reliability-audit-2026-08-31.md`](../../src/architecture/llm-target-reliability-audit-2026-08-31.md)
> (enforcement reachability; itself corrected in revision 2).

---

## §1 Thesis

**Vox has a recurring class of mechanisms that are built, committed, documented — and never
wired to anything that can fail.** Not a dominant failure mode: an unbiased sample of 11 `vox ci`
gate implementations found 11/11 armed, 9/11 carrying unit tests, and only 14 of 145 `ci.yml`
steps marked `continue-on-error`, each with an inline justification. One of those justifications
reads *"Blocking on purpose. `continue-on-error: true` here is what let the baseline sit at
all-zeros for months"* (`ci.yml:1013`) — the repo has already diagnosed this class at least once
and written the lesson at the site.

It is nonetheless the class that produced every significant finding in this audit, and the
confirmed instances are worse than Revision 1 knew:

| # | Mechanism | Evidence | State |
| --- | --- | --- | --- |
| 1 | **The five CR-L LLM harnesses were deleted by a bad merge** | `3c7b3b917` (2026-05-27) dropped 3,858 lines across six files, present in both parents, absent in the result. `lib.rs:265-273` now registers `RepairCorpusStub`, `PlanFidelityStub`, `SpecToAppStub`, `MensOnDistributionStub`, `DeployStub`. `stubs.rs:30-40` returns `InfrastructureError` unconditionally. `panel.rs` (1,028 lines, the OpenRouter client) is unreferenced. | **Silently reverted 96 days.** A committed test, `every_stub_returns_infrastructure_error_with_incomplete_report`, now *asserts* the regression. |
| 2 | **`semcov-gates` is never invoked** | Sole production reader of both toestub baselines (`bin/semcov-gates.rs:28-29`). Absent from every workflow, from `lefthook.yml`, and from `core_gates.rs:177-190`'s `run_core_all()` trio. `ci.yml:1243` carries a comment asserting it "IS blocking". | 1,642 baseline entries suppress a gate that has never run. |
| 3 | **33 of 45 detector rules score a vacuous F1 = 1.0** | `bench.rs:95-103` scores zero-fixture rules at precision 1.0 / recall 1.0. `bench.rs:51-56` splits the rule id at the first `/`, seeking `fixtures/security/hardcoded-secret-aws-key_pos` while the file is `fixtures/security-hardcoded-secret/aws_key_pos.txt`. The per-rule `fixtures:` blocks in `rules.v1.yaml` are never read. | `--min-f1 0.70` (`ci.yml:1320`) passes on an empty measurement for every `stub/*`, `scaling/*`, `ai-laziness/*`, and `security/hardcoded-secret/*` rule. |
| 4 | **The `FULL=true` TOESTUB branch scans 22 files** | `ci.yml:751-752` invokes `toestub-scoped` with no roots; `matrix.rs:556-558` defaults an empty root list to `crates/vox-repository` — 22 of 3,889 `.rs` files. | The full-scan branch is narrower than the affected-crates branch. |
| 5 | **The scoped TOESTUB severity floor is two notches too high** | `engine.rs:450-453`: `EnforceWarn => severity >= Critical`. Exactly one rule can reach it — `security/hardcoded-secret/aws-key`, `rules.v1.yaml:464`. | Narrower than Revision 1 claimed (see §10.1) but still ~1/45 coverage. |
| 6 | **A purpose-built CI cost meter runs and is discarded** | `vox ci job-timings` (`job_timings.rs:1-22`, 600s slow threshold) runs after every CI run via `ci-timings.yml:36 --annotate`. Output is ephemeral check annotations — no artifact, no time series. | Every cost decision in this document was made without it. |
| 7 | **Budget enforcement is opt-in and off** | `pre_push.rs:264`: `if opts.enforce_budgets { check_tier_budget(...) }`. Default false. | The fast tier is 1.3-1.5x over its fail wall and nothing reports it (§1.2). |
| 8 | **`suppressions.v1.json` is never loaded** | `ToestubConfig::suppression_path` defaults to `None` (`engine.rs:82`); `lefthook.yml:48` passes no `--suppressions`. | The structured ledger — owner, reason, the whole expiry design — is not read by the gate meant to honour it. |
| 9 | **`enforced_by` is never deserialized** | `contracts_index.rs:38-43` destructures `{id, path, kind}` only. 92 of 189 entries (49 %) name `vox ci contracts-index` as their sole enforcer, which is a file-exists check. | An enforcement annotation that enforces nothing. |
| 10 | **Placeholder values in integrity fields** | `build-bench-baseline.v1.json`: 6/6 records `wall_ms: 0`. `repair-corpus/manifest.v1.yaml`: `corpus_hash: "blake3:0000…0000"`. | `build_bench.rs:181` already detects its case and bails (`ci.yml:1013` blocking) — so build-bench should be **failing on main right now**. The corpus hash has no such check. |

**Design consequence.** The highest-value mechanism is not another detector, and not a new
registry. It is (a) **restoring measurement**, (b) **proving the mechanisms that exist can
fail**, and (c) doing both inside a cost budget that is already exceeded. Ordering follows in §8.

### §1.1 RETRACTED — the "measured LLM-target profile"

Revision 1 read the 2026-07-27 snapshot's five exit-2 gates as failures and concluded *"models
can write Vox; they cannot yet repair, extend, or faithfully execute a plan in Vox."*

**That is wrong.** `report.rs:44-54`: exit **1** is `BarMissed` (measured, bar unmet); exit **2**
is `InfrastructureError` — *no measurement taken*, and per its own contract it "does not block
CI". All five exit-2 gates are the stubs of §1 row 1. Exit **−1** is a missing binary
(`ga.rs:128-151`), a `required-features` packaging bug fixed by `1153557e8` **the same day** as
the snapshot.

What the last executed run (2026-05-23, panel = claude-sonnet-4-6 + gpt-5.4, $2.56 of a $25 cap)
actually measured:

| Gate | Result | Bar | `threshold.met` |
| --- | --- | --- | --- |
| repair-corpus (CR-L3) | 0.775 | 0.70 | **true** |
| plan-fidelity (CR-L4) | 0.910 | 0.85 | **true** |
| spec-to-app (CR-L0) | 0.683 | 0.60 | **true** |
| deploy (CR-L7) | 1.0 | 1.0 | **true** |
| mens-on-distribution (CR-L2) | — | 0.95 | never measured; blocked on a CUDA `CUDA_ERROR_INVALID_IMAGE` per `evidence-ledger.v1.json → blocked_claims` |

`humaneval`'s green is also not a model measurement: `humaneval.rs:159` records
`median_pass_rate: Some(pass_rate), // single scorer — no LLM panel yet`, and the 2026-07-27
report carries `"llm_panel": []`. It typechecks and runs the corpus the repo authored. It is a
corpus self-check.

**Correct statement of what we know:** *As of 2026-08-31 the CR-L LLM gates are **unmeasured**,
not failing. On the last real run, four of five were above bar. We do not currently know whether
Vox has a repair deficit, and nothing in this document may be ranked as though we do.*

Revision 1's Design Consequence #2 — rank every investment by whether it moves
repair/fidelity/on-distribution — is withdrawn. Its §7 ranking was derived from failures that
did not occur and has been rebuilt from scratch in §8.

### §1.2 The cost reality — measured, and worse than the comment

The `48s` figure Revision 1 built its cost section on is a **hand-typed YAML comment**
(`test-tier-budgets.v1.yaml:25-33`) with no artifact behind it. `contracts/reports/` holds
`test-baseline.v1.schema.json` and **no instance**.

Measured on a developer machine:

| | Warm | Cold |
| --- | --- | --- |
| `ssot-drift` | **125.2s** | **196.3s** |

Top generators (warm): `command_compliance` **33.5s**, `completion_quality` **24.8s**,
`check_docs_ssot` **18.3s**, `affected_cmd::check_graph` **15.2s** (shells `cargo metadata`),
`sql_surface_guard` 12.9s — the top four are 73 % of the total. Other fast-tier steps:
`check-links` 13.5s, `canonical-map-verify` 11.1s (comment claims 6s), `retired-symbol-check`
9.0s.

**Real fast tier ≈ 175–200s against `fail_ms: 135000`.** Already 1.3–1.5× over, unreported
because of §1 row 7.

Other corrected figures: **~154** `vox ci` subcommands (not 91), **45** workflow files (not 25),
145 named `ci.yml` steps (verified). `contracts/**` forces a full gate on **11–12 %** of the last
200 commits (not "constantly"), and mostly on *generated* files. Declared job ceilings the cost
section never named: `docker-vox-image-smoke` **180 min**, `setup` **40 min on every run**.

**Design consequence.** No cost claim in this document is actionable until §1 row 6 is fixed.
Measurement precedes optimization, and the meter already exists.

---

## §2 Principles

1. **Measure before arming.** A threshold flipped against an unmeasured population is an outage.
2. **Extend the mechanism that exists.** Six Revision-1 proposals rebuilt shipped code. Search first.
3. **Every mechanism proves it can fail** — by the cheapest available means, which is usually a
   unit test, not a new subsystem.
4. **Every exemption expires** — and the expiry must be enforced by something, which today
   nothing in this repo does (§3.3).
5. **Cost is a property of a rule.** An unmeasured gate cost is an unbounded gate cost.
6. **Deletion counts as delivery.** Four dead ratchets and one dead crate-dependency were found;
   removing them is progress.

---

## §3 Actuation

### §3.1 Prove gates can fail — three mechanisms, cheapest first

Revision 1 proposed a `contracts/canaries/` tree for every blocking gate. Audit found: 3 of the
5 proposed canaries are infeasible (`config-hygiene` takes no path argument; `crate-edges` and
`arch-check` both shell `cargo metadata`, seconds not milliseconds), 2 duplicate tests that
already exist (`crate_edges.rs:420 new_edge_fails`, `:486 upward_layer_edge_fails`,
`:514 missing_layer_fails`), and one targets a warn-only path that cannot fail at all
(`main.rs:1488 wtl_parity_warns`). The estimated cost was ~2s; realistic is **+30–90s**, because
these gates are whole-workspace walkers whose cost is the walk, not the input.

Revised, in ascending cost:

**(a) Negative unit test — the default.** Already the pattern in 9 of 11 sampled gates. Compiled
against the gate, cannot rot silently, runs in the existing nextest tier, costs no new contract
directory and no fast-tier budget. **Every new gate ships with one; this is a review rule, not a
subsystem.**

**(b) `cargo mutants` scope extension — one config line.** Already runs on PRs touching
`crates/vox-compiler/**` and `crates/vox-codegen/**`. Extending it to
`crates/vox-code-audit/src/engine.rs` would have killed §1 row 5 outright.

**(c) Cross-process canaries — the residue only.** A canary earns its cost only where the failure
path crosses a process boundary into shell/YAML exit-code wiring, which no unit test observes.
That is a small set, and §1 rows 4 and 5 are both in it. Ship **one**: `toestub.enforce-warn`.
Add others only as that boundary is found elsewhere.

Two properties Revision 1 missed:

- **One canary certifies one threshold, not the floor.** Its proposed content ("a hardcoded
  secret + a retired import") would have caught §1 row 5 only by accident — the secret happens to
  be the single Critical rule. Require **one canary per severity tier the gate claims to
  enforce**.
- **A cached PASS is a disabled gate.** Every canary must run twice — cache bypassed and cache
  enabled. If a canary passes under the cache, the cache is unsound. This is the check that makes
  §6.3 safe, and it did not exist in Revision 1.

### §3.2 Gate actuation ledger — check for overlap first

The intent stands: nobody can answer "which of our ~154 gates can fail, and what do they cost?"
without reading 145 CI steps.

**But `contracts/policy/policy-registry.v1.yaml` is 2,261 generated lines already carrying
`severity`, `blocking`, `runs_on: [pre-push, ci]`, and a `source: {kind, ref, detail}` producer
pointer per policy** — plausibly 70 % of the ledger. The design work is a gap analysis against
that file, not a new contract. Whatever is genuinely missing (`cost_ms_p50`, `canary`,
`cacheable`, `last_effective_failure`) is added as columns there.

`cost_ms_p50` must be **written from measurement** — the pre-push report (`pre_push.rs:344`,
schema already committed) and the JUnit elapsed times — never hand-entered. See §6.1 for why a
third independent cost opinion is the failure mode to avoid.

### §3.3 Expiry — no working reference exists in this repo

`expires_after` is parsed and inert (`suppression.rs:30-32`, `#[allow(dead_code)]`). It appears
in exactly one non-test location workspace-wide. `suppression.v1.schema.json` makes it optional
with no format validation, and `validate_toestub_suppression_contracts` (`suppression.rs:88-112`)
validates only `suppressions.v1.json` and its example — **neither baseline is schema-validated**.

**Revision 1 said to "mirror the working implementation in `check_links.rs:79-87`". There is no
working implementation.** Read past line 87: on expiry it prints `WARN … allowlist entry expired
(still skipping)` and returns `true`. The reference model is itself detection theater. Enforcing
expiry means writing the first enforcement in the repo and fixing `check_links` in the same
change, or shipping a second decorative field.

**The 1,642 are dead data, not suppressions** (§1 row 2). Reframed, the work is:

1. **Wire `semcov-gates`** — declare it as a `[[bin]]`, run it non-blocking for one week, publish
   the real beyond-baseline count. Every number below is unmeasured until this runs.
2. **De-pin `line` before anything else.** `suppression.rs:170-176` matches `path_glob` **and
   exact line**. All 1,642 entries are line-pinned; measured churn is **0 % cold**, median 3–5
   commits since the freeze, hottest file 90 commits. Line numbers have drifted, drifted entries
   no longer match, and the findings resurface as *new*. Arming the gate as-is fails on an
   unknown large fraction of 1,642, indistinguishable from real regressions. Replace exact-line
   matching with a per-file monotone remaining-count budget.
3. **Order by file, densest-first — not by churn.** Revision 1's §8 hedged that churn-staggering
   might fail because the code is cold. It fails for the opposite reason: the population is
   uniformly hot, so churn has no discriminating power — a cliff dressed as a trickle.
   Per-file has a real tail (`vox-compiler/src/eval/builtins.rs` holds 45 entries in one file)
   and matches the review unit. Line-pinned entries in one file also drift together.
4. **Drop "escalate advisory → blocking".** `run_silent_drop_gate` / `run_weak_test_gate` are
   count-based with no severity rung (`core_gates.rs:83-89, 118-124`). The lever is the per-file
   count budget.

Also: make `expires_after` `required` with a `pattern` in the schema; extend the validator to
both baselines; and load `suppressions.v1.json` in the runners that are supposed to honour it
(§1 row 8).

**Not a hazard:** `ssot-autoregen`'s 11 regenerators (`ci.yml:247-258`) touch no suppression
baseline. Revision 1 raised this as a risk; it does not exist. **A real one does:**
`cr-l8-corpus-feedback.yml:401-405` `cp`s the current pass-set over
`scripts-pass-baseline.txt` with no monotonicity check — inert only because no commit step
follows it. Any auto-commit work must be scoped to `contracts/reports/` evidence artifacts and
must never reach a pass-set baseline.

---

## §4 The language and the compiler

### §4.1 A `strict` profile — as an axis on the table that already exists

`syntax_version` does nothing to the compiler: it is a regex over comment text in one
code-audit detector (`detectors/syntax_version.rs:27`, Warning), and `syntax_version` appears
zero times in `crates/vox-compiler/src/`. `typeck/policy.rs:3` is an explicit stub.

The right host is `crates/vox-compiler/src/feature_matrix.rs` — an exhaustive `(Feature, Target)
→ Support` table with **no `_` catch-all**, so adding a variant fails the build until every cell
is declared (`support`, line 649). A profile is a third axis on that table. Building
`syntax_profile` as a parallel subsystem beside it would be the split-brain this document exists
to prevent.

The ambiguity is real, all three cases verified — and one is worse than described: `@v0` is not
merely a no-op, it is **silently dropped from HIR with no diagnostic at all**
(`hir/lower/mod.rs:417-419`).

Two corrections to Revision 1: **do not emit GBNF** — `grammar-export/src/lib.rs:97` refuses it
repo-wide over CVE-2026-2069 (ReDoS); emit EBNF / Lark / XGrammar-2. And **fix the compiler's own
deprecation text first**: `head.rs:46` says `"@mcp.tool is deprecated; use @tool instead"` while
AGENTS.md §Grammar Unification says the canonical form is bare `tool`. A live split-brain between
a diagnostic and the policy SSOT, one line, worth more than the profile ladder.

### §4.2 Constrained generation — wire one call site before designing a second mode

**`vox-constrained-gen` has zero call sites.** Three crates declare the dependency
(`vox-populi`, `vox-cli`, `vox-ml-cli`); nothing in `crates/` references `GrammarMode`,
`build_sampler`, or `mask_logits`. The gate that would exercise it is a `println!`:
`mens.rs:33-35`, *"constrained-gen-smoke (placeholder): would validate {n} samples."*

So Revision 1's premise ("`GrammarMode` is a whole-generation switch") is true and meaningless —
it is never set. And `RevisionSampler` cannot serve as a phase boundary: `revision.rs:60-105`
un-masks a `<backtrack>` sentinel when all logits are `NEG_INFINITY`, never checkpoints, never
rewinds, and `max_depth` is unread (`revision.rs:39`).

The A/B Revision 1 proposed (`None` / `Vox` / `TwoPhase`) is unrunnable: `Vox` has no path to a
model. **Wire one real call site first.** Only then does `TwoPhase` have a baseline to beat, and
only then does the literature's 10–30 % reasoning cost become a claim about *this* sampler rather
than about other people's grammars.

Separately: implement `RevisionSampler`'s checkpoint stack or delete `RevisionConfig`. A
max-depth field that is never read is §1's class in miniature.

### §4.3 Diagnostic amplification factor — real, but it needs its own fixtures

The metric stands. **Revision 1's premise that repair-corpus already encodes fault injection is
wrong.** `problems/*/` are hand-authored `buggy.vox` / `fixed.vox` pairs with pre-existing,
sometimes multi-line bugs across five classes including **`logic`** — which compiles clean, emits
zero diagnostics, and would score DAF 0, flattering the corpus average toward the ≤1.3 target and
tripping the stop condition on an artifact of corpus composition. The 2026-05-23 report already
notes *"10 logic-class fixture(s) excluded — test execution not implemented."*

Two facts make the task cheaper than it looks anyway:

- `projects/*/expected.json` carries `expected_diagnostic_count_before` — the DAF denominator,
  already in the data, **read by nothing**.
- `vox check --json` already exists (`pipeline.rs:270 format_diagnostics_json`).

**Revised:** build a small fault injector over known-good fixtures (`examples/golden/**`),
restricted to syntactic and type faults. Report per-fixture, and **split parse from typecheck** —
`descent/mod.rs:571 recover_to_top_level()` already resynchronizes at declaration boundaries with
brace-depth tracking, so parse-side DAF is likely near 1 and "improve parser recovery" is
probably the wrong fix. `run_frontend_str` accumulates `Vec<Diagnostic>` with no dedup and no
`caused_by` suppression; that is where a cascade would live. Wire the unread
`expected_diagnostic_count_before` as a second, free data source.

**Tier: nightly.** Injecting and compiling per fixture is minutes. It must never enter the fast
tier.

### §4.4 The typed diagnostic contract — ~80 % shipped

Already in the tree: `typeck/diagnostics.rs` defines `VoxCompilerDiagnosticPayload`,
`SuggestedFix { label, replacement, span }`, `DiagnosticFix`, `SpanPayload`, `LineCol`,
`DiagnosticExcerpt`, `MinimalRepro`; `codes::ALL_COMPILER_DIAGNOSTIC_CODES` holds ~72 codes with
a compile-time sync test; `contracts/diagnostics/registry.v1.yaml` is a namespace contract
collision-tested against audit rule ids; `check.rs:84-92` ships `--json` / `--for-llm`. Its own
doc comment cites the CR-L audit as the reason it exists.

The genuine delta is narrow: `one_fix_site`, `confidence`, `caused_by`, and a schema version pin.
Note that span+replacement is **strictly better than a unified diff** for machine application —
Revision 1's "must be a diff, not prose" was arguing against prose, and the repo already won that
argument.

**Re-cost the one large item separately:** "every diagnostic code owns a repair fixture" is
**72+ new fixtures against a corpus of 15** (`manifest.v1.yaml: count_current: 15, count_target:
50, status: minimum-viable`). That is larger than every other Phase-4 task combined and cannot
sit inside a two-week estimate.

### §4.5 Token economy — DELETED, it exists and is armed tighter than proposed

`vox ci source-token-budget` (`cmd_enums.rs:570-579`, `syntax_k.rs:146`) measures every
canonical-ladder golden, fails on regression, and runs via `pipeline_parity.rs:47` at
**tolerance 0.0** in `ci.yml:880`. Revision 1's "fail on >10 % regression" would have **loosened a
shipped gate tenfold**.

One real note: `syntax_k.rs:141-142` states these are *"structural lexer tokens … NOT model BPE
tokens."* Revision 1's context-window argument was a BPE argument applied to a lexer metric. If
the context-window property is the one worth gating, add a `bpe_tokens` field alongside — do not
touch the tolerance.

### §4.6 Wiring — the load-bearing claim was false

> Revision 1: *"Vox is a full-stack language with a closed world at compile time — it knows the
> mount points."*

**It does not.** `pipeline.rs:96` is `run_frontend_str(source, file_path)` — one string in, one
`HirModule` out. No module graph, no `Vec<HirModule>`, no link phase. `build.rs:156` compiles one
file. `app_contract.rs:92` projects from a **singular** module. `hir/lower/mod.rs:195-199`
records `ImportPathKind::LocalFile` and comments that resolution and cycle detection happen at
`Interpreter::run_module` time — check time never opens the imported file.

A `route` in `handlers.vox` mounted from `app.vox` is invisible. The proposal as written would
fire false errors on exactly the multi-file layouts it targets.

Revision 1 also mis-stated the status quo: `unwired_module` and `reachability` are **Rust/TS
only** (`reachability.rs:161`, `unwired_module.rs:26-36`). Nothing polices unwired surfaces in
Vox today — which strengthens the motivation and invalidates the "spent on a Warning-severity
regex" line.

**Split:**

- **4.6a — intra-module, decidable now.** A `route` in a file whose own `routes` block does not
  mount it and which exports nothing; a `state_machine` state with no inbound transition (pure
  AST; the outbound case already ships as `state_machine_unreachable`). Land as an
  **Error-severity lint with an allowlist**, measure the false-positive rate on
  `examples/golden/**`, and only then consider promoting.
- **4.6b — cross-file, requires a compiler feature that does not exist.** `vox check --project`
  resolving `HirImport::local_file_path` at check time and merging `AppContractModule`s. Its own
  phase, weeks, not a bullet. Until then the honest home is a project-scoped lint over the merged
  set. Note the pipeline hazard: making an early stage depend on the emitted MCP manifest is
  either circular or a forced two-pass compile.
- **Never gate corpus generation on a half-landed dialect.** Revision 1's "require `strict` for
  all generated code and all MENS corpus" would have the corpus generator rejecting its own
  output, silently narrowing the training set to whatever survives a new error. That is §3.3's
  "baselines teach the pattern" hazard running in reverse, on the training data.

### §4.7 The Rust core — the two bold claims were both wrong

**`std::env::var` cannot be made unreachable.** No crate can prevent another writing
`std::env::var("OPENAI_API_KEY")`; it is std. The enforceable substitutes are `clippy.toml`
`disallowed-methods` or the existing detector. "Delete `env_secret_shape`" is unreachable.

But the real population is tiny. Of 293 files calling `env::var` and 98 secret-shaped matches,
after excluding tests, audit fixtures, and `VOX_SECRETS_*` control knobs: **5 genuine production
reads in 4 crates** — `vox-actor-runtime/src/builtins/mod.rs:1162`,
`vox-orchestrator-mcp/src/agent_tools.rs:31`, `vox-plugin-webhook/src/lib.rs:74,135`,
`voxup/src/channel.rs:44`. Migrate those to `resolve_secret` and the detector's true-positive
rate goes to zero. That is the whole win and it needs no type system.

**`#[non_exhaustive]` does the opposite of what Revision 1 claimed.** It *requires* downstream
matches to carry a wildcard arm, so adding a variant compiles cleanly everywhere and falls into
`_ =>`. Applying it would destroy the call-site inventory it was proposed to create. The
construct that delivers that property is a plain exhaustive enum with no `_` — which
`feature_matrix.rs:639-651` already uses and documents. **The actionable version is a lint
against stray `_` arms in cross-crate matches, and against applying `#[non_exhaustive]` to
workspace-internal enums.**

The HTTP half is real but is a dependency-level change, not a type: `vox-http-client` already
provides the chokepoint (`lib.rs:26,34`) and only 3 crates construct `reqwest::Client` directly —
but **30 crates depend on `reqwest`** and ~20 use its types. Newtype the return, drop `reqwest`
from consumer manifests, and let the already-armed `crate-edges` ratchet hold the line. Heaviest
first: `vox-publisher` (17 call sites), `vox-orchestrator-mcp` (15), `vox-gamify` (10).

---

## §5 SSOT — extend the registry that exists

### §5.1 The fact registry already exists

`contracts/documentation/canonical-map.v1.yaml` (18 domains) with `canonical_docs.rs:15-37`:

| Revision 1's proposed `facts.v1.yaml` | Shipped `canonical-map.v1.yaml` |
| --- | --- |
| `fact:` | `id:` + `title:` |
| `producer:` | `canon_doc:` (human) + `spec_paths:` (machine) |
| `consumers[] mode: generated` | `generated_docs:` |
| `consumers[] mode: duplicated` | `aliases:` (already weakly enforced, `canonical_docs.rs:133-157`) |
| — (absent from Revision 1) | `owning_crate_globs:` — code consumers |
| — | `tier: A-spec \| B-canon \| C-generated \| D-index` |
| `vox ci facts-parity` → `ssot-drift` | `vox ci check-docs-ssot` → already in `ssot-drift` |

It even contains the seed row, at line 144 — with **two one-line defects that are the actual
bug**: the producer is inverted (`AGENTS.md` listed as an *alias of*
`agent-instruction-architecture.md`), and `.cursor/rules/` is absent entirely.

**`contracts/ssot/facts.v1.yaml` and `vox ci facts-parity` are deleted from this design.** The
work is: fix the row, promote `aliases` from `array<string>` to
`array<string | {path, owner, reason, expires_after}>` (string form retained), and extend
`verify_alias_rules` to enforce the object form.

Also worth checking before any new registry lands: **`contracts/index.yaml` is *not* a fact
registry** (six fields, `additionalProperties: false`, all 189 paths under `contracts/`), and
only 189 of 519 contract files are indexed with no completeness gate.

### §5.2 Retired surfaces — 10 of 17, and three copies not two

Corrected count: `.cursor/rules/retired-surfaces.mdc` holds 7 of AGENTS.md's 17 rows. Missing 10:
`@endpoint(kind:…) fn`, `@py.import`, `@native`, `@capacitor/*`, `axum::serve`,
`vox-sherpa-transcribe`, `crates/vox-dashboard`, `crates/vox-oratio`, `vox-dei-shim`,
`vox-bootstrap`.

**There is a third copy:** `contracts/documentation/retired-symbols.v1.yaml` (15 entries, a
*different* set — it has `vox-ml-cli-standalone` and lacks ten of AGENTS.md's). Any generator must
target **both** consumers or it hard-codes a two-thirds fix.

### §5.3 Per-tool rule files — 2 of 10 are derivable

Revision 1's Task 3.2 ("generate `.cursor/rules/*.mdc` and `GEMINI.md` from AGENTS.md") is wrong
as written. Audited per file: **2 derivable** (`retired-surfaces.mdc`, `secrets-policy.mdc`),
**3 partial**, **5 fully independent** — `build-environment.mdc` is CUDA paths, linker-retry
lore, and machine-specific `CARGO_TARGET_DIR` hygiene with no AGENTS.md source at all.
`GEMINI.md`'s overlapping sections **already link to AGENTS.md as normative rather than
copying** — the correct pattern, already in place.

"Modelled exactly on `sync-ignore-files`" also transfers nothing: `sync_ignore_files.rs:23-45`
strips a fixed header and copies every remaining line verbatim. A `.mdc` needs section
extraction, table reformatting, per-tool YAML frontmatter with no AGENTS.md source, and
deliberate abridgment.

**Revised into three parts:**

- **Generate the one derivable fact** — `vox ci sync-retired-surfaces`, one source (AGENTS.md's
  17 rows), **two** targets (`.mdc` + `retired-symbols.v1.yaml`).
- **Lint the other eight for contradiction** — this is where the real bugs are and it is cheaper
  than a generator. Four are live today: `documentation-policy.mdc` tells agents to use
  `{{#include}}` (AGENTS.md forbids it) and to add `last_updated` (AGENTS.md forbids that too);
  `voxscript-first-automation.mdc` cites a path that moved to `docs/src/archive/` and says
  `vox-runtime` where AGENTS.md says `vox-actor-runtime`. The two policy inversions need a human
  edit, not a generator.
- **Register the independent eight** in `canonical-map.v1.yaml` against their *real* producers
  (`runner-contract.md`, `data-storage-ssot-2026.md`, `cli-design-rules-ssot.md`), not AGENTS.md.

### §5.4 RETRACTED — "28 phantom crate references"

Zero of the 28 are stale. **19** sit inside a section titled `## Planned but not yet landed`
under a column header `Planned crate`; **3** are inline `_(planned)_`; **6** are deliberate
`(was vox-primitives)`-style consolidation notes. Every markdown *link* in the file resolves —
0 broken. The document already encodes the distinction by convention (linked = real,
bare-backtick = planned or historical) and is 100 % consistent. The audit's regex counted the
convention as the defect. A generator would additionally *delete* the six "was X" notes, which
are precisely what stops an agent re-inventing a retired crate.

**The two real defects run the other way and no phantom-crate lint would catch either:**
`vox-cli-ci` is listed under `## Planned but not yet landed` while shipping 80+ modules, and
`vox-dashboard` is listed as planned while AGENTS.md records it deleted 2026-05-12 per ADR-037.

**Replace the generator with a 5-rule link lint (~40 lines, no new contract):** (1) every
`crates/X/` link resolves — passes today, so it is a free canary; (2) every bare-backticked
`vox-*` is inside the planned section, inline-tagged, or in a `(was …)` clause — passes today;
(3) no crate under `## Planned` has a directory — **fails today**; (4) no AGENTS.md retired name
appears as a `Planned crate` — **fails today**; (5) every directory in `crates/` appears
somewhere — the real coverage gap, currently unmeasured.

### §5.5 Shadowing — the proposed method finds nothing

Of 321 non-test env read sites, **0** carry a same-line `unwrap_or`. The codebase idiom hoists
the default into a named resolver (`runtime.rs:40`). And `VOX_SEARCH_BM25_K1` — Revision 1's own
example — has **no `env::var` site anywhere**. A read-site scan cannot see it.

The shadowing that exists is registry-vs-registry. Three hand-maintained sources record defaults:
`CONFIG_KEYS` (124), `OperatorEnvSpec` (119), `contracts/config/registry.v1.yaml` (99). They
share **3 keys**, of which **2 diverge** (`VOX_CIRCUIT_BREAKER_CONTRACT`,
`VOX_GAMIFY_ECONOMY_PATH` — string vs `null`). 96 keys are recorded only in the YAML, 121 only in
Rust. `config_registry_parity.rs:9-18` compares **name sets only** and *unions* the three
sources, so nothing can ever flag.

**Revised:** extend `config_registry_parity` from name parity to **(name, default) parity** —
~30 lines against code that already loads all three. Land the 2 divergences at `Error`
immediately (sample of 2, precision 100 %); land the 96/121 coverage gap at `Info` with a
ratchet. Additionally enforce the invariant already written at `config_key.rs:21` — *"MUST equal
the in-code constant"* — which is documented and never checked.

**Still explicitly out of scope:** a Rust `let`-rebinding detector.

---

## §6 The cost plane

### §6.0 Measure first — the meter already runs

Commit `vox ci job-timings` output as `contracts/reports/perf/ci-job-timings.v1.json` on every
main run. The workflow already computes it (`ci-timings.yml:36`); this is one commit step,
mirroring `ci.yml:1753-1763`. **Everything else in §6 is unfalsifiable until this lands, and it
is the cheapest task in the document.**

Then re-baseline the tiers: `vox ci pre-push --report-json` on a clean tree, commit the artifact,
regenerate `test-tier-budgets.v1.yaml` from measurement. The 90s figure is fiction and every
decision downstream of it inherits the error.

### §6.1 Arm and unify the budget gate — do not add a third

Two implementations already read `test-tier-budgets.v1.yaml` with duplicated parsing:
`tier_budget_check.rs:68-125` (JUnit elapsed, CI) and `pre_push.rs:415-445` (wall clock, local,
**opt-in**). The split-brain is documented in-source at `tier_budget_check.rs:57`. Revision 1's
Task 2.4 would have made it three, with *a-priori* ledger-sum semantics contradicting two
*measured* ones — and a static sum necessarily diverges from wall clock under parallelism and
caching (my own numbers: 196s cold vs 125s warm for one step).

**Revised:** flip `enforce_budgets` default-on (`pre_push.rs:264`), extract one shared YAML
reader, and have the ledger's `cost_ms_p50` be *written from* those measurements so the sum and
the total are the same numbers by construction. Re-baseline before arming, or every push bricks.

Note `ci.yml:1123-1126` runs `tier-budget-check` under `continue-on-error: true` "until tightened
budgets land (Phase 5 follow-up)" — a temporary flag with no expiry. Arming means removing that
too, or the rule lands advisory by inheritance.

### §6.2 Cheap wins available today, no new machinery

1. **Demote `command_compliance` (33.5s) and `completion_quality` (24.8s) from fast to
   complete** — 58s, 46 % of `ssot-drift`, neither a fast-feedback concern. No cache, no ledger,
   no registry required. **This is the single largest cost reduction in the document.**
2. **Cache `affected_cmd::check_graph`** (15.2s → ~0). Inputs are exactly `is_sentinel`'s set
   (`Cargo.lock` + every `Cargo.toml`). One gate, one obvious key, biggest clean cache win.
3. **Exclude `contracts/reports/**` and `*.generated.json` from the full-gate sledgehammer** —
   two lines, kills roughly a third of the trigger set, needs no fact registry.
4. **Persist the per-generator timings that already exist** (`docs.rs:560-568`, shipped
   2026-06-26, printed to stderr and discarded).

### §6.3 Gate caching — two implementations exist; the hazard is temporal

"No gate result is content-hash cached anywhere" was wrong twice:

- `visus_review/mod.rs:92-110` keys on `screenshot_sha256 ‖ model ‖ prompt_version` — exactly the
  proposed structure — persists to `contracts/reports/gui-visual-review/cache.v1.json`, commits
  it back to main (`ci.yml:1753-1763`), and prunes dead keys. It solves cross-runner sharing via
  git, better than a `.vox/cache/` would.
- `vox-arch-check/src/cache.rs` keys on SHA-256 of `Cargo.lock` + `layers.toml` +
  `where-things-live.md`.

**Generalize the working precedent. Do not invent `.vox/cache/gates/`.**

The `gate_version` primitive also exists: `vox ci` already refuses to run when the binary's build
commit ≠ working-tree commit. Use that identity — a hand-maintained per-gate version number is
exactly the wiring step this repo skips.

**`cacheable` must be opt-in with a mandatory declared input set**, because the dangerous class
is gates whose verdict changes with *no input change*:

- **Clock-dependent:** `check-links` (a link that 200s today 404s tomorrow with byte-identical
  inputs), `retirement-audit`, `ignored-test-age`, `evidence_ledger` (filename dates vs
  `max_age_days`), arch-check Rule 6 (crate mtime vs CHANGELOG date). Caching these memoizes a
  green verdict *across the moment the deadline passes*. **§3.3's own expiry work creates more of
  them.**
- **Env-dependent:** `retired-symbol-check` reads `VOX_CI_RETIRED_SYMBOL_SCAN_CRATES`, which
  *narrows the scan set*; `gui-smoke` / `backend-tests` read switches that gate whether the check
  runs at all — a cached PASS from a skipped run is a fabricated PASS.
- **Existential scans** (17 of them: `sql-surface-guard`, `retired-symbol-check`,
  `no-tauri-in-core`, …) assert "this symbol appears nowhere". Key on files-read and a new
  violating file is invisible; key on a full walk and you have paid the scan. Hidden inputs
  include the allowlists and **`.voxignore`**, whose edit silently rescopes every walker.

**Required:** a clock-perturbation test — run each gate twice with `SOURCE_DATE_EPOCH` advanced
400 days; differing verdicts ⇒ uncacheable. Plus §3.1's cache-bypass canary rule.

Realistically cacheable and worth it: `check_graph` (15.2s), `command_compliance` +
`check_docs_ssot` if enumerable (52s combined — the real prize, never named in Revision 1),
`grammar-*`, `contracts-index`, `capability-sync`, `command-sync`. **The 60–80 % literature
figure does not transfer — this gate mix is unusually cache-hostile, and the design should say so
rather than spend the number.**

### §6.4 Budget arithmetic for this document's own proposals

Baseline ~175–200s against a 135s fail wall. New work, costed: canaries **+30–90s** if placed in
fast (hence §3.1's move to `complete`); ledger generation +~1s; retired-surfaces generator
+~0.5s; default-parity +~1s. `daf` and any corpus-wide compile are **nightly, never fast** — state
the tier explicitly or they will land in `ssot-drift` like everything else has.

**Under the zero-sum rule, §6.2 item 1 must land before any new gate merges.** That is the rule
working as intended.

---

## §7 The unexamined surface — and the LSP

Neither Revision 1 nor the source audit touched `vox-lsp`, `vox-mcp-registry`, `vox-plugin-host`,
the `vox-orchestrator*` family, `vox-gui`, `vox-search`, `vox-skills`, `vox-eval`, or
`vox-corpus` — the majority of the agent-facing surface by crate count.

**`vox-lsp` is the larger lever and the least defended thing found.** It reuses compiler
diagnostics directly (`lib.rs:9-11`) and already ships `code_action_provider: true` with
quick-fixes built from `data.fixes` (`lib.rs:139-178`) — so §4.3's DAF and §4.4's payload reach
the editor for free. And **`vox-lsp` appears zero times in `.github/workflows/`**: no job builds,
runs, or smoke-tests the language server; its only tests are 15 in `lib.rs`.

Editor-embedded agents read diagnostics from the LSP, not from `vox check --json`. Revision 1
spent two weeks hardening the CLI path that already exists and zero days on the untested one, and
**no measurement exists of which harness this repo's agents actually consume** — so the
CLI-vs-LSP ordering has no basis. Add LSP CI coverage; measure harness usage before ranking
further.

Runner-up: the MCP tool surface (`vox-orchestrator-mcp`, dozens of `*_tools.rs`,
`merged_tool_registry`). Tool-surface size drives agent context cost and tool-selection error
rate, and nothing budgets it.

**Restored from the source audit, dropped by Revision 1:** probe-and-refine repository guidance
(`graphify query` as probe + `vox ci` as validate, made contract rather than hook nudge) and the
AGENTS.md-size A/B through `vox eval`. Agent navigation cost applies to every task in this repo,
not only Vox-language ones, and dropping it was a regression against the document being extended.

**Not addressed anywhere and ranked for a future pass:** test quality vs presence (968 weak-test
entries never reach a Definition of Done); flakiness (zero mentions, and §6.3 would memoize
flakes); multi-agent concurrent edits (AGENTS.md already lists parallel-agent fmt drift as
perennial, and this design adds auto-staged generated artifacts to that conflict surface); skill
freshness.

---

## §8 Ranking — rebuilt

Revision 1's ranking was derived from §1.1's fabricated failures and is discarded. Re-derived by
risk-adjusted value, with read-only and measurement work first:

| # | Item | § | Cost | Why here |
| --- | --- | --- | --- | --- |
| 1 | Commit `job-timings` output; re-baseline tiers | 6.0 | ~1 day | Every cost claim is unfalsifiable without it; the meter already runs |
| 2 | Populate `build-bench-baseline` | §1 row 10 | 1 commit | Already blocking and should be red now; unblocks build-time ranking |
| 3 | Demote `command_compliance` + `completion_quality` to `complete` | 6.2 | hours | −58s; largest single reduction; no new machinery |
| 4 | Fix `bench.rs` fixture resolution; zero-fixture ⇒ hard error | §1 row 3 | ~1 day | Restores real precision measurement for 33 rules at once |
| 5 | Restore the five CR-L harnesses from `40a798545`; add a stub-count guard | §1 row 1 | ~3 days | Restores all LLM-target measurement; the guard prevents silent re-amputation |
| 6 | `should_fail_build` unit test; then floor → `Error` **with `god_object` handled in the same commit**; pass `crates` explicitly at `ci.yml:751` | §1 rows 4–5 | ~1 week | Blast radius is 327–549, 98 % `god_object` — the threshold flip alone would jam the merge queue |
| 7 | Wire `semcov-gates` non-blocking; de-pin `line`; measure for one week | 3.3 | ~1 week | Prerequisite to any expiry work; prevents the line-drift outage |
| 8 | Fix the four live `.cursor/rules` contradictions by hand | 5.3 | hours | Two are policy inversions in `alwaysApply: true` files |
| 9 | `where-things-live` 5-rule link lint | 5.4 | ~40 lines | Two live bugs plus the unmeasured coverage gap |
| 10 | Registry-vs-registry default parity | 5.5 | ~30 lines | 2 real divergences, 96/121 coverage gap |
| 11 | Fix the `canonical-map` seed row; promote `aliases`; enforce expiry (incl. `check_links`) | 5.1, 3.3 | ~1 week | First real expiry enforcement in the repo |
| 12 | Cache `check_graph`; generalize the `visus_review` cache; `cacheable` opt-in + clock test | 6.3 | ~1 week | Only after §6.0 says what it is worth here |
| 13 | Arm + unify the budget gate | 6.1 | ~3 days | After 1, 3, and 12 make the tier passable |
| 14 | LSP CI coverage; measure harness usage | 7 | ~3 days | Least-defended agent surface |
| 15 | `sync-retired-surfaces` (two targets) | 5.2 | ~2 days | The one genuinely derivable fact |
| 16 | DAF injector over `examples/golden`, nightly; split parse vs typecheck | 4.3 | ~1 week | Measure before any ratchet |
| 17 | Wire one `vox-constrained-gen` call site | 4.2 | ~1 week | Prerequisite to any sampling A/B |
| 18 | Migrate the 5 secret reads; lint stray `_` arms | 4.7 | ~2 days | The whole realistic win |
| 19 | Diagnostic payload delta (`one_fix_site`, `confidence`, `caused_by`, version pin) | 4.4 | ~3 days | ~80 % already shipped |
| 20 | `Profile` axis on `feature_matrix`; fix the `@mcp.tool` message first | 4.1 | ~3 weeks | Large, and unranked until §1.1 is re-measured |
| 21 | 4.6a intra-module wiring lint, allowlisted, FP-measured | 4.6 | ~2 weeks | 4.6b needs `vox check --project` first |

Items 1–4 are days and carry near-zero risk. Item 6 is the one with a real blast radius and it is
deliberately behind the measurement that sizes it.

## §9 Falsifiers

- If restoring the CR-L harnesses reproduces the 2026-05-23 numbers, **there is no repair deficit**
  and items 16–21 need re-justification from scratch.
- If §6.0's job timings show `setup` and `docker-vox-image-smoke` dominate, §6.2's 58s is noise
  and the cost programme should move to build caching entirely.
- If item 6's blast radius exceeds ~550, the floor flip is a remediation programme needing its own
  plan.
- If the clock-perturbation test disqualifies most of the expensive gates, §6.3 does not fund
  §6.1 and the arming schedule must shrink.
- If DAF over `examples/golden` is already ≤1.3 for both parse and typecheck, item 16 stops there.

## §10 What Revision 1 got wrong

Kept rather than deleted, per repo convention — several were wrong instructively.

1. **"`enforce-warn` cannot fail; zero detectors emit `Critical`."** Severity is *data*, not a Rust
   literal (`rule_pack_detector.rs:92`, `rule_pack_bridge.rs:13`). One rule reaches Critical and
   has fired (`baseline-freeze.json: "critical": 1`). The grep published as evidence will keep
   producing the wrong answer. Also over-scoped: `Legacy` mode fails on `>= Error` and the
   pre-commit hook runs `enforce-strict`, so only the scoped CI step was neutered.
2. **The LLM-target profile** — §1.1. Misread `InfrastructureError` as failure.
3. **"1,642 frozen suppressions"** — dead data; the gate is unwired.
4. **"Mirror the working expiry in `check_links.rs`"** — it warns and returns `true`.
5. **"Stagger by churn"** — 0 % of the population is cold; churn has no discriminating power.
6. **"28 phantom crates"** — 0 stale; the convention was misread as the defect.
7. **The fact registry** — duplicates `canonical-map.v1.yaml`.
8. **"No gate caching anywhere"** — two implementations exist.
9. **"`ssot-drift` is 48s"** — 125s warm / 196s cold; the source was a comment.
10. **"91 gates, 25 workflows"** — ~154 and 45.
11. **Token-economy gate** — exists, armed at 0 %; the proposal would have loosened it 10×.
12. **Typed diagnostics** — ~80 % shipped.
13. **`#[non_exhaustive]`** — grants downstream a wildcard arm; does the opposite of the claim.
14. **"Vox knows its mount points at compile time"** — compilation is per-file.
15. **"repair-corpus encodes fault injection"** — hand-authored pairs, a third of them
    zero-diagnostic logic bugs.
16. **"495 HumanEval / 197 repair-corpus files"** — `find | wc -l`. Manifests say **164** and
    **15**.
17. **Tier-budget proposal** — would have been a third implementation contradicting two.
18. **"Dominant failure mode"** — 11/11 sampled gates armed; downgraded to "recurring class".
19. **`contracts/**` as "the single largest source of avoidable CI cost"** — 11–12 % of commits.
20. **Canary cost ~2s** — realistically +30–90s; 3 of 5 infeasible, 2 duplicated existing tests.

One live doc/contract divergence found while auditing: `local-ci-pre-push.md:22` says the fast
tier is "≤60s"; the budgets file measures 90s and fails at 135s; reality is 175–200s. Three
numbers, three places.

## §11 Sources

- [Thinking Before Constraining](https://arxiv.org/html/2601.07525v2) · [CRANE](https://arxiv.org/pdf/2502.09061) · [The Hidden Cost of Structured Generation](https://arxiv.org/pdf/2603.03305) — the reasoning cost of hard constraints. **Reported for other grammars; unmeasured for `GrammarMode::Vox`, which has no call sites.**
- [Vercel's Zero](https://hackernoon.com/vercels-zero-wants-compilers-to-talk-to-ai-agents) — typed diagnostic payloads; agents collapse past 3–4 noisy loop rounds.
- [Not the Silver Bullet](https://dl.acm.org/doi/fullHtml/10.1145/3689535.3689554) — prose error messages are ineffective; structured fixes are the remedy.
- [CODESTRUCT](https://arxiv.org/pdf/2604.05407) — AST operations cut errors 76–88 % for capable models.
- [Ideas for an Agent-Oriented Programming Language](https://davi.sh/blog/2026/02/markov-ideas/) — token-optimized grammar; errors as diffs.
- [TypePilot](https://arxiv.org/html/2510.11151v1) — type-guided pipelines as mitigation.
- [Probe-and-Refine Repository Guidance](https://arxiv.org/pdf/2606.20512) · [Harness Engineering for Agentic AI Coding Tools](https://arxiv.org/pdf/2602.14690) — probe-then-validate over static context files.
- [Incremental SCA for monorepos](https://www.arnica.io/blog/incremental-sca-strategies-monorepos) · [Monorepo build tools 2026](https://sourcegraph.com/blog/monorepo-build-tools) — **60–80 % is reported elsewhere; this repo's gate mix is cache-hostile and the figure should not be spent before §6.0 measures it.**
- [Refactoring Runaway](https://arxiv.org/pdf/2605.22526) · [SWE Atlas](https://arxiv.org/pdf/2605.08366) — performance decay with refactor size.
