# Vox Cross-Platform Text Robustness Implementation Plan (v2, audited)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `.vox` files compile and run identically on every OS — no manual CRLF/BOM/UTF-8/console fixes — across **both** execution backends (native codegen + interpreter), via one pure normalization rule routed through the runtime SSOT.

**Architecture:** A pure `normalize_text` (BOM strip + CRLF/CR→LF) lives in `vox-bounded-fs`. The native read path is fixed in its single SSOT (`vox_actor_runtime::builtins::vox_fs_read` + a new `vox_fs_read_bytes`); the interpreter applies the same rule inline; a parity test locks them together. Source normalization happens in `lexer::lex()`. The Windows console code page is set once in `vox-cli` (per-console, inherited by the spawned native child).

**Tech Stack:** Rust (edition 2024 — `unsafe extern` required; `unsafe_code = "warn"` workspace lint → annotate FFI with `#[allow(unsafe_code)]`). `windows-sys` is already a workspace dep. `cargo test -p <crate>`.

**Spec:** `docs/superpowers/specs/2026-06-29-vox-cross-platform-text-robustness-design.md`

**Verified codebase facts (from adversarial audit — do not re-litigate):**
- Lexer already tolerates CRLF/BOM (newline regex `\n|\r\n`; bare `\r`/BOM skipped; indentation non-significant). Seam S fixes **string-literal values + spans**, not "won't compile".
- `vox run` defaults to **native codegen**; `builtins.rs` is the `--interp` path. Both need fixing.
- Native `fs.read` already calls `vox_actor_runtime::builtins::vox_fs_read` ([builtin_registry.rs:880](../../../crates/vox-compiler/src/builtin_registry.rs)). Native `read_bytes` inlines `::std::fs::read` → `Vec<u8>` while the signature is `(str)->Result[str,str]` ([builtin_registry.rs:923, 572](../../../crates/vox-compiler/src/builtin_registry.rs)) — a **pre-existing type-mismatch bug** this plan fixes.
- `call_builtin_method(.., caps=None)` **allows** fs access; `VoxValue::Result` is `Result<Box<VoxValue>, Box<VoxValue>>`; no `Bytes` variant exists (all verified).
- Dep direction `vox-compiler → vox-actor-runtime` is acyclic (runtime does not depend on compiler), but we avoid that heavy edge — both crates depend on the leaf `vox-bounded-fs` for the shared rule instead.

**Cargo discipline (project memory):** never `cargo fmt --all` (use `cargo fmt -p <crate>`); never pipe `cargo` to `head`/`grep` (redirect to a file).

---

### Task 1: `normalize_text` pure helper in `vox-bounded-fs`

**Files:**
- Modify: `crates/vox-bounded-fs/src/lib.rs` (add `pub fn normalize_text` after line 35; tests in the existing `mod tests` at line 58)

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/vox-bounded-fs/src/lib.rs`:

```rust
#[test]
fn normalize_strips_leading_bom() {
    assert_eq!(normalize_text("\u{feff}hello".to_string()), "hello");
}
#[test]
fn normalize_crlf_to_lf() {
    assert_eq!(normalize_text("a\r\nb\r\n".to_string()), "a\nb\n");
}
#[test]
fn normalize_lone_cr_to_lf() {
    assert_eq!(normalize_text("a\rb".to_string()), "a\nb");
}
#[test]
fn normalize_bom_and_crlf_together() {
    assert_eq!(normalize_text("\u{feff}x\r\ny".to_string()), "x\ny");
}
#[test]
fn normalize_clean_string_is_noop() {
    assert_eq!(normalize_text("a\nb\n".to_string()), "a\nb\n");
}
#[test]
fn normalize_only_leading_bom_not_interior() {
    assert_eq!(normalize_text("a\u{feff}b".to_string()), "a\u{feff}b");
}
#[test]
fn normalize_is_idempotent() {
    let once = normalize_text("\u{feff}a\r\nb\rc".to_string());
    assert_eq!(normalize_text(once.clone()), once);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-bounded-fs normalize`
Expected: FAIL — `cannot find function 'normalize_text'`.

- [ ] **Step 3: Implement `normalize_text`**

Add after `read_utf8_path_capped` (after line 35) in `crates/vox-bounded-fs/src/lib.rs`:

```rust
/// Normalize source/text bytes for cross-platform consistency: strip one
/// leading UTF-8 BOM and convert CRLF/CR line endings to LF. Pure, idempotent,
/// and allocation-light (returns input unchanged when already clean). Shared by
/// the compiler lexer and the runtime text-read functions.
#[must_use]
pub fn normalize_text(s: String) -> String {
    let s = match s.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => s,
    };
    if !s.contains('\r') {
        return s;
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-bounded-fs normalize`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-bounded-fs/src/lib.rs
git commit -m "feat(bounded-fs): add normalize_text (BOM strip + CRLF/CR->LF)"
```

---

### Task 2: Native SSOT — `vox_fs_read` normalizes; add `vox_fs_read_bytes` (+ fix codegen bug)

**Files:**
- Modify: `crates/vox-actor-runtime/Cargo.toml` (add `vox-bounded-fs` dep)
- Modify: `crates/vox-actor-runtime/src/builtins/mod.rs` (`vox_fs_read` ~line 1724; add `vox_fs_read_bytes`)
- Modify: `crates/vox-compiler/src/builtin_registry.rs:923-926` (repoint native `read_bytes` codegen)

- [ ] **Step 1: Add the dependency**

In `crates/vox-actor-runtime/Cargo.toml` under `[dependencies]`:

```toml
vox-bounded-fs = { workspace = true }
```

(If the workspace doesn't define it, use `vox-bounded-fs = { path = "../vox-bounded-fs" }`.)

- [ ] **Step 2: Write the failing tests**

Add a test module to `crates/vox-actor-runtime/src/builtins/mod.rs` (end of file):

```rust
#[cfg(test)]
mod fs_text_robustness_tests {
    use super::*;

    #[test]
    fn vox_fs_read_normalizes_bom_and_crlf() {
        let dir = std::env::temp_dir().join("vox_rt_read_norm");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.txt");
        std::fs::write(&p, b"\xEF\xBB\xBFa\r\nb\r\n").unwrap();
        assert_eq!(vox_fs_read(p.to_str().unwrap()).unwrap(), "a\nb\n");
    }

    #[test]
    fn vox_fs_read_bytes_is_byte_exact() {
        let dir = std::env::temp_dir().join("vox_rt_read_raw");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("b.txt");
        std::fs::write(&p, b"\xEF\xBB\xBFa\r\nb\r\n").unwrap();
        assert_eq!(vox_fs_read_bytes(p.to_str().unwrap()).unwrap(), "\u{feff}a\r\nb\r\n");
    }

    #[test]
    fn vox_fs_read_bytes_errors_on_non_utf8() {
        let dir = std::env::temp_dir().join("vox_rt_read_badutf8");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("c.bin");
        std::fs::write(&p, [0xFF, 0xFE, 0x00]).unwrap();
        assert!(vox_fs_read_bytes(p.to_str().unwrap()).is_err());
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-actor-runtime fs_text_robustness`
Expected: FAIL — `vox_fs_read` returns un-normalized bytes; `vox_fs_read_bytes` does not exist.

- [ ] **Step 4: Normalize `vox_fs_read` and add `vox_fs_read_bytes`**

In `crates/vox-actor-runtime/src/builtins/mod.rs`, change `vox_fs_read` (~line 1724) from:

```rust
pub fn vox_fs_read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}
```

to:

```rust
pub fn vox_fs_read(path: &str) -> Result<String, String> {
    // Universal-newlines read: strip BOM, CRLF/CR -> LF. Byte-exact round-trips
    // use `vox_fs_read_bytes`.
    std::fs::read_to_string(path)
        .map(vox_bounded_fs::normalize_text)
        .map_err(|e| e.to_string())
}

/// Byte-exact text read (`std.fs.read_bytes`): preserves BOM/CR. Vox has no
/// Bytes value, so a non-UTF-8 file surfaces as an error rather than corruption
/// (do NOT use `from_utf8_lossy`).
pub fn vox_fs_read_bytes(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| format!("read_bytes: {path}: invalid UTF-8: {e}"))
}
```

- [ ] **Step 5: Repoint native codegen for `read_bytes`**

In `crates/vox-compiler/src/builtin_registry.rs`, replace lines 923-926:

```rust
("fs", "read_bytes") if !args.is_empty() => Some(format!(
    "::std::fs::read({}).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)",
    args[0]
)),
```

with (mirrors the `vox_fs_read` emit at line 880, fixing the `Vec<u8>` vs `String` mismatch):

```rust
("fs", "read_bytes") if !args.is_empty() => Some(format!(
    "::vox_actor_runtime::builtins::vox_fs_read_bytes(({}).as_str())",
    args[0]
)),
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p vox-actor-runtime fs_text_robustness`
Expected: PASS (3 tests).
Run: `cargo build -p vox-compiler`
Expected: builds (codegen string change is syntactically valid).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-actor-runtime/Cargo.toml crates/vox-actor-runtime/src/builtins/mod.rs crates/vox-compiler/src/builtin_registry.rs
git commit -m "feat(runtime): vox_fs_read normalizes; add byte-exact vox_fs_read_bytes (+fix native codegen type mismatch)"
```

---

### Task 3: Interpreter parity — `fs.read` normalize; `read_bytes` strict UTF-8

**Files:**
- Modify: `crates/vox-compiler/Cargo.toml` (add `vox-bounded-fs` dep — shared with Task 4)
- Modify: `crates/vox-compiler/src/eval/builtins.rs:979` (`fs.read`) and `:992-994` (`read_bytes`)
- Test: new `#[cfg(test)]` module in the same file (mirrors `time_namespace_interp_tests` at line 2655; `caps=None` allows fs — verified)

- [ ] **Step 1: Add the dependency**

In `crates/vox-compiler/Cargo.toml` under `[dependencies]`:

```toml
vox-bounded-fs = { workspace = true }
```

- [ ] **Step 2: Write the failing tests**

Add to `crates/vox-compiler/src/eval/builtins.rs`:

```rust
#[cfg(test)]
mod fs_text_robustness_tests {
    use super::*;

    fn fs_namespace() -> VoxValue {
        VoxValue::object(vec![(
            "__namespace__".to_string(),
            VoxValue::Str("fs".to_string()),
        )])
    }
    fn result_ok_str(v: Option<VoxValue>) -> String {
        match v {
            Some(VoxValue::Result(Ok(boxed))) => match *boxed {
                VoxValue::Str(s) => s,
                other => panic!("expected Str, got {other:?}"),
            },
            other => panic!("expected Result(Ok(Str)), got {other:?}"),
        }
    }

    #[test]
    fn interp_fs_read_normalizes() {
        let dir = std::env::temp_dir().join("vox_interp_read_norm");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.txt");
        std::fs::write(&p, b"\xEF\xBB\xBFa\r\nb\r\n").unwrap();
        let got = result_ok_str(call_builtin_method(
            &fs_namespace(), "read",
            vec![VoxValue::Str(p.to_string_lossy().to_string())], None,
        ));
        assert_eq!(got, "a\nb\n");
    }

    #[test]
    fn interp_fs_read_bytes_is_byte_exact() {
        let dir = std::env::temp_dir().join("vox_interp_read_raw");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("b.txt");
        std::fs::write(&p, b"\xEF\xBB\xBFa\r\nb\r\n").unwrap();
        let got = result_ok_str(call_builtin_method(
            &fs_namespace(), "read_bytes",
            vec![VoxValue::Str(p.to_string_lossy().to_string())], None,
        ));
        assert_eq!(got, "\u{feff}a\r\nb\r\n");
    }

    #[test]
    fn interp_fs_write_preserves_no_cr_inserted() {
        let dir = std::env::temp_dir().join("vox_interp_write_preserve");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("c.txt");
        let _ = call_builtin_method(
            &fs_namespace(), "write",
            vec![
                VoxValue::Str(p.to_string_lossy().to_string()),
                VoxValue::Str("x\ny\n".to_string()),
            ], None,
        );
        assert_eq!(std::fs::read(&p).unwrap(), b"x\ny\n");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-compiler fs_text_robustness`
Expected: FAIL — `interp_fs_read_normalizes` (returns `"\u{feff}a\r\nb\r\n"`). (`write` test already passes; `read_bytes` test passes today by luck since BOM/CR are valid UTF-8 — it locks the contract.)

- [ ] **Step 4: Normalize `fs.read`; make `read_bytes` strict**

In `crates/vox-compiler/src/eval/builtins.rs`, change `fs.read` (line 979) from:

```rust
let res = match std::fs::read_to_string(path) {
    Ok(s) => Ok(Box::new(VoxValue::Str(s))),
    Err(e) => Err(e.to_string()),
};
```

to (same rule as native `vox_fs_read`):

```rust
let res = match std::fs::read_to_string(path) {
    Ok(s) => Ok(Box::new(VoxValue::Str(vox_bounded_fs::normalize_text(s)))),
    Err(e) => Err(e.to_string()),
};
```

Change `read_bytes` (lines 992-994) from:

```rust
let res = match std::fs::read(&path) {
    Ok(bytes) => Ok(Box::new(VoxValue::Str(
        String::from_utf8_lossy(&bytes).to_string(),
    ))),
    Err(e) => Err(e.to_string()),
};
```

to:

```rust
// Byte-exact escape hatch: preserve BOM/CR; error on non-UTF-8 (no Bytes value).
let res = match std::fs::read(&path) {
    Ok(bytes) => match String::from_utf8(bytes) {
        Ok(s) => Ok(Box::new(VoxValue::Str(s))),
        Err(e) => Err(format!("read_bytes: {path}: invalid UTF-8: {e}")),
    },
    Err(e) => Err(e.to_string()),
};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vox-compiler fs_text_robustness`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/Cargo.toml crates/vox-compiler/src/eval/builtins.rs
git commit -m "feat(interp): fs.read normalize + byte-exact read_bytes (native parity)"
```

---

### Task 4: Seam S — normalize source in `lexer::lex()` (string-literal/span determinism)

**Files:**
- Modify: `crates/vox-compiler/src/lexer/cursor.rs:20` (`lex` — normalize before tokenizing; leave `lex_preserving` raw)

- [ ] **Step 1: Write the failing tests**

Add to the `cursor.rs` test module (or create `#[cfg(test)] mod text_norm_tests`):

```rust
#[test]
fn lex_normalizes_crlf_and_bom_equivalently() {
    let dirty = "\u{feff}let x = 1\r\nlet y = 2\r\n";
    let clean = "let x = 1\nlet y = 2\n";
    let td: Vec<_> = lex(dirty).into_iter().map(|s| s.token).collect();
    let tc: Vec<_> = lex(clean).into_iter().map(|s| s.token).collect();
    assert_eq!(td, tc, "CRLF+BOM source must lex to the same tokens as LF");
}

#[test]
fn lex_normalizes_string_literal_contents() {
    // A multi-line string literal authored with CRLF must carry LF, not \r.
    let toks = lex("let s = \"a\r\nb\"");
    let has_cr = toks.iter().any(|s| format!("{:?}", s.token).contains('\r'));
    assert!(!has_cr, "string-literal token must not retain \\r");
}

#[test]
fn lex_preserving_keeps_raw_cr() {
    // The formatter relies on byte preservation — lex_preserving must NOT strip \r.
    let toks = lex_preserving("let s = \"a\r\nb\"");
    let has_cr = toks.iter().any(|s| format!("{:?}", s.token).contains('\r'));
    assert!(has_cr, "lex_preserving must retain raw \\r for the formatter");
}
```

(If `Token`'s `Debug` does not surface the literal's bytes, adapt the assertion to the project's token-value accessor — but keep the LF-vs-CR distinction between `lex` and `lex_preserving`.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-compiler -- lex_normalizes`
Expected: `lex_normalizes_string_literal_contents` FAILS (token retains `\r`); the token-equivalence test may already pass (newlines tokenize either way) — the string-literal test is the real guard.

- [ ] **Step 3: Normalize inside `lex` only**

In `crates/vox-compiler/src/lexer/cursor.rs`, at the top of `lex` (line 20):

```rust
pub fn lex(source: &str) -> Vec<Spanned> {
    // Seam S: BOM-free, LF-only source so string-literal values and spans are
    // platform-independent. lex_preserving stays raw (formatter byte contract).
    let normalized = vox_bounded_fs::normalize_text(source.to_owned());
    let source = normalized.as_str();
    // ...existing body, now using the normalized `source`...
```

Leave `lex_preserving` (line 43) unchanged.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-compiler -- lex_normalizes lex_preserving`
Expected: PASS (3 tests).

- [ ] **Step 5: Run the lexer + formatter suites for regressions**

Run: `cargo test -p vox-compiler lexer` then `cargo test -p vox-compiler fmt`
Expected: PASS (normalization must not disturb existing lexer/formatter goldens; if a formatter golden changes, that's a real signal — investigate, do not blindly accept).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/lexer/cursor.rs
git commit -m "feat(lexer): normalize BOM/CRLF in lex() for platform-independent literals/spans"
```

---

### Task 5: Seam C — Windows console UTF-8, set once in `vox-cli`

**Files:**
- Modify: `crates/vox-actor-runtime/Cargo.toml` (windows-sys with Console feature, cfg(windows))
- Modify: `crates/vox-actor-runtime/src/builtins/mod.rs` (add `vox_console_init_utf8`)
- Modify: `crates/vox-cli/src/main.rs` (call it first in `main`)
- (Secondary) Modify: vox-codegen script `main` preamble (emit the same call)

- [ ] **Step 1: Add the dependency**

In `crates/vox-actor-runtime/Cargo.toml`:

```toml
[target.'cfg(windows)'.dependencies]
windows-sys = { workspace = true, features = ["Win32_System_Console"] }
```

- [ ] **Step 2: Write the idempotency test**

Add to the `fs_text_robustness_tests` module (or its own) in `crates/vox-actor-runtime/src/builtins/mod.rs`:

```rust
#[test]
fn console_init_is_idempotent_and_safe() {
    // No panic; safe to call repeatedly (Once-guarded). No-op off Windows.
    // The real emoji-rendering check is the manual Windows smoke test (Step 5).
    vox_console_init_utf8();
    vox_console_init_utf8();
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-actor-runtime console_init_is_idempotent`
Expected: FAIL — `cannot find function 'vox_console_init_utf8'`.

- [ ] **Step 4: Implement `vox_console_init_utf8`**

Add to `crates/vox-actor-runtime/src/builtins/mod.rs`:

```rust
/// Force the Windows console output code page to UTF-8 so program output renders
/// Unicode/emoji instead of mojibake. The code page is a property of the console
/// (inherited by child processes), so one call from the `vox run` entry covers
/// both the interpreter and the spawned native binary. Idempotent; no-op off
/// Windows and a harmless no-op when stdout is redirected (bytes were UTF-8).
//
// ponytail: SetConsoleOutputCP covers redirect + the common console case. Full
// supplementary-plane emoji glyph correctness needs WriteConsoleW (both
// surrogate halves in one call) — deferred; upgrade path is a console writer.
#[cfg_attr(windows, allow(unsafe_code))]
pub fn vox_console_init_utf8() {
    #[cfg(windows)]
    {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            const CP_UTF8: u32 = 65001;
            // SAFETY: single FFI call with a constant code page; ignore failure
            // (e.g. no console attached / output redirected).
            unsafe {
                let _ = windows_sys::Win32::System::Console::SetConsoleOutputCP(CP_UTF8);
            }
        });
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-actor-runtime console_init_is_idempotent`
Expected: PASS.

- [ ] **Step 6: Call it once at `vox-cli` startup**

Confirm vox-cli depends on vox-actor-runtime: `grep -n actor-runtime crates/vox-cli/Cargo.toml` (expected: present). Then in `crates/vox-cli/src/main.rs`, as the **first statement** of `main` (find with `grep -n "fn main" crates/vox-cli/src/main.rs`):

```rust
// Render Unicode/emoji correctly on the Windows console for both `--interp`
// (this process) and the spawned native binary (shared console code page).
::vox_actor_runtime::builtins::vox_console_init_utf8();
```

- [ ] **Step 7: Build + manual Windows smoke test (the real verification)**

Run: `cargo build -p vox-cli`
Then on Windows, in Windows Terminal: create `emoji.vox` with `print("café 🚀")` and run `vox run emoji.vox` (and also `vox run --interp emoji.vox`).
Expected: `café 🚀` renders (not `cafÃ© ??`). Legacy `conhost` with a non-emoji font may still box-glyph the emoji — Windows Terminal is the supported target. Record the result in the PR description.

- [ ] **Step 8 (secondary): standalone binaries**

So that a `vox build` binary run *without* `vox-cli` is also correct, emit `::vox_actor_runtime::builtins::vox_console_init_utf8();` as the first statement of the generated `main()`. Find the script-main template: `grep -rn "fn main" crates/vox-codegen/src | grep -i script` (the preamble generator, per audit: `vox-codegen` `generate_script`). Add a focused codegen test asserting the emitted source contains `vox_console_init_utf8`. If the generator is non-obvious, leave a `ponytail:` TODO referencing this step rather than guessing — `vox run` is already covered by Step 6.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-actor-runtime/Cargo.toml crates/vox-actor-runtime/src/builtins/mod.rs crates/vox-cli/src/main.rs
git commit -m "feat(cli): force Windows console UTF-8 once at startup (covers interp + native child)"
```

---

### Task 6: Backend parity gate

**Files:**
- Use: existing parity harness (`crates/vox-integration-tests/tests/greaterfool_parity_gates_test.rs` and siblings)

- [ ] **Step 1: Run the existing parity + pipeline suites**

Run: `cargo test -p vox-integration-tests` (redirect to a file if output is large: `cargo test -p vox-integration-tests > parity.txt 2>&1`)
Expected: PASS. The read/read_bytes changes touch both backends through the same rule; this gate confirms interp and native still agree. Any snapshot that legitimately changed (e.g. the previously-broken native `read_bytes` emit) should be reviewed and accepted with `cargo insta review` if the project uses insta.

- [ ] **Step 2 (if a fixture is warranted): add a CRLF round-trip fixture**

If the parity harness supports adding a `.vox` fixture, add one that reads a CRLF+BOM temp file and prints its length / first line, exercised under both backends, asserting identical output. Follow the harness's existing fixture pattern exactly — do not invent a new harness. If the harness can't easily host file-I/O fixtures, the unit tests in Tasks 2–3 (same rule on both backends) plus Step 1 are sufficient; note that here rather than fabricating a fixture.

- [ ] **Step 3: Commit (if anything changed)**

```bash
git add -A
git commit -m "test: backend parity gate for cross-platform text reads"
```

---

### Task 7: Born-correct hygiene — `.gitattributes` + `.editorconfig`

**Files:**
- Create or verify: `.gitattributes`, `.editorconfig` (repo root)

- [ ] **Step 1: Check what exists**

Run: `git ls-files .gitattributes .editorconfig`
If both already enforce LF + UTF-8 for `.vox`, note "already present" and skip to Step 4.

- [ ] **Step 2: `.gitattributes`** — ensure it contains (append/create):

```gitattributes
# Vox sources are LF-only, UTF-8, no BOM — checked in normalized.
*.vox text eol=lf
```

- [ ] **Step 3: `.editorconfig`** — ensure (append a `[*.vox]` section, or create with a root block):

```editorconfig
root = true

[*.vox]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true
```

- [ ] **Step 4: Verify no mass churn**

Run: `git diff --stat`
Expected: only `.gitattributes` / `.editorconfig` changed (repo is already LF per the `vox-cli-ci` check; no re-normalization storm).

- [ ] **Step 5: Commit**

```bash
git add .gitattributes .editorconfig
git commit -m "chore: born-correct LF/UTF-8 hygiene for .vox"
```

---

## Notes for the implementer

- **Do NOT route the CI checks through `normalize_text`.** `crates/vox-cli-ci/src/line_endings.rs` (LF-only + no-BOM) reads raw via `read_utf8_path_capped` and must keep seeing raw bytes, or it stops detecting violations. Normalization is applied only at the seams in Tasks 2–4.
- **Order matters:** Task 1 (helper) → Task 2 (native, the default path) → Task 3 (interp parity) → Task 4 (source) → Task 5 (console) → Task 6 (parity gate) → Task 7 (hygiene). Each task is independently committable and leaves the tree green.
- **Deferred (separate work, per spec §7):** a real `VoxValue::Bytes` type; legacy non-UTF-8 decoding (`encoding:` arg); stdin normalization; `WriteConsoleW` for supplementary-plane emoji glyphs.
