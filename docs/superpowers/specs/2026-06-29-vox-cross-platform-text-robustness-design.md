# Vox Cross-Platform Text Robustness — Design (v2, adversarially audited)

**Date:** 2026-06-29
**Status:** Design (approved direction; hardened against codebase audit)
**Charter:** A `.vox` script authored on any OS compiles and runs identically on
Windows, macOS, and Linux — no hand-fixing line endings, BOMs, encodings, or
console mojibake. Vox is **UTF-8 + LF everywhere, by definition**, on **both
execution backends**.

## 1. Philosophy

Two layers people conflate; we fix both, separately:

- **Layer A — Source files** (`.vox` on disk, compile time).
- **Layer B — Program runtime I/O** (what a running program prints / reads / writes).

**Decided philosophy: maximally opinionated, narrow opt-in escape hatches.**
Python bled for ~15 years from configurable, platform-dependent defaults
(`open()` → locale encoding → "silent data corruption", finally fixed by
[PEP 686](https://peps.python.org/pep-0686/)). Go/Rust/Deno avoided the whole
bug class by being opinionated from day one. We take that bet: correct defaults,
**no ambient text config**, raw bytes only when explicitly requested per call.

## 2. Adversarial Audit Corrections (what v1 got wrong)

v1 of this spec was checked against the code by four parallel read-only audits.
It contained **four false positives and missed a real bug**. Recording them so
reviewers trust the corrected design:

1. **FALSE: "the lexer breaks on CRLF, so CRLF files won't compile."** The lexer
   already tolerates CRLF — newline regex is `\n|\r\n`
   ([lexer/token.rs:525](../../../crates/vox-compiler/src/lexer/token.rs)), bare
   `\r` and a leading BOM are silently skipped as unrecognized chars
   ([lexer/cursor.rs:54](../../../crates/vox-compiler/src/lexer/cursor.rs)), and
   indentation is **non-significant** (brace-delimited). The *real* residual
   issue is narrow: a multi-line **string literal** in CRLF source carries `\r`
   into its runtime value, and spans drift. Layer A is reframed accordingly —
   it's about *deterministic string-literal values & spans*, not "won't compile".

2. **FALSE: "`run_frontend_str_with_options` (pipeline.rs:110) is the single
   compile chokepoint."** There are **15+ direct `lexer::lex()` callers** (LSP,
   `fmt`, typeck, eval, codegen-ts, MCP, migrate, db). The true single seam is
   `lexer::lex()` itself
   ([lexer/cursor.rs:20](../../../crates/vox-compiler/src/lexer/cursor.rs)).

3. **GAP: v1 patched only the `--interp` path.** `vox run` defaults to **native
   codegen** (compiles a Rust binary); the `builtins.rs` `print`/`fs.read`
   builtins are only `--interp`. Fixing one backend leaves the other wrong. Both
   backends must be covered, ideally through a shared runtime SSOT.

4. **GAP/BUG: native `read_bytes` is already broken.** Native codegen emits
   `::std::fs::read(...)` → `Result<Vec<u8>>`
   ([builtin_registry.rs:923](../../../crates/vox-compiler/src/builtin_registry.rs)),
   but the typecheck signature is `read_bytes: (str) -> Result[str,str]`
   ([builtin_registry.rs:572](../../../crates/vox-compiler/src/builtin_registry.rs)).
   A type mismatch that this work must fix, not paper over.

## 3. The Two-Backend Reality & the SSOT

`vox run` has two backends that must agree:

- **Native (default):** compiles to Rust. Builtins emit calls into
  **`vox_actor_runtime::builtins::vox_fs_*`** (verified: `fs.read` already routes
  through `vox_fs_read`; `mkdir`, `glob`, `http_*`, `now_ms` likewise).
- **Interpreter (`--interp`):** `vox-compiler/src/eval/builtins.rs`, which today
  **re-implements** I/O inline with `std::fs` — a parallel copy that already
  drifted (read_bytes).

**Design principle (AI-first, anti-drift):** the *normalization rule* lives in
**one pure function**, and the **native read path is fixed in its single SSOT
function** (`vox_fs_read` / a new `vox_fs_read_bytes`). The interpreter applies
the *same pure rule* inline and a **parity test** locks the two backends
together. We do **not** couple the whole `vox-compiler` crate (lexer, typeck) to
the heavy `vox_actor_runtime`; we share the leaf helper instead. (Dep direction
verified safe: `vox_actor_runtime` does **not** depend on `vox-compiler`.)

## 4. Decided Behavior (applies to BOTH backends)

| Concern | Default | Escape hatch |
|---|---|---|
| Source BOM / CRLF / lone CR | normalized to BOM-free LF before lexing (in `lex()`, not `lex_preserving()`) | n/a |
| `print` / any stdout on Windows console | console code page forced to UTF-8 once at startup | n/a (no-op when redirected) |
| `fs.read` / `read_file` / `read_to_string` | strip BOM, CRLF/CR → LF | `fs.read_bytes()` |
| `fs.read_bytes` | exact bytes, BOM/CR preserved; **errors** on non-UTF-8 (Vox has no `Bytes` value — verified) | n/a (it *is* the hatch) |
| `fs.write` / `write_file` / `write_to_file` | emit bytes exactly as given — no newline/BOM transform | n/a (already raw) |

**Why read-normalizes but write-preserves:** read normalization makes scripts
"just work"; preserve-on-write means we never silently rewrite a file's endings
on round-trip. The one program that round-trips bytes (a formatter / in-place
editor) opts into `read_bytes()`.

## 5. Architecture — One Pure Helper + Four Wirings

**`normalize_text(String) -> String`** — strip one leading UTF-8 BOM; convert
CRLF and lone CR to LF. Pure, allocation-light, **idempotent**
(`normalize_text(normalize_text(x)) == normalize_text(x)`). Home:
**`vox-bounded-fs`** (existing low leaf; deps are only `anyhow` +
`vox-scaling-policy`, so both the compiler lexer and the runtime can depend on it
without heavy coupling).

### Seam R1 — native + interp `fs.read` normalize (shared rule)
- Native SSOT: `vox_actor_runtime::builtins::vox_fs_read`
  ([builtins/mod.rs ~1724](../../../crates/vox-actor-runtime/src/builtins/mod.rs))
  wraps its result in `normalize_text`. Fixes the entire native read path in one
  spot (codegen already calls it).
- Interp: `builtins.rs:979` wraps its `read_to_string` result in the same
  `normalize_text`.

### Seam R2 — `read_bytes` becomes byte-exact on both backends (+ fixes the bug)
- Add `vox_actor_runtime::builtins::vox_fs_read_bytes(path) -> Result<String,String>`
  using `String::from_utf8` (errors on non-UTF-8; preserves BOM/CR exactly).
- Repoint native codegen at it:
  [builtin_registry.rs:923](../../../crates/vox-compiler/src/builtin_registry.rs)
  emits `::vox_actor_runtime::builtins::vox_fs_read_bytes(...)` instead of inline
  `::std::fs::read(...)` — this resolves the existing `Vec<u8>` vs `String` type
  mismatch.
- Interp: `builtins.rs:992` uses `String::from_utf8` (drops `from_utf8_lossy`).

### Seam S — source normalization at the lexer (reframed benefit)
- `lexer::lex()` ([cursor.rs:20](../../../crates/vox-compiler/src/lexer/cursor.rs))
  normalizes its `source` via `normalize_text` before tokenizing. Covers all 15+
  lex callers uniformly. **`lex_preserving()` stays raw** — the formatter relies
  on byte-preservation; it sets its own newline policy.
- Benefit (honest): platform-independent **string-literal values** and **spans**,
  not a compile fix (the lexer already compiles CRLF/BOM).

### Seam C — Windows console UTF-8, set once (per-console, shared by children)
- The console output code page is a property of the **console**, inherited by
  child processes. So setting it once in **`vox-cli`** (the `vox run` entry, which
  already depends on `windows-sys`) covers **both** `--interp` (same process) and
  the spawned **native** child (shared console). When stdout is redirected to a
  pipe/file, the code page is irrelevant and UTF-8 bytes pass through unchanged.
- Add `vox_actor_runtime::builtins::vox_console_init_utf8()` (cfg(windows):
  `SetConsoleOutputCP(CP_UTF8)`; no-op elsewhere). Call it once at `vox-cli`
  startup.
- **Completeness:** also emit a `vox_console_init_utf8()` call as the first
  statement of the generated `main()` (vox-codegen script preamble), so a
  standalone `vox build` binary run *without* `vox-cli` is correct too. Lower
  priority — `vox run` is already covered by the `vox-cli` call.
- ponytail ceiling: `SetConsoleOutputCP` + UTF-8 bytes covers redirect + the
  common console case. Full supplementary-plane emoji glyph correctness needs
  `WriteConsoleW` (both surrogate halves in one call) — deferred; upgrade path is
  to route output through a console writer if a real glyph bug appears.

### Parity & write
- `fs.write`/`write_file`/`write_to_file` share one impl on both backends and
  already pass bytes through unchanged — **no change**; a guard test locks it.
- **Parity test:** a CRLF+BOM fixture read by interp and by native produces the
  identical LF string; `read_bytes` produces identical exact bytes. Run via the
  existing parity-gate harness (`greaterfool_parity_gates`) where feasible.

### Born-correct hygiene (defense in depth, not the mechanism)
- Ship/verify `.gitattributes` (`*.vox text eol=lf`) and `.editorconfig`
  (`charset=utf-8`, `end_of_line=lf`, `insert_final_newline=true`).
- Keep the existing `vox-cli-ci` LF-only + no-BOM checks. **They must keep
  reading raw bytes** (via `read_utf8_path_capped`, *not* `normalize_text`), or
  they'd stop detecting violations. Their role shifts from "the compiler depends
  on this" to "keep diffs clean".

## 6. Why this is AI-first

- **Determinism across OS:** same `.vox` source → identical tokens, identical
  string-literal values, identical program output on every platform. An agent
  editing a file on Windows CI vs a human on macOS cannot produce divergent
  behavior or spurious diffs.
- **No silent corruption an agent can't see:** mojibake and stray `\r` are
  invisible-ish to an LLM reading tool output; eliminating them at the boundary
  prevents a class of "works on my machine" bugs agents can't diagnose.
- **Single SSOT prevents drift:** routing both backends through one rule (and one
  runtime function) is *why* the `read_bytes` parity bug existed — and fixing the
  structure stops the next one. Parity is enforced by test, not vigilance.

## 7. Out of Scope (this spec)

- **`VoxValue::Bytes`** (a real raw-bytes type). Today `read_bytes` returns a
  `Str`, so true arbitrary-binary round-trips are impossible; non-UTF-8 surfaces
  as an explicit error rather than corruption. A bytes type is a separate
  feature.
- **Legacy non-UTF-8 decoding** (`encoding:` arg). Hatch today: `read_bytes` +
  decode yourself. Build when a concrete case appears.
- **`stdin` normalization.** Add only on a real need.
- **Path separators** — handled by Rust `std::path`; no Vox-level work.

## 8. Risks

- **Reframed Seam S is low-value-ish.** Since the lexer already compiles CRLF/BOM
  and CI keeps in-repo files LF, Seam S only helps out-of-repo CRLF scripts'
  string literals. It's cheap and correct, but if it complicates `lex()` hot-path
  perf measurably, it can ship last or be gated. (normalize_text is a no-op fast
  path when there's no `\r`/BOM.)
- **Native read normalization changes behavior for programs that currently get
  CRLF back.** Intended per §4; `read_bytes` is the escape hatch. Zero real
  `read_bytes` callers exist today (verified), so the strict-UTF-8 flip is safe.
