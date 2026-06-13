



# Semantic Behavior Map — vox-test-harness

This map synthesizes 14 extracted Behavior claims (deduped to 13 distinct behaviors across 9 symbols) for `crates/vox-test-harness`, the workspace's test-scaffolding crate. The crate provides environment isolation (`EnvScratch`, `TempRoot`), a fluent synthetic-Cargo-workspace builder (`SyntheticWorkspaceBuilder` + `MemberSpec`), and workspace-root discovery (`find_workspace_root`). Proven coverage is almost entirely happy-path: 12 of 13 behaviors are `happy`, with a single `invariant` proof on workspace marker files and **zero error-path proofs** anywhere in the crate. Several symbols whose contracts clearly have failure/empty/conflict modes are proven only on the success path — those are the semantic holes below.

## EnvScratch
File: `src/env_scratch.rs`
- Restores a previously **unset** env var back to unset state when dropped after `set()`. (happy)
- Restores the **prior value** of an existing env var when dropped after `set()`. (happy)
- Error/edge proof: none. No proof for nested scratches, re-`set()` within scope, or restore correctness if `set()` itself fails.

## TempRoot
File: `src/temp_root.rs`
- `new()` creates a writable temporary directory accessible via `path()`. (happy)
- Error/edge proof: none. No creation-failure or drop-cleanup proof.

## SyntheticWorkspaceBuilder
File: `src/synthetic_workspace.rs`
- `build()` writes a `Cargo.toml` at the workspace root. (happy)
- `build()` creates member directories each with `Cargo.toml` and `src/lib.rs`. (happy)
- `with_git_stub()` creates `.git/HEAD` containing a ref pointer. (happy)
- `with_changelog()` writes version + date to `CHANGELOG.md` in expected format. (happy)
- `with_extra_file()` writes content at a relative path, creating parent directories. (happy)
- Error/edge proof: none. No path-conflict, double-call, empty-workspace, or overwrite edge proofs.

## MemberSpec
File: `src/synthetic_workspace.rs`
- `with_dep()` adds a path-dependency declaration to the member's `Cargo.toml` in correct format. (happy)
- `with_description()` sets the `description` field. (happy)
- `binary()` produces a member with `src/main.rs` instead of `src/lib.rs`. (happy)
- `with_source()` replaces default `lib.rs` content with custom source. (happy)
- Error/edge proof: none. No proof for dep on a missing member, duplicate decls, empty source, or source-override on a `binary()` member.

## find_workspace_root() / Workspace
File: `src/workspace_paths.rs`
- `find_workspace_root()` locates the root by walking parent dirs until it finds a `Cargo.toml` with a `[workspace]` table. (happy)
- **Invariant:** the found root contains `AGENTS.md` and `contracts/config/env-vars.v1.yaml`. (invariant)
- Error/edge proof: invariant present; **no failure-path proof** for walking to the filesystem root without finding a `[workspace]` manifest — its central failure contract.

## Semantic gaps

Symbols proven only on the happy path whose contract clearly has a failure/empty/conflict mode:

1. **`find_workspace_root()` — missing not-found path (highest priority).** Discovery/search functions live or die on the not-found branch. Only the success search and a marker-file invariant are proven; nothing exercises walking to the root with no `[workspace]` manifest. This is a validator-shaped surface with no rejection test.
2. **`EnvScratch` — no edge/error proof on a global-mutable-state surface.** Restore-on-drop is proven for both unset and prior-value cases, but env vars are process-global; nested/overlapping scratches, re-`set()` within scope, and restore-under-`set()`-failure are unproven. Highest-risk because a leaked restore silently corrupts sibling tests.
3. **`SyntheticWorkspaceBuilder::with_extra_file()` — mutator with no conflict/overwrite proof.** Writes content and creates parents on the happy path; no proof for collision with a generated file, overwrite semantics, or escaping/absolute relative paths.
4. **`MemberSpec::with_dep()` — declaration emitter with no invalid-input proof.** Correct format proven; no proof for a dep on a non-existent member or duplicate dep declarations.
5. **`TempRoot::new()` — creation/cleanup failure unproven.** Only the writable-dir success case exists; no creation-failure or drop-cleanup proof for this RAII resource.

The remaining `MemberSpec`/builder methods (`with_description`, `binary`, `with_source`, `with_changelog`, `with_git_stub`) are happy-only but lower-risk as straightforward formatters; the empty-source and binary-vs-lib override interaction for `with_source()` is the one worth a follow-up edge test.