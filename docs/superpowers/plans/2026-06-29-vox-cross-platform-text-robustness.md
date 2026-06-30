# Vox Cross-Platform Text Robustness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `.vox` files authored on any OS compile and run identically — no manual CRLF/BOM/UTF-8/console fixes — by adding one pure normalization helper wired at two seams, plus a Windows console UTF-8 init.

**Architecture:** A single pure `normalize_text` (strip leading BOM + CRLF/CR→LF) lives in `vox-bounded-fs` and is reused at (1) the compiler's source-string entry and (2) the runtime text-read builtin. Reads normalize; writes preserve bytes exactly; `read_bytes` is the byte-exact escape hatch. The Windows console is forced to UTF-8 once on first `print`.

**Tech Stack:** Rust, `cargo test -p <crate>`. Windows console via a tiny `extern "system"` FFI (no new dependency). No global text config — opinionated defaults only.

**Spec:** `docs/superpowers/specs/2026-06-29-vox-cross-platform-text-robustness-design.md`

**Refinement vs spec:** Seam 1 is applied in `run_frontend_str_with_options` (pipeline.rs:110), not the file-read line (pipeline.rs:99). This covers file reads *and* in-memory sources (stdin/MCP/tests/embedded) and is synchronously testable.

**Windows-gating note:** Tasks 1–3 and 5 are OS-neutral and run on any platform. Task 4 (console) is `cfg(windows)`; its automated test only proves the call is idempotent and panic-free — the meaningful check is a manual emoji smoke test on Windows (called out in the task).

**Cargo discipline (from project memory):** never `cargo fmt --all` (use `cargo fmt -p <crate>`); never pipe `cargo` to `head`/`grep` (redirect to a file if needed).

---

### Task 1: `normalize_text` pure helper in `vox-bounded-fs`

**Files:**
- Modify: `crates/vox-bounded-fs/src/lib.rs` (add `pub fn normalize_text` after `read_utf8_path_capped`, ~line 35; add tests in the existing `#[cfg(test)] mod tests` at line 58)

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
    // A BOM mid-string is a real ZWNBSP and must be preserved.
    assert_eq!(normalize_text("a\u{feff}b".to_string()), "a\u{feff}b");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-bounded-fs normalize`
Expected: FAIL — `cannot find function 'normalize_text'`.

- [ ] **Step 3: Implement `normalize_text`**

Add after `read_utf8_path_capped` (after line 35) in `crates/vox-bounded-fs/src/lib.rs`:

```rust
/// Normalize source/text bytes for cross-platform consistency:
/// strip a single leading UTF-8 BOM and convert CRLF/CR line endings to LF.
/// Pure and allocation-light (returns input unchanged when already clean).
/// Reused at the compiler source-string entry and the runtime text-read seam.
#[must_use]
pub fn normalize_text(s: String) -> String {
    let s = match s.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => s,
    };
    if !s.contains('\r') {
        return s;
    }
    // Convert CRLF and lone CR to LF in one pass.
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
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-bounded-fs/src/lib.rs
git commit -m "feat(bounded-fs): add normalize_text (BOM strip + CRLF/CR->LF)"
```

---

### Task 2: Seam 1 — compiler tolerates BOM/CRLF source

**Files:**
- Modify: `crates/vox-cli/src/pipeline.rs:110` (`run_frontend_str_with_options` — normalize the incoming source before lexing)
- Verify dep: `crates/vox-cli/Cargo.toml` already depends on `vox-bounded-fs` (it calls `read_utf8_path_capped`), so no new dependency.

- [ ] **Step 1: Write the failing test**

Add a test module at the end of `crates/vox-cli/src/pipeline.rs`:

```rust
#[cfg(test)]
mod text_robustness_tests {
    use super::*;
    use std::path::Path;

    /// A BOM + CRLF source must compile identically to its clean LF twin.
    /// Without Seam 1 the lexer breaks on `\r` (newlines are significant).
    #[test]
    fn bom_crlf_source_compiles() {
        let dirty = "\u{feff}let x = 1\r\nlet y = 2\r\n".to_string();
        let res = run_frontend_str(&dirty, Path::new("t.vox"), false);
        assert!(res.is_ok(), "BOM+CRLF source failed to compile: {res:?}");
    }
}
```

(If `run_frontend_str` requires valid Vox syntax different from the above, use the smallest valid program for this codebase — e.g. whatever a known-good fixture uses — but keep the `\u{feff}` prefix and `\r\n` endings.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli bom_crlf_source_compiles`
Expected: FAIL — lexer error from the stray `\r` / BOM.

- [ ] **Step 3: Normalize at the source-string entry**

In `crates/vox-cli/src/pipeline.rs`, at the top of `run_frontend_str_with_options` (line 110), before the source is used:

```rust
pub fn run_frontend_str_with_options(
    source: &str,
    file: &Path,
    json: bool,
    options: &PipelineOptions,
) -> Result<FrontendResult> {
    // Seam 1: every source string (file read, stdin, MCP, embedded, tests)
    // is BOM-free and LF-only before it reaches the lexer.
    let normalized = vox_bounded_fs::normalize_text(source.to_owned());
    let source = normalized.as_str();
    // ...existing body continues, now using the normalized `source`...
```

Ensure the rest of the function body references this shadowed `source`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli bom_crlf_source_compiles`
Expected: PASS.

- [ ] **Step 5: Run the broader pipeline suite for regressions**

Run: `cargo test -p vox-cli`
Expected: PASS (no regressions from the normalization).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/pipeline.rs
git commit -m "feat(pipeline): normalize BOM/CRLF at source entry (Seam 1)"
```

---

### Task 3: Seam 2 + Seam 4 — runtime read normalizes; `read_bytes` is byte-exact

**Files:**
- Modify: `crates/vox-compiler/src/eval/builtins.rs:979` (`fs.read`/`read_file`/`read_to_string` — normalize result)
- Modify: `crates/vox-compiler/src/eval/builtins.rs:992-994` (`fs.read_bytes` — stop using `from_utf8_lossy`)
- Verify dep: `crates/vox-compiler/Cargo.toml` must depend on `vox-bounded-fs`. If absent, add `vox-bounded-fs = { path = "../vox-bounded-fs" }` under `[dependencies]`.
- Test: new `#[cfg(test)]` module in the same file (mirror the existing `time_namespace_interp_tests` pattern at line 2655).

- [ ] **Step 1: Write the failing tests**

Add a test module near the existing interp tests in `crates/vox-compiler/src/eval/builtins.rs`:

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

    /// `fs.read` strips BOM and normalizes CRLF/CR -> LF (universal newlines).
    #[test]
    fn fs_read_normalizes_bom_and_crlf() {
        let dir = std::env::temp_dir().join("vox_fs_read_norm");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.txt");
        std::fs::write(&p, b"\xEF\xBB\xBFa\r\nb\r\n").unwrap();
        let got = result_ok_str(call_builtin_method(
            &fs_namespace(),
            "read",
            vec![VoxValue::Str(p.to_string_lossy().to_string())],
            None,
        ));
        assert_eq!(got, "a\nb\n");
    }

    /// `fs.read_bytes` preserves the exact bytes (BOM + CR intact) — the
    /// byte-exact escape hatch for round-trip-faithful work.
    #[test]
    fn fs_read_bytes_is_byte_exact() {
        let dir = std::env::temp_dir().join("vox_fs_read_raw");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("b.txt");
        std::fs::write(&p, b"\xEF\xBB\xBFa\r\nb\r\n").unwrap();
        let got = result_ok_str(call_builtin_method(
            &fs_namespace(),
            "read_bytes",
            vec![VoxValue::Str(p.to_string_lossy().to_string())],
            None,
        ));
        assert_eq!(got, "\u{feff}a\r\nb\r\n");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-compiler fs_text_robustness`
Expected: FAIL — `fs.read` returns `"\u{feff}a\r\nb\r\n"` (not normalized); both assertions fail.

- [ ] **Step 3: Normalize `fs.read` and make `read_bytes` exact**

In `crates/vox-compiler/src/eval/builtins.rs`, change the `fs.read` impl (line 979) from:

```rust
let res = match std::fs::read_to_string(path) {
    Ok(s) => Ok(Box::new(VoxValue::Str(s))),
    Err(e) => Err(e.to_string()),
};
```

to:

```rust
// Seam 2: universal-newlines read — strip BOM, CRLF/CR -> LF.
// Byte-exact round-trips go through `read_bytes`.
let res = match std::fs::read_to_string(path) {
    Ok(s) => Ok(Box::new(VoxValue::Str(vox_bounded_fs::normalize_text(s)))),
    Err(e) => Err(e.to_string()),
};
```

Change the `read_bytes` impl (lines 992-994) from:

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
// Seam 4: byte-exact escape hatch. Preserve BOM/CR; do NOT use
// from_utf8_lossy (it silently mangles). Vox has no Bytes value, so a
// non-UTF-8 file surfaces as an explicit error rather than corruption.
// (True arbitrary-binary support is a separate VoxValue::Bytes feature — out of scope.)
let res = match std::fs::read(&path) {
    Ok(bytes) => match String::from_utf8(bytes) {
        Ok(s) => Ok(Box::new(VoxValue::Str(s))),
        Err(e) => Err(format!("read_bytes: {path}: invalid UTF-8: {e}")),
    },
    Err(e) => Err(e.to_string()),
};
```

If the dep was missing, add to `crates/vox-compiler/Cargo.toml` under `[dependencies]`:

```toml
vox-bounded-fs = { path = "../vox-bounded-fs" }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-compiler fs_text_robustness`
Expected: PASS (2 tests).

- [ ] **Step 5: Confirm write-preserve (no code change, guard test)**

`fs.write` (line 1022) already passes content through unchanged — preserve-on-write is the existing behavior, so no edit. Add a guard test to the same module to lock it in:

```rust
#[test]
fn fs_write_preserves_lf_no_cr_inserted() {
    let dir = std::env::temp_dir().join("vox_fs_write_preserve");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("c.txt");
    let _ = call_builtin_method(
        &fs_namespace(),
        "write",
        vec![
            VoxValue::Str(p.to_string_lossy().to_string()),
            VoxValue::Str("x\ny\n".to_string()),
        ],
        None,
    );
    let raw = std::fs::read(&p).unwrap();
    assert_eq!(raw, b"x\ny\n", "write must not insert CR or alter newlines");
}
```

Run: `cargo test -p vox-compiler fs_text_robustness`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/eval/builtins.rs crates/vox-compiler/Cargo.toml
git commit -m "feat(runtime): normalize fs.read; byte-exact read_bytes (Seam 2+4)"
```

---

### Task 4: Seam 3 — Windows console UTF-8 on first `print`

**Files:**
- Modify: `crates/vox-compiler/src/eval/builtins.rs` (add `init_console_utf8` helper near the top, ~after line 67; call it in `print` at line 2357)

- [ ] **Step 1: Write the idempotency test**

Add to `crates/vox-compiler/src/eval/builtins.rs` (can go in the `fs_text_robustness_tests` module or its own):

```rust
#[test]
fn console_init_is_idempotent_and_safe() {
    // Must not panic, and must be safe to call repeatedly (guarded by Once).
    // On non-Windows this is a no-op. The real emoji-rendering check is a
    // manual Windows smoke test (see Step 5), not automatable in CI.
    init_console_utf8();
    init_console_utf8();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler console_init_is_idempotent`
Expected: FAIL — `cannot find function 'init_console_utf8'`.

- [ ] **Step 3: Implement the console init**

Add near the top of `crates/vox-compiler/src/eval/builtins.rs` (after `vox_flush_exit_commands`, ~line 67):

```rust
/// Force the Windows console output code page to UTF-8 so `print` renders
/// Unicode/emoji instead of mojibake. Idempotent (guarded by `Once`); a no-op
/// on non-Windows and a harmless no-op when stdout is redirected to a pipe/file
/// (the bytes were already UTF-8). Called on first `print`.
///
// ponytail: SetConsoleOutputCP(CP_UTF8) covers redirect + the common console
// case. Full supplementary-plane emoji glyph correctness needs WriteConsoleW
// (both surrogate halves in one call) — deferred; upgrade path is to route
// `print` through a console writer if a real glyph-rendering bug appears.
pub fn init_console_utf8() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        #[cfg(windows)]
        {
            const CP_UTF8: u32 = 65001;
            extern "system" {
                fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
            }
            // Safe: a single FFI call with a constant; failure (no console
            // attached) is ignored on purpose.
            unsafe {
                let _ = SetConsoleOutputCP(CP_UTF8);
            }
        }
    });
}
```

Then in `print` (line 2357), call it before writing:

```rust
"print" => {
    init_console_utf8();
    let msg = args
        .iter()
        .map(vox_value_display)
        .collect::<Vec<_>>()
        .join(" ");
    println!("{msg}");
    Some(VoxValue::Null)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler console_init_is_idempotent`
Expected: PASS.

- [ ] **Step 5: Manual Windows smoke test (the real verification)**

On a Windows machine, create `emoji.vox` containing a print of a non-ASCII string + emoji (e.g. `print("café 🚀")`) and run it via `vox run emoji.vox` in Windows Terminal.
Expected: `café 🚀` renders correctly (not `cafÃ© ??`). Note: legacy `conhost` with a non-emoji font may still box-glyph the emoji — Windows Terminal is the supported target. Record the result in the PR description.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/eval/builtins.rs
git commit -m "feat(runtime): force Windows console UTF-8 on first print (Seam 3)"
```

---

### Task 5: Born-correct hygiene — `.gitattributes` + `.editorconfig`

**Files:**
- Create or verify: `.gitattributes` (repo root)
- Create or verify: `.editorconfig` (repo root)

- [ ] **Step 1: Check whether they already exist**

Run: `git ls-files .gitattributes .editorconfig`
Expected: lists any that exist. If both already enforce LF + UTF-8 for `.vox`, skip to Step 4 and note "already present".

- [ ] **Step 2: Ensure `.gitattributes` enforces LF for `.vox`**

Ensure `.gitattributes` contains (append if the file exists, create if not):

```gitattributes
# Vox sources are LF-only, UTF-8, no BOM — checked in normalized.
*.vox text eol=lf
```

- [ ] **Step 3: Ensure `.editorconfig` sets UTF-8 + LF**

Ensure `.editorconfig` contains (append a `[*.vox]` section if the file exists, create with a root block if not):

```editorconfig
root = true

[*.vox]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true
```

- [ ] **Step 4: Verify no churn and existing CI line-ending check still passes**

Run: `git diff --stat`
Expected: only `.gitattributes` / `.editorconfig` changed (no mass re-normalization — repo is already LF per the existing `vox-cli-ci` check).

- [ ] **Step 5: Commit**

```bash
git add .gitattributes .editorconfig
git commit -m "chore: born-correct LF/UTF-8 hygiene for .vox (gitattributes + editorconfig)"
```

---

## Notes for the implementer

- **Existing CI checks stay as-is.** `crates/vox-cli-ci/src/line_endings.rs` (LF-only + no-BOM) keeps the repo tidy. It reads files via `read_utf8_path_capped` (raw, un-normalized) — do **not** route those checks through `normalize_text`, or they'd stop detecting violations. Normalization is applied only at the two seams in Tasks 2 and 3.
- **Phase 2 (separate plan):** emitted/compiled binaries (codegen `main`) need the same `init_console_utf8` injected; the interpreter (`vox run`) is covered here. Also deferred per spec: legacy non-UTF-8 decoding (`encoding:` arg) and stdin normalization — build only when a concrete case appears.
