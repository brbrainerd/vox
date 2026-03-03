//! High-level orchestration for Python environment setup.
//!
//! `PySetup` combines Python environment detection, `pyproject.toml` generation,
//! `uv sync`, and (optionally) a Python-capable Dockerfile generation into a
//! single call from `vox container init`.

use std::path::Path;
use crate::env::{PythonEnv, SetupPlan};
use crate::pyproject::generate_pyproject_toml;
use crate::python_dockerfile::generate_python_dockerfile;

/// Options for the Python environment setup step.
#[derive(Debug, Clone, Default)]
pub struct PySetupOpts {
    /// Project name used in generated files.
    pub project_name: String,
    /// Python module names from `@py.import` declarations.
    pub py_imports: Vec<String>,
    /// Whether to generate a Dockerfile (in addition to pyproject.toml).
    pub generate_dockerfile: bool,
    /// Output directory for generated files.
    pub out_dir: std::path::PathBuf,
}

/// Execute the full Python environment setup flow.
///
/// 1. Detect `uv`, Python, CUDA.
/// 2. Auto-install Python 3.12 via `uv python install 3.12` (if uv is available).
/// 3. Generate `pyproject.toml` with correct wheel sources.
/// 4. Run `uv sync` — creates `.venv` in `out_dir` automatically.
/// 5. Optionally generate a CUDA-aware `Dockerfile`.
///
/// After this runs, compiled Vox binaries that use `@py.import` do **not**
/// need `PYTHONPATH` to be set — the `vox-py` runtime auto-detects `.venv`.
pub fn run_py_setup(opts: &PySetupOpts) -> anyhow::Result<()> {
    let env = PythonEnv::detect();
    let plan = SetupPlan::from_env(&env, &opts.py_imports);

    println!("🐍 Vox Python environment setup");
    plan.print_summary();

    if plan.needs_uv {
        println!("\n⚠  uv is not installed. Install it first:");
        if cfg!(target_os = "windows") {
            println!("     PowerShell: irm https://astral.sh/uv/install.ps1 | iex");
            println!("     or: winget install --id=astral-sh.uv -e");
        } else {
            println!("     curl -LsSf https://astral.sh/uv/install.sh | sh");
        }
        return Ok(());
    }

    // Auto-install Python 3.12 via uv (idempotent — uv skips if already present)
    println!("⬇  Ensuring Python 3.12 is available via uv…");
    let py_install = std::process::Command::new("uv")
        .args(["python", "install", "3.12"])
        .status();
    match py_install {
        Ok(s) if s.success() => println!("✓ Python 3.12 ready"),
        Ok(_) => println!("⚠  `uv python install 3.12` exited with a non-zero code — continuing anyway"),
        Err(e) => println!("⚠  Could not run `uv python install 3.12`: {e} — continuing anyway"),
    }

    // Ensure the output directory exists.
    std::fs::create_dir_all(&opts.out_dir)
        .map_err(|e| anyhow::anyhow!("Could not create output dir {:?}: {e}", opts.out_dir))?;

    // Generate pyproject.toml
    let pyproject_path = opts.out_dir.join("pyproject.toml");
    let pyproject_content = generate_pyproject_toml(
        &opts.project_name,
        &opts.py_imports,
        &env,
    );
    std::fs::write(&pyproject_path, &pyproject_content)
        .map_err(|e| anyhow::anyhow!("Could not write pyproject.toml: {e}"))?;
    println!("✓ Wrote {}", pyproject_path.display());

    // Run uv sync — creates .venv automatically
    env.uv_sync(&opts.out_dir)?;
    println!("✓ uv sync complete");

    // Optionally generate a Dockerfile
    if opts.generate_dockerfile {
        let dockerfile_path = opts.out_dir.join("Dockerfile");
        let dockerfile_content = generate_python_dockerfile(&opts.project_name, &env, &opts.py_imports);
        std::fs::write(&dockerfile_path, &dockerfile_content)
            .map_err(|e| anyhow::anyhow!("Could not write Dockerfile: {e}"))?;
        println!("✓ Wrote {}", dockerfile_path.display());
    }

    println!("\n✅ Python environment ready");
    if env.has_gpu {
        println!("   GPU detected — CUDA {} wheels installed", env.cuda_version.as_deref().unwrap_or("unknown"));
    } else {
        println!("   CPU-only wheels installed (no GPU detected)");
    }

    // Show the resolved site-packages dir so developers know where packages live
    if let Some(sp) = env.site_packages_path() {
        println!("   📦 site-packages: {}", sp.display());
        println!("   ℹ️  No PYTHONPATH needed — vox-py auto-detects .venv at runtime");
    }

    Ok(())
}

/// Check what setup actions are required without executing them.
pub fn check_setup_needed(out_dir: &Path, py_imports: &[String]) -> bool {
    if py_imports.is_empty() {
        return false;
    }
    // If pyproject.toml doesn't exist yet, setup is needed.
    !out_dir.join("pyproject.toml").exists()
}
