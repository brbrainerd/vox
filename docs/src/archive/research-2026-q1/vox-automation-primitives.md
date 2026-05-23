---
title: "Automation primitives"
description: "Official automation primitive surface for Vox script-mode builtins and runtime semantics."
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Automation primitives

Script-mode codegen (feature **`script-execution`**) exposes:

| Surface | Semantics |
| ------- | --------- |
| `print(str)` | Line to stdout (`println!`). |
| `std.args` | `Vec<String>` of argv after the script path. |
| `std.env.get(key: str)` | `Option[str]` via `std::env::var`. |
| `std.fs.read(path)` | `Result[str]` — UTF-8 text. |
| `std.fs.write(path, data)` | `Result[Unit]`. |
| `std.fs.read_bytes(path)` | `Result[str]` — bytes as string (lossy where needed at boundary). |
| `std.fs.exists(path)` | `bool`. |
| `std.fs.is_file(path)` | `bool` — path exists and is a regular file (not a directory). |
| `std.fs.is_dir(path)` | `bool` — path exists and is a directory. |
| `std.fs.canonicalize(path)` | `Result[str]` — absolute, normalized path (`Resolve-Path`-style); error if missing. |
| `std.fs.remove(path)` | `Result[Unit]` — file remove. |
| `std.fs.mkdir(path)` | `Result[Unit]` — `create_dir_all`. |
| `std.fs.list_dir(path)` | `Result[List[str]]]` — file names only (non-recursive). |
| `std.fs.glob(pattern)` | `Result[List[str]]]` — sorted paths matching a [`glob`](https://docs.rs/glob/) pattern. |
| `std.fs.remove_dir_all(path)` | `Result[Unit]` — recursive directory removal. |
| `std.fs.copy(src, dst)` | `Result[Unit]` — copy a file. |
| `std.path.join(a, b)` | `str` — platform path join. |
| `std.path.join_many(segments)` | `str` — join a `List[str]` with the platform separator (empty list → `"."`). |
| `std.path.basename` / `dirname` / `extension` | `str` — path helpers. |
| `std.process.which(name)` | `Option[str]` — resolve executable on `PATH` to an absolute path (empty/whitespace name → `None`). |
| `std.process.run(cmd, args)` | `Result[int]` — success exit code; non-zero → `Error`. |
| `std.process.run_ex(cmd, args, cwd, env)` | `Result[int]` — like `run`, optional `cwd` (`""` = inherit) and `env` as `List[str]` of `KEY=value` pairs merged into the subprocess environment. |
| `std.process.run_capture(cmd, args)` | `Result[Record]` — `{ exit: int, stdout: str, stderr: str }`; spawn/read errors → `Error`; **non-zero exit is still `Ok`** (inspect `exit`). |
| `std.process.run_capture_ex(cmd, args, cwd, env)` | Same as `run_capture`, with optional `cwd` and `env` (same shape as `run_ex`). |
| `std.process.exit(code)` | Terminates the process (`std::process::exit`). |
| `std.json.read_str(json, key)` | `Result[str]` — parse a JSON object and read a string field (top-level). |
| `std.json.read_f64(json, key)` | `Result[float]` — parse a JSON object and read a numeric field (ints coerced). |
| `std.json.quote(s)` | `str` — JSON-encode a string value (quotes + escapes). |
| `std.http.get_text(url)` | `Result[str]` — HTTP GET and return response body text for 2xx responses. |
| `std.http.post_json(url, body_json)` | `Result[str]` — HTTP POST with JSON string payload and text response for 2xx responses. |

Type-checker routing: `crates/vox-compiler/src/typeck/checker/expr_field.rs` (`StdFsNs`, `StdPathNs`, `StdEnvNs`, `StdProcessNs`, `StdJsonNs`, `StdHttpNs`). Codegen: `crates/vox-compiler/src/codegen_rust/emit/stmt_expr.rs` (`std.fs.*` / `std.process.*` / `std.json.*` / `std.http.*` builtins). Runtime: `crates/vox-runtime/src/builtins.rs` (`vox_list_dir`, `vox_process_run`, `vox_process_run_capture`, `vox_fs_glob`, `vox_http_get_text`, `vox_http_post_json`, …).

## Security

`std.process.run`, `run_capture`, `run_ex`, and `run_capture_ex` use the host `Command` API — **trusted dev** contexts only. Untrusted inputs should use the WASI / sandbox lanes documented for `vox script`, not arbitrary command strings.

## Where PowerShell fits

- **Agent and contributor shell sessions** (terminal instructions, IDE runners, docs examples for “run this locally”) target **PowerShell** when **`pwsh`** is available — see [`AGENTS.md`](../../../AGENTS.md) and [`docs/src/reference/cli.md`](../reference/cli.md) (`vox shell check`). That policy governs **strings you paste into a shell** around the repo.
- **`std.process.*` and `std.fs.*` in Vox** are **not** PowerShell: they lower to Rust `std::process::Command` / filesystem APIs (see codegen/runtime links above). A `.vox` script uses the table in this document regardless of whether you launched `vox` from **pwsh**, **bash**, or **cmd** — the Vox runtime stays host-neutral at the language level while still using OS-specific paths at the edge.
- **Design lexicon:** PowerShell-like habits (explicit path kind, normalize before compare, resolve tools on `PATH`) map to the `std.fs` / `std.path` / `std.process` table above; see [Standard library surfaces](../reference/std-surfaces.md#lessons-from-powershell-shaped-ergonomics-mapped-to-std) and [Vox shell operations boundaries](vox-shell-operations-boundaries.md).


