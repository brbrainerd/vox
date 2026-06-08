# Semantic behavior map — `vox-project-scaffold`

Synthesized from 24 extracted Behavior claims (`crates/vox-project-scaffold/src/lib.rs`), deduped and grouped across 4 symbols. Two mutators (`render_fn_stub`, `append_fn_stub`) carry both error-path and edge/invariant proofs and are in good shape. `file_defines_fn` has a meaningful substring-rejection edge proof. The one actionable hole is `scaffold_vox_project_at()` — the crate's primary public surface — which is proven only on the happy path even though its contract has clear failure and fallback branches.

## `render_fn_stub()` (lib.rs:215)
Proven behaviors:
- Emits a function call with one underscore placeholder per parameter; echoes param declarations into the generated signature.
- Generates a `@test` block using `assert(result is _expected)` syntax.
- Identifier validation accepts `_`-leading and `snake_case` names.

Coverage: happy + **error** + edge.
- Error path proven: rejects digit-leading, empty, space-containing, and kebab-case identifiers (drives the `is_valid_vox_identifier` bail at :216).
- Edge/invariant proven: placeholder count matches arg count (zero-param → empty arg list).

## `append_fn_stub()` (lib.rs:273)
Proven behaviors:
- Creates the target file when missing; returns a positive byte/line count; writes the stub content.
- Appends without clobbering existing definitions.

Coverage: happy + **error** + edge.
- Error path proven: refuses to append a fn whose name is already defined (the `!force && file_defines_fn` bail at :288).
- Edge/invariant proven: inserts a separating newline before the stub when the existing file lacks a trailing newline (the `existing.ends_with('\n')` branch at :305).

## `file_defines_fn()` (lib.rs:314)
Proven behaviors:
- Matches exact fn-name definitions and definitions with leading whitespace.

Coverage: happy + **edge** (no error path applicable — pure predicate, returns `bool`).
- Edge/invariant proven: does NOT match names where the search term is only a substring (guards the `after.chars().next()` delimiter check at :321–322).

## `scaffold_vox_project_at()` (lib.rs:369)
Proven behaviors:
- Skill kind: writes a `.skill.md` file; returns a summary with that file in `created_relative_paths`.
- Application kind: creates the `Vox.toml` manifest and `src/main.vox` entry point; tracks `Vox.toml` in `created_relative_paths`.

Coverage: **happy only.** No error, edge, or invariant proof.

## Semantic gaps

**`scaffold_vox_project_at()` — happy-path-only on a mutator with multiple uncovered failure/fallback branches.** This is the most actionable gap. The contract clearly has non-happy modes that no test exercises:
- **Invalid input rejection:** `project_name` / `package_kind` are not validated by any proven test; an invalid project name or kind has no rejection proof.
- **Unknown-template fallback:** `main_vox_content` (:355) returns an "Unknown template '{other}'" fallback body for unrecognized template keys. Only known/known-good paths are proven — the fallback branch is untested, so a typo'd template silently produces a degraded scaffold with no test catching the regression.
- **Strict-repo path resolution rejection:** `resolve_scaffold_target_under_repo` (:337) delegates to `vox_repository::resolve_strict_repo_relative_path` to keep the target inside the repo root — a path-traversal / out-of-repo security surface. No claim proves it rejects an escaping `target_subdir`.

Recommended next tests: a rejection test for an invalid `project_name`/`package_kind`, a fallback-content assertion for an unknown `template` key, and an out-of-repo `target_subdir` rejection test against the strict path resolver.

The other three symbols are not gaps: each mutator/predicate with a real failure or boundary mode already has a corresponding error or edge proof.