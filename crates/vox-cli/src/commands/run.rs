// toestub-ignore(arch/sprawl)
use crate::cli_args::BuildMode;
use crate::commands::build;
use crate::config;
use crate::frontend;
use anyhow::{Context, Result};
use clap::ValueEnum;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How `vox run` chooses between app (compilerd / generated server) and script execution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum RunMode {
    /// If the file has no `@page` (first 8 KiB scan), run as a script when `script-execution` is enabled; else app path. Override with `Vox.toml` `[web] run_mode` or `VOX_WEB_RUN_MODE`.
    #[default]
    Auto,
    /// Always use the app / dev-server path (build + `target/generated` server).
    App,
    /// Always use the script runner (`fn main()`), requires `--features script-execution`.
    Script,
    /// Tree-walking HIR Interpreter execution (fast execution for scripts).
    Interp,
}

/// Parse run mode strings from CLI / `vox-compilerd` JSON (`auto`, `app`, `script`).
///
/// Unknown values map to [`RunMode::Auto`] so older clients stay compatible.
pub fn parse_run_mode_from_str(s: &str) -> RunMode {
    match s.trim().to_ascii_lowercase().as_str() {
        "app" => RunMode::App,
        "script" => RunMode::Script,
        "interp" => RunMode::Interp,
        _ => RunMode::Auto,
    }
}

/// Run a `.vox` file via the tree-walking HIR interpreter (no native compile step).
/// Extracted so it can be invoked from both `--mode interp` and the `--mode auto`
/// fallback path when `script-execution` Cargo feature is not compiled in.
async fn run_interp(file: &Path, _args: &[String]) -> Result<()> {
    let source = std::fs::read_to_string(file).context("Failed to read file")?;

    let mut caps = std::collections::HashSet::new();
    let mut has_caps_directive = false;
    if let Some(first_line) = source.lines().next() {
        if first_line.starts_with("// vox:caps ") {
            has_caps_directive = true;
            for cap in first_line
                .trim_start_matches("// vox:caps ")
                .split_whitespace()
            {
                caps.insert(cap.to_string());
            }
        }
    }

    let tokens = vox_compiler::lexer::lex(&source);
    let module = vox_compiler::parser::parse_script(tokens)
        .map_err(|e| anyhow::anyhow!("Parse failed: {:?}", e))?;
    let lowered = vox_compiler::hir::lower::lower_module(&module);

    let mut interpreter = vox_compiler::eval::Interpreter::new(10_000_000);
    if has_caps_directive {
        interpreter.caps = Some(caps);
    }
    if let Ok(abs) = std::fs::canonicalize(file) {
        interpreter.set_source_path(abs);
    } else {
        interpreter.set_source_path(file.to_path_buf());
    }

    interpreter
        .run_module(&lowered)
        .map_err(|e| anyhow::anyhow!("Eval failed: {:?}", e))?;

    let res = interpreter
        .call("main", vec![])
        .map_err(|e| anyhow::anyhow!("Eval failed calling main: {:?}", e))?;
    // Only print the return value when it's meaningful (non-Null). Suppresses
    // the spurious trailing `Null` that scripts using bare `return;` produced.
    // Use the value's *display* form (e.g. `ok`), not Debug (`Str("ok")`), so
    // `vox run --mode interp` prints user-facing output, not internal repr.
    if !matches!(res, vox_compiler::eval::value::VoxValue::Null) {
        println!("{}", vox_compiler::eval::builtins::vox_value_display(&res));
    }

    vox_compiler::eval::builtins::vox_flush_exit_commands();
    Ok(())
}

/// Execute the `vox run` command (dispatch to App or Script mode).
pub async fn run(file: &Path, args: &[String], mode: RunMode) -> Result<()> {
    if mode == RunMode::Interp {
        return run_interp(file, args).await;
    }

    let use_script = match mode {
        RunMode::App => false,
        RunMode::Script => true,
        RunMode::Interp => unreachable!(),
        RunMode::Auto => match vox_config::VoxConfig::load().web_run_mode {
            vox_config::WebRunMode::App => false,
            vox_config::WebRunMode::Script => true,
            vox_config::WebRunMode::Auto => {
                crate::commands::runtime::run::run::is_script_file_by_page_heuristic(file)
            }
        },
    };

    #[cfg(feature = "script-execution")]
    if use_script {
        tracing::info!(
            target: "vox.script",
            path = %file.display(),
            ?mode,
            "dispatch native script execution lane"
        );
        let opts = crate::commands::runtime::run::script::ScriptOpts {
            sandbox: false,
            allow_mcp: false,
            no_cache: false,
            isolation: None,
            trust_class: Some("trusted_dev".into()),
            wasi_dirs: Vec::new(),
            target_triple: None,
        };
        return crate::commands::runtime::run::script::run(file, args, &opts).await;
    }

    #[cfg(not(feature = "script-execution"))]
    if use_script {
        if matches!(mode, RunMode::Auto) {
            // Build was compiled without `script-execution`. Auto mode falls
            // through to the always-available HIR interpreter rather than
            // emitting an undiscoverable feature-gate error. Users who want
            // native script execution can build with `--features
            // script-execution` or pass `--mode script` explicitly to opt in
            // to the error path below.
            tracing::info!(
                target: "vox.script",
                path = %file.display(),
                "script-execution feature absent; auto-falling back to --mode interp"
            );
            return run_interp(file, args).await;
        }
        anyhow::bail!(
            "`vox run --mode script` requires a vox build with `--features script-execution`. \
             Try `vox run --mode interp {}` (interpreter is always available), \
             or rebuild with `cargo build --features script-execution`.",
            file.display()
        );
    }

    // 1. Build using existing build command logic
    let out_dir = PathBuf::from("dist");

    println!("Building {}...", file.display());
    build::run(
        file,
        &out_dir,
        None,
        None,
        false,
        false,
        BuildMode::App,
        vox_codegen::codegen_rust::RustAppShell::default(),
        None,
        None,
    )
    .await?;

    // 2. Check if we have frontend components to bundle.
    // BuildTarget::Server forces has_frontend = false regardless of what's in dist/.
    let resolved_target = vox_config::VoxConfig::load().build_target;
    let heuristic = fs::read_dir(&out_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().is_some_and(|ext| ext == "tsx"))
        })
        .unwrap_or(false);
    let has_frontend = resolve_has_frontend(resolved_target, heuristic);

    if has_frontend {
        println!("\nBundling frontend...");
        build_frontend(&out_dir)?;
    }

    // 3. Run backend (Rust)
    let generated_dir = Path::new("target").join("generated");

    let (_orchestrated_vite, ssr_env) = if has_frontend {
        frontend::OrchestratedViteGuard::maybe_spawn(&out_dir.join("app"))?
    } else {
        (frontend::OrchestratedViteGuard::disabled(), None)
    };

    let port = config::default_port();
    println!("\nStarting server...");
    if has_frontend {
        println!("  Frontend + Backend at http://127.0.0.1:{port}");
    } else {
        println!("  Backend at http://127.0.0.1:{port}");
    }
    println!("  Press Ctrl+C to stop\n");

    if which::which("cargo").is_err() {
        anyhow::bail!(missing_cargo_toolchain_message());
    }

    let mut cargo_cmd = Command::new("cargo");
    cargo_cmd
        .arg("run")
        .arg("--")
        .args(args)
        .current_dir(&generated_dir);
    if let Some((k, v)) = ssr_env {
        cargo_cmd.env(k, v);
    }
    let status = cargo_cmd
        .status()
        .context("Failed to execute cargo run in generated directory")?;

    if !status.success() {
        anyhow::bail!("Application exited with error code: {:?}", status.code());
    }

    Ok(())
}

/// Build the frontend React application and copy assets to backend public dir.
fn build_frontend(generated_ts_dir: &Path) -> Result<()> {
    let app_dir = generated_ts_dir.join("app");
    let tanstack_start = vox_config::VoxConfig::load().web_tanstack_start;
    frontend::scaffold_react_app(&app_dir, generated_ts_dir, tanstack_start)
        .context("Failed to scaffold Vite + React app")?;
    crate::commands::build::verify_app_src_generated_imports(&app_dir.join("src"))
        .context("Scaffold entry import graph (main.tsx / routes/index.tsx)")?;

    // pnpm install (skip if node_modules exists and is fresh)
    let node_modules = app_dir.join("node_modules");
    let pnpm = frontend::pnpm_executable();
    if which::which(pnpm).is_err() {
        anyhow::bail!(missing_pnpm_message(pnpm));
    }
    if !node_modules.exists() {
        println!("  Installing frontend dependencies (pnpm)...");
        crate::frontend::run_pnpm(
            &app_dir,
            &["install", "--prefer-offline"],
            "pnpm install failed",
        )?;
    }

    println!("  Building frontend assets...");
    crate::frontend::run_pnpm(&app_dir, &["run", "build"], "Frontend build failed")?;

    // Copy built assets to target/generated/public/
    let public_dir = Path::new("target").join("generated").join("public");
    let built_dir = app_dir.join("dist");

    if built_dir.exists() {
        if public_dir.exists() {
            fs::remove_dir_all(&public_dir).ok();
        }
        fs::create_dir_all(&public_dir)?;
        copy_dir_recursive(&built_dir, &public_dir)?;
        println!("  Frontend assets copied to {}", public_dir.display());
    }

    Ok(())
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());
        if from_path.is_dir() {
            fs::create_dir_all(&to_path)?;
            copy_dir_recursive(&from_path, &to_path)?;
        } else {
            fs::copy(&from_path, &to_path)?;
        }
    }
    Ok(())
}

/// Message for a missing `cargo` toolchain, shown before `vox run` shells out
/// to `cargo run` in the generated project directory.
///
/// `vox run` generates a Rust crate and compiles it, so a Rust toolchain is
/// genuinely required (spec §9.1's cargo-invocation exemption applies: this is
/// not a repo-relative path, and the dependency is stated honestly rather than
/// silently assumed).
fn missing_cargo_toolchain_message() -> String {
    "`vox run` compiles the generated backend, which requires a Rust toolchain, \
     but `cargo` was not found on PATH. Install Rust from https://rustup.rs and \
     try again."
        .to_string()
}

/// Message for a missing `pnpm`, shown only on the `has_frontend` path so a
/// backend-only `.vox` program never mentions it.
fn missing_pnpm_message(pnpm_exe: &str) -> String {
    format!(
        "This program has a frontend, which `vox run` bundles with pnpm, but \
         `{pnpm_exe}` was not found on PATH. Install Node.js from \
         https://nodejs.org, then `npm install -g pnpm`, and try again."
    )
}

/// Determine `has_frontend` given a `BuildTarget` and an optional existing `dist/` scan.
///
/// - `BuildTarget::Server` always returns `false` (no frontend regardless of what's in dist/).
/// - `BuildTarget::Fullstack` falls through to the heuristic.
/// - `BuildTarget::Client` is not expected in `vox run`; falls through to heuristic.
/// - `BuildTarget::Mobile` ships a TypeScript/RN bundle to a device; for `vox run` purposes
///   we treat it as having a frontend so the dev-server proxy plumbing stays consistent
///   with other TS-emitting targets.
pub fn resolve_has_frontend(target: vox_config::BuildTarget, heuristic: bool) -> bool {
    match target {
        vox_config::BuildTarget::Server => false,
        vox_config::BuildTarget::Fullstack
        | vox_config::BuildTarget::Client
        | vox_config::BuildTarget::Mobile => heuristic,
    }
}

#[cfg(test)]
mod parse_mode_tests {
    use super::{RunMode, parse_run_mode_from_str};

    #[test]
    fn parse_run_mode_from_str_maps_variants() {
        assert_eq!(parse_run_mode_from_str("SCRIPT"), RunMode::Script);
        assert_eq!(parse_run_mode_from_str("App "), RunMode::App);
        assert_eq!(parse_run_mode_from_str("auto"), RunMode::Auto);
        assert_eq!(parse_run_mode_from_str("unknown"), RunMode::Auto);
    }
}

#[cfg(test)]
mod toolchain_preflight_message_tests {
    use super::{missing_cargo_toolchain_message, missing_pnpm_message};

    #[test]
    fn cargo_message_names_the_dependency_and_install_url_with_no_repo_path() {
        let msg = missing_cargo_toolchain_message();
        assert!(msg.contains("Rust toolchain"));
        assert!(msg.contains("https://rustup.rs"));
        assert!(
            !msg.contains("crates/"),
            "installed-user message must not reference a repo-relative path: {msg}"
        );
    }

    #[test]
    fn pnpm_message_names_the_missing_executable_and_install_path() {
        let msg = missing_pnpm_message("pnpm");
        assert!(msg.contains("pnpm"));
        assert!(msg.contains("https://nodejs.org"));
        assert!(msg.contains("npm install -g pnpm"));
    }

    #[test]
    fn pnpm_message_uses_the_platform_specific_executable_name() {
        let msg = missing_pnpm_message("pnpm.cmd");
        assert!(msg.contains("pnpm.cmd"));
    }
}

#[cfg(test)]
mod build_target_gate_tests {
    use super::resolve_has_frontend;
    use vox_config::BuildTarget;

    #[test]
    fn server_target_forces_has_frontend_false_regardless_of_heuristic() {
        assert!(!resolve_has_frontend(BuildTarget::Server, true));
        assert!(!resolve_has_frontend(BuildTarget::Server, false));
    }

    #[test]
    fn fullstack_target_preserves_heuristic_result() {
        assert!(resolve_has_frontend(BuildTarget::Fullstack, true));
        assert!(!resolve_has_frontend(BuildTarget::Fullstack, false));
    }

    #[test]
    fn client_target_preserves_heuristic_result() {
        assert!(resolve_has_frontend(BuildTarget::Client, true));
        assert!(!resolve_has_frontend(BuildTarget::Client, false));
    }
}
