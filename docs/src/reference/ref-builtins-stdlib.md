---
title: "Standard Library Built-ins"
description: "Core execution environment capabilities exposed in Vox (std.* and built-ins)."
category: "Language Reference"
status: "current"
last_updated: "2026-05-23"
training_eligible: true

schema_type: "TechArticle"
---

# Reference: Standard Library Built-ins

Vox includes a minimal, highly optimized standard library focused exclusively on system I/O, core conversions, and process lifecycle capabilities inherently trusted by the compiler orchestrator.

> ### Two execution modes — two stdlib surfaces
>
> Vox has two execution paths and the stdlib surface differs slightly between them:
>
> | Mode | When | Built from |
> |------|------|------------|
> | **Interpreter** (`vox run --mode interp` or the auto-fallback) | Always available; default for `vox run foo.vox` when the binary lacks the `script-execution` Cargo feature | `crates/vox-compiler/src/eval/builtins.rs` |
> | **Native script** (`vox run --mode script`, requires `--features script-execution`) | Faster cold start, compiles to native via `vox-actor-runtime` | `crates/vox-compiler/src/builtin_registry.rs` + the runtime crate |
>
> Most stdlib symbols (`fs.*`, `path.*`, `str.*`, `list.*`, `process.*`, `regex.*`, `env.*`, `json.*`, `csv.*`, `toml.*`, `yaml.*`, `io.*`, `log.*`, `secrets.*`, `agentos.*`) are available in **both** modes.
>
> A small set is **native-only** today — they exist via `vox-actor-runtime` but aren't yet implemented in the tree-walking interpreter:
>
> - `Browser.*` (Chromium / CDP automation)
> - `OpenClaw.*` (MCP-style tool invocation)
> - `std.uuid()`, `std.now_ms()`, `std.hash_fast()`, `std.hash_secure()`
> - `std.http.get_text()`, `std.http.post_json()`
> - `std.mobile.*`
>
> Conversely, a set was added to **interp** during the 2026-05-23 audit
> but not yet wired through `vox-actor-runtime`. Scripts that use these
> under `--mode interp` work; under `--mode script` they need
> actor-runtime impls (tracked in audit §13 Phase H):
>
> - `fs.cwd()`, `fs.remove(path)`, `fs.walk(dir)`
> - `path.extension/parent/file_name/stem/is_absolute`
> - `process.cwd()`, `process.which(cmd)`
> - `regex.replace/is_match/captures` (the namespace form; the `Regex`
>   value-object methods already exist in actor-runtime)
> - **Closures + collection methods** (`xs.map(fn(x) { ... })`,
>   `Option.map`, `Result.map_err`, `xs.filter`, `xs.fold`, `xs.any`,
>   `xs.all`) — interp-only in v0.6; Rust/TS emit lands in a follow-up
>   per closures-rfc-2026-05-23.md §6/§9.8.
>
> Scripts that call these will fail with `UndefinedVariable` or
> `Method X not found` under native. To use them either keep
> `vox run` (auto-falls back to interp) or invoke explicitly:
> `vox run --mode interp foo.vox`. The
> [`stdlib-coverage`](../architecture/vox-stdlib-gap-audit-2026-05-23.md)
> drift gate treats native-only symbols as registered; the
> interp-only set is tracked separately under the Phase H "native parity"
> item.

## Global Built-ins

These core functions are evaluated globally across any lexical space in the application without module imports.

| Signature | Description |
|-----------|-------------|
| `fn len(collection: T) to int` | Returns the number of elements in a sequence, string, list, or mapping dictionary structure. |
| `fn str(val: T) to str` | Explicitly coerces arbitrary object types and scalar values strictly into UTF-8 strings. |
| `fn assert(condition: bool) to Unit` | Halts execution contexts raising terminal logic failures safely. |
| `fn print(message: str) to Unit` | Synchronous STDOUT writer. Always newlines. Vox has no `println` / `eprint` / `eprintln` variants — use `print` for stdout, and the level-tagged `std.log.warn` / `std.log.error` for stderr-channel diagnostics. |

> **Negation note:** Vox uses **`not`** for boolean negation. The bare `!`
> character is *not* a valid operator and produces a parse error with a
> "use `not`" hint. This matches Vox's phonetic-operators identity
> (`not`, `and`, `or`, `is`, `isnt`) and keeps one canonical form per
> concept. See `docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md` §8.

## Process and Execution IO (`std.fs.*`)

File system operations interact securely via WASI/os permission mappings. Error cascades explicitly require `Result`.

| Signature | Description |
|-----------|-------------|
| `fn read(path: str) to Result[str]` | Reads file at `path` as UTF-8 text. Aliases: `read_file`, `read_to_string`. |
| `fn write(path: str, content: str) to Result[Unit]` | Creates or overwrites the target file. Aliases: `write_file`, `write_to_file`. |
| `fn exists(path: str) to bool` | Evaluates whether a file or directory exists at the given path. |
| `fn is_file(path: str) to bool` | Returns true if the path is a file. |
| `fn is_dir(path: str) to bool` | Returns true if the path is a directory. |
| `fn cwd() to Result[str]` | Returns the current working directory. |
| `fn list_dir(path: str) to Result[list[str]]` | Returns a list of filenames in the directory. |
| `fn list_dir_detailed(path: str) to Result[list[Record]]` | Like `list_dir` but each row carries `name`, `path`, `is_dir`. |
| `fn glob(pattern: str) to Result[list[str]]` | Returns a list of paths matching the glob pattern. |
| `fn walk(dir: str) to Result[list[str]]` | Recursive lister; equivalent to `glob(dir + "/**/*")`. |
| `fn copy(src: str, dst: str) to Result[Unit]` | Copies a file from source to destination. |
| `fn remove(path: str) to Result[Unit]` | Removes the file at the given path. |
| `fn mkdir(path: str) to Result[Unit]` | Creates a directory (parents created as needed). |
| `fn remove_dir_all(path: str) to Result[Unit]` | Recursively removes a directory and all of its contents. |
| `fn stat(path: str) to Result[Record]` | Returns `Record[is_dir, is_file, size]` for the given path. |

## Path Manipulation (`std.path.*`)

Canonical naming follows Rust's `std::path::Path` (not the older
`basename`/`dirname` convention). One name per concept, not two.

| Signature | Description |
|-----------|-------------|
| `fn join(a: str, b: str) to str` | Joins two path parts. |
| `fn extension(p: str) to str` | Returns the file extension without the dot, or `""` if absent. |
| `fn parent(p: str) to str` | Returns the parent directory, or `""` for a root. |
| `fn file_name(p: str) to str` | Returns the basename including extension. |
| `fn stem(p: str) to str` | Returns the basename without the extension. |
| `fn is_absolute(p: str) to bool` | Whether the path is absolute on the host platform (drive letter on Windows, leading `/` on Unix). |

## Environment (`std.env.*`)

| Signature | Description |
|-----------|-------------|
| `fn get(key: str) to Option[str]` | Retrieves an environment variable. |

## Process Execution (`std.process.*`)

| Signature | Description |
|-----------|-------------|
| `fn which(cmd: str) to Option[str]` | Finds a command in the PATH. |
| `fn run(cmd: str, args: list[str]) to Result[int]` | Runs a command and returns the exit code. |
| `fn run_ex(cmd: str, args: list[str], cwd: str, env: map[str, str]) to Result[int]` | Runs a command with specific cwd and environment. |
| `fn run_capture_json(cmd: str, args: list[str]) to Result[Json]` | Runs a command, captures stdout, parses it as JSON. |
| `fn run_capture_lines(cmd: str, args: list[str]) to Result[list[str]]` | Runs a command, returns stdout split on newlines. |
| `fn exit(code: int) to never` | Terminates the process with the given exit code. |

## JSON Processing (`std.json.*`)

The free-function `json.*` namespace handles parse + render. Indexed
access (`get_str`, `get_int`, `get_object`, etc.) lives on the `Json`
value type returned by `json.parse` — see the **Json value methods**
section below.

| Signature | Description |
|-----------|-------------|
| `fn parse(s: str) to Json` | Parse a JSON string into a typed Json value. Returns `null` on parse error. |
| `fn render(v: any) to Result[str]` | Serialize a Vox value to a JSON string. Aliases: `stringify`, `encode`. |

## Regex (`std.regex.*`)

Pattern syntax follows the `regex` crate (Rust-flavored). Patterns that fail
to compile return a benign default (empty replacement, `false` for `is_match`,
`None` for `captures`); use `try_…` variants once added if you need loud errors.

| Signature | Description |
|-----------|-------------|
| `fn replace(haystack: str, pattern: str, replacement: str) to str` | Replaces all matches of `pattern` in `haystack` with `replacement`. |
| `fn is_match(haystack: str, pattern: str) to bool` | Whether `pattern` matches anywhere in `haystack`. |
| `fn captures(haystack: str, pattern: str) to Option[list[str]]` | Returns the first match as `Some([full, group_1, group_2, ...])`, or `None` if no match. |

## Strings and lists are method-only

Vox operations on strings and lists are accessed via **method syntax**, never
free-function namespace calls. This is a deliberate K-complexity choice:
one canonical form per operation, matching the Python / Rust / Swift prior.

```vox
// vox:skip
let trimmed = s.trim();              // ✓ canonical
let trimmed = str.trim(s);           // ✗ parse-time / eval-time error with hint

xs.push(item);                       // ✓ canonical
list.push(xs, item);                 // ✗ parse-time / eval-time error with hint
```

Free-function namespaces (`fs.*`, `path.*`, `regex.*`, `process.*`, `env.*`,
`json.*`, `log.*`, etc.) remain canonical for stateless utilities with no
natural receiver.

## Cryptography and UUID (`std.*`)

These are top-level `std.` calls (not `std.crypto.*`) — the binary
registers them directly under the `std` namespace. Available in the
native script-execution mode (via `vox-actor-runtime`); not exposed in
the tree-walking interpreter.

| Signature | Description |
|-----------|-------------|
| `fn hash_fast(s: str) to str` | Fast, non-cryptographic hash. |
| `fn hash_secure(s: str) to str` | Secure cryptographic hash (SHA-256). |
| `fn uuid() to str` | Generates a UUID v4 string. |

## Time (`std.*`)

| Signature | Description |
|-----------|-------------|
| `fn now_ms() to int` | Returns current UNIX timestamp in milliseconds. |

## Logging (`std.log.*`)

| Signature | Description |
|-----------|-------------|
| `fn debug(msg: str) to Unit` | Logs a debug message. |
| `fn info(msg: str) to Unit` | Logs an info message. |
| `fn warn(msg: str) to Unit` | Logs a warning message. |
| `fn error(msg: str) to Unit` | Logs an error message. |

## OpenClaw Invocation (`OpenClaw.*`)

| Signature | Description |
|-----------|-------------|
| `fn list_skills() to Result[str]` | Lists available OpenClaw skills. |
| `fn call(skill: str, args: str) to Result[str]` | Invokes an OpenClaw skill. |
| `fn subscribe(topic: str) to Result[str]` | Subscribes to an OpenClaw topic. |
| `fn unsubscribe(topic: str) to Result[str]` | Unsubscribes from an OpenClaw topic. |
| `fn notify(topic: str, msg: str) to Result[str]` | Notifies an OpenClaw topic. |

## CDP System Automation (`Browser.*`)

*Note: These are native-script only (not available when compiled to WASM).*

| Signature | Description |
|-----------|-------------|
| `fn open() to Result[Unit]` | Opens the default automation browser. |
| `fn close() to Result[Unit]` | Closes the automation browser. |
| `fn goto(url: str) to Result[Unit]` | Navigates to a specific URL. |
| `fn click(selector: str) to Result[Unit]` | Clicks on the DOM element matched by selector. |
| `fn fill(selector: str, value: str) to Result[Unit]` | Fills a DOM element with a text value. |
| `fn wait_for(selector: str) to Result[Unit]` | Waits for a selector to appear on the page. |
| `fn text(selector: str) to Result[str]` | Returns the inner text of an element. |
| `fn html(selector: str) to Result[str]` | Returns the inner HTML of an element. |
| `fn screenshot(path: str) to Result[Unit]` | Takes a screenshot and saves it to the path. |

## Network (`std.http.*`)

| Signature | Description |
|-----------|-------------|
| `fn get_text(url: str) to Result[str]` | Submits an HTTP GET request to the target URL and returns the response body as text. |
| `fn post_json(url: str, body: str) to Result[str]` | Submits an HTTP POST request to the target URL with the provided JSON body string. |

---

**Related Topics**:
- [Reference: Database Query Surface](ref-db-surface.md)
- [Explanation: The Runtime](../explanation/expl-runtime.md)
