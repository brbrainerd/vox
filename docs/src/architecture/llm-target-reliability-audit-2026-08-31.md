---
title: "LLM-Target Reliability Audit — Enforcement Reachability, Ratchet Permanence, and Instruction Drift"
description: "Revision 2, corrected after a seven-track adversarial re-audit. Measured account of how Vox's guardrail stack actually behaves against the recurring LLM-authoring failure classes: a severity floor two notches too high, a full-scan branch covering 22 of 3,889 files, 33 of 45 detector rules scoring a vacuous F1, baselines guarding a gate that never runs, and three retracted findings from revision 1."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# LLM-Target Reliability Audit (2026-08-31)

**Method:** read-only measurement of the enforcement chain (detector registry → engine exit
semantics → CI wiring → baselines), plus paced web research on 2026 agent-failure literature.
Every claim below carries a file:line or a command you can re-run.

**Acted on by:** [`2026-08-31-vox-llm-target-hardening-design.md`](../../superpowers/specs/2026-08-31-vox-llm-target-hardening-design.md)
(design) and [`2026-08-31-vox-llm-target-hardening.md`](../../superpowers/plans/2026-08-31-vox-llm-target-hardening.md)
(implementation plan) — which extend these findings to the language, compiler, and CI cost plane.

**Companions — read these first; this doc does not restate them:**
[`codegen-ssot-and-split-brain-audit-2026.md`](codegen-ssot-and-split-brain-audit-2026.md) ·
[`discoverability-audit-2026.md`](discoverability-audit-2026.md) ·
[`detector-coverage-ledger.md`](../contributors/detector-coverage-ledger.md) ·
[`ai-ui-generators-and-vox-as-target-research-2026-06-18.md`](ai-ui-generators-and-vox-as-target-research-2026-06-18.md) (VUV-as-target).

## Executive summary

> **Revision 2 (2026-08-31).** This document was audited along seven independent tracks against
> the source tree after revision 1 was published. Four of its findings were corrected, one was
> retracted outright, and three larger findings were added. Corrections are marked inline rather
> than deleted.

Vox's *detection* surface is genuinely strong — 54 registered detectors, an F1-scored rule SSOT,
an arch-check dep-graph gate, ratcheted config hygiene, and a `docs-reality-audit`. That is well
ahead of typical practice.

The weakness is a recurring class of mechanisms that are **built, committed, documented, and
never wired to anything that can fail**. Not the dominant failure mode — an unbiased sample of 11
`vox ci` gate implementations found 11/11 armed, 9/11 with unit tests, and only 14 of 145 `ci.yml`
steps `continue-on-error`, each justified inline. But it produced every significant finding here:

1. **The scoped TOESTUB CI gate has a severity floor two notches too high** — one rule in 45 can
   trip it — and its `FULL=true` branch scans **22 of 3,889 files** (F1).
2. **33 of 45 detector rules score a vacuous F1 = 1.0** because zero-fixture rules are scored as
   perfect and the fixture path convention does not match the files on disk (F1c).
3. **1,642 baseline entries suppress a gate that is never invoked**, and the field designed to
   expire them is `#[allow(dead_code)]` (F2).
4. **10 of 17 retired surfaces are missing from the always-applied Cursor rule**, and a third
   copy of the same fact exists in a contracts YAML with a different set again (F3).

The instructive case is not in this document's original scope: the five CR-L LLM measurement
harnesses were **deleted by a bad merge on 2026-05-27** and replaced with stubs that return
`InfrastructureError` unconditionally, with a committed test now asserting that state. Nothing
noticed for 96 days, because the stub's exit code is contractually non-blocking.

---

## F1 — The scoped TOESTUB severity floor is two notches too high

> **Corrected 2026-08-31 (revision 2).** Revision 1 claimed this gate "cannot fail" on the
> evidence `grep -r 'Severity::Critical' detectors/ → 0 hits`. **That evidence is wrong and must
> not be re-run.** Severity is *data*, not a Rust literal.

**Evidence:**

| Link in the chain | Location | Says |
| --- | --- | --- |
| CI step comment | `.github/workflows/ci.yml:741` | "enforce-warn mode — all 23 detectors, **Warnings+ block**" (stale: the registry holds 54) |
| CI invocation | `.github/workflows/ci.yml:752,755,761,771` | `vox ci toestub-scoped --mode enforce-warn` |
| Runner | `crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs` | delegates exit status to the `toestub` subprocess |
| Exit rule | `crates/vox-code-audit/src/engine.rs:450-453` | `EnforceWarn => findings.any(severity >= Severity::Critical)` |
| Severity source | `rule_pack_detector.rs:92`, `rule_pack_bridge.rs:13` | severity comes from the embedded YAML, not from Rust literals |
| Reachability | `contracts/code-audit/rules.v1.yaml:464` | `security/hardcoded-secret/aws-key` is `severity: critical` — and has fired (`baseline-freeze.json: "critical": 1`) |

So the gate **can** fail — on exactly one condition, an `AKIA[0-9A-Z]{16}` literal in a scanned
file. That is one rule out of 45, not zero, and the correct framing is a severity floor set two
notches too high rather than a dead code path.

**Scope, also corrected.** Only the *scoped TOESTUB CI step* is neutered. `Legacy` mode fails on
`>= Error` (`engine.rs:446-460`) and the pre-commit `tdd-guard` runs `enforce-strict`, so the same
detectors do block elsewhere. Revision 1's "every normative policy in AGENTS.md inherits its teeth
from this comparison operator" was over-scoped.

**F1b — the `FULL=true` branch scans 22 files.** `ci.yml:751-752` invokes `toestub-scoped` with no
roots, and `matrix.rs:556-558` defaults an empty root list to `crates/vox-repository` — **22 of
3,889 `.rs` files**. The full-scan branch is narrower than the affected-crates branch. Fixing the
severity floor without this leaves the gate ~99 % blind on full runs. This is the larger of the
two bugs and Revision 1 missed it entirely.

**F1c — the precision evidence is itself hollow.** `bench.rs:95-103` scores a rule with zero
fixtures at precision 1.0 / recall 1.0 / **F1 1.0**, which passes `--min-f1 0.70` (`ci.yml:1320`).
`bench.rs:51-56` splits the rule id at the first `/`, seeking
`fixtures/security/hardcoded-secret-aws-key_pos` while the file on disk is
`fixtures/security-hardcoded-secret/aws_key_pos.txt`; the per-rule `fixtures:` declarations in
`rules.v1.yaml` are never read. **33 of 45 rules score vacuously**, including every `stub/*`,
`scaling/*`, `ai-laziness/*`, and `security/hardcoded-secret/*`.

**Fix:** flip `EnforceWarn` to `>= Severity::Error` **and** handle `arch/god_object` in the same
commit — the measured blast radius is 327 (fresh proxy) to 549 (committed `scaling-audit`
report), of which ~98 % is `god_object`. Pass `crates` explicitly at `ci.yml:751`. Write the
missing `should_fail_build` unit test first: `grep should_fail_build` returns three call sites and
zero tests, which is why this survived.

## F2 — The baselines are dead data, and expiry has no working reference

> **Corrected (revision 2).** Revision 1 framed this as ratchet *permanence*. The counts are
> exact, but they suppress nothing: the gate that reads them has never run. This is the same
> actuation class as F1, not a separate one.

| Baseline | Entries | Read by a running gate? |
| --- | --- | --- |
| `contracts/toestub/weak-test-baseline.v1.json` | 968 | **No** — sole reader is `bin/semcov-gates.rs:28-29` |
| `contracts/toestub/silent-drop-baseline.v1.json` | 674 | **No** — same |
| `contracts/config/config-hygiene-baseline.txt` | 379 | yes |
| `contracts/config/config-registry-baseline.txt` | 348 | yes (name-set only) |
| `contracts/ci/crate-edges.allow.v1.json` | 532 edges + 34 exceptions | yes, and `--tighten` works ✅ |
| `contracts/reports/toestub-remediation/baseline-freeze.json` | 6,091 frozen `2026-03-25` | index-referenced only |
| `contracts/hir/legacy-baseline.toml` | — | **No consumers at all** — delete it |

**`semcov-gates` is never invoked.** Not in any workflow, not in `lefthook.yml`, not in
`core_gates.rs:177-190`'s `run_core_all()` trio. `ci.yml:1243` carries a comment asserting it "IS
blocking"; that comment is false.

`expires_after` is parsed and inert (`suppression.rs:30-32`, `#[allow(dead_code)]`), appears in
exactly one non-test location workspace-wide, is optional and unvalidated in the schema, and
neither baseline is covered by `validate_toestub_suppression_contracts` (`suppression.rs:88-112`).

> **Correction.** Revision 1 said to "mirror the working implementation in
> `check_links.rs:79-87`". **There is no working implementation.** Read past line 87: on expiry it
> prints `WARN … allowlist entry expired (still skipping)` and returns `true`. Enforcing expiry
> means writing the first one in this repo.

`suppressions.v1.json` — the disciplined ledger with `owner` and `reason`, 7 entries — is also
never loaded: `ToestubConfig::suppression_path` defaults to `None` (`engine.rs:82`) and
`lefthook.yml:48` passes no `--suppressions`.

**Why it matters beyond debt:** per AGENTS.md, this corpus trains MENS. A baselined violation is a
code sample the model sees as accepted.

**Fixes, in order:**

1. **Wire `semcov-gates`** — declare it as a `[[bin]]`, run non-blocking for one week, publish the
   real beyond-baseline count. Every number above is unmeasured until this runs.
2. **De-pin `line` first.** `suppression.rs:170-176` matches `path_glob` **and exact line**. All
   1,642 are line-pinned and **0 % sit in cold code** (median 3–5 commits since the freeze,
   hottest file 90). Line numbers have drifted; arming the gate as-is fails on an unknown large
   fraction, indistinguishable from real regressions. Replace with a per-file monotone count.
3. Expire **per file, densest-first** — not by churn, which has no discriminating power on a
   uniformly hot population. One file (`vox-compiler/src/eval/builtins.rs`) holds 45 entries.
4. Enforce `expires_after`, and fix `check_links` in the same change.

> **Correction.** Revision 1 proposed running `--tighten` inside `ssot-autoregen`. Its 11
> regenerators (`ci.yml:247-258`) touch no suppression baseline, so the hazard raised there does
> not exist — but a real one does: `cr-l8-corpus-feedback.yml:401-405` `cp`s the current pass-set
> over `scripts-pass-baseline.txt` with no monotonicity check, inert only because no commit step
> follows it.

## F3 — Instruction-layer split-brain (measured, live)

Four always-loaded instruction surfaces, none generated from a common source:

| Surface | Bytes | Generated? |
| --- | --- | --- |
| `AGENTS.md` | 48,037 (533 lines) | no |
| `CLAUDE.md` | 952 | no |
| `GEMINI.md` | 5,188 | no |
| `.cursor/rules/*.mdc` (9 files, `alwaysApply: true`) | 14,327 | only `cli-command-registry.mdc` |

`AGENTS.md` §Retired Surfaces lists 17 retired symbols. `.cursor/rules/retired-surfaces.mdc` is
hand-maintained and omits **9** of them:

```text
@endpoint(kind:…) fn · @py.import · @native · @capacitor/* · axum::serve
vox-sherpa-transcribe · crates/vox-dashboard · crates/vox-oratio · vox-dei-shim · vox-bootstrap
```

> **Corrected (revision 2).** Revision 1 said 9; the re-derived count is **10**. And there is a
> **third** copy of this fact, not two: `contracts/documentation/retired-symbols.v1.yaml` holds 15
> entries — a *different* set again, with `vox-ml-cli-standalone` that AGENTS.md lacks and missing
> ten that AGENTS.md has. Any generator must target both consumers or it hard-codes a two-thirds
> fix.

**Four live contradictions in the non-derivable rule files**, found while checking derivability —
these are worse than the missing rows, because they are in `alwaysApply: true` files and they
*invert* policy rather than omit it:

- `documentation-policy.mdc` tells agents to use `{{#include}}`; AGENTS.md §Markdown Hygiene
  forbids it.
- `documentation-policy.mdc` requires `last_updated` frontmatter; AGENTS.md forbids hand-adding it.
- `voxscript-first-automation.mdc` cites `vox-as-glue-research-2026.md`, which moved to
  `docs/src/archive/`.
- Same file says `vox-runtime` where AGENTS.md says `vox-actor-runtime`.

A Cursor session gets roughly half the retirement guard. Re-run:

```bash
awk '/^\| Retired \/ Deprecated/,/^$/' AGENTS.md | grep '^|' | tail -n +3 | cut -d'|' -f2 > /tmp/rows.txt
```

then check each row's first backticked token against `.cursor/rules/retired-surfaces.mdc`.

> **Corrected (revision 2).** Revision 1's fix — "generate every per-tool rule file from
> `AGENTS.md`, modelled on `vox ci sync-ignore-files`" — is wrong twice. Only **2 of 10** files are
> derivable (`retired-surfaces.mdc`, `secrets-policy.mdc`); 5 are fully independent
> (`build-environment.mdc` is CUDA paths and linker lore with no AGENTS.md source), and
> `GEMINI.md` already links to AGENTS.md as normative rather than copying — the correct pattern,
> already in place. And no mechanism transfers: `sync_ignore_files.rs:23-45` strips a fixed header
> and copies every remaining line verbatim.

**Fix:** one generator for the one derivable fact (`vox ci sync-retired-surfaces`, two targets); a
contradiction lint for the other eight (every referenced path resolves, every named crate exists);
and register the independent files in `contracts/documentation/canonical-map.v1.yaml` against
their real producers. The two policy inversions need a human edit, not a generator.

Secondary: `AGENTS.md` opens with *"Keep it short, stable"* and is 48 KB (~12k tokens loaded every
session). The 2026 literature is split on repo context files — curated ones cut agent runtime
~28.6 %, while bloated or LLM-generated ones *reduce* resolve rates — so size here is a measurable
variable, not a style question. Worth an A/B through the existing `vox eval` harness before
trimming on instinct.

## F4 — RETRACTED: "28 phantom crate references"

> **Retracted in full (revision 2).** Zero of the 28 are stale. The regex counted a documented
> convention as the defect.

`docs/src/architecture/where-things-live.md` backtick-references 28 `vox-*` names with no
directory under `crates/`. Re-derived and classified:

| Class | Count |
| --- | --- |
| Inside a section titled `## Planned but not yet landed`, under a column header `Planned crate` | 19 |
| Inline-tagged `_(planned)_` | 3 |
| Historical mentions inside `(was vox-primitives)` / "merged from retired X" consolidation notes | 6 |
| **Genuinely stale** | **0** |

Every markdown *link* of the form `(../../../crates/X/)` resolves — 0 broken out of the full set.
The document already encodes the distinction by convention — **linked = real, bare backtick =
planned or historical** — and is 100 % consistent today. A generator would additionally *delete*
the six "was X" notes, which are exactly what stops an agent re-inventing a retired crate.

**The two real defects run the opposite way, and no phantom-crate lint would catch either:**

- `where-things-live.md:392` lists **`vox-cli-ci`** under `## Planned but not yet landed` while
  the crate ships 80+ modules.
- `where-things-live.md:421` lists **`vox-dashboard`** as planned while AGENTS.md §Retired
  Surfaces records it deleted 2026-05-12 (`af5f26278`, ADR-037). The navigation table and the
  retirement table disagree, and `retired-symbol-check` has no pattern for it.

**Fix — a 5-rule link lint (~40 lines), not a generator:** (1) every `crates/X/` link resolves
*(passes today — a free canary)*; (2) every bare-backticked `vox-*` is in the planned section,
inline-tagged, or in a `(was …)` clause *(passes today)*; (3) no crate under `## Planned` has a
directory *(fails today)*; (4) no AGENTS.md retired name appears as a `Planned crate` *(fails
today)*; (5) every directory in `crates/` appears somewhere in the file — the real coverage gap,
currently unmeasured.

## F5 — Confirmed non-issues (checked, clean)

Recorded so the next audit does not re-litigate them:

- **GUI config defaults are *not* split-brain.** `crates/vox-gui/src/config/generated_fields.rs`
  is `@generated by vox ci config-gui-codegen --fields from CONFIG_KEYS`. Its 82 default values are
  derived, not copied. Good pattern — this is the shape F3 should adopt.
- **Unregistered env vars are gated, not unguarded.** 743 distinct `VOX_*` names appear in
  `crates/*/src` against ~475 in the config/secrets SSOTs, but `config_hygiene.rs` already carries
  an `env-var-not-in-registry` check with fine-grained per-var baseline keys
  (`baseline_key`, `config_hygiene.rs:24-30`), so *new* unregistered vars are caught. The gap is F2
  (the 290 baselined ones never expire), not missing detection.
- **`VOX_SECRETS_BACKEND` duplicated across four crates** is test env save/restore scaffolding, not
  divergent production defaults.
- **`ssot-autoregen` does not re-widen any suppression baseline.** Its 11 regenerators
  (`ci.yml:247-258`) are doc/catalog/surface generators plus `affected-crates --regen`.
- **`evidence_ledger.rs` parses the artifact *filename* date, not mtime** (`parse_artifact_date`,
  line 170). Correct and non-obvious — git does not preserve mtimes. Do not "fix" it.
- **11 of 11 sampled `vox ci` gate implementations are armed**, 9 of 11 with unit tests, several
  of them positive-detection tests. The unarmed cases in this document are a recurring class, not
  the norm.

## F6 — Added in revision 2: three findings larger than the originals

- **The five CR-L LLM harnesses were deleted by merge `3c7b3b917` (2026-05-27)** — 3,858 lines
  across six files, present in both parents, absent in the result. `lib.rs:265-273` now registers
  stubs returning `InfrastructureError` unconditionally (`stubs.rs:30-40`), `panel.rs` (1,028
  lines) is unreferenced, and a committed test asserts the stub behaviour. Unnoticed for 96 days
  because exit 2 is contractually non-blocking. **The last real run (2026-05-23) had
  repair-corpus 0.775/0.70 and plan-fidelity 0.910/0.85, both met** — so nothing currently
  supports a claim that Vox is a poor repair target.
- **A purpose-built CI cost meter runs and is discarded.** `vox ci job-timings`
  (`job_timings.rs:1-22`) runs after every CI run via `ci-timings.yml:36 --annotate`; its output
  is ephemeral check annotations with no artifact and no time series.
- **Budget enforcement is opt-in and off** (`pre_push.rs:264`), and the fast tier is **175–200s
  measured against a 135s fail wall**. The `48s`/`90s` figures in
  `contracts/budgets/test-tier-budgets.v1.yaml` are hand-typed comments with no artifact behind
  them; measured `ssot-drift` alone is 125s warm / 196s cold.

## Open gap: shadowing has no detector

`grep -ri shadow crates/vox-code-audit/src` returns nothing — the class is unmodelled. Three
variants, ranked by expected value in this codebase:

1. **Default-value shadowing** (highest) — but **not** via read sites. Of 321 non-test env read
   sites, **0** carry a same-line `unwrap_or`; the idiom hoists defaults into named resolvers, and
   `VOX_SEARCH_BM25_K1` has no `env::var` site at all. The shadowing that exists is
   registry-vs-registry: `CONFIG_KEYS` (124), `OperatorEnvSpec` (119) and
   `contracts/config/registry.v1.yaml` (99) all record defaults, share only **3** keys, of which
   **2 diverge** — and `config_registry_parity.rs:9-18` compares name sets only and *unions* the
   sources, so nothing can ever flag. Extend it to (name, default) parity: ~30 lines against code
   that already loads all three.
2. **Instruction shadowing** — F3 above. Same class, documentation layer.
3. **Rust `let` rebinding** (lowest) — the 2026 literature does not find this a leading LLM failure
   mode in Rust; attention goes to type confusion, async/lock misuse, and crypto misuse. Clippy's
   `shadow_unrelated` already exists. **Recommend against building this** — the detector budget is
   better spent on (1).

---

## Researched improvement vectors

Grounded in the 2026 literature (sources below), mapped onto what Vox already has.

### Adopt (high confidence, low cost)

- **Persist the measurements that already run** — `job-timings`, the per-generator `ssot-drift`
  timings shipped 2026-06-26, the pre-push report whose schema is committed with no instance.
  Cheapest item here and a prerequisite for every cost claim.
- **Make the existing signals blocking (F1)** — floor, scan root, and fixture resolution together.
  Not one of them alone.
- **Wire `semcov-gates` before touching its baselines (F2).**
- **Fix the four policy inversions in `.cursor/rules` by hand (F3)** — a generator cannot fix a
  semantic inversion, and only 2 of 10 files are derivable anyway.
- **Link-lint the navigation map (F4)** — not a generator, which would delete the useful
  historical notes.

### Evaluate (promising, needs measurement first)

- **Probe-and-refine repository guidance.** 2026 work (arXiv 2606.20512) tunes repo guidance by
  *probing* the repository read-only at each stage and validating intermediate artifacts against
  the environment, rather than trusting a static context file. Vox already owns both halves —
  `graphify query` (probe) and `vox ci *` (validate). The missing seam is making "resolve the
  target file via graphify before editing" a contract rather than a session-hook nudge.
- **AGENTS.md size A/B.** Contradictory findings in the literature make this an empirical question;
  `vox eval` can answer it for this repo specifically.
- **Semantic-equivalence split-brain** (already a known GAP in the ledger). The cheapest usable
  oracle is not a normalized AST — it is the graph already on disk: two functions whose call-target
  sets and return types match across crates, ranked by shared-callee overlap. Reuses
  `graphify-out/` instead of building new analysis.

### Reject (considered, not worth it here)

- **A Rust shadowing detector** — see above; Clippy covers it and the class is not load-bearing.
- **LLM-judged detectors for the covered classes** — the F1-scored rule SSOT is the right shape
  and cheaper than a judge. *But note F1c: 33 of 45 rules currently score vacuously, so it is not
  yet the contract it appears to be.* Fix the bench before relying on it; reserve the judge for
  `vox audit effort`, where no static oracle exists.
- **More detectors generally.** The marginal detector is worth less than making the existing ones
  reachable and their precision real.
- **A `contracts/canaries/` tree for every blocking gate** — considered and rejected. 9 of 11
  sampled gates already carry negative unit tests (`crate_edges.rs:420/486/514`), which are
  compiled against the gate and cost nothing in the tier. Extending `cargo mutants` scope to
  `vox-code-audit/src/engine.rs` — one config line — would have caught F1. Reserve cross-process
  canaries for failure paths that live in shell/YAML exit-code wiring, which is a small subset.

### VUV-as-LLM-target

The VUV analysis is carried by
[`ai-ui-generators-and-vox-as-target-research-2026-06-18.md`](ai-ui-generators-and-vox-as-target-research-2026-06-18.md);
this audit adds one cross-cutting point. The clearest finding in the repair literature is that
**success collapses when errors are dense and clustered, and stays high when a single error sits in
otherwise well-formed code**. That argues the highest-value VUV investment is not more surface area
but **error isolation**: parser recovery that keeps one bad view-call local, and diagnostics that
name exactly one fix site. The same logic applies to Vox proper — the grammar-unification rule
(bare keywords declare scope, decorators modify) is already the right shape, because it makes a
whole category of confusion a single-token error rather than a structural one.

---

## Recommended order

> **Rebuilt in revision 2.** The original ordering assumed working expiry in `check_links`, an
> `ssot-autoregen` tightening path, a generator for the rule files, and 28 stale crate names —
> none of which hold. Read-only measurement now comes first.

| # | Change | Cost | Why here |
| --- | --- | --- | --- |
| 1 | Commit `job-timings` output; re-baseline the tiers from measurement | ~1 day | Every cost claim is unfalsifiable without it; the meter already runs |
| 2 | Fix `bench.rs` fixture resolution; zero-fixture ⇒ hard error | ~1 day | Restores real precision measurement for 33 rules |
| 3 | Restore the five CR-L harnesses; add a stub-count guard | ~3 days | Restores all LLM-target measurement |
| 4 | `should_fail_build` unit test → floor `>= Error` **with `god_object` handled in the same commit** → pass `crates` at `ci.yml:751` | ~1 week | Blast radius is 327–549; the flip alone would jam the merge queue |
| 5 | Wire `semcov-gates` non-blocking; de-pin `line`; measure a week | ~1 week | Prerequisite to any expiry work; prevents a line-drift outage |
| 6 | Fix the four live `.cursor/rules` contradictions by hand | hours | Two are policy inversions in `alwaysApply: true` files |
| 7 | `where-things-live` 5-rule link lint | ~40 lines | Two live bugs plus the unmeasured coverage gap |
| 8 | Registry-vs-registry default parity | ~30 lines | 2 real divergences, 96/121 coverage gap |
| 9 | Enforce `expires_after` (and fix `check_links` in the same change) | ~1 week | First real expiry enforcement in the repo |

Full design and sequencing:
[`2026-08-31-vox-llm-target-hardening-design.md`](../../superpowers/specs/2026-08-31-vox-llm-target-hardening-design.md).

## Sources

- [Why AI Agents Fail: A Taxonomy of Failure Modes in Autonomous LLM-Based Systems](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=6572478)
- [What Breaks When LLMs Code? Characterizing Operational Safety Failures of Agentic Code Assistants](https://arxiv.org/html/2605.30777v1)
- [How Coding Agents Fail Their Users: A Large-Scale Analysis of 20,574 Real-World Sessions](https://arxiv.org/pdf/2605.29442)
- [9 Critical Failure Patterns of Coding Agents — DAPLab, Columbia](https://daplab.cs.columbia.edu/general/2026/01/08/9-critical-failure-patterns-of-coding-agents.html)
- [Probe-and-Refine Tuning of Repository Guidance for Coding Agents](https://arxiv.org/pdf/2606.20512)
- [Spec Kit Agents: Context-Grounded Agentic Workflows](https://arxiv.org/html/2604.05278v1)
- [The Productivity-Reliability Paradox: Specification-Driven Governance for AI-Augmented Software Development](https://arxiv.org/pdf/2605.01160)
- [Harness Engineering for Agentic AI Coding Tools: An Exploratory Study](https://arxiv.org/pdf/2602.14690)
- [Coding Agents Need Codebase Maps, Not Bigger Prompts](https://www.developersdigest.tech/blog/codebase-knowledge-graphs-ai-coding-agents)
- [An Empirical Security Evaluation of LLM-Generated Cryptographic Rust Code](https://arxiv.org/html/2604.27001)
- [The New Compiler Stack: A Survey on the Synergy of LLMs and Compilers](https://arxiv.org/html/2601.02045v1)
