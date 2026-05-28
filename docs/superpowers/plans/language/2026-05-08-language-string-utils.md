# Language: String utilities (`split`, `slice`, `char_at`, `index_of`) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

> **amended (2026-05-08):** Scope reduced. `split`, `starts_with`, `ends_with` were already registered in `typeck/builtins.rs` and lowered in `eval/builtins.rs` before this plan; only `slice`, `char_at`, `index_of` were actually missing and were added. The `builtin_registry/**` step in "File Structure" did not apply — `Str` methods are registered directly in `typeck/builtins.rs` and not routed through `builtin_registry`. The `docs/src/reference/stdlib-str.md` reference does not exist in the repo; A5 deferred to a follow-up. `char_at` returns `Option[str]` (not `Optional[str]` — Vox uses `Option`); `index_of` uses char-index semantics (not byte-index) for cross-target consistency. TS codegen wraps `charAt`/`indexOf` to convert sentinel values (`""`, `-1`) to `null` for the Vox `Option` shape. Rust codegen lowers all three with char-aware logic (`chars().skip().take()`, `chars().nth()`, `find()` + char-count translation).

**Goal:** Add the missing common string helpers to Vox's `str` type:
- `split(separator: str) to List[str]`
- `slice(start: int, end: int) to str`
- `char_at(index: int) to Optional[str]` (returns single-char string or `None`)
- `index_of(needle: str) to Optional[int]`
- `starts_with(prefix: str) to bool`
- `ends_with(suffix: str) to bool`

**Why now:** Smaller than the other language plans, encountered during the vox-mental-tracker work. `replace`, `contains`, `to_lower`, and `length` already exist; the omissions above are the difference between writing readable transcript-classification code in Vox and writing chained `replace`/`contains` workarounds. Also useful in: CSV/JSON ad-hoc scanners (Phase 4 export work), URL parsing, content-type detection.

**Architecture:**
- Pure additions to `Str` method registration in `typeck/builtins.rs`.
- TS codegen: lower to native `String.prototype.{split, slice, charAt, indexOf, startsWith, endsWith}`.
- Rust codegen: lower to `&str` methods (`split`, `get`, `chars().nth`, `find`, `starts_with`, `ends_with`); collect into `Vec<String>` for `split`.
- No new types.

**Tech Stack:** Rust (compiler builtin registration + codegen).

**Out of scope:**
- Regex-based split (covered by the regex stdlib plan).
- Multi-byte / grapheme-aware indexing (default is byte-position semantics for Rust path; UTF-16 code units for JS path; document the gap and mark gracefully).

---

## File Structure

**Modified:**
- `crates/vox-compiler/src/typeck/builtins.rs` — extend the `Str` methods map.
- `crates/vox-compiler/src/builtin_registry/**` — add entries.
- `crates/vox-compiler/src/codegen_ts/**` — short-circuit lowering per method.
- `crates/vox-compiler/src/codegen_rust/**` — short-circuit lowering per method.

**Created:**
- `examples/golden/str_utils.vox` covering each method.

---

## Tasks

- [ ] **A1.** Register the 6 methods on `Str` with the listed signatures.
- [ ] **A2.** TS codegen for each (one-liners).
- [ ] **A3.** Rust codegen for each (one-liners; `split` needs `.into_iter().map(String::from).collect::<Vec<_>>()`).
- [ ] **A4.** Golden example + snapshot.
- [ ] **A5.** Update `docs/src/reference/stdlib-str.md` to enumerate the new methods alongside the existing ones.

---

## Verification

- [ ] `cargo nextest run -p vox-compiler` passes.
- [ ] `vox check examples/golden/str_utils.vox` passes.
- [ ] Both emitted TS and Rust compile and produce identical results on the golden's expected outputs.
