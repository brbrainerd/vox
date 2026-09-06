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

### Task 0.0 — Citation sweep (do first)

PR #469 (this plan's stated baseline) is still open as of writing, and one
citation already drifted during this plan's own adversarial review — the
Track 2 template originally cited `crates/vox-compiler/tests/mens_system_prompt_syntax_test.rs`;
the real path is `crates/vox-integration-tests/tests/mens_system_prompt_syntax_test.rs`
(different crate). Before starting any other task: re-verify every file:line
and file-existence citation in this plan against the current branch tip
(`grep`/`Read` each one), fix drift found, and only then proceed. This is a
standing empirical finding, not a hypothetical — treat "re-verify before
editing" as a real task with a real pass/fail outcome, not ambient caution.

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
- `fix_applies_multiple_replacements_correctly`: a file with ≥2 tolerated
  spellings (e.g. two trailing `;`s on different lines) — assert BOTH are
  removed and the resulting file is byte-for-byte what a human hand-edit would
  produce. This is the actual regression test for "back-to-front by span": a
  naive forward-order (or any offset-unaware) implementation corrupts the
  second replacement once the first shifts byte positions, and would fail only
  this test, not the single-replacement ones above.
- `fix_rejects_overlapping_spans`: construct (via a hand-built `Vec<ParseError>`
  in the test, not real source) two replacement spans that overlap; assert the
  routine detects this defensively *before* splicing, reports the file as
  skipped/failed rather than panicking or silently corrupting it, and leaves
  the file byte-identical.
- `fix_preserves_crlf_line_endings`: a tempfile written with CRLF line endings
  containing a tolerated `;` — assert the fix is applied and every other line
  ending in the file is still CRLF afterward. `.vox` files are LF-only by repo
  convention (see the line-endings gate,
  `crates/vox-cli-ci/src/line_endings.rs`, which strips `\r` on its own autofix
  path) — a fixer that silently normalizes line endings as a side effect of
  unrelated text edits would violate that gate's contract. Confirm during
  discovery whether `lex`/`parse_and_warnings` already assumes LF-normalized
  input (per `crates/vox-compiler/src/lexer/cursor.rs`'s `normalize_text` call)
  and decide explicitly whether `vox fix` operates pre- or post-normalization;
  whichever is chosen, this test proves the choice doesn't corrupt endings.

**Implementation sketch.** For each input file: `lex` →
`parse_and_warnings`/`parse_script_and_warnings` (reuse
`is_script_like` — canonical copy `crates/vox-cli/src/commands/check.rs:26`);
collect warnings carrying `replacement: Some(_)`; **defensively check for
overlapping/out-of-order spans and reject the file (report, don't touch) if
found** — do not assume the parser can never produce them; build the fully
rewritten output as one in-memory buffer by applying replacements back-to-front
by span byte offsets (descending start order so earlier spans stay valid); only
after re-parsing that buffer and confirming warning-count strictly decreased
and no new errors, write it to disk via a write-to-temp-then-rename (atomic
replace), never incremental in-place edits — a panic or crash mid-splice must
never leave a partially-rewritten file on disk. Else revert (make no write) and
report. Wire as `vox fix <globs>` (`--check`, `--verbose`). Registered in the
CLI command registry; regenerate the command-surface baseline fixture
(`crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt` via
`UPDATE_CLI_CATALOG_BASELINE=1 cargo test -p vox-cli --test command_catalog_paths_baseline`)
in the same commit — this exact fixture went stale on PR #469, don't repeat it.
Note (not a blocking requirement, since `vox fix` only reaches file-write via
this program's own controlled corpus/golden sweeps, not arbitrary user input):
the glob expansion this reuses (`expand_globs` in
`crates/vox-cli-ci/src/parse_check.rs`) does no symlink or repo-root
confinement filtering; this is the first *write*-path consumer of that pattern
in the codebase, so if `vox fix` is ever exposed for arbitrary/untrusted glob
input later, add that filtering then — not gold-plating it now for a tool that
only ever runs against this repo's own tracked corpus.

### Task 0.2 — `--deny-warnings` for vox-parse-check + golden canonicalization

**Failing test first** (`crates/vox-cli-ci/src/parse_check.rs` tests):
- `vox_parse_check_deny_warnings_fails_on_tolerated_semicolon`: fixture
  `fn main() {\n    let x = 1;\n}\n` passes default mode (existing test
  `vox_parse_check_tolerates_statement_boundary_semicolons` stays green) but
  fails with `--deny-warnings`.
- `vox_parse_check_deny_warnings_passes_on_canonical`: canonical fixture passes
  both modes.
- `vox_parse_check_deny_warnings_error_file_fails_for_error_not_warning`: a
  fixture with a genuine hard parse error (not just a tolerated warning) under
  `--deny-warnings` — assert it fails, AND assert the failure reason/message
  distinguishes "parse error" from "denied warning" (the refactor from
  `parse`/`parse_script` to the `_and_warnings` variants must not conflate
  these two distinct failure modes into one code path that reports both
  identically).

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

**Budget check (required, not optional).** `contracts/budgets/test-tier-budgets.v1.yaml`
sets a ceiling on the `full` tier; these two gates walk hundreds of files each.
After wiring, run `vox ci pre-push --full --enforce-budgets` locally. If it
warns (>1.2x) or fails (>1.5x) the measured baseline, update
`contracts/budgets/test-tier-budgets.v1.yaml` in the *same commit* — don't
land new gates that silently eat the tier's time budget for later PRs to
discover.

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
- `tokenizer_hash_matches_recorded_value`: SHA-256 of the vendored file equals
  the recorded hash (the value lives as a field in this program's reports, not
  a separate "manifest" file — name reflects that, avoiding the terminology
  drift an earlier draft of this plan had).
- `tokenizer_hash_mismatch_is_rejected`: corrupt/swap one byte of a copy of the
  vendored artifact; loading against the recorded hash must fail loudly, not
  silently measure against a corrupted tokenizer.
- `bpe_count_before_load_errors_not_panics`: calling the counting function
  before the artifact is loaded returns an `Err`/explicit failure, not a panic
  — every downstream caller (Tasks 1.2, 1.3) depends on this being a clean
  failure mode.

**Implementation.** `tokenizers` crate (workspace dep exists, v0.21,
`Cargo.toml:280`); `fn bpe_count(text: &str) -> usize` + artifact loading with
hash verification. Vendoring provenance note
(`contracts/eval/tokenizer/SOURCES.toml`, following the `assets/skills/SOURCES.toml`
precedent) must record upstream repo, revision SHA, **and license** — not just
the hash; `assets/skills/SOURCES.toml`'s own precedent (and `scripts/vendor-skills.vox`'s
handling of upstream LICENSE files) requires this, don't skip the license field
just because it isn't independently tested.

### Task 1.2 — Extend the absolute series (no new report)

**Goal.** Add BPE token counts to the *existing* per-golden budget file instead
of standing up a parallel report. `contracts/eval/source-token-budget.v1.json`
already stores `{ fixture_id: { tokens, bytes } }` and
`run_source_token_budget` (`crates/vox-cli/src/commands/ci/run_body_helpers/syntax_k.rs`)
already implements the exact ratchet/tolerance/`--update` gate this needs — a
new schema+report+CLI-verb for one more integer per fixture would duplicate
working infrastructure (confirmed by direct comparison during spec review).

**Failing test first** (same file, extending its existing test module):
- `source_token_entry_carries_bpe_tokens`: `SourceTokenEntry` gains
  `bpe_tokens: Option<usize>`; a budget file with the field round-trips through
  (de)serialization.
- `bpe_ratchet_fails_on_regression_beyond_tolerance`: mirrors the existing
  token/byte ratchet test in this file — a fixture whose measured `bpe_tokens`
  exceeds its budgeted value beyond `tolerance` fails `run_source_token_budget`.
- `update_mode_records_bpe_tokens`: `--update` writes the measured `bpe_tokens`
  into the budget file alongside `tokens`/`bytes`, same as it already does for
  those two fields.

**Implementation.** Add the field, call `k_complexity::bpe_count` (Task 1.1)
inside the existing per-fixture loop, extend the existing gate/report/`--update`
logic to cover it. No new file, no new schema, no new CLI subcommand — this
task's failing tests are extensions of the existing test module in `syntax_k.rs`.

### Task 1.3 — Paired corpus (ratio series): extend the existing manifest

**Selection.** ~25 problems from `contracts/eval/humaneval-vox/problems/`
(164 exist; manifest `contracts/eval/humaneval-vox/manifest.v1.yaml`), chosen to
span the manifest's difficulty/feature axes.

**No new manifest file.** The existing manifest already carries a per-problem
`files:` map (`spec`, `reference`, `tests`, plus `training_eligible`,
`provenance`, `slug`). Extend that map with optional `solution_py`/`solution_ts`
keys and a `paired_for_k_complexity: true` flag on the ~25 selected entries,
instead of a sibling `k-complexity-pairs.v1.yaml` that would independently
track a subset of IDs the first manifest already owns. Each selected problem
directory gains `solution.py` and `solution.ts` — idiomatic reference
implementations of the same task the `.vox` solution solves.

**Verification bar (honesty).** References are review-verified static text,
never executed; each file carries a header comment stating it is a measurement
reference. The pairing task's reviewer checks idiomatic quality (a deliberately
verbose Python baseline would flatter Vox's ratio — that is the failure mode to
review against).

**Failing test first:**
- `paired_entries_have_both_solution_files`: extend/add to the manifest's
  existing consistency test (discover it first — `grep -rn` for the manifest's
  current validation test before adding a parallel one) — every entry flagged
  `paired_for_k_complexity: true` has both `solution_py` and `solution_ts`
  files on disk; no file on disk is referenced by an entry that isn't flagged.
- `ratio_series_present_for_all_pairs`: the Task 1.4 report contains a `pairs`
  array with `vox_over_py`/`vox_over_ts` per flagged manifest entry.

**Implementation.** Authoring the ~50 reference files is one of the program's
two genuinely wide fan-outs — see §Orchestration.

### Task 1.4 — `vox ci k-complexity`: ratio report, gate, trend

**This is the one genuinely new CLI surface and report in Track 1** — the
absolute series lives in the (now-extended) existing budget file; this command
owns the cross-language ratio series plus the aggregate gate and history trend.

**Failing test first:**
- Schema-validation test (pattern:
  `crates/vox-compiler/tests/language_surface_coverage_schema_test.rs`) for
  `contracts/eval/k-complexity-ratios-report.v1.schema.json`: valid report
  validates; a report missing `tokenizer_sha256` or `x_vox_version` fails.
  Schema carries `x-vox-version: 1` in `required` (the convention CodeRabbit
  flagged as missing on the CR-F3 schema — start compliant here and fix the
  CR-F3 schema in Track 3).
- `gate_passes_at_exactly_two_percent_regression`: aggregate ratio regressed
  by exactly 2% vs. committed report — passes (spec: fails only when regression
  is **more than** 2%; this is a single binary threshold, no separate warn
  tier — a fixed inconsistency from an earlier plan draft that implied a
  1%-warn/2%-fail scheme the spec never authorized).
- `gate_fails_just_over_two_percent_regression`: 2.01% (or the smallest
  representable step above 2%) — fails.
- `gate_fails_on_regression`: 3% regression — fails (sanity case).
- `write_mode_regenerates_report_with_correct_values`: `--write` against a
  stale committed report produces a new report whose per-pair ratios and
  aggregate match freshly recomputed values — not stale/zeroed data. This
  mode is load-bearing (every Track 2 flip task depends on it to land its
  measured delta), so it gets its own explicit test, not just incidental
  coverage from the gate tests.
- `trend_walks_report_history`: `--trend` against a repo fixture with two
  committed report versions prints both data points in order.
- `trend_defaults_to_bounded_window`: a fixture with >50 committed report
  revisions — `--trend` (no `--since`) only walks the most recent 50, not the
  full history; `--trend --since <ref>` overrides the window explicitly.

**Implementation.** `vox ci k-complexity` (no flags) = gate mode, single
threshold (>2% aggregate regression on the ratio series fails, nothing else);
`--write` = regenerate `contracts/reports/k-complexity-ratios.v1.json`;
`--trend` = `git log -n 50 --format=%H -- <report>` (or `--since`-bounded) +
`git show <sha>:<report>` per revision, printing per-pair and aggregate series.
Wire gate into `full` pre-push tier + CI (subject to the same
`--enforce-budgets` check as Task 0.3). Document in the command's help:
intentional syntax changes regenerate the ratio report via `--write` and the
absolute budget via `--update` (Task 1.2) in the same PR, making the K-delta
part of the reviewed diff.

**Human checkpoint:** operator reviews the first full report (absolute budget +
ratio baseline) before Track 2 flips begin — these numbers are the "before"
line (spec S6b checkpoint 1).

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

**Closure criterion.** The audit is complete, and the table locked, once every
candidate in the spec's inventory has a row and one bounded sweep of Python's
keyword list + common builtins has checked for anything missed. It does not
reopen without a new task — new candidates found later start a fresh,
separately-scoped audit rather than extending this one indefinitely.

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
   `crates/vox-integration-tests/tests/mens_system_prompt_syntax_test.rs`
   guards it — re-verify this path per Task 0.0 before relying on it).
6. CR-F3 ledger row `covered` with the new fixture, same commit as the fixture.
   **Coordination note:** Track 3 also writes rows to this same
   `contracts/spec/language-surface-coverage.v1.yaml` file (its own ledger-to-
   100% work, running in parallel — see §Orchestration). Different row keys
   (`pythonic/<name>` here vs. `decl/<variant>` in Track 3), so no semantic
   collision, but commits touching this file must be rebased/serialized against
   each other — check for a concurrent Track 3 commit before landing this step.
7. `vox ci k-complexity --write` (ratio series) + `--update` (absolute budget,
   Task 1.2) regen — the measured delta lands in the PR diff as two files, not
   one.

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

**Failing test first** (extend
`crates/vox-compiler/tests/language_surface_coverage_schema_test.rs`):
- `ledger_row_exists_for_every_decl_variant`: reflectively-maintained list of
  all `Decl` variants (source: the enum in `crates/vox-ast/src/decl/` —
  re-derive the count at execution; the audit's figure of 41 is the
  writing-time value) — every variant name must have a ledger row (any
  status). Red until seeded.
- `ledger_schema_tightening_does_not_break_existing_covered_rows`: run schema
  validation over the ledger file's CURRENT 12 rows (8 covered + 4 todo)
  against the tightened schema — all still validate. Guards against the
  `fixture: minLength: 1` tightening retroactively invalidating a real,
  already-shipped covered row.
- `ledger_seed_rejects_duplicate_row_names`: seeding the 41 `todo` rows must
  not produce a duplicate name against any of the 8 already-`covered` rows
  from Steps 0-1 (some `Decl` variants are almost certainly already covered —
  the seeding step must skip those, not double-row them).

**Implementation.** Add `decl/<variant-kebab>` rows, `status: todo`,
`fixture: null`, to `contracts/spec/language-surface-coverage.v1.yaml` —
**skipping any variant name that already has a row** (per the dedup test
above). Also tighten the schema in the same commit: `status: covered` requires
`fixture: { "type": "string", "minLength": 1 }` (closes the `fixture: null`
hole — CodeRabbit 3743086234), and add `x-vox-version` to schema + yaml
(CodeRabbit 3743086230).

### Task 3.2 — Fixture batches (wide fan-out)

**Barrier: this task does not start until Task 3.1's seed + schema commit has
landed.** Task 3.2 flips rows from `todo` to `covered` and depends on those
rows (and the tightened schema) existing; a Workflow-tool fan-out dispatched
before 3.1 lands would target nonexistent or wrong-shaped rows. Named
explicitly here (not just implied by task numbering) because 3.2 is one of the
two pieces this program recommends for background/parallel dispatch — see
§Orchestration.

Batches of ~8 variants; per batch, per variant: a parse test proving the
variant's canonical form round-trips (parse → AST assert), placed in the parser
test module nearest the variant's dispatch site; flip the row + extend the
expected-covered list in `language_surface_coverage_schema_test.rs` in the same
commit (the list currently pins only some covered rows — extend as flipped, per
CodeRabbit 3743086253). Batches touch disjoint test files by construction
(different variants live in different `descent/decl/*` modules) — see
§Orchestration. **Coordination note:** batch commits, like Task 2's flip
commits, write to the shared ledger YAML — serialize/rebase against any
concurrent Track 2 commit before landing (same file, see Task 2.2..2.N step 6).

### Task 3.3 — Structural completeness check + tmLanguage row

**Scope correction:** this task does **not** flip `mode: warn` → `mode:
enforce`. The ledger's own file header (written by the predecessor program)
explicitly chose warn-then-observe: *"No CI gate reads this file yet —
enforcement lands in a later Sequencing Step."* Flipping to hard enforcement in
the same program that authors all 41 fresh rows gives zero soak time. This
task's actual exit state: 100% of `Decl` variants have a row (any status),
schema is tightened, and a narrower **structural** check is added (new `Decl`
variant ships without any ledger row → fails; this doesn't require soak time,
it's a completeness invariant, not a coverage-quality judgment). The `mode:
enforce` flip is a separate future task, gated by its own human checkpoint,
after the ledger has run green in warn mode for at least one full review cycle
— do not schedule it as part of this program.

**Failing test first:** `ledger_new_decl_variant_without_any_row_fails_check`
— add a `Decl` variant in a test-only fixture context (or assert the reflective
enumeration test from Task 3.1 catches it directly) without a corresponding
row of any status; the check errors. (This replaces the originally-planned
`ledger_enforce_mode_rejects_todo_rows` test, which tested the now-descoped
enforce flip.)

**Implementation.** Wire the structural completeness check (extend the
existing ssot-drift family). Separately: regenerate
`apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json` from
`vox-language-surface`'s decorator list instead of the hardcoded copy in
`scripts/generate-grammars.vox:53`, flip the
`editor-tooling/vscode-tmlanguage-decorators` row, and add a drift test
comparing the generated grammar's decorator set to `LEXER_AT_DECORATORS`
(`crates/vox-language-surface/src/lib.rs`).

---

## Track 4 — Pilot Axis surface (sequences after Track 2's flip tasks are exhausted)

**Independent of how many flips Track 2 approved.** This track proceeds
whether Track 2 landed many canonical flips or none (the disposition table can
legitimately reject everything, per its own rules) — the sequencing dependency
exists only so the pilot is written once, against settled canon, not to avoid
mid-flip churn. Track 4's value (codegen gaps + TS-elimination evidence base)
does not depend on Track 2's outcome.

### Task 4.1 — Surface selection + TSX inventory (human checkpoint)

Inventory at least 2-3 candidate small surfaces from
`crates/vox-gui/ui/src/components/`
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
`<Dialog.Root open={true}>`), red today. **Add a negative test in the same
commit, not later:** `namespace_member_positional_arg_stays_plain_call` —
`Dialog.Root(open)` (positional, not named) must still lower to a plain call
expression, never a JSX tag. This is the plan's own stated constraint ("keep
the existing rule's constraints: all-named args; positional falls through to a
plain call") but without this explicit negative test, updating/replacing the
existing documenting test could silently remove the only check distinguishing
"dotted-member JSX" from "all dotted-member calls now become tags" — a real
regression risk for any legitimate namespaced function call
(e.g. `Utils.parse(x)`).

**Implementation.** Extend the call-to-JSX sugaring
(`crates/vox-compiler/src/parser/descent/expr/pratt_match.rs:409-477` decides
call vs JSX; member-call paths currently excluded) and/or the codegen tag
emission so a capitalized-namespace member with all-named args follows the same
JSX rule as a bare capitalized ident. Keep the existing rule's constraints
(all-named args; positional falls through to a plain call). Update the
documenting test rather than deleting it, and land the negative test alongside
it in the same commit.

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
flag; exact mechanism decided at execution with the checkpoint reviewer). Build
success alone proves the output is syntactically valid and bundles — it proves
nothing about behavior, so add the cheapest real automated signal beyond it:
`tsc --noEmit` on the generated file (type-correctness, catches a large class
of wrong-lowering bugs for free), and if Task 4.3 wired a Tauri `invoke` call,
a source-level assertion that the generated file contains the expected
`invoke("<command>", ...)` call shape. Visual/behavioral parity beyond that is
human-reviewed, not pixel-tested (the existing GUI visual-AI-review harness,
`crates/vox-cli-ci/src/gui_visual_review.rs`, may be used advisorily).

**Exit disposition (required, not left open-ended):** on parity approval —
delete the hand-written original, remove the flag/parallel-file mechanism, the
generated file becomes canonical. On parity failure — either delete the
generated file, or keep it explicitly marked with a
`// vox-deprecated-since=... retire-by=... reason="pilot parity failed"`
marker per AGENTS.md's migration-vestige policy. Never leave it as
undecided, indefinitely-parallel scaffolding — that's exactly the pattern the
retirement-marker policy exists to prevent.

### Task 4.5 — Gap inventory finalization

Complete `axis-tsx-gap-inventory-2026.md`: every TSX pattern hit, status
(expressible / fixed-in-program / inexpressible), with the inexpressible set
explicitly framed as the requirements list for the follow-up TS-elimination
program. Set valid frontmatter on the new page (`title`, `description`, `category`, `status`).
Starlight lists it. Do **not** create or edit `docs/src/architecture/research-index.md` (retired 2026-09).

---

## Orchestration & parallelism (for execution)

**Parallel-safe groups (disjoint writes, no cross-dependency):**
- Track 3's *task-level work* (writing tests, code) runs parallel to Tracks
  1-2. **Ledger-file commits do not**: Track 2's flip template (step 6) and
  Track 3 (Tasks 3.1/3.2) both write rows to the same
  `contracts/spec/language-surface-coverage.v1.yaml`. Different row-key
  namespaces (`pythonic/*` vs `decl/*`) mean no semantic collision, but
  concurrent uncoordinated writes to one YAML file are a real git-level
  collision. Serialize/rebase any commit touching this file against other
  in-flight commits touching it — this applies regardless of which track's
  task produced the commit.
- Within Track 1: Task 1.1 ∥ Task 1.3's *selection* step; 1.2 → 1.4 sequential
  (1.4 reads both the extended budget file from 1.2 and the extended manifest
  from 1.3).
- Within Track 3: **Task 3.1 is a hard barrier before Task 3.2's fan-out.**
  3.2 flips rows that must already exist with the tightened schema from 3.1;
  dispatching 3.2's Workflow-tool batch before 3.1's seed commit lands targets
  nonexistent/wrong-shaped rows.
- Track 2 flips (2.2..2.N) are sequential with each other (each sweeps the
  corpus; concurrent sweeps would collide) but each is internally small.
- Track 4 strictly after Track 2's flip tasks are exhausted (zero or many —
  see Track 4 header) — not gated on flips having occurred, only on Track 2
  being *done churning* so the pilot isn't rewritten mid-program.

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

## Appendix — optional hygiene (explicitly out of program scope, not authorized under any condition)

- `parse_fn_decl_inner` (`crates/vox-compiler/src/parser/descent/decl/head_fn.rs`,
  ~1353 lines, ~50 threaded locals) decomposition — compiler-internal
  complexity, not VoxScript K-complexity. This is a note for future reference,
  not a task; nothing in this program authorizes doing it, including as a
  side effect of a Track 2 flip touching that file.
- `eat_return_arrow`'s bespoke warning → route through
  `warn_mainstream_operator_alias` when the next alias task touches that file
  (recorded as a PLAUSIBLE finding in the PR #469 review). Same status: a note,
  not an authorized task.
