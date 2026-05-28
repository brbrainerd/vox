//! Writes a minimal generated backend crate at `<workspace>/_bundle_ai_fixture_<id>/gen_pkg`
//! so generated `Cargo.toml` path deps (`../../crates/...`) resolve, then runs `cargo check`.
//!
//! `generate()` always emits `public/index.html` so `rust_embed` in `main.rs` compiles.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use vox_codegen::codegen_rust::RustAppShell;
use vox_codegen::codegen_rust::emit::generate;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct CleanupScratch(PathBuf);

impl Drop for CleanupScratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Slow: generates a Rust crate bundle from a Vox AI fixture and runs `cargo check`
/// against it in a scratch directory (~25s). Excluded from default local
/// `vox ci pre-push --full`; use `--include-slow` or rely on CI.
#[test]
#[ignore = "slow; runs nested cargo check (~25s); owner: codegen sunset: never; use --include-slow or CI"]
fn generated_ai_fixture_bundle_passes_cargo_check() {
    let src = r#"
        @ai(model = "openrouter/auto")
        @uses(net)
        fn hello(x: str) to str {
            return x
        }
    "#;
    let ast = parse(lex(src)).expect("parse");
    let hir = lower_module(&ast);

    let out =
        generate(&hir, "ai_fixture_bundle_gen", RustAppShell::AxumLocalServer).expect("generate");
    let uniq = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch = workspace_root().join(format!("_bundle_ai_fixture_{uniq}"));
    let pkg = scratch.join("gen_pkg");
    fs::create_dir_all(pkg.join("src")).expect("mkdir");

    let _cleanup = CleanupScratch(scratch.clone());

    for (rel, contents) in &out.files {
        let path = pkg.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("mkdir parent");
        }
        fs::write(path, contents).expect("write");
    }

    let cargo_bin = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo_bin)
        .current_dir(&pkg)
        .args(["check", "-q"])
        .status()
        .expect("spawn cargo check");

    assert!(
        status.success(),
        "cargo check failed for generated ai_fixture bundle under {}",
        pkg.display()
    );
}
