# Vox Syntax Optimization Program — Design Spec

**Date:** 2026-08-09
**Status:** approved-pending-review
**Predecessor:** `2026-08-08-vox-core-syntax-convergence-design.md` (Sequencing Steps 0-1 shipped on PR #469)
**Plan:** `docs/superpowers/plans/2026-08-09-vox-syntax-optimization-program.md`

## Problem

The core-syntax convergence program (PR #469) landed the tolerant-reader/strict-writer
foundation: unknown bytes always tokenize, `;` and `==`/`!=` parse with
warn-toward-canonical + machine-applicable `Replacement` fix-its, warnings surface
through `vox check`, and decorators compose with bare `query`/`mutation`/`server`
declarations. Four axes remain open, per operator direction:

1. **K-complexity of VoxScript itself is unmeasured.** We have compression-based
   estimators over compiler *outputs* (`crates/vox-codegen/src/syntax_k.rs`) and a
   lexer-token verbosity ratchet over goldens
   (`crates/vox-cli/src/commands/ci/run_body_helpers/syntax_k.rs` +
   `contracts/eval/source-token-budget.v1.json`), but nothing measures what our
   target training model actually pays to *emit* Vox source — model-BPE tokens
   under the MENS (Qwen3-family) tokenizer — and nothing trends that cost as the
   grammar changes. This is a proxy, not a universal "frontier model" claim:
   BPE vocabularies differ across tokenizers, and a spelling cheaper under
   Qwen3-BPE is not guaranteed cheaper under a different model's tokenizer. We
   measure against MENS specifically because that's the model this program's
   output actually trains; cross-tokenizer variance is a known, accepted
   limitation (see Risks), not solved by this program.
2. **Pythonic surface is unaudited.** Frontier models are saturated with Python.
   Where a Python spelling is free (token cost ≤ current spelling, no grammar
   collision), adopting it as canonical lowers emission cost and transition
   friction. No systematic pass has been done.
3. **CR-F3 coverage is 12 rows.** The ledger
   (`contracts/spec/language-surface-coverage.v1.yaml`) covers the 8 productions
   from Steps 0-1 plus 4 todo rows. The grammar has 41 `Decl` variants
   (`crates/vox-ast/src/decl/`); most have zero verified-coverage claims.
4. **TypeScript elimination has no evidence base.** The goal is VoxScript producing
   the GUI and harness. `vox-gui`'s UI is hand-written TSX that Vox cannot yet
   express; the concrete codegen blockers are known
   (dotted-member JSX unsupported — documented limitation in
   `crates/vox-codegen-ts/src/reactive/mod.rs:104-111`; mutations lower to a
   bespoke `voxRuntime` journal — `crates/vox-codegen-ts/src/hir_emit/mod.rs`)
   but no real surface has ever been ported to prove or extend the pipeline.

## Operator decisions (locked)

| Question | Decision |
|---|---|
| K-complexity metric | Model-BPE tokens-per-task against a paired corpus (same task in Vox/Python/TS), plus absolute Vox token counts trended over time |
| Enforcement point | `vox ci k-complexity` trend gate on language PRs (not TOESTUB per-script warnings, not advisory-only) |
| Pythonic disposition | **Adopt Python as canonical where free** (token cost ≤ current AND no grammar collision; migration churn displayed per-flip but not disqualifying) |
| TS-elimination scope | **Pilot surface**: port one small real Axis surface to `.vox` end-to-end, letting the port drive codegen fixes; full GUI rewrite stays a mapped future program |
| Sequencing | Approach A (measurement-first): harness → Pythonic flips (metric-justified) → pilot; ledger track fully parallel |
| Semicolons | Already retired as grammar (statements end at newline; `;` is `Token::Unknown(';')` + tolerated warn). The tolerance is kept permanently — zero K-cost, maximal LLM-emission forgiveness. No further action. |

## S1. Program shape

Four tracks plus one enabler phase, sequenced per Approach A:

```text
Phase 0 (enabler): corpus auto-migration tooling + golden canonicalization
   │
Track 1: K-complexity harness ──► Track 2: Pythonic audit + flips ──► Track 4: pilot Axis surface
Track 3: ledger → 100%  (parallel with everything; disjoint files)
```

Folded in from the predecessor program's backlog:
- Dotted-member JSX + idiomatic mutation lowering → Track 4 (driven by the pilot).
- `vox ci vox-parse-check` enforcement wiring (corpus is now clean) → Phase 0.
- tmLanguage-regen ledger row (`editor-tooling/vscode-tmlanguage-decorators`) → Track 3.
- `parse_fn_decl_inner` (~1350-line Rust function) refactor → **out of scope**: it is
  compiler-internal K-complexity, not VoxScript K-complexity, per operator
  correction. Recorded as optional hygiene in the plan appendix only.

## S2. Phase 0 — Corpus migration enabler

Language changes only stay cheap if migrating the corpus is mechanical. Two
capabilities, both building on infrastructure that already exists:

**`vox fix` (new subcommand).** Applies `Replacement` payloads
(`crates/vox-compiler/src/parser/error.rs` — `from`/`to`/`code`) from
Warning-severity parse diagnostics to source files, in-place, span-accurate.
Warnings already flow through `parse_and_warnings` /
`run_frontend_str_with_options` (PR #469), and every tolerated legacy spelling
already carries a `Replacement`. `vox fix --check` reports without writing.
This is the migration engine for every future canonical flip: flip lands →
`vox fix` sweeps the corpus in one command → diff is reviewable and mechanical.

**Golden canonicalization.** Goldens are training data (strict-writer surface),
so they must be zero-warning, not merely zero-error. Add `--deny-warnings` to
`vox ci vox-parse-check` (`crates/vox-cli-ci/src/parse_check.rs::run_vox`
currently fails only on Error severity); run `vox fix` over
`examples/golden/**/*.vox`; wire two enforced gates:
- `vox ci vox-parse-check "examples/golden/**/*.vox" --deny-warnings` (goldens: canonical or fail)
- `vox ci vox-parse-check "scripts/**/*.vox" "apps/**/*.vox"` (corpus: parseable or fail)

Both wire into the `full` pre-push tier and CI (not the fast tier — they walk
hundreds of files).

## S3. Track 1 — K-complexity harness

**Extends the existing syntax-K family; does not replace it.** Verified against
the real implementation (`crates/vox-cli/src/commands/ci/run_body_helpers/syntax_k.rs`):
`source-token-budget.v1.json` already stores `{ fixture_id: { tokens, bytes } }`
per golden ladder fixture, and `run_source_token_budget` already implements the
exact ratchet/tolerance/`--update` pattern the absolute series needs. Standing
up a second, parallel schema+report+CLI-surface to carry one more integer per
fixture would duplicate working infrastructure — so the **absolute series
extends this file and function** (adds a `bpe_tokens: Option<usize>` field,
one more ratchet check) rather than inventing a sibling system. Two lanes
remain genuinely distinct:

| Lane | Measures | Home |
|---|---|---|
| `syntax_k.rs` compression/NCD | Output (WebIR/TSX) complexity | existing, unchanged |
| `source-token-budget.v1.json` + `bpe_tokens` | Per-golden BPE emission cost, absolute + trended | **extended** existing file |
| **k-complexity ratios (this track, new)** | **Vox-vs-Python/TS emission-cost ratio over a paired corpus** | **new** — this is the only genuinely new report |

**Tokenizer.** Model-BPE counts via the `tokenizers` crate (already a workspace
dependency, v0.21). The tokenizer artifact (a `tokenizer.json`) is vendored and
pinned by SHA-256 so every count is reproducible forever; the MENS tokenizer
(Qwen3 family, per the MENS training stack) is the artifact, with its content
hash recorded alongside every measurement. Locating/vendoring the artifact is a
plan discovery task — if no tokenizer.json exists in-repo today, it is vendored
under `contracts/eval/tokenizer/` with provenance in a SOURCES note (upstream
repo, revision SHA, **and license** — the `assets/skills/SOURCES.toml`
precedent requires license capture, and this program follows that requirement,
not just the hash-pinning half of it).

**Two data series, two homes:**

1. **Absolute series (the moving data), extends `source-token-budget.v1.json`.**
   BPE token count per golden ladder fixture, added as a field alongside the
   existing `tokens`/`bytes` measurement, reusing its existing ratchet/tolerance/
   `--update` gate. This is the per-syntax-change trend line: when a canonical
   flip lands and the corpus is re-migrated, `--update` regenerates the budget
   and the delta is visible in the diff — no new report file, no new schema.
2. **Ratio series (the cross-language anchor), new: `vox ci k-complexity`.**
   For a paired subset (~25 tasks) drawn from
   `contracts/eval/humaneval-vox/problems/` (164 problems exist), the **existing**
   `contracts/eval/humaneval-vox/manifest.v1.yaml` gains optional
   `solution_py`/`solution_ts` file references and a `paired_for_k_complexity:
   true` flag on the selected entries — extending that manifest's existing
   `files:` map rather than a new sibling manifest. Per task:
   `vox_tokens / py_tokens` and `vox_tokens / ts_tokens`; aggregate = median
   ratio. New report: `contracts/reports/k-complexity-ratios.v1.json`, schema
   `contracts/eval/k-complexity-ratios-report.v1.schema.json`, `x-vox-version: 1`.
   `vox ci k-complexity --trend` walks the report's `git log` (bounded — see
   below) and prints the aggregate time series, so the data is *read*
   periodically, not just written.

**Gate.** `vox ci k-complexity` recomputes the ratio-series aggregate and fails
when it regresses **more than 2%** vs the committed report — a single binary
threshold. There is no separate warning tier: the locked operator decision is
"trend gate... not advisory-only," and this program does not introduce one.
Intentional changes regenerate via `--write` in the same PR (the
`gui-surface-coverage --write` pattern) for the ratio report, and via the
existing `--update` for the (now-extended) absolute budget file. Both wire into
the `full` pre-push tier + CI. `--trend` defaults to the most recent 50 report
revisions (or `--since <ref>`) — unbounded history walking is not acceptable
given the report regenerates on every Track 2 flip. The paired-corpus reference
solutions are static text measured by the tokenizer — they are never executed,
so the gate adds no Python/Node runtime dependency to CI.

**Honesty constraints.** Reference `.py`/`.ts` solutions are review-verified
idiomatic implementations, not executed artifacts; the report records this.
Ratio comparisons are apples-to-apples only per-task, never across tasks.

## S4. Track 2 — Pythonic audit + canonical flips

**Deliverable 1: the audit table** (`docs/src/architecture/pythonic-surface-audit-2026.md`,
frontmatter category "Architecture SSOTs", `status: research`). Every
Python-surface candidate gets a row:

| Candidate | Vox today | Token Δ (measured) | Collision? | Corpus churn (files) | Disposition |

Candidate inventory (audit must complete it, not just these):
`elif` (Vox: `else if`), `def` (Vox: `fn`), `None`/`Some` vs Vox option syntax,
f-strings vs Vox interpolation (`TemplateStringLit`), list/dict comprehensions,
`lambda`, `with` context managers, `try`/`except` vs `Result`, `True`/`False`
capitalization (Vox: `true`/`false`), `pass`, `del`, slice syntax, `in` as
membership test, `is not` (already canonical), chained comparisons, `**`/`//`
operators, `range()` (exists), `len()` (exists), `print()` (exists).

Token deltas are measured with the Track 1 tokenizer over minimal paired
fixtures — that is why Track 1 sequences first.

**Disposition rules (locked by operator):**
- **Adopt-canonical** when Python spelling's token cost ≤ current spelling AND
  no grammar collision. Churn is displayed, not disqualifying.
- **Tolerate-alias** when the Python spelling helps LLM emission but the Vox
  spelling stays canonical (the `==`→`is` pattern).
- **Reject** when structural (significant whitespace, exception-based control
  flow) or colliding.
- The completed table is a **human checkpoint**: operator approves dispositions
  before any flip is implemented.
- **Closure criterion:** the audit is done, and the table locked, once every
  candidate in this spec's inventory has a row AND one bounded search over
  Python's keyword list + common builtins has been swept for anything the
  inventory missed. It is not reopened without a new task; new candidates found
  later start a fresh, separately-scoped audit.
- Token deltas in the table are measured under the Track 1 tokenizer (MENS/
  Qwen3-family) — a proxy for this program's target model, not a universal
  frontier-model claim (see Problem §1).

**Deliverable 2: the flips.** Each approved adopt/tolerate row ships as one task
following the proven Steps-0-1 shape: failing test → grammar change →
old-spelling warn + `Replacement` fix-it → `vox fix` corpus sweep → golden
re-canonicalization → `mens/config/system_prompt.txt` regen → CR-F3 ledger row
in the same commit → k-complexity report regen showing the delta.

## S5. Track 3 — Ledger to 100%

- Enumerate all 41 `Decl` variants from `crates/vox-ast/src/decl/` into
  pre-declared `todo` rows (one enumeration task; the count is re-derived from
  the enum at execution time, not trusted from this spec).
- Author fixtures in parallel batches (~8 rows per batch, disjoint test files);
  flip each row `todo → covered` in the same commit as its fixture, per the
  ledger's own policy header.
- Tighten the schema: `status: covered` requires a non-empty string `fixture`
  (currently `fixture: null` validates — known gap, also flagged by CodeRabbit
  on PR #469).
- Extend `crates/vox-compiler/tests/language_surface_coverage_schema_test.rs`'s
  expected-covered list as rows flip, so coverage claims cannot silently vanish.
- **This program does NOT flip `mode: warn` → `mode: enforce`.** The ledger's
  own file header, written by the predecessor program, explicitly chose
  warn-then-observe before gating: *"No CI gate reads this file yet —
  enforcement lands in a later Sequencing Step."* Flipping to hard enforcement
  in the same pass that authors all 41 fresh rows gives zero soak time between
  "row exists" and "row blocks CI." This program's Track 3 exit state is: 100%
  of `Decl` variants covered, schema tightened, `mode: warn` still active. The
  enforce flip is a separate, later, explicitly human-checkpointed follow-up
  task once the ledger has been observed green in warn mode for at least one
  full review cycle. (A narrower, unconditional check — new `Decl` variants
  must ship with SOME ledger row, any status — is in scope now; that's a
  structural completeness check, not the soak-sensitive enforce flip.)
- Ledger-file coordination: Track 2's flip template also writes a row to this
  same YAML file (its own `covered` row per flip, in the flip's own commit).
  Track 2 and Track 3 are parallel at the *task* level (independent test/code
  work), but commits that touch `contracts/spec/language-surface-coverage.v1.yaml`
  must serialize (rebase-before-commit) — true simultaneous writes to one YAML
  file are a real collision, not just a conceptual one. See plan §Orchestration.
- Includes the `editor-tooling/vscode-tmlanguage-decorators` row: regenerate
  `apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json` from
  `vox-language-surface` (today it hardcodes retired decorators via
  `scripts/generate-grammars.vox:53`).

## S6. Track 4 — Pilot Axis surface

**Independent of Track 2's flip count.** Track 4 proceeds whether Track 2
approved many canonical flips or none — the sequencing dependency exists only
so the pilot is written once, against settled canon, not to avoid rewriting it
mid-flip-churn. It is not gated on flips existing; the disposition table could
legitimately reject everything (the audit's own rules allow "Reject" as an
outcome) and Track 4 still runs, since its value (codegen gaps + evidence base
for TS-elimination) is independent of the Pythonic-surface question.

**Selection.** One small, real `vox-gui` UI surface — candidate: a settings
panel (small real IPC + state + list rendering); final pick is a plan task with
a human checkpoint, chosen by TSX-pattern inventory (smallest surface that still
exercises state, a Tauri invoke, and a list).

**Port.** Rewrite the surface as a `.vox` `component` whose emitted TSX builds
inside `vox-gui/ui`'s bundle (pnpm build passes with the generated file replacing
the hand-written one). The port drives, in-program, the codegen fixes it
actually hits — known blockers going in:
- Dotted-member JSX (`<Dialog.Root>`) — documented unsupported
  (`crates/vox-codegen-ts/src/reactive/mod.rs:104-111`); needed for any
  Radix/shadcn-style compound component.
- Idiomatic effect/IPC lowering — the reactive lanes already emit real
  `useState`/`useMemo`/`useEffect`
  (`crates/vox-codegen-ts/src/reactive/effects.rs`), but data access lowers to
  the bespoke `voxRuntime` journal; the pilot determines whether a Tauri-invoke
  surface needs a cleaner lowering and implements the minimal version.
- Named-import component story (`import react { X } from "..."` exists —
  `crates/vox-compiler/src/parser/descent/decl/head_import.rs`; the pilot
  validates it against real `vox-gui` components).

**Exit artifacts.** (1) The working ported surface, behind a build-time flag or
as a parallel file until parity is reviewed — with an explicit disposition on
review, not an indefinite parallel state: **parity passes** → the hand-written
TSX is deleted, the flag/parallel-file scaffolding is removed, the generated
file becomes canonical; **parity fails** → the generated file is either deleted
or kept behind an explicit `vox-deprecated-since`/`retire-by` marker per
AGENTS.md's migration-vestige policy, never left as untracked, undecided
scaffolding. (2) A gap-inventory doc
(`docs/src/architecture/axis-tsx-gap-inventory-2026.md`): every TSX pattern the
port encountered, marked expressible / fixed-in-program / still-inexpressible —
the evidence base for the full TS-elimination program. Tauri stays as the shell;
this program eliminates TypeScript authorship, not the runtime.

## S6b. Human checkpoints (program-wide)

Four, explicitly authorized here so the plan's Orchestration section has spec
grounding for all of them, not just two:
1. **After Track 1's baseline report lands** — operator reviews the first
   absolute + ratio numbers before Track 2 flips begin; these are the "before"
   line every flip's delta is measured against.
2. **Track 2's disposition table (hard gate)** — operator approves dispositions
   before any flip is implemented (stated above, S4).
3. **Track 4 surface pick** — operator picks the pilot surface from the
   inventory (stated above, S6).
4. **Track 4 parity review** — operator reviews the ported surface against the
   hand-written original before the exit-artifact disposition (pass/fail,
   above) is applied.

## S7. Testing & process

- TDD per the repository policy: every code-producing task names its failing
  test before implementation; the plan carries red/green sequencing per task.
- Dual independent review per task (spec-compliance, then code-quality), as run
  throughout Steps 0-1.
- CR-F3 same-commit ledger policy for every grammar-touching task.
- Never amend commits in this multi-agent context; fixes are new commits.
- No new `.ps1`/`.sh`/`.py` automation; corpus tooling is `vox` subcommands or
  `.vox` scripts. Reference `solution.py`/`solution.ts` files in the paired
  corpus are measurement *data*, not automation, and are never executed.
- Formatting via `vox run scripts/fmt.vox` (never `cargo fmt` workspace-wide on
  Windows).

## Non-goals

- Full `vox-gui` rewrite in Vox (Track 4 produces the evidence base only).
- Replacing Tauri (explicitly out; the target is TS authorship elimination).
- TOESTUB per-script K-warnings (deferred until the corpus database is large
  enough to define outliers; the CI trend gate is the enforcement point now).
- Removing the `;`/`==`/`!=` tolerances (kept permanently as reader-compat).
- Executing/CI-testing the paired corpus's Python/TS reference solutions.
- `parse_fn_decl_inner` refactor — recorded as a note in the plan's appendix
  only; this program does not authorize doing it under any condition, including
  "a Track 2 flip happens to touch that file."
- Flipping CR-F3's `mode: warn` → `mode: enforce` (Track 3 reaches
  100%-covered and stops there; the flip is a separate future task, see S5).
- Triggering or scheduling an actual MENS training/eval run against the
  regenerated `mens/config/system_prompt.txt` or k-complexity reports. This
  program produces measurement infrastructure and data; consuming it in a
  training cycle is a separate, unscoped follow-up. (Prior session history
  records MENS training blocked on unrelated corpus-reproducibility issues —
  this program does not depend on or resolve that.)
- Validating token-cost claims against any tokenizer other than the vendored
  MENS/Qwen3-family one (see Problem §1 and Risks).

## Risks

- **Tokenizer artifact drift.** Mitigated: vendored, SHA-pinned, hash recorded;
  load-time verification rejects a hash mismatch rather than silently measuring
  against a corrupted/swapped artifact (plan Task 1.1).
- **Single-tokenizer narrowing.** The K-metric is MENS/Qwen3-BPE-specific, not a
  universal frontier-model claim (Problem §1). Accepted, not solved, by this
  program — see Non-goals.
- **Pythonic flips churn the corpus mid-program.** Mitigated: Phase 0's
  `vox fix` makes each sweep mechanical; Track 4 sequences after Track 2 so the
  pilot is written once, in post-flip canon (and proceeds regardless of how
  many flips were approved — S6).
- **Paired-corpus authorship quality.** Reference solutions must be idiomatic to
  be a fair baseline; mitigated by review-verification and by measuring medians
  over ~25 tasks rather than trusting any single pair.
- **Ledger fixture fan-out stalls, or collides with Track 2's own ledger
  writes.** 41 variants is large; mitigated by batch structure, an explicit
  barrier (seed rows before fixture batches begin), a resumable pipeline for
  the two genuinely wide fan-outs, and same-file commit serialization with
  Track 2's flip-driven ledger rows (plan §Orchestration).
- **`--trend` / CI wall-clock growth.** The ratio report regenerates on every
  flip; unbounded git-log walking and unbudgeted full-tier gate additions both
  risk silently degrading over the program's lifetime. Mitigated: `--trend`
  windows to the most recent 50 revisions by default; Task 0.3 explicitly runs
  `--enforce-budgets` and updates the tier budget baseline if the new gates
  need headroom.
- **Baseline drift.** This plan cites dozens of exact file:line locations
  against PR #469, which is still open as of this writing — not yet merged.
  Citations already drifted once during this spec's own review (a wrong test
  path was found and fixed). Mitigated: an explicit citation-reverification
  pass is Phase 0's first task, not just ambient per-task caution.
- **Track 4 exit ambiguity.** Without an explicit disposition rule, a
  parity-reviewed pilot could sit indefinitely as untracked parallel
  scaffolding. Mitigated: pass/fail disposition is specified in S6.
