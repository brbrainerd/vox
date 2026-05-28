# Vox-DB Audit — Task 0.1 Baseline

Date: 2026-05-08
Worktree: `cc_bdesktop2/magical-thompson-3fc3a4`

## Facts established

1. **Build:** `cargo build -p vox-cli --bin vox` — PASS (4m 58s).
2. **Integration test:** `cargo test -p vox-cli --test turso_import_guard_integration` — **FAIL**.
3. **Manual guard:** `vox ci turso-import-guard --all` — **FAIL** (exit 1).
4. **`vox-secrets` contains `turso::`** — YES, in `crates/vox-secrets/src/backend/vox_vault.rs` (15+ hits incl. `use turso::params;`, `turso::Connection`, `turso::Builder`).
5. **Why does the guard fail?** Two independent reasons; the first short-circuits the second:
   - **(a) UTF-8 read error.** A stray UTF-16 LE-BOM file `crates/vox-secrets/test.rs` (30 bytes, `fn main() {}`, tracked in git since commit `160b016be` "rename vox-clavis → vox-secrets") is hit by `visit_rs_files` and `read_utf8_path_capped` returns `Err`, which propagates via `?` in `run_turso_import_guard` and aborts the whole scan with: `Error: …/crates/vox-secrets/test.rs: invalid UTF-8`. So in this worktree the guard never produces a normal "offenders" report.
   - **(b) Allowlist gap.** Even if (a) were fixed, `vox-secrets` is NOT in the built-in prefixes (`vox-db/`, `vox-package/`, `vox-compiler/`) nor in `docs/agents/turso-import-allowlist.txt` (which lists `vox-corpus/`, `vox-cli/src/commands/db/`, `vox-cli/.../coderabbit/`, `vox-clavis/`, `vox-workflow-runtime/tests/`, `vox-ludus/`, `vox-populi/src/transport/store/`). `crates/vox-secrets/src/backend/vox_vault.rs` would be reported as an offender.

## Scan scope

`scan_targets(root, true)` calls `visit_rs_files(root.join("crates"), …)` — recurses every `.rs` file under `crates/` regardless of crate. `vox-secrets` IS in scope. The `\btur` + `so::` regex matches `vox_vault.rs` contents.

## Implication for next phases

Audit premise validated with a twist: the guard is currently broken (UTF-8 error), masking the underlying allowlist drift. Next phase must (i) delete or fix `crates/vox-secrets/test.rs`, then (ii) decide whether to add `vox-secrets/` to the turso allowlist (matches `data-storage-policy.v1.yaml:22-25`) or migrate vox-secrets off direct Turso.
