# Vox Syntax Optimization Program — Implementation Plan

**Date:** 2026-08-09
**Spec:** `docs/superpowers/specs/2026-08-09-vox-syntax-optimization-program-design.md`
**Baseline:** branch `claude/box-language-syntax-audit-b80815` tip after PR #469 merges (the plan assumes the tolerant-reader machinery — `parse_and_warnings`, `Replacement` payloads, `vox ci vox-parse-check` — is on `main`).

Execution discipline (unchanged from Steps 0-1): TDD per task (failing test first,
named below), dual independent review per task, CR-F3 ledger rows flip in the same
commit as their fixture, never amend — fix with new commits, format via
`vox run scripts/fmt.vox`.

Verification note for every task: file/line citations below were verified against
the branch at writing time; each task's first step is to re-verify its own
citations before editing (the codebase moves).

---

## Phase 0 — Corpus migration enabler

### Task 0.1 — `vox fix`: apply Replacement payloads to source

**Goal.** One command that mechanically migrates `.vox` files off tolerated
legacy spellings, driven by the machine-readable `Replacement` payloads
(`crates/vox-compiler/src/parser/error.rs` struct `Replacement { from, to, code }`)
already attached to Warning-severity diagnostics.

**Pre-check (discovery).** Confirm no existing fixer surface:
`grep -rn "fn run_fix\|--fix" crates/vox-cli/src/commands/` — at writing time
`fmt.rs` has no fix mode and no `fix.rs` exists. If discovery finds one, extend
it instead of adding a command.

**Failing test first** (`crates/vox-cli/src/commands/fix.rs`, same-file
`#[cfg(test)] mod tests`):
- `fix_removes_tolerated_semicolon`: write a tempfile `let x = 5;\n`, run the
  fix routine, assert the file now reads `let x = 5\n` and the routine reports
  1 applied fix.
- `fix_check_mode_writes_nothing`: same input with `--check`; file unchanged,
  exit signals pending fixes.
- `fix_is_idempotent`: running twice yields identical bytes and 0 fixes on the
  second pass.
- `fix_skips_error_files`: a file with a hard parse error is reported and left
  byte-identical (never rewrite text we can't fully parse).

**Implementation sketch.** For each input file: `lex` →
`parse_and_warnings`/`parse_script_and_warnings` (reuse
`is_script_like` — canonical copy `crates/vox-cli/src/commands/check.rs:26`);
collect warnings carrying `replacement: Some(_)`; apply replacements
back-to-front by span byte offsets (descending start order so earlier spans stay
valid); re-parse the result and require warning-count strictly decreased and no
new errors, else revert that file and report. Wire as `vox fix <globs>`
(`--check`, `--verbose`). Registered in the CLI command registry; regenerate the
command-surface baseline fixture
(`crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt` via
`UPDATE_CLI_CATALOG_BASELINE=1 cargo test -p vox-cli --test command_catalog_paths_baseline`)
in the same commit — this exact fixture went stale on PR #469, don't repeat it.

### Task 0.2 — `--deny-warnings` for vox-parse-check + golden canonicalization

**Failing test first** (`crates/vox-cli-ci/src/parse_check.rs` tests):
- `vox_parse_check_deny_warnings_fails_on_tolerated_semicolon`: fixture
  `fn main() {\n    let x = 1;\n}\n` passes default mode (existing test
  `vox_parse_check_tolerates_statement_boundary_semicolons` stays green) but
  fails with `--deny-warnings`.
- `vox_parse_check_deny_warnings_passes_on_canonical`: canonical fixture passes
  both modes.

**Implementation.** `run_vox` gains a `deny_warnings: bool`; it must switch from
`parse`/`parse_script` to the `_and_warnings` variants so Ok-path warnings are
visible (today `run_vox` calls the warning-discarding functions —
`crates/vox-cli-ci/src/parse_check.rs:135-139`; this also resolves CodeRabbit
finding 3743086242). Then run `vox fix examples/golden/**/*.vox`, hand-review
the diff, and land the canonicalized goldens + flag in one commit, with
`vox ci vox-parse-check "examples/golden/**/*.vox" --deny-warnings` green.

### Task 0.3 — Enforce the parse gates

**Failing test first:** extend the pre-push tier test surface (locate via
`grep -rn "check_fmt\|fast tier" crates/vox-cli/src/commands/ci/` for the tier
definition module) with an assertion that the `full` tier includes the two new
steps; red until wired.

**Implementation.** Add to the `full` pre-push tier + CI workflow:
`vox ci vox-parse-check "scripts/**/*.vox" "apps/**/*.vox"` (deny errors only)
and the goldens `--deny-warnings` variant from Task 0.2. Not the fast tier
(hundreds of files). Both corpora are green at writing time (all 8 known
failures fixed on PR #469), so enforcement starts clean.

---

## Track 1 — K-complexity harness

### Task 1.1 — Vendored tokenizer + counting core

**Goal.** Deterministic model-BPE token counts, pinned forever.

**Discovery step.** Locate an existing tokenizer artifact:
`ls mens/config mens/data; grep -rn "tokenizer" mens/ --include=*.toml -l`.
If a MENS (Qwen3-family) `tokenizer.json` exists in-repo, pin that; otherwise
vendor one under `contracts/eval/tokenizer/tokenizer.json` with a
`SOURCES.toml` note (upstream repo, revision SHA, license) following the
`assets/skills/SOURCES.toml` precedent. Record the artifact's SHA-256; the
report embeds it.

**Failing test first** (new module — placement per
`docs/src/architecture/where-things-live.md`; default `crates/vox-cli-ci/src/k_complexity.rs`
beside `parse_check.rs`/`benchmark_telemetry.rs`, no new crate):
- `bpe_count_is_deterministic`: tokenizing a fixed literal twice yields the same
  count.
- `bpe_count_matches_pinned_fixture`: a known string yields an exact count
  committed alongside the pinned artifact (locks artifact + library version).
- `tokenizer_hash_matches_manifest`: SHA-256 of the vendored file equals the
  recorded hash.

**Implementation.** `tokenizers` crate (workspace dep exists, v0.21,
`Cargo.toml:280`); `fn bpe_count(text: &str) -> usize` + artifact loading with
hash verification.

### Task 1.2 — Report schema + absolute series

**Failing test first:**
- Schema-validation test (pattern:
  `crates/vox-compiler/tests/language_surface_coverage_schema_test.rs`) for
  `contracts/eval/k-complexity-report.v1.schema.json`: valid report validates;
  a report missing `tokenizer_sha256` fails. Schema carries `x-vox-version: 1`
  in `required` (the convention CodeRabbit flagged as missing on the CR-F3
  schema — start compliant here and fix the CR-F3 schema in Track 3).
- `k_report_covers_every_budgeted_fixture`: every fixture id in
  `contracts/eval/source-token-budget.v1.json` appears in the generated report
  (the two lanes must not silently diverge in coverage).

**Implementation.** `vox ci k-complexity` computes, per golden fixture already
tracked by the lexer-token budget: `{ fixture_id, bpe_tokens, bytes,
lexer_tokens }`, plus report-level `{ schema_version, x_vox_version,
tokenizer_sha256, generated_from_commit, aggregate: { total_bpe_tokens,
median_bpe_per_fixture } }`. Written to
`contracts/reports/k-complexity.v1.json` with `--write`; without `--write`,
recompute and diff against the committed report (the
`gui-surface-coverage` pattern — `vox ci gui-surface-coverage --write`).

### Task 1.3 — Paired corpus (ratio series)

**Selection.** ~25 problems from `contracts/eval/humaneval-vox/problems/`
(164 exist; manifest `contracts/eval/humaneval-vox/manifest.v1.yaml`), chosen to
span the manifest's difficulty/feature axes. Each selected problem directory
gains `solution.py` and `solution.ts` — idiomatic reference implementations of
the same task the `.vox` solution solves.

**Verification bar (honesty).** References are review-verified static text,
never executed; each file carries a header comment stating it is a measurement
reference. The pairing task's reviewer checks idiomatic quality (a deliberately
verbose Python baseline would flatter Vox's ratio — that is the failure mode to
review against).

**Failing test first:**
- `paired_manifest_lists_only_existing_triples`: a new
  `contracts/eval/k-complexity-pairs.v1.yaml` manifest names each selected
  problem; test asserts every entry has all three solution files on disk and
  every triple on disk is manifested.
- `ratio_series_present_for_all_pairs`: generated report contains a
  `pairs` array with `vox_over_py` and `vox_over_ts` per manifest entry.

**Implementation.** Extend the Task 1.2 report with the ratio lane; aggregate =
median of per-pair ratios. Authoring the ~50 reference files is the program's
first wide fan-out — see §Orchestration.

### Task 1.4 — Gate + trend

**Failing test first:**
- `gate_fails_on_regression`: with a doctored committed report whose aggregate
  is 3% lower than recomputed, the check errors; at 1% it passes (2% threshold,
  warn at >1%).
- `trend_walks_report_history`: `--trend` against a repo fixture with two
  committed report versions prints both data points in order.

**Implementation.** `vox ci k-complexity` (no flags) = gate mode;
`--write` = regenerate; `--trend` = `git log --format=%H -- <report>` +
`git show <sha>:<report>` per revision, printing per-fixture and aggregate
series. Wire gate into `full` pre-push tier + CI. Document in the command's
help: intentional syntax changes regenerate the report in the same PR, making
the K-delta part of the reviewed diff.

**Human checkpoint:** operator reviews the first full report (absolute + ratio
baselines) before Track 2 flips begin — these numbers are the "before" line.

---

## Track 2 — Pythonic audit + canonical flips

### Task 2.1 — The audit table

**Deliverable.** `docs/src/architecture/pythonic-surface-audit-2026.md`
(frontmatter: category "Architecture SSOTs", `status: research`,
`training_eligible: false`). For every candidate in the spec's inventory (and
any the audit surfaces): current Vox spelling, Python spelling, measured BPE
delta (via Task 1.1's counter over minimal paired snippets), grammar collision
analysis (cite the lexer/parser site that would conflict), corpus churn count
(`grep -c` across `**/*.vox`), disposition per the locked rules.

**Verification bar.** Every collision claim cites a real
`crates/vox-compiler/src/lexer/token.rs` or `parser/descent/**` line. Every
churn count is a reproducible command written into the table row.

**Human checkpoint (hard gate).** Operator approves the disposition table before
any flip task is dispatched. Expected shape (illustrative, the audit decides):
`elif` → adopt (shorter than `else if`, no collision — `elif` is not a token
today); `True`/`False` → tolerate-alias toward `true`/`false`; `def` → reject
or tolerate (collides with nothing but saves nothing over `fn`; churn ~743
files); comprehensions → reject (new grammar, real K increase).

### Task 2.2..2.N — One task per approved flip (template)

Each approved row becomes a task with this fixed shape (proven by Steps 0-1's
Tasks 5-6):

1. **Failing test:** parser/lexer test asserting the new spelling parses to the
   same AST as the old, plus (adopt-canonical only) the old spelling now warns
   with a `Replacement { from: <old>, to: <new>, code: "vox/pythonic/<name>" }`.
2. Grammar change (lexer token or soft-keyword dispatch, whichever the audit
   row specified).
3. `vox fix` corpus sweep (Task 0.1) — mechanical migration commit, separate
   from the grammar commit, reviewed as a diff.
4. Golden re-canonicalization stays green under `--deny-warnings` (Task 0.2 gate).
5. `mens/config/system_prompt.txt` regen (its Construct Reference must teach the
   new canon; the syntax test
   `crates/vox-compiler/tests/mens_system_prompt_syntax_test.rs` guards it).
6. CR-F3 ledger row `covered` with the new fixture, same commit as the fixture.
7. `vox ci k-complexity --write` regen — the measured delta lands in the PR diff.

Worked first instance (assuming Task 2.1 approves it): **`elif`**. Red test:
`elif_chain_parses_as_nested_else_if` in
`crates/vox-compiler/src/parser/descent/expr/pratt_match.rs` tests (if-parsing
lives at `parse_if`, `pratt_match.rs:737-767`; `else if` chains recurse at
`:746-753`). `elif` becomes a soft keyword recognized after `}` of an if-body,
desugaring to the existing `else if` AST — no new AST node, no ambiguity with
identifiers named `elif` in expression position.

---

## Track 3 — Ledger to 100% (parallel with Tracks 1-2)

### Task 3.1 — Enumerate + seed rows

**Failing test first:** extend
`crates/vox-compiler/tests/language_surface_coverage_schema_test.rs` with
`ledger_row_exists_for_every_decl_variant`: reflectively-maintained list of all
`Decl` variants (source: the enum in `crates/vox-ast/src/decl/` — re-derive the
count at execution; the audit's figure of 41 is the writing-time value) — every
variant name must have a ledger row (any status). Red until seeded.

**Implementation.** Add `decl/<variant-kebab>` rows, `status: todo`,
`fixture: null`, to `contracts/spec/language-surface-coverage.v1.yaml`. Also
tighten the schema in the same commit: `status: covered` requires
`fixture: { "type": "string", "minLength": 1 }` (closes the `fixture: null`
hole — CodeRabbit 3743086234), and add `x-vox-version` to schema + yaml
(CodeRabbit 3743086230).

### Task 3.2 — Fixture batches (wide fan-out)

Batches of ~8 variants; per batch, per variant: a parse test proving the
variant's canonical form round-trips (parse → AST assert), placed in the parser
test module nearest the variant's dispatch site; flip the row + extend the
expected-covered list in `language_surface_coverage_schema_test.rs` in the same
commit (the list currently pins only some covered rows — extend as flipped, per
CodeRabbit 3743086253). Batches touch disjoint test files by construction
(different variants live in different `descent/decl/*` modules) — see
§Orchestration.

### Task 3.3 — Enforcement flip + tmLanguage row

**Failing test first:** `ledger_enforce_mode_rejects_todo_rows` — with
`mode: enforce` and any `todo` row, the CI check errors.

**Implementation.** When the last Decl row flips: `mode: warn → enforce`; a
`vox ci` check (extend the existing ssot-drift family) fails on `todo` rows and
on new parser productions without rows (heuristic: new `Decl` variant without a
matching row fails Task 3.1's test already). Separately: regenerate
`apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json` from
`vox-language-surface`'s decorator list instead of the hardcoded copy in
`scripts/generate-grammars.vox:53`, flip the
`editor-tooling/vscode-tmlanguage-decorators` row, and add a drift test
comparing the generated grammar's decorator set to `LEXER_AT_DECORATORS`.

---

## Track 4 — Pilot Axis surface (sequences after Track 2 flips land)

### Task 4.1 — Surface selection + TSX inventory (human checkpoint)

Inventory 3 candidate small surfaces from `crates/vox-gui/ui/src/components/`
(settings-type panels preferred). For each: LOC, hooks used, Tauri `invoke`
calls, external components (Radix/dotted-member usage), list rendering.
Operator picks one. Deliverable: the inventory table appended to the (new,
initially skeletal) `docs/src/architecture/axis-tsx-gap-inventory-2026.md`
(frontmatter: "Architecture SSOTs", `status: research`, `training_eligible: false`).

### Task 4.2 — Dotted-member JSX lowering

**Failing test first:** in `crates/vox-codegen-ts/src/reactive/mod.rs` tests —
the existing test at `:104-111` *documents* that `Dialog.Root()` under a
namespace import lowers to a call, not a tag; write the inverse assertion
(`namespace_member_lowers_to_jsx_tag`: `import react * as Dialog from
"@radix-ui/react-dialog"` + `Dialog.Root(open=true) { ... }` emits
`<Dialog.Root open={true}>`), red today.

**Implementation.** Extend the call-to-JSX sugaring
(`crates/vox-compiler/src/parser/descent/expr/pratt_match.rs:409-477` decides
call vs JSX; member-call paths currently excluded) and/or the codegen tag
emission so a capitalized-namespace member with all-named args follows the same
JSX rule as a bare capitalized ident. Keep the existing rule's constraints
(all-named args; positional falls through to a plain call). Update the
documenting test rather than deleting it.

### Task 4.3 — IPC/effect lowering (scope set by 4.1)

Discovery-first: the selected surface's data access pattern (Tauri `invoke` vs
`db.*`). If `invoke`: red test asserting a Vox-side effect calling a declared
extern invoke emits idiomatic `invoke("cmd", args)` + `useEffect` wiring (the
reactive lanes already emit real hooks —
`crates/vox-codegen-ts/src/reactive/effects.rs:175-221`). If the surface needs
`db.*`, the bespoke `voxRuntime` journal lowering
(`crates/vox-codegen-ts/src/hir_emit/mod.rs:1063-1093`) is used as-is and the
gap is recorded in the inventory rather than fixed in-pilot (YAGNI fence).

### Task 4.4 — Port + build gate

**Failing test first:** a build-level test (or CI step) asserting
`pnpm --dir crates/vox-gui/ui build` succeeds with the generated TSX replacing
the hand-written file for the chosen surface (behind a directory swap or feature
flag; exact mechanism decided at execution with the checkpoint reviewer).
Visual/behavioral parity is human-reviewed, not pixel-tested (the existing
GUI visual-AI-review harness may be used advisorily).

### Task 4.5 — Gap inventory finalization

Complete `axis-tsx-gap-inventory-2026.md`: every TSX pattern hit, status
(expressible / fixed-in-program / inexpressible), with the inexpressible set
explicitly framed as the requirements list for the follow-up TS-elimination
program. Update `docs/src/architecture/research-index.md`.

---

## Orchestration & parallelism (for execution)

**Parallel-safe groups (disjoint writes, no cross-dependency):**
- Track 3 runs fully parallel to Tracks 1-2 (ledger yaml + test files vs
  eval contracts + cli-ci code).
- Within Track 1: Task 1.1 ∥ Task 1.3's *selection* step; 1.2 → 1.4 sequential.
- Track 2 flips (2.2..2.N) are sequential with each other (each sweeps the
  corpus; concurrent sweeps would collide) but each is internally small.
- Track 4 strictly after Track 2's last corpus sweep.

**Workflow-tool recommendation (narrow, justified):** exactly two pieces have
genuine wide fan-out earning a resumable background pipeline:
1. **Task 1.3 reference authoring** — ~25 problems × 2 languages = ~50
   independent, similarly-shaped authoring items, each with a verify step
   (reviewer agent checks idiomatic quality + task fidelity against the `.vox`
   solution). Fan-out with per-item verify-after-generate; a barrier before the
   manifest commit.
2. **Task 3.2 fixture batches** — up to 41 independent, similarly-shaped
   parse-test items across disjoint files, with per-item verification (test
   passes, row flipped correctly).
Everything else is a handful of sequential tasks — dispatching individual
subagents per task (the Steps 0-1 pattern) is cheaper and easier to checkpoint;
recommending Workflow beyond these two would be over-orchestration.

**Human checkpoints:** after Task 1.4 (baseline report review), after Task 2.1
(disposition table approval — hard gate), at Task 4.1 (surface pick), at
Task 4.4 (parity review).

---

## Appendix — optional hygiene (explicitly out of program scope)

- `parse_fn_decl_inner` (`crates/vox-compiler/src/parser/descent/decl/head_fn.rs`,
  ~1300 lines, ~50 threaded locals) decomposition — compiler-internal
  complexity; do only if a Track 2 flip forces edits there anyway.
- `eat_return_arrow`'s bespoke warning → route through
  `warn_mainstream_operator_alias` when the next alias task touches that file
  (recorded as a PLAUSIBLE finding in the PR #469 review).
