---
title: "Crate Restructuring Proposal (2026-07)"
description: "Ranked, evidence-backed dependency cuts and crate splits from the crate-graph build-time program; hygiene findings first. Every candidate that survived manual verification turned out to be a false positive — read the reliability findings before acting on anything here."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-07-02"
training_eligible: true
training_rationale: "Documents a real methodology finding (symbol-graph blind spots to use-import and macro/derive usage causing false-positive unused-dependency signals) useful for future crate-dependency analysis, independent of whether any specific restructuring recommendation is later acted on."
authored: "2026-07-02"
---

# Crate Restructuring Proposal — 2026-07

Produced by the crate-graph build-time program (spec:
`docs/superpowers/specs/2026-07-02-crate-graph-buildtime-program-design.md`, plan:
`docs/superpowers/plans/2026-07-02-crate-graph-buildtime-program.md`). Evidence pack:
`graphify-out/evidence_index.v1.json` and the artifacts it indexes.

## Headline finding: the tool's own candidate list is not trustworthy at face value

Before any recommendation: **every one of the 9 zero-weight ("candidate-unused")
dependency edges sampled this session turned out to be a false positive** when
manually verified.

- 8 candidate edges into `vox-db` (`vox-cli-core`, `vox-cli-research`,
  `vox-cli-review`, `vox-corpus`, `vox-gamify`, `vox-openclaw-runtime`,
  `vox-orchestrator-queue`, `vox-package`) all had genuine `use vox_db::...`
  usage that the symbol graph's `calls`/`references` extraction does not track
  (bare imports without a subsequent call in the same extracted scope). One
  (`vox-openclaw-runtime`) was confirmed by an actual `cargo check` removal
  attempt: `E0432 unresolved import`.
- The single largest blast_s "win" found (`vox-codegen -> vox-workflow-runtime`,
  2604.9s) turned out to be a **deliberate** `[dev-dependencies]` compile-time
  guard (`crates/vox-codegen/Cargo.toml:44-52`) — codegen emits
  `vox_workflow_runtime::...` symbol paths as *string literals* into generated
  code, and this dependency exists solely so a test
  (`tests/durability_compiles.rs`) breaks the build if those symbols are ever
  renamed or removed upstream. Removing it would delete a real safety net, not
  dead weight.

**Implication for this document**: nothing below is a "do this" instruction.
Every ranked item is a *lead worth individually verifying by removal*
(`cargo check -p <consumer> --all-targets`, revert regardless of outcome, then
decide), never an accept-on-the-tool's-word recommendation. The tool's real,
durable value demonstrated this session is the **what-if blast_s math**
(pure dependency-graph arithmetic, not symbol-graph-dependent) and the
**rebuild-cause diagnostic** — both are load-bearing findings below; the
symbol-weighted candidate list is a ranking aid only.

## 1. Rebuild hygiene findings

A live `vox graphify why-rebuilt --capture` was run this session
(`graphify-out/rebuild_causes.json`, raw log `graphify-out/rebuild_fingerprint.log`).
Result: 20 crates recompiled between two consecutive `cargo check --workspace`
runs, classified `dep_rebuilt` (18) and `build_script_rerun` (2, both from
`vox-arch-check`).

**This run is confounded and should not be read as a hygiene verdict.** The
capture happened *during* this same session's active development — the
instrumented run landed while commits were still being made, and
`vox-arch-check`'s build script watches `.git/HEAD` via `rerun-if-changed`
(confirmed in the raw log: `FsStatusOutdated(StaleItem(MissingFile {
path: "...vox-arch-check\\.git/HEAD" }))`). Each commit during the capture
window legitimately changed `.git/HEAD`, which correctly triggered
`vox-arch-check`'s rebuild and cascaded downstream via `dep_rebuilt`. This is
the tool working correctly, not evidence of a hygiene bug.

**Action**: re-run `vox graphify why-rebuilt --capture` on a quiescent
checkout (no commits between the warm-up and instrumented runs) before
drawing any hygiene conclusion. No hygiene fix is proposed here because none
was actually demonstrated — only a valid one was ruled out.

**Tooling finding (already fixed, not a repo hygiene issue)**: the diagnostic
itself had two real bugs, found and fixed via this exact dogfooding run
(commits `1a203043d5`, `b18c6aba55`): cargo 1.96's `FsStatusOutdated(
StaleDepFingerprint {...})` phrasing wasn't recognized as `dep_rebuilt`, and
the bail gate used a raw per-line unknown rate (structurally always ~50%,
since every dirty target emits one reason-less header line) instead of the
correct per-crate rate. Both are fixed and covered by regression tests.

## 2. Ranked structural signal — blast_s (dependency-graph math, NOT symbol-graph-dependent)

This ranking comes from `vox-graph-reader::crate_model::crate_metrics` +
`what_if::top_cuts` — pure adjacency-graph arithmetic over
`contracts/ci/crate-graph.v1.json`, independent of the symbol-graph blind spot
above. It answers "if this edge were cut, how much less would rebuild" — it
does **not** claim the edge is safe or correct to cut; that requires the
per-edge verification the headline finding describes.

| Cut | blast_s saved | symbols_used (evidence) | Verified? |
|---|---:|---|---|
| `vox-codegen -> vox-workflow-runtime` | 2604.9s | 0 (candidate) | **VERIFIED FALSE POSITIVE — deliberate dev-dep compile guard, do not touch** |
| `vox-workflow-runtime -> vox-populi` | 1864.3s | 4 | not low-visibility; real usage evidenced — not a removal candidate |
| `vox-populi -> vox-tensor` | 778.1s | 1 | thin but nonzero — worth a manual look, not verified this session |
| `vox-tensor -> vox-actor-runtime` | 577.4s | 0 (candidate) | not verified — same blind-spot risk as the vox-db cases |
| `vox-actor-runtime -> vox-skills` | 417.9s | 0 (candidate) | not verified |
| `vox-config -> vox-config-derive` | 343.0s | 0 (candidate) | **suspicious on its face**: `-derive` crates are near-universally consumed via proc-macro attribute, which the symbol graph cannot see at all (macro expansion, not a `calls`/`references` edge) — expect another false positive; do not act without checking for `#[derive(VoxConfig)]` usage in vox-config |
| `vox-sql -> vox-db` | 331.2s | 0 (candidate) | not verified — vox-db pattern from Section headline suggests high false-positive risk |
| `vox-term -> vox-terminal-core` | 300.4s | 0 (candidate) | not verified |
| `vox-populi -> vox-corpus` | 277.0s | 0 (candidate) | not verified |
| `vox-workflow-runtime -> vox-journal` | 245.2s | 0 (candidate) | not verified |
| `vox-populi -> vox-identity` | 244.6s | 0 (candidate) | not verified |
| `vox-populi -> vox-constrained-gen` | 243.8s | 0 (candidate) | not verified |
| `vox-bounded-fs -> vox-scaling-policy` | 235.8s | 0 (candidate) | not verified |

Full 20-item ranking: `graphify-out/top_cuts.json`. Per-edge symbol evidence:
`graphify-out/edge_weights.json` (248 total candidate rows across the
workspace, ranked by `target_blast_s`).

**Risk classification** for any future verified item: `dep-removal` (delete
the Cargo.toml line) < `feature-gate` (make optional) < `crate-split` (extract
a types-only crate, following the existing precedent pattern of
`vox-db-types`/`vox-package-types`/`vox-container-types` alongside their
parent crates).

**Dependency-kind caveat** (verified 2026-07-02): `crate-graph.v1.json`
includes ALL dependency kinds — normal, build, and dev — indistinguishably
(`vox-cli-ci/src/affected_cmd.rs::graph_from_metadata` never inspects
`dep_kinds`). So every blast_s number above is **CI-shaped**
(`--all-targets` rebuild cost), not production-build cost. The
`vox-workflow-runtime` case above is the concrete illustration: its edge is
`[dev-dependencies]`-only, so its "2604.9s" is entirely CI/test cost, and (as
established) removing it is the wrong move regardless.

## 3. Timings and corpus confidence

- **Compile-time coverage: 76%** (95/125 crates have `compile_s > 0`), below
  the program's 90% target. The timings HTML consumed is from 2026-06-28 (4
  days before this analysis) and predates ~30 crates including the newly
  split `vox-cli-research`/`vox-cli-contracts`/`vox-cli-share`. A cold
  `VOX_AUDIT_BUILD=1 vox run --mode interp scripts/crate-build-audit.vox`
  rebuild (multi-hour cost) was not run this session. **Recommendation**: run
  it before treating any specific blast_s number as final; the relative
  ranking is still directionally useful today.
- **Symbol-corpus confidence: good.** `refs_not_in_dep_graph` ratio = 15.2%
  (3,758 declared / 675 undeclared cross-crate refs), under the program's 20%
  low-confidence threshold. A real corpus-hygiene bug was found and fixed
  during this program: stray gitignored `.worktrees/`/`.clone/` directories
  (leftover agent-session artifacts) were 68.6% of the extracted corpus
  before the fix (commit `5033099d30`); the corpus dropped from 103,265 to
  32,453 nodes with zero pollution after.

## 4. Reconciliation with the in-flight vox-cli extraction

The in-flight vox-cli monolith split
(`docs/superpowers/specs/2026-06-30-vox-cli-split-design.md`,
`2026-06-30-vox-cli-contracts-ci-extraction.md`) has landed `vox-cli-core` and
thickened `vox-cli-ci`; remaining planned work is `vox-cli-share` /
`vox-cli-review` / `vox-cli-extras` / `vox-cli-research` (Tier 1, since
landed per the crate-graph regen this session), then `vox-cli-model` /
`vox-cli-runtime` / `vox-cli-db` (Tier 2), then thickening `vox-cli-ci`
further and a `vox-cli-diagnostics` crate (Tier 3) — plus the `HeavyGuardHost`
callback-trait seam for the ~15 guards staying in `vox-cli` itself.

**No overlap found.** Neither document mentions `vox-db` as an extraction
target, nor any of `vox-gamify`, `vox-corpus`, `vox-orchestrator-queue`,
`vox-openclaw-runtime`, or `vox-package` (the crates surfaced by this
program's vox-db candidate sampling). `vox-codegen`, `vox-workflow-runtime`,
and `vox-populi` are likewise absent from the split plan. Every item in this
document is genuinely new territory relative to that effort — nothing here
duplicates or should be sequenced against the split's remaining tiers.

## 5. Stated limitations

- Split-savings modeling (`what_if::what_if_split`) does not attribute
  self-compile-time to the hypothetical new crate — reported savings for any
  future split analysis are an **upper bound** on the dependency-shape win,
  not the true post-split number.
- The symbol corpus cannot see macro/derive/trait-impl/re-export usage
  (concretely demonstrated above for `vox-config-derive`) — any
  `-derive`/`-macro`/`-codegen`-suffixed crate in the candidate list should be
  assumed false-positive until proven otherwise.
- Timings are single-machine, 4 days stale relative to this analysis, and
  cover 76% of crates.
- `refs_not_in_dep_graph` at 15.2% means roughly 1 in 6.6 cross-crate symbol
  references has no corresponding declared dependency edge — worth a
  follow-up look (could indicate transitive-dependency reliance that should
  become direct, a separate finding from unused-dependency candidates).
- No crate splits or dependency removals were executed as part of this
  program; the two removal attempts described above were reverted
  immediately after verification, per the program's design.

## 6. Machine-readable appendix

```json
{
  "schema_version": 1,
  "generated": "2026-07-02",
  "evidence_index": "graphify-out/evidence_index.v1.json",
  "hygiene_findings": [
    {
      "summary": "why-rebuilt --capture confounded by concurrent git commits during the session",
      "action": "re-run on a quiescent checkout before drawing a hygiene conclusion",
      "severity": "informational"
    }
  ],
  "structural_recommendations": [
    {
      "edge": "vox-codegen:vox-workflow-runtime",
      "blast_s_saved": 2604.9,
      "symbols_used": 0,
      "risk_class": "dep-removal",
      "verdict": "DO_NOT_REMOVE",
      "reason": "deliberate [dev-dependencies] compile-guard for tests/durability_compiles.rs"
    },
    {
      "edge": "vox-config:vox-config-derive",
      "blast_s_saved": 343.0,
      "symbols_used": 0,
      "risk_class": "dep-removal",
      "verdict": "LIKELY_FALSE_POSITIVE_UNVERIFIED",
      "reason": "proc-macro derive crates are invisible to the symbol graph's calls/references extraction"
    },
    {
      "edge": "vox-sql:vox-db",
      "blast_s_saved": 331.2,
      "symbols_used": 0,
      "risk_class": "dep-removal",
      "verdict": "UNVERIFIED_HIGH_FALSE_POSITIVE_RISK",
      "reason": "matches the pattern of 8/8 confirmed false positives in vox-db candidates this session"
    }
  ],
  "verified_false_positives": [
    {"edge": "vox-cli-core:vox-db", "reason": "use-import usage invisible to symbol graph"},
    {"edge": "vox-cli-research:vox-db", "reason": "use-import usage invisible to symbol graph"},
    {"edge": "vox-cli-review:vox-db", "reason": "use-import usage invisible to symbol graph"},
    {"edge": "vox-corpus:vox-db", "reason": "use-import usage invisible to symbol graph"},
    {"edge": "vox-gamify:vox-db", "reason": "use-import usage invisible to symbol graph (use vox_db::Codex)"},
    {"edge": "vox-openclaw-runtime:vox-db", "reason": "confirmed via cargo check: E0432 unresolved import on removal"},
    {"edge": "vox-orchestrator-queue:vox-db", "reason": "use-import usage invisible to symbol graph"},
    {"edge": "vox-package:vox-db", "reason": "use-import usage invisible to symbol graph"},
    {"edge": "vox-codegen:vox-workflow-runtime", "reason": "deliberate dev-dependency compile guard, not dead weight"}
  ],
  "reconciliation_with_vox_cli_split": "no overlap found",
  "limitations": [
    "split savings are an upper bound (self-time attribution unmodeled)",
    "symbol graph blind to macros/derives/trait-impls/re-exports",
    "timings 76% coverage, 4 days stale",
    "refs_not_in_dep_graph ratio 0.152 (under 0.20 threshold, but nonzero)"
  ]
}
```
