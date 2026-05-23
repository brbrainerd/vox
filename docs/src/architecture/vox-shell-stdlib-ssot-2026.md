---
title: "Vox shell-tier stdlib SSOT (2026)"
description: "Argv-first Rust builtins for filesystem, process, and structured formats; separation from host shells and MCP vox_run_shell."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Agents must not confuse Vox stdlib with PowerShell/Nushell lowering."
related:
  - docs/src/architecture/terminal-exec-policy-ssot.md
  - docs/src/architecture/agentos-ssot-2026.md
  - crates/vox-compiler/src/builtin_registry.rs
  - crates/vox-actor-runtime/src/builtins/mod.rs
---

# Vox shell-tier stdlib SSOT (2026)

## 1. Non‑negotiable: stdlib does **not** lower to a shell

`std.fs`, `std.path`, `std.env`, `std.process`, `std.json`, `std.csv`, `std.toml`, `std.yaml`, `std.io`, and `std.agentos` are implemented as **Rust callouts** (see [`builtin_registry.rs`](../../../crates/vox-compiler/src/builtin_registry.rs) → [`vox_actor_runtime::builtins`](../../../crates/vox-actor-runtime/src/builtins/mod.rs)).

- **Argv‑first:** `std.process.run*` passes `(executable, args: list[str])` to `std::process::Command` — no `/bin/sh -c`, no `pwsh -c`, no implicit shell parsing.
- **Filesystem:** `std.fs.*` maps to `std::fs` / `glob` via `vox_actor_runtime` (same patterns as `list_dir`, `glob`, etc.).

### 1.1 Interpreter tier (`vox run --interp`)

`vox-compiler` **cannot** depend on `vox-actor-runtime` (Cargo cycle via `vox-db` → `vox-codegen` → `vox-compiler`). The interpreter therefore mirrors the same semantics in [`shell_stdlib.rs`](../../../crates/vox-compiler/src/eval/shell_stdlib.rs). **Keep behavior aligned** when changing CSV/TOML/YAML/IO/fs‑detailed helpers — update both crates in one PR.

## 2. Host shells (AgentOS only)

The **only** first‑party surfaces that invoke PowerShell or Nushell are MCP / CLI **execution probes** under `vox-cli` `commands/runtime/shell/` (`ShellBackendKind`, `run_shell_probe`). These are used for AgentOS `execution_probe` metadata — **not** for `std.process` or `std.fs`.

`vox_run_shell` MCP tool results carry `aci.shell_backend` (`powershell` | `nushell`) derived from the tool argument `backend` when ACI envelopes are enabled (see [`agentos-ssot-2026.md`](./agentos-ssot-2026.md)).

## 3. Method inventory (shell‑tier extensions, 2026‑05)

Registered in `std_namespace_method_ty` / `std_namespace_runtime_call`:

| Namespace | Method | Vox type (approx.) | Role |
|-----------|--------|-------------------|------|
| `std.fs` | `list_dir_detailed` | `fn(str) -> Result[list[FileRecord]]` | Structured directory rows (`name`, `path`, `size`, `modified_ms`, `is_dir`, `is_file`, `is_symlink`). |
| `std.fs` | `stat` | `fn(str) -> Result[FileRecord]` | Single‑path metadata. |
| `std.csv` | `parse` | `fn(str) -> Result[Json]` | CSV → JSON array‑of‑arrays (no header semantics). |
| `std.csv` | `parse_records` | `fn(str) -> Result[Json]` | Header row + records → JSON array of objects. |
| `std.csv` | `render` | `fn(list[list[str]]) -> Result[str]` | Rows → CSV text. |
| `std.toml` | `parse` / `render` | `fn(str)->Result[Json]`, `fn(Json)->Result[str]` | TOML ↔ JSON. |
| `std.yaml` | `parse` / `render` | `fn(str)->Result[Json]`, `fn(Json)->Result[str]` | YAML ↔ JSON. |
| `std.io` | `open` | `fn(str)->Result[Json]` | Extension dispatch: `.json`, `.toml`, `.yaml`/`.yml`, `.csv`, else UTF‑8 string JSON value. |
| `std.io` | `save` | `fn(str, Json)->Result[unit]` | Inverse of `open` for structured extensions; plain extension expects JSON string value. |
| `std.process` | `run_capture_json` | `fn(str, list[str])->Result[Json]` | Parse stdout as JSON (trimmed). |
| `std.process` | `run_capture_lines` | `fn(str, list[str])->Result[list[str]]` | Non‑zero exit → `Err`; otherwise stdout split on lines. |

Existing `std.fs` / `std.process` / `std.json` methods remain as documented in [`builtin_registry.rs`](../../../crates/vox-compiler/src/builtin_registry.rs).

## 4. “If you want X, use Y” (migration)

| Goal | Prefer |
|------|--------|
| List files with sizes / mtimes | `std.fs.list_dir_detailed` / `std.fs.stat` |
| Parse CLI CSV output | `std.csv.parse` / `std.csv.parse_records` |
| Read config files | `std.io.open` or explicit `std.toml.parse` / `std.yaml.parse` / `std.json.parse` |
| Parse subprocess JSON stdout | `std.process.run_capture_json` |
| Line‑oriented subprocess output | `std.process.run_capture_lines` |
| Host shell policy / allowlists | [`terminal-exec-policy-ssot.md`](./terminal-exec-policy-ssot.md) — **not** `std.process` |

## 5. Golden examples

- [`examples/golden/structured_shell_listings.vox`](../../../examples/golden/structured_shell_listings.vox)
- [`examples/golden/format_conversion.vox`](../../../examples/golden/format_conversion.vox)
- [`examples/golden/io_polymorphic.vox`](../../../examples/golden/io_polymorphic.vox)
- [`examples/golden/tabular_subprocess.vox`](../../../examples/golden/tabular_subprocess.vox)

## 6. Change checklist

- [ ] Update **both** `vox-actor-runtime` builtins and `vox-compiler` `eval/shell_stdlib.rs` when changing interpreter‑visible semantics.
- [ ] Extend [`builtin_registry.rs`](../../../crates/vox-compiler/src/builtin_registry.rs), [`expr_field.rs`](../../../crates/vox-compiler/src/typeck/checker/expr_field.rs), and [`expr.rs`](../../../crates/vox-compiler/src/typeck/checker/expr.rs) namespace dispatch for new `std.*` fields.
- [ ] Run `cargo test -p vox-actor-runtime`, `cargo test -p vox-compiler shell_stdlib`, `vox check` on touched goldens.
- [ ] If `attach_aci_envelope` or `shell_backend` behavior changes, run `cargo test -p vox-orchestrator-mcp aci_`.
