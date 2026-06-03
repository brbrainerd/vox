//! `vox run` script-mode execution engine.
//!
//! Compiles a `.vox` file with a top-level `fn main()` to a Rust binary (or
//! WASI module) and executes it. Results are cached by content hash in
//! `~/.vox/script-cache/<hash>/`. All script builds share a single
//! `~/.vox/script-target/` so `vox-actor-runtime` and its transitive dependencies
//! are only compiled once.

use anyhow::Result;

use crate::commands::runtime::run::backend::{NativeBackend, RunBackend, WasiBackend};
use std::fs;
use std::path::{Path, PathBuf};

// ── Error taxonomy (P0) ───────────────────────────────────────────────────────

/// Configuration for script execution.
#[derive(Debug, Clone)]
pub struct ScriptOpts {
    /// Enable platform-native sandbox (Landlock/JobObjects).
    pub sandbox: bool,
    /// Allow script to resolve and call MCP tools.
    pub allow_mcp: bool,
    /// Force fresh compilation, bypassing content-hash cache.
    pub no_cache: bool,
    /// Explicit isolation tier string (e.g. `"wasm"`, `"container"`).
    /// When `Some("wasm")` the script is compiled to WASI and run via Wasmtime.
    pub isolation: Option<String>,
    /// Trust classification string (e.g. `"trusted_dev"`, `"untrusted"`).
    /// When set, governs the default isolation tier if `isolation` is `None`.
    pub trust_class: Option<String>,
    /// P1.3: Preopened directories for WASI: (host_path, guest_path, mode)
    #[cfg(feature = "script-execution")]
    pub wasi_dirs: Vec<(PathBuf, String, crate::wasi_dir_mode::WasiDirMode)>,
    /// Optional target triple for cross-compilation (Wave 4).
    pub target_triple: Option<String>,
}

impl ScriptOpts {
    /// Returns `true` when the WASI execution lane should be used.
    ///
    /// WASI is active when:
    /// - `--isolation wasm` / `--isolation wasi` is explicit, OR
    /// - `--trust-class untrusted` is set and no explicit isolation overrides it
    pub fn use_wasi(&self) -> bool {
        if let Some(iso) = self.isolation.as_deref() {
            return matches!(iso.to_lowercase().as_str(), "wasm" | "wasi" | "wasmtime");
        }
        // Default derived from trust class
        matches!(
            self.trust_class
                .as_deref()
                .unwrap_or("trusted_dev")
                .to_lowercase()
                .as_str(),
            "untrusted"
        )
    }

    /// Resolve the effective isolation tier name for display.
    pub fn effective_isolation(&self) -> &str {
        if let Some(iso) = self.isolation.as_deref() {
            return iso;
        }
        match self
            .trust_class
            .as_deref()
            .unwrap_or("trusted_dev")
            .to_lowercase()
            .as_str()
        {
            // WASI (Wasmtime) is the correct sandbox for both untrusted and semi-trusted scripts.
            // The former `container` mapping for semi_trusted silently fell through to NativeBackend
            // because no ContainerBackend is implemented. Docker seccomp is shared-kernel and not
            // meaningfully more secure than WASI anyway; WASI is the correct answer here.
            "untrusted" | "semi_trusted" | "semi-trusted" => "wasm",
            _ => "permissive",
        }
    }

    /// P2: Select the appropriate backend for this execution.
    pub fn backend(&self) -> anyhow::Result<Box<dyn RunBackend>> {
        if self.use_wasi() {
            return Ok(Box::new(WasiBackend));
        }

        // Gate: container/gvisor/microvm tiers are not yet implemented as backends.
        // Callers who explicitly request them should get an error, not silent permissive.
        if let Some(iso) = self.isolation.as_deref() {
            use crate::isolation::IsolationPolicy;
            let policy: IsolationPolicy = iso.parse().unwrap_or(IsolationPolicy::Permissive);
            match policy {
                IsolationPolicy::Container => anyhow::bail!(
                    "--isolation container is not yet implemented.\n\
                     Use --isolation wasm for portable sandboxing, or --isolation permissive\n\
                     for trusted code. See docs/src/reference/isolation.md"
                ),
                IsolationPolicy::Gvisor => anyhow::bail!(
                    "--isolation gvisor requires runsc on PATH and is not yet wired into vox run.\n\
                     Use --isolation wasm instead."
                ),
                IsolationPolicy::MicroVM => anyhow::bail!(
                    "--isolation microvm requires Firecracker/Hyper-V and is not yet wired into vox run."
                ),
                _ => {}
            }
        }

        Ok(Box::new(NativeBackend))
    }
}

/// Print the execution plan for `vox run --explain` without executing.
///
/// When `as_json` is `true`, emits machine-readable JSON instead of human text
/// (useful for IDE/tooling integration).
pub fn print_execution_plan(
    file: &Path,
    isolation: Option<&str>,
    trust_class: Option<&str>,
    sandbox: bool,
    as_json: bool,
) {
    let tc = trust_class.unwrap_or("trusted_dev");
    let opts = ScriptOpts {
        sandbox,
        allow_mcp: false,
        no_cache: false,
        isolation: isolation.map(str::to_string),
        trust_class: trust_class.map(str::to_string),
        #[cfg(feature = "script-execution")]
        wasi_dirs: Vec::new(),
        target_triple: None,
    };
    let tier = opts.effective_isolation();
    let artifact = if opts.use_wasi() {
        "wasi_component"
    } else {
        "native_dev"
    };
    let backend = if opts.use_wasi() {
        "Wasmtime WASI P1"
    } else {
        "Native binary (cargo)"
    };

    let cache_dir = vox_config::paths::script_cache_dir(opts.use_wasi()).join("<source-hash>");

    let isolation_src = if isolation.is_some() {
        "explicit --isolation flag"
    } else if trust_class.is_some() {
        "derived from --trust-class"
    } else if sandbox {
        "derived from --sandbox"
    } else {
        "default for trust class"
    };

    let security = {
        use crate::isolation::IsolationPolicy;
        tier.parse::<IsolationPolicy>()
            .map(|p: IsolationPolicy| p.security_statement().to_string())
            .unwrap_or_else(|_| "Unknown tier".to_string())
    };

    if as_json {
        // Machine-readable output for IDE/tooling consumption (P3)
        println!("{{");
        println!(
            "  \"file\": \"{}\",",
            file.display().to_string().replace('\\', "/")
        );
        println!("  \"trust_class\": \"{tc}\",");
        println!("  \"isolation\": \"{tier}\",");
        println!("  \"isolation_source\": \"{isolation_src}\",");
        println!("  \"artifact\": \"{artifact}\",");
        println!("  \"backend\": \"{backend}\",");
        println!(
            "  \"cache_dir\": \"{}\",",
            cache_dir.display().to_string().replace('\\', "/")
        );
        println!("  \"security\": \"{security}\"");
        println!("}}");
    } else {
        println!();
        println!("Execution plan for: {}", file.display());
        println!("  TrustClass:   {tc}");
        println!("  Isolation:    {tier} ({isolation_src})");
        println!("  Artifact:     {artifact}");
        println!("  Backend:      {backend}");
        println!("  CacheDir:     {}/", cache_dir.display());
        println!();
        println!("  Security:     {security}");
        println!();
    }
}

/// Compile and execute a `.vox` source file as a script.
///
/// Uses content-hash caching to avoid redundant recompiles. Dispatches
/// to [`NativeBackend`] or [`WasiBackend`] depending on `opts`.
pub async fn run(file: &Path, args: &[String], opts: &ScriptOpts) -> Result<()> {
    let (artifact_path, backend) = compile(file, opts).await?;
    execute_binary(&artifact_path, args, opts, &*backend).await
}

/// Compile a Vox script to an executable binary (native or WASI).
/// Returns the path to the compiled artifact.
pub(crate) async fn compile(
    file: &Path,
    opts: &ScriptOpts,
) -> Result<(PathBuf, Box<dyn RunBackend>)> {
    let mut pipeline_opts = vox_compiler::pipeline::PipelineOptions::default();
    pipeline_opts.script_mode = true;
    let result: crate::pipeline::FrontendResult =
        crate::pipeline::run_frontend_with_options(file, false, &pipeline_opts).await?;

    if !result.module.has_entrypoint() {
        anyhow::bail!(
            "No `fn main()` found in {}. Script files must contain a top-level main function.",
            file.display()
        );
    }

    if result.has_errors() {
        crate::pipeline::print_diagnostics(&result, file, false);
        anyhow::bail!("Type checking failed");
    }

    let hir = &result.hir;
    let source = &result.source;
    let backend = opts.backend()?;

    let hash = {
        use xxhash_rust::xxh3::xxh3_64;
        let mut key = Vec::with_capacity(b"vox-cache-v4".len() + 1 + source.len());
        key.extend_from_slice(b"vox-cache-v4\0");
        key.extend_from_slice(source.as_bytes());
        format!("{:016x}", xxh3_64(&key))
    };

    let cache_dir = vox_config::paths::script_cache_dir(opts.use_wasi()).join(&hash);
    let ws = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let lane = if opts.use_wasi() {
        crate::build_lock::BuildLane::ScriptWasi
    } else {
        crate::build_lock::BuildLane::ScriptNative
    };
    let shared_target = crate::build_lock::resolve_target_dir(
        lane,
        &ws.display().to_string(),
        crate::build_lock::lane_isolation(),
    );

    let stamp_path = cache_dir.join(".compiled");
    let binary_name = if backend.cache_label().contains("wasi") {
        "vox-script.wasm"
    } else if cfg!(target_os = "windows") {
        "vox-script.exe"
    } else {
        "vox-script"
    };
    let cached_binary = cache_dir.join(binary_name);

    // A cache hit requires BOTH the `.compiled` stamp AND the actual binary.
    // Previously only the stamp was checked, so a stamp left without its binary
    // (evicted/partial cache, or an interrupted prior compile) made the launcher
    // try to execute a missing path — surfacing as a bare, path-less `os error 3`
    // ("the system cannot find the path specified") on Windows, which is exactly
    // how the clean-room idempotency (second) run failed. Recompile instead.
    let artifact_path = if cache_artifact_reusable(opts.no_cache, &stamp_path, &cached_binary) {
        cached_binary
    } else {
        std::fs::create_dir_all(&cache_dir)?;
        let path = backend.compile(hir, &cache_dir, &shared_target, opts)?;
        std::fs::write(&stamp_path, &hash).ok();

        // Optional GC on miss
        let max_entries =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxScriptCacheMaxEntries)
                .expose()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100usize);
        let max_mb = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxScriptCacheMaxSizeMb)
            .expose()
            .and_then(|v| v.parse().ok())
            .unwrap_or(500u64);
        let _ = crate::fs_utils::gc_script_cache(max_entries, max_mb);

        path
    };

    Ok((artifact_path, backend))
}

/// Execute a pre-compiled binary via the specified backend.
pub(crate) async fn execute_binary(
    artifact_path: &Path,
    args: &[String],
    opts: &ScriptOpts,
    backend: &dyn RunBackend,
) -> Result<()> {
    let status = backend.execute(artifact_path, args, opts)?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Evaluate a Vox expression inline — wraps it in a synthetic `fn main`.
pub async fn eval_inline(expr: &str, sandbox: bool) -> Result<()> {
    let synthetic_source = format!("fn main():\n    print(str({}))\n", expr);

    // Convention: stable path for the inline eval scratch file, shared across
    // repeated `vox eval` invocations. Do NOT replace with tempfile::tempdir()
    // — the compiler subprocess needs to find this file by a predictable path.
    let tmp_dir = std::env::temp_dir().join("vox-eval");
    fs::create_dir_all(&tmp_dir)?;
    let tmp_file = tmp_dir.join("eval_script.vox");
    fs::write(&tmp_file, &synthetic_source)?;

    let opts = ScriptOpts {
        sandbox,
        allow_mcp: false,
        no_cache: false,
        isolation: None,
        trust_class: None,
        #[cfg(feature = "script-execution")]
        wasi_dirs: Vec::new(),
        target_triple: None,
    };

    run(&tmp_file, &[], &opts).await
}

/// A cached script artifact is reusable only when caching is enabled, the
/// `.compiled` stamp is present, AND the compiled binary actually exists on
/// disk. A stamp without its binary (evicted/partial cache, or an interrupted
/// prior compile) must trigger a recompile rather than an attempt to execute a
/// missing path — which on Windows surfaces as a bare, path-less `os error 3`
/// (the clean-room idempotency failure mode).
fn cache_artifact_reusable(no_cache: bool, stamp: &Path, binary: &Path) -> bool {
    !no_cache && stamp.exists() && binary.exists()
}

#[cfg(test)]
mod tests {
    use super::cache_artifact_reusable;

    #[test]
    fn cache_hit_requires_both_stamp_and_binary() {
        let dir = tempfile::tempdir().expect("tempdir");
        let stamp = dir.path().join(".compiled");
        let binary = dir.path().join("vox-script.exe");

        // Nothing on disk yet -> not reusable.
        assert!(!cache_artifact_reusable(false, &stamp, &binary));

        // Stamp present but binary missing (the bug) -> NOT reusable; recompile.
        std::fs::write(&stamp, "hash").expect("write stamp");
        assert!(!cache_artifact_reusable(false, &stamp, &binary));

        // Both present -> reusable.
        std::fs::write(&binary, b"\0").expect("write binary");
        assert!(cache_artifact_reusable(false, &stamp, &binary));

        // `--no-cache` always recompiles, even with both present.
        assert!(!cache_artifact_reusable(true, &stamp, &binary));
    }
}
