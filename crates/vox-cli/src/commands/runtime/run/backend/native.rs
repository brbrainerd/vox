use super::{RunBackend, ScriptOpts, parse_cargo_error};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
/// Backend for running native binaries via cargo.
pub struct NativeBackend;

impl RunBackend for NativeBackend {
    fn cache_label(&self) -> &str {
        "script-cache"
    }

    fn compile(
        &self,
        hir: &vox_compiler::hir::HirModule,
        cache_dir: &Path,
        shared_target: &Path,
        opts: &ScriptOpts,
    ) -> Result<PathBuf> {
        // Native scripts share the same generated crate name (`vox-script`). A single workspace-level
        // `CARGO_TARGET_DIR` would let parallel compiles clobber `vox-script.exe` before copy.
        let _ = shared_target;
        let script_target_dir = cache_dir.join("target");

        let is_windows_target = if let Some(t) = opts.target_triple.as_ref() {
            t.contains("windows")
        } else {
            cfg!(target_os = "windows")
        };
        let binary_name = if is_windows_target {
            "vox-script.exe"
        } else {
            "vox-script"
        };
        let per_entry_binary = cache_dir.join(binary_name);

        let output = vox_codegen::codegen_rust::generate_script(
            hir,
            "vox-script",
            crate::fs_utils::resolve_vox_runtime_path().as_deref(),
        )
        .map_err(|e| anyhow::anyhow!("Rust code generation failed: {e}"))?;

        output.write_to_dir(cache_dir)?;

        let use_release = opts.trust_class.as_deref() != Some("trusted_dev")
            && std::env::var("VOX_SCRIPT_RELEASE").is_ok();

        let mut profile_args: Vec<&str> = if use_release {
            vec!["build", "--release"]
        } else {
            vec!["build", "--profile", "script-dev"]
        };

        if let Some(t) = opts.target_triple.as_ref() {
            profile_args.push("--target");
            profile_args.push(t);
        }

        // Per-entry `script-dev` profile only under this cache dir. Do not write under
        // `script-cache/.cargo/` — that path is shared by all hashes and parallel `vox run`
        // invocations would race on the same files (Windows: flaky "path not found").
        let cargo_config_dir = cache_dir.join(".cargo");
        if !cargo_config_dir.exists() {
            let _ = std::fs::create_dir_all(&cargo_config_dir);
        }
        let config_content = r#"[profile.script-dev]
        inherits = "dev"
        opt-level = 1
        codegen_units = 256
        incremental = true
        debug = false
        overflow-checks = false
        "#;
        for filename in &["config", "config.toml"] {
            let _ = std::fs::write(cargo_config_dir.join(filename), config_content);
        }
        let args: Vec<String> = profile_args[1..].iter().map(|s| (*s).to_string()).collect();
        let req = crate::build_service::CargoRequest::build(
            cache_dir.to_path_buf(),
            Some(script_target_dir.clone()),
            args,
        );
        let build_out = crate::build_service::run_cargo(&req)?;

        if !build_out.status.success() {
            let stderr = String::from_utf8_lossy(&build_out.stderr).to_string();
            let (summary, suggestion) = parse_cargo_error(&stderr, false);
            return Err(anyhow::anyhow!(
                "Compilation failed: {}\n{}\n---\n{}",
                summary,
                suggestion,
                stderr
            ));
        }

        let profile_dir = if use_release { "release" } else { "script-dev" };
        let binary_path = if let Some(t) = opts.target_triple.as_ref() {
            script_target_dir
                .join(t)
                .join(profile_dir)
                .join(binary_name)
        } else {
            script_target_dir.join(profile_dir).join(binary_name)
        };
        std::fs::copy(&binary_path, &per_entry_binary).with_context(|| {
            format!(
                "failed to copy compiled script binary {} -> {}",
                binary_path.display(),
                per_entry_binary.display()
            )
        })?;

        Ok(per_entry_binary)
    }

    fn execute(&self, artifact: &Path, args: &[String], opts: &ScriptOpts) -> Result<ExitStatus> {
        let mut cmd = Command::new(artifact);
        cmd.args(args);
        if opts.sandbox {
            cmd.env("VOX_SANDBOX", "1");
        }
        if !opts.allow_mcp {
            cmd.env("VOX_NO_MCP", "1");
        }

        // Propagate the resolved cargo binary path so run_cmd("cargo", ...) inside the
        // script can find cargo even when the script binary is not launched via `cargo run`.
        // Precedence: VOX_CARGO_BIN > CARGO env var > which-resolved "cargo".
        let cargo_bin = std::env::var("VOX_CARGO_BIN")
            .or_else(|_| std::env::var("CARGO"))
            .unwrap_or_else(|_| {
                which::which("cargo")
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "cargo".to_string())
            });
        cmd.env("CARGO", &cargo_bin);
        cmd.env("VOX_CARGO_BIN", &cargo_bin);

        // P4: Redact sensitive env vars
        if opts.trust_class.as_deref() != Some("trusted_dev") {
            for (key, _) in std::env::vars() {
                let ku = key.to_uppercase();
                if ku.ends_with("_KEY")
                    || ku.ends_with("_SECRET")
                    || ku.ends_with("_TOKEN")
                    || ku.ends_with("_PASSWORD")
                {
                    cmd.env_remove(&key);
                }
            }
        }

        // P4: Native sandbox enforcement (Landlock on Linux, Job Objects on Windows)
        if opts.sandbox {
            super::super::sandbox::enforce_sandbox(&mut cmd, opts)?;
        }

        // On Windows with --sandbox, spawn then assign Job Object immediately
        #[cfg(target_os = "windows")]
        if opts.sandbox {
            let mut child = cmd.spawn().with_context(|| {
                format!("failed to launch script binary {}", artifact.display())
            })?;
            super::super::sandbox::post_spawn_sandbox(&child)?;
            let status = child.wait()?;
            return Ok(status);
        }

        // Attach the binary path so a launch failure is not a bare, path-less
        // `os error 3` (the clean-room idempotency failure mode on Windows).
        cmd.status()
            .with_context(|| format!("failed to launch script binary {}", artifact.display()))
    }
}
