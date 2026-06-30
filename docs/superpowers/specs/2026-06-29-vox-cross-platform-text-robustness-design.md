# Vox Cross-Platform Text Robustness — Design

**Date:** 2026-06-29
**Status:** Design (approved direction, pending spec review)
**Charter:** A `.vox` script authored on any OS compiles and runs identically on
Windows, macOS, and Linux without the user ever hand-fixing line endings, BOMs,
encodings, or console mojibake. Vox is **UTF-8 + LF everywhere, by definition.**

## 1. Problem & Philosophy

Two distinct layers get conflated; we fix both, deliberately separately:

- **Layer A — Source files** (`.vox` on disk, compile time). Today the lexer
  breaks on CRLF and CI *rejects* BOM files. A file authored on Windows can
  fail to compile.
- **Layer B — Program runtime I/O** (what a running Vox program prints / reads /
  writes). Today `print()` uses bare `println!` (no Windows console code page),
  and `fs.read` returns raw bytes (a `\r` or BOM silently poisons string
  comparisons).

**Philosophy (decided): maximally opinionated, narrow opt-in escape hatches.**
Python spent ~15 years bleeding from configurable, platform-dependent defaults
(`open()` defaulting to the locale encoding → "silent data corruption", finally
fixed by [PEP 686](https://peps.python.org/pep-0686/)). Go/Rust/Deno never had
that bug class because they were opinionated from day one. We take that bet:
**correct defaults, no ambient text config; the 1% that needs raw bytes has to
ask for them per call.** No global `[text]` config block — that *is* the disease.

## 2. Decided Behavior

| Concern | Default behavior | Escape hatch |
|---|---|---|
| Source `.vox` with BOM | stripped before lexing | n/a (always) |
| Source `.vox` with CRLF / lone CR | normalized to LF before lexing | n/a (always) |
| `print()` on Windows console | console forced to UTF-8; emoji/Unicode render | n/a (always-on, no-op when redirected) |
| `fs.read` / `read_to_string` | strip leading BOM, normalize CRLF/CR → LF in the returned string (Python universal-newlines) | `fs.read_bytes()` returns exact bytes |
| `fs.read_lines` / `.lines()` | each line has no trailing `\r` (falls out of read normalization; also stripped defensively) | `fs.read_bytes()` + manual split |
| `fs.write` | **emit exactly the bytes given** — no auto-newline conversion, no BOM | n/a (already raw) |

**Why normalize-on-read but preserve-on-write:** read normalization makes
scripts "just work" (a later `text == "ready"` never sees a stray `\r`).
Preserve-on-write means we never silently mangle a file's line endings on
round-trip. The only program that round-trips bytes (a formatter / in-place
editor) opts into `read_bytes()` and stays byte-exact — and Vox's own toolchain
paths will use `read_bytes()` where exactness matters.

## 3. Architecture — One Pure Helper, Wired at the Choke Points

All normalization is **one pure function**, unit-tested in isolation, reused by
both layers. No new crate.

```rust
// crates/vox-bounded-fs/src/lib.rs  (new pub fn; pure, no I/O)
/// Strip a leading UTF-8 BOM and normalize CRLF/CR line endings to LF.
/// Used at the compiler source-load seam and the runtime text-read seam.
pub fn normalize_text(s: String) -> String { ... }
```

`vox-bounded-fs` is the right home: it is already the workspace SSOT for file
reads (CLI, MCP, publisher, Populi). **Critically, `normalize_text` is NOT
folded into `read_utf8_path_capped`** — the CI line-ending/BOM checkers read via
that function and must see raw bytes. Normalization is applied only at the two
seams below.

### Seam 1 — Layer A: compiler source load
`crates/vox-cli/src/pipeline.rs:99` wraps the existing read:
`vox_bounded_fs::normalize_text(read_utf8_path_capped(file)?)`. The lexer
(`vox-compiler/src/lexer/cursor.rs`) now always receives BOM-free, LF-only
source. CRLF inside string literals also normalizes to LF — consistent with
"source is LF by definition," matches Go/Python source handling.

### Seam 2 — Layer B: runtime text read
`crates/vox-compiler/src/eval/builtins.rs:979` (`fs.read` / `read_to_string`)
returns `normalize_text(std::fs::read_to_string(..)?)`. `read_lines` strips a
trailing `\r` per line defensively.

### Seam 3 — Layer B: Windows console
New small `cfg(windows)` startup hook called once before any program output
(interpreter entry). Sets the console to UTF-8:

```rust
// ponytail: SetConsoleOutputCP(CP_UTF8) covers redirected-to-file and the
// common console case. Full surrogate-pair correctness for emoji glyphs needs
// WriteConsoleW (both halves in one call) — deferred; upgrade path is to route
// print() through a console writer if a real glyph-rendering bug shows up.
```

`print()` (`builtins.rs:2357`) is unchanged in shape — it keeps writing UTF-8
bytes; the code-page init makes them render. When stdout is a pipe/file the init
is a harmless no-op (redirected bytes were already UTF-8).

### Seam 4 — `read_bytes` must be genuinely raw
`fs.read_bytes` (`builtins.rs:992`) currently routes through
`String::from_utf8_lossy`, so it cannot return true binary — which undermines it
as the byte-exact escape hatch. Fix it to return real bytes (or add `read_raw`).
This is load-bearing: it is the *only* escape hatch for round-trip-exact work.

### Born-correct hygiene (defense in depth, not the mechanism)
- Ship/verify `.gitattributes` (`*.vox text eol=lf`) and `.editorconfig`
  (`charset = utf-8`, `end_of_line = lf`, `insert_final_newline = true`) so
  files are *created* clean.
- Keep the existing `vox-cli-ci` LF-only + no-BOM checks as repo hygiene. Their
  role shifts from "the compiler depends on this" to "keep diffs clean" — the
  compiler now *tolerates* violations rather than breaking on them.

## 4. Out of Scope (this spec)

- **Emitted/compiled binaries** (codegen path): a Vox program compiled to a Rust
  binary needs the same console init injected into its generated `main`. Real,
  but a follow-up — this spec covers the interpreter (`vox run`), which is how
  `.vox` scripts execute today. Noted as Phase 2.
- **`stdin` reading** (interactive input normalization) — add only if a concrete
  need appears. YAGNI.
- **Legacy non-UTF-8 file decoding** (`encoding:` argument). Not built until
  someone actually needs to read a Windows-1252 file. The escape hatch today is
  `read_bytes()` + decode-it-yourself.
- **Path separators** — already handled by Rust `std::path`; no Vox-level work.

## 5. Testing

- `normalize_text` unit tests (pure, in `vox-bounded-fs`): BOM-only, CRLF, lone
  CR, mixed, no-op clean string, BOM+CRLF together.
- Layer A golden: a `.vox` fixture saved with BOM+CRLF tokenizes/compiles
  identically to the clean LF twin.
- Layer B read: write a temp file with BOM+CRLF → `fs.read` returns LF-only,
  BOM-free; `fs.read_bytes` returns the exact original bytes (incl. `\r`, BOM).
- Layer B write: write a string with `\n` → read back raw bytes, assert no `\r`
  was inserted (preserve-on-write).
- Console: one `cfg(windows)` test asserting the output code page is 65001 after
  init; assert UTF-8 emoji bytes pass through unchanged when stdout is
  redirected to a pipe.

## 6. Implementation Order

1. `normalize_text` + its unit tests (pure, zero risk).
2. Seam 1 (compiler load) + Layer A golden — kills the "won't compile on
   Windows" class immediately.
3. Seam 2 + 4 (runtime read normalize, raw `read_bytes`) + read tests.
4. Seam 3 (Windows console UTF-8 init) + console test.
5. `.gitattributes` / `.editorconfig` born-correct hygiene.
