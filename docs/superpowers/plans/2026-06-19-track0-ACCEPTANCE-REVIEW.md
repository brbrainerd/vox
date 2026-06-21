# Track 0 — Acceptance Review (run when Flash reports done)

> Ledger: AGH-0013. Plan: `2026-06-19-track0-distribution-ssot.md`. Handoff: `2026-06-19-track0-FLASH-HANDOFF.md`.
> Purpose: verify the things Flash **cannot** verify about itself — semantic correctness, policy
> compliance, and anything outside the IDE's reach. Mechanical green ≠ correct.

## A. Reproduce the green (don't trust the report)

```bash
git log --oneline -12                      # expect ~9 task commits, sane messages
cargo build -p voxup > build-out.txt 2>&1  # exit 0
cargo test -p voxup --test distribution_parity > test-out.txt 2>&1   # ALL pass; count >= 8 tests
cargo fmt -p voxup -- --check               # no diff (Flask must have fmt'd)
```
- [ ] All three commands clean. Test count ≥ 8 (Tasks 3–7 add: parse + 5 internal + toolchain + 2 publish + binaries).

## B. Policy compliance (Flash forgets these)

- [ ] **No `cargo fmt --all`** was run — `git diff --stat <first-task>^..HEAD` touches ONLY: `contracts/distribution/`, `crates/voxup/**`, `.github/workflows/distribution-parity.yml`, `crates/_public.toml`(maybe), `Cargo.lock`, `crates/workspace-hack/Cargo.toml`(maybe), `docs/src/architecture/distribution-ssot.md`. Anything else = scope violation; revert it.
- [ ] **No new `.ps1`/`.sh`/`.py`** added (`git diff --name-only <base>..HEAD | grep -E '\.(ps1|sh|py)$'` → empty).
- [ ] **No scratch files committed** — `git ls-files | grep -E '(test|build|voxup-deps)-out\.txt'` → empty (the `.gitignore` guard should have caught these).
- [ ] **`Cargo.lock` committed** in the same series as the `toml` dep add (else CI `--locked` fails).
- [ ] **Frontmatter** present on `docs/src/architecture/distribution-ssot.md` (title/description/category/status).

## C. Semantic correctness — the SSOT must be RIGHT, not just parseable

Open `contracts/distribution/profiles.v1.yaml` and confirm by hand:
- [ ] `rust_version` == `versions.rust` in `contracts/toolchain/workspace-toolchain.v1.yaml` (currently `1.96.0`). The test asserts this, but confirm the contract wasn't edited to force-match.
- [ ] `binaries` == exactly `[vox, vox-ml-cli, voxup]` — NOT `vox-bootstrap`/`vox-schola` (retired, see `2026-06-17-nightly-release-pipeline-design.md`).
- [ ] `publish.crates` ⊆ `crates/_public.toml` AND leaf-first: `vox-crypto`, then `vox-plugin-types → vox-plugin-api → vox-plugin-sdk` (ABI trio order), then `voxup` last. Wrong order is not caught by the test — eyeball it.
- [ ] `publish.enabled: false` (must stay false — going public is deferred).
- [ ] `non_publishable` contains `vox-orchestrator-mcp`; `agy` appears ONLY in `tiers.full.runtime_optional`. (Test `agy_only_in_full_tier_runtime_optional` covers this — confirm it actually ran, not skipped.)
- [ ] Tier dep closures are sane: `minimal.build_deps=[rust]`; `default`/`full` add `node`+`tauri-system-libs`.

## D. Reader quality (Flash writes brittle Rust)

Open `crates/voxup/src/profiles.rs`:
- [ ] Parsing helpers (`toolchain_rust_version`, `public_toml_crates`, `cargo_publish_is_false`) live HERE and return std types — NOT inline in the test (the dev-dep rule). If Flash moved parsing into the test and named `serde_yaml::`/`toml::` there, it would not have compiled — but double-check it didn't add `serde_yaml`/`toml` to `[dev-dependencies]` as a workaround (that's the wrong fix; remove it).
- [ ] `[lib]` added without breaking the `voxup` binary — `cargo build -p voxup` produced both targets, and `main.rs` is unchanged.
- [ ] No `.unwrap()` on the SSOT read path inside library code that a consumer would call (test code may `expect`).

## E. Things ONLY a human/Claude can finish (Flash CANNOT)

- [ ] **Make `distribution-parity` a required check** — GitHub branch-protection setting on `main`. Flash cannot touch repo settings. Do this via `gh api` or the repo settings UI after merge.
- [ ] **Confirm the workflow actually triggers** — open a throwaway PR touching `contracts/distribution/` and verify the check runs green on hosted ubuntu (the test compiles all of voxup incl. reqwest/tokio; confirm no missing system lib on `ubuntu-latest`).
- [ ] **Reconcile with the existing crates.io program** before Track C — `project_gamify_gui_pluginization_plan_2026_06_18` owns the publish *machinery* (hakari-aware publish, R18 gate). Confirm this SSOT's `publish` block is the data source it should read, no duplication.

## F. Close-out

- [ ] Fill ledger AGH-0013: `delivered`, `verification` (real values), `commits` (SHAs), `outcome: green|partial|failed`, and the prose section (what Flash deviated on, any contradiction it reported).
- [ ] If Flash reported a contradiction (e.g. a publish crate at a non-`crates/<name>` path, `cargo-hakari` behavior), fold the resolution into the plan BEFORE writing Tracks A–D.
- [ ] Decide merge: simple verified → push to main (admin bypass) per repo norms; or open PR if the required-check wiring needs to prove itself first.

## Likely Flash failure modes (pre-mortem — check these first if something's off)

1. Wrote `../../_public.toml` again (copied an old draft) → `expect("_public.toml must exist")` panic. Fix to `../_public.toml`.
2. Forgot `cargo fmt -p voxup` → `--check` diff. Just run it.
3. Added `toml` but didn't commit `Cargo.lock` → CI `--locked` red (local green). Commit the lock.
4. Edited the toolchain contract to make the version test pass instead of fixing the SSOT → check git diff on `workspace-toolchain.v1.yaml` is empty.
5. `deny_unknown_fields` or a typo'd YAML key → `parse` panic; reconcile struct field names with the YAML.
