//! Generate (instruction -> idiomatic Rust) SFT pairs from workspace .rs files.

use serde_json::json;

/// Build one SFT pair JSON from a function name + its source. Lane: vox_rust_authoring.
pub fn make_authoring_pair(fn_name: &str, rust_src: &str) -> serde_json::Value {
    let instruction = format!("Write an idiomatic Rust function named `{fn_name}`.");
    json!({
        "prompt": instruction,
        "response": format!("```rust\n{rust_src}\n```"),
        "messages": [
            {"role": "user", "content": instruction},
            {"role": "assistant", "content": format!("```rust\n{rust_src}\n```")}
        ],
        "category": "rust_authoring",
        "lane": "vox_rust_authoring",
        "origin": "human",
        "response_mode": "code_only",
        "task_family": "rust_authoring"
    })
}

/// Map a batch of `cargo check` JSON diagnostics back to per-snippet pass/fail.
/// `n` snippets were emitted as modules `snippet_0..snippet_{n-1}`; a snippet
/// fails iff any error diagnostic's span path contains its module name.
pub fn batch_pass_flags(n: usize, error_modules: &[String]) -> Vec<bool> {
    (0..n)
        .map(|i| !error_modules.iter().any(|m| m == &format!("snippet_{}", i)))
        .collect()
}

/// Compile `snippets` together as modules in a throwaway workspace member,
/// reusing the workspace target dir. Returns one pass-flag per snippet.
/// Wraps each snippet in `mod snippet_i { ... }` and parses `cargo check
/// --message-format=json` diagnostics via `batch_pass_flags`.
/// Spawns with the no-flashing-window helper on Windows.
pub fn compile_batch_in_workspace(
    workspace_root: &std::path::Path,
    snippets: &[String],
) -> Vec<bool> {
    run_batch_command_in_workspace(workspace_root, snippets, false)
}

/// Clippy version of `compile_batch_in_workspace`.
pub fn clippy_batch_in_workspace(
    workspace_root: &std::path::Path,
    snippets: &[String],
) -> Vec<bool> {
    run_batch_command_in_workspace(workspace_root, snippets, true)
}

fn run_batch_command_in_workspace(
    workspace_root: &std::path::Path,
    snippets: &[String],
    clippy: bool,
) -> Vec<bool> {
    let n = snippets.len();
    if n == 0 {
        return Vec::new();
    }

    let tmp_dir = workspace_root.join("crates").join("_corpus_verify_tmp");
    let src_dir = tmp_dir.join("src");

    // Create directories
    if let Err(e) = std::fs::create_dir_all(&src_dir) {
        eprintln!(
            "Failed to create directories for compile verification: {}",
            e
        );
        return vec![false; n];
    }

    // Write Cargo.toml
    let cargo_toml_content = r#"[package]
name = "_corpus_verify_tmp"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
vox-compiler = { path = "../vox-compiler" }
vox-repository = { path = "../vox-repository" }
vox-bounded-fs = { path = "../vox-bounded-fs" }
"#;

    if let Err(e) = std::fs::write(tmp_dir.join("Cargo.toml"), cargo_toml_content) {
        eprintln!("Failed to write Cargo.toml: {}", e);
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return vec![false; n];
    }

    // Write lib.rs declaring all modules and each snippet_i.rs
    let mut lib_content = String::new();
    for (i, snippet_src) in snippets.iter().enumerate().take(n) {
        lib_content.push_str(&format!("pub mod snippet_{};\n", i));

        if let Err(e) = std::fs::write(src_dir.join(format!("snippet_{}.rs", i)), snippet_src) {
            eprintln!("Failed to write snippet_{}.rs: {}", i, e);
            let _ = std::fs::remove_dir_all(&tmp_dir);
            return vec![false; n];
        }
    }

    if let Err(e) = std::fs::write(src_dir.join("lib.rs"), lib_content) {
        eprintln!("Failed to write lib.rs: {}", e);
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return vec![false; n];
    }

    // Spawn command
    let mut cmd = std::process::Command::new("cargo");
    if clippy {
        cmd.args([
            "clippy",
            "-p",
            "_corpus_verify_tmp",
            "--message-format=json",
            "--",
            "-D",
            "warnings",
        ]);
    } else {
        cmd.args(["check", "-p", "_corpus_verify_tmp", "--message-format=json"]);
    }
    cmd.current_dir(workspace_root);

    // Apply Windows quiet spawn flags (CREATE_NO_WINDOW)
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = match cmd.output() {
        Ok(out) => out,
        Err(e) => {
            eprintln!("Failed to execute cargo check/clippy: {}", e);
            let _ = std::fs::remove_dir_all(&tmp_dir);
            return vec![false; n];
        }
    };

    // Parse cargo JSON messages and collect failing snippet names
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let mut error_modules = std::collections::HashSet::new();

    for line in stdout_str.lines() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if val.get("reason").and_then(|r| r.as_str()) == Some("compiler-message") {
                if let Some(msg) = val.get("message") {
                    let level = msg.get("level").and_then(|l| l.as_str()).unwrap_or("");
                    if level == "error" {
                        if let Some(spans) = msg.get("spans").and_then(|s| s.as_array()) {
                            for span in spans {
                                if let Some(file_name) =
                                    span.get("file_name").and_then(|f| f.as_str())
                                {
                                    if let Some(start_idx) = file_name.find("snippet_") {
                                        let sub = &file_name[start_idx..];
                                        let end_idx = sub.find(".rs").unwrap_or(sub.len());
                                        let name = &sub[..end_idx];
                                        error_modules.insert(name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Clean up temporary crate directory
    let _ = std::fs::remove_dir_all(&tmp_dir);

    let error_modules_vec: Vec<String> = error_modules.into_iter().collect();
    batch_pass_flags(n, &error_modules_vec)
}

/// Walk workspace Rust sources and emit `vox_rust_authoring` SFT pairs.
///
/// Uses `extract_rs::walk_and_extract` for extraction, then re-frames each
/// function as a `make_authoring_pair` row (lane = vox_rust_authoring).
/// The function name is extracted from the first `pub fn` / `fn` token in the response.
pub fn corpus_from_workspace(
    workspace_root: &std::path::Path,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let config = crate::corpus::extract_rs::ExtractRsConfig {
        root: workspace_root.join("crates"),
        skip_tests: true,
        ..Default::default()
    };
    let pairs = crate::corpus::extract_rs::walk_and_extract(&config)?;
    let mut out = Vec::with_capacity(pairs.len());
    for pair in &pairs {
        let fn_name = extract_fn_name(&pair.response).unwrap_or_else(|| "unnamed".to_string());
        out.push(make_authoring_pair(&fn_name, &pair.response));
    }
    Ok(out)
}

/// Extract the first function name from a Rust source snippet.
///
/// Finds the `fn ` keyword as a standalone token (start of line or after
/// whitespace), so it handles every modifier combination — `pub fn`,
/// `pub(crate) fn`, `async fn`, `pub async fn`, `const fn`, `unsafe fn`,
/// `pub unsafe fn`, etc. — not just the bare `pub fn` / `fn` prefixes.
fn extract_fn_name(src: &str) -> Option<String> {
    for line in src.lines() {
        let bytes = line.as_bytes();
        let mut search = 0;
        while let Some(rel) = line[search..].find("fn ") {
            let idx = search + rel;
            let boundary_ok = idx == 0 || bytes[idx - 1].is_ascii_whitespace();
            if boundary_ok {
                let rest = &line[idx + 3..];
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    return Some(name);
                }
            }
            search = idx + 3;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_has_lane_and_assistant_turn() {
        let p = make_authoring_pair("add", "fn add(a: i32, b: i32) -> i32 { a + b }");
        assert_eq!(p["lane"], "vox_rust_authoring");
        let msgs = p["messages"].as_array().unwrap();
        assert_eq!(msgs.last().unwrap()["role"], "assistant");
    }

    #[test]
    fn flags_only_failing_modules() {
        let flags = batch_pass_flags(3, &["snippet_1".to_string()]);
        assert_eq!(flags, vec![true, false, true]);
    }

    #[test]
    fn empty_errors_all_pass() {
        assert_eq!(batch_pass_flags(2, &[]), vec![true, true]);
    }

    #[test]
    fn extract_fn_name_finds_pub_fn() {
        assert_eq!(
            extract_fn_name("pub fn compute_rate(x: usize) -> f64 { x as f64 }"),
            Some("compute_rate".to_string())
        );
    }

    #[test]
    fn extract_fn_name_finds_bare_fn() {
        assert_eq!(extract_fn_name("fn inner() {}"), Some("inner".to_string()));
    }

    #[test]
    fn extract_fn_name_returns_none_for_non_fn() {
        assert_eq!(extract_fn_name("let x = 42;"), None);
    }

    #[test]
    fn extract_fn_name_handles_modifiers() {
        // The bare strip_prefix approach returned "unnamed" for all of these.
        assert_eq!(
            extract_fn_name("pub async fn fetch(url: &str) -> Bytes { todo!() }"),
            Some("fetch".to_string())
        );
        assert_eq!(
            extract_fn_name("pub(crate) fn helper() {}"),
            Some("helper".to_string())
        );
        assert_eq!(
            extract_fn_name("const fn sized() -> usize { 8 }"),
            Some("sized".to_string())
        );
        assert_eq!(
            extract_fn_name("    unsafe fn raw(p: *const u8) {}"),
            Some("raw".to_string())
        );
        assert_eq!(
            extract_fn_name("async fn run() {}"),
            Some("run".to_string())
        );
    }

    #[test]
    fn corpus_from_workspace_returns_vox_rust_authoring_lane() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let pairs = corpus_from_workspace(root).expect("corpus_from_workspace");
        assert!(!pairs.is_empty(), "should find Rust fns in workspace");
        for p in &pairs {
            assert_eq!(
                p["lane"], "vox_rust_authoring",
                "all pairs have correct lane"
            );
            assert_eq!(p["category"], "rust_authoring");
        }
    }
}

#[cfg(test)]
mod verify_tests {
    use super::*;

    #[test]
    #[ignore] // requires cargo + workspace; run locally with --ignored
    fn batch_accepts_valid_rejects_invalid() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let flags = compile_batch_in_workspace(
            root,
            &[
                "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
                "pub fn broken() -> i32 { return }".to_string(),
            ],
        );
        assert_eq!(flags, vec![true, false]);
    }
}
