# Semantic Behavior Map — `vox-doc-pipeline`

All extracted behavior lives in `crates/vox-doc-pipeline/src/pipeline/lint.rs`. The 6 claims dedup to **2 distinct symbols**. `lint_file` is reasonably exercised for its duplicate-frontmatter rule (positive + negative + two false-positive-suppression edges). `skip_unlabeled_code_fence_rel` is a pure path predicate proven only on the two obvious branches. Below, behaviors are grouped per symbol with error-path / edge coverage flags, followed by the actionable semantic gaps.

## `lint_file`

A validator over markdown files emitting `LintKind` diagnostics.

Proven behaviors:
- **Positive detection (happy):** two YAML frontmatter blocks separated by content → emits `LintKind::DuplicateFrontmatter`.
- **Negative / no-false-alarm (happy):** a single YAML frontmatter block → no `DuplicateFrontmatter` diagnostic.
- **Edge — code-fence suppression:** triple-dashes inside a markdown code fence are not counted as a second frontmatter block (no diagnostic).
- **Edge — horizontal-rule + vox-fence suppression:** horizontal rules and `vox` code fences after the frontmatter do not trigger `DuplicateFrontmatter`.

Coverage:
- Error-path proof: **partial** — only the `DuplicateFrontmatter` rule is asserted; no other `LintKind` rejection is proven.
- Edge/invariant proof: **yes** (two false-positive-suppression edges).

## `skip_unlabeled_code_fence_rel`

A boolean predicate deciding whether unlabeled-code-fence linting is suppressed for a given relative path.

Proven behaviors:
- **True branch (happy):** returns `true` for paths whose names contain `plan` or `design`.
- **False branch (happy):** returns `false` for ordinary reference-doc paths lacking those keywords.

Coverage:
- Error-path proof: **n/a** (total predicate, no error path).
- Edge/invariant proof: **none**.

## Semantic gaps

These symbols are proven only on the happy path despite contracts with clear failure / collision / empty modes:

1. **`skip_unlabeled_code_fence_rel` — no edge coverage on a substring-matching predicate.** Keyword matching on `plan`/`design` is exactly the kind of rule that mis-fires on substrings (`planet`, `redesigned`, `template`), case variants (`Plan`, `DESIGN`), and path-separator boundaries (matching a parent dir vs the filename). None of these corner cases are proven. This is the most actionable gap: a one-line predicate with an unverified collision surface.

2. **`lint_file` — single-rule integrity coverage.** As a validator/integrity surface, `lint_file` presumably emits more than `DuplicateFrontmatter`, but only that one rule is proven. Unproven: malformed / unterminated frontmatter, completely empty files, and zero-frontmatter files (the empty/invariant mode). A validator with only one asserted rejection path is a meaningful hole.