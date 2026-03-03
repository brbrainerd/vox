//! Python environment detection and setup.
//!
//! Detects `uv`, Python, CUDA version, and determines the correct PyTorch
//! wheel source (CPU-only vs CUDA). Works on Linux, macOS, and Windows.

use std::process::Command;

/// Information about the detected Python environment.
#[derive(Debug, Clone)]
pub struct PythonEnv {
    /// Whether `uv` is installed and on PATH.
    pub uv_available: bool,
    /// `uv` version string, if available.
    pub uv_version: Option<String>,
    /// Python version string (from `uv python --version` or `python --version`).
    pub python_version: Option<String>,
    /// Detected CUDA version (e.g. `"12.4"`), if available.
    pub cuda_version: Option<String>,
    /// Whether NVIDIA GPU is present (nvcc or nvidia-smi detected).
    pub has_gpu: bool,
}

impl PythonEnv {
    /// Detect the Python environment on the current machine.
    pub fn detect() -> Self {
        let uv_out = run_cmd("uv", &["--version"]);
        let uv_available = uv_out.is_some();
        let uv_version = uv_out;

        let python_version = run_cmd("uv", &["python", "--version"])
            .or_else(|| run_cmd("python3", &["--version"]))
            .or_else(|| run_cmd("python", &["--version"]));

        // Detect CUDA via nvcc (compile-time) or nvidia-smi (runtime).
        let cuda_version = detect_cuda();
        let has_gpu = cuda_version.is_some() || run_cmd("nvidia-smi", &["-L"]).is_some();

        PythonEnv {
            uv_available,
            uv_version,
            python_version,
            cuda_version,
            has_gpu,
        }
    }

    /// Returns the PyTorch extra-index-url for the detected CUDA version.
    ///
    /// Falls back to CPU-only wheels when no GPU is detected, or when the
    /// CUDA version does not have a specific wheel source.
    ///
    /// Reference: https://pytorch.org/get-started/locally/
    pub fn pytorch_index_url(&self) -> &'static str {
        match self.cuda_version.as_deref() {
            Some(v) if v.starts_with("13.") => {
                "https://download.pytorch.org/whl/cu130"
            }
            Some(v) if v.starts_with("12.4") || v.starts_with("12.5") || v.starts_with("12.6") => {
                "https://download.pytorch.org/whl/cu124"
            }
            Some(v) if v.starts_with("12.1") || v.starts_with("12.2") || v.starts_with("12.3") => {
                "https://download.pytorch.org/whl/cu121"
            }
            Some(v) if v.starts_with("11.8") => {
                "https://download.pytorch.org/whl/cu118"
            }
            _ => {
                "https://download.pytorch.org/whl/cpu"
            }
        }
    }

    /// Install Python dependencies from pyproject.toml using `uv sync`.
    ///
    /// Requires `uv` to be installed. Returns an error if `uv` is not found.
    pub fn uv_sync(&self, project_dir: &std::path::Path) -> anyhow::Result<()> {
        if !self.uv_available {
            let install_hint = if cfg!(target_os = "windows") {
                "  PowerShell: irm https://astral.sh/uv/install.ps1 | iex\n  \
                 or: winget install --id=astral-sh.uv -e"
            } else {
                "  curl -LsSf https://astral.sh/uv/install.sh | sh"
            };
            anyhow::bail!(
                "uv is not installed. Install it with:\n{install_hint}\n\
                 Then ensure Python 3.12 is available via `uv python install 3.12`"
            );
        }

        tracing::info!("Running uv sync in {:?}", project_dir);
        let status = Command::new("uv")
            .arg("sync")
            .current_dir(project_dir)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run uv sync: {e}"))?;

        if !status.success() {
            anyhow::bail!("uv sync failed with exit code: {:?}", status.code());
        }
        Ok(())
    }

    /// Install a list of packages using `uv add`, with CUDA-aware index URL.
    pub fn uv_add_packages(
        &self,
        project_dir: &std::path::Path,
        packages: &[&str],
    ) -> anyhow::Result<()> {
        if !self.uv_available {
            anyhow::bail!("uv is not installed");
        }
        if packages.is_empty() {
            return Ok(());
        }

        let index_url = self.pytorch_index_url();
        let mut cmd = Command::new("uv");
        cmd.arg("add")
            .arg("--extra-index-url")
            .arg(index_url);
        for pkg in packages {
            cmd.arg(pkg);
        }
        cmd.current_dir(project_dir);

        tracing::info!("Running uv add {:?} with index {}", packages, index_url);
        let status = cmd
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run uv add: {e}"))?;

        if !status.success() {
            anyhow::bail!("uv add failed");
        }
        Ok(())
    }

    /// Return the path of the uv-managed virtual environment, if one can be
    /// located. Checks `UV_PROJECT_ENVIRONMENT`, `VIRTUAL_ENV`, then `.venv`
    /// in the current working directory, then falls back to querying uv.
    ///
    /// The returned path is the *venv root*, not site-packages. Use
    /// [`Self::site_packages_path`] to get the importable directory.
    pub fn venv_path(&self) -> Option<std::path::PathBuf> {
        use std::env;
        use std::path::PathBuf;

        // 1. UV_PROJECT_ENVIRONMENT
        if let Ok(p) = env::var("UV_PROJECT_ENVIRONMENT") {
            let base = PathBuf::from(p);
            if base.exists() {
                return Some(base);
            }
        }

        // 2. VIRTUAL_ENV
        if let Ok(p) = env::var("VIRTUAL_ENV") {
            let base = PathBuf::from(p);
            if base.exists() {
                return Some(base);
            }
        }

        // 3. .venv in cwd
        if let Ok(cwd) = env::current_dir() {
            let venv = cwd.join(".venv");
            if venv.exists() {
                return Some(venv);
            }
        }

        // 4. Ask uv
        let out = Command::new("uv")
            .args(["run", "python", "-c", "import sys; print(sys.prefix)"])
            .output()
            .ok()?;
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let path = std::path::PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }

        None
    }

    /// Return the `site-packages` directory inside the uv venv, if found.
    ///
    /// On Windows: `<venv>/Lib/site-packages`
    /// On POSIX: `<venv>/lib/python<version>/site-packages`
    pub fn site_packages_path(&self) -> Option<std::path::PathBuf> {
        let venv = self.venv_path()?;

        // Windows fast path
        let win = venv.join("Lib").join("site-packages");
        if win.exists() {
            return Some(win);
        }

        // POSIX: scan lib/ for python3.x/site-packages
        let lib = venv.join("lib");
        if lib.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&lib) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name.to_string_lossy().starts_with("python") {
                        let sp = entry.path().join("site-packages");
                        if sp.exists() {
                            return Some(sp);
                        }
                    }
                }
            }
        }

        None
    }

    /// Run a quick script to literally check if PyTorch reports CUDA is available.
    /// This validates actual driver bindings, avoiding silently falling back to CPU.
    pub fn validate_pytorch_cuda(&self, project_dir: &std::path::Path) -> anyhow::Result<bool> {
        if !self.uv_available || !self.has_gpu {
            return Ok(false);
        }

        let out = Command::new("uv")
            .arg("run")
            .arg("python")
            .arg("-c")
            .arg("import torch; print(torch.cuda.is_available())")
            .current_dir(project_dir)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed testing torch.cuda: {e}"))?;

        if !out.status.success() {
            // Could mean torch isn't installed at all
            return Ok(false);
        }

        let is_available = String::from_utf8_lossy(&out.stdout).trim() == "True";

        if self.has_gpu && !is_available {
            tracing::warn!("GPU was detected but PyTorch reports CUDA is NOT available!");
            tracing::warn!("This means PyTorch will fall back to slower CPU inference.");
            tracing::warn!("Verify NVIDIA drivers match the toolkit version: {}", self.cuda_version.as_deref().unwrap_or("unknown"));
        } else if self.has_gpu && is_available {
            tracing::info!("PyTorch successfully bound to CUDA GPU!");
        }

        Ok(is_available)
    }
}

/// Detect CUDA version from `nvcc --version` or `nvidia-smi`.
fn detect_cuda() -> Option<String> {
    // Try nvcc first (most reliable — it's the compiler version)
    if let Some(out) = run_cmd("nvcc", &["--version"]) {
        // Output line: "Cuda compilation tools, release 12.4, V12.4.131"
        for line in out.lines() {
            if line.contains("release") {
                if let Some(v) = line
                    .split("release")
                    .nth(1)
                    .and_then(|s| s.split(',').next())
                    .map(|s| s.trim().to_string())
                {
                    return Some(v);
                }
            }
        }
    }

    // Fall back to nvidia-smi (runtime version, may differ from nvcc)
    if let Some(out) = run_cmd("nvidia-smi", &[]) {
        // First line usually contains "CUDA Version: 12.4"
        for line in out.lines() {
            if line.contains("CUDA Version:") {
                if let Some(v) = line.split("CUDA Version:").nth(1) {
                    let ver = v
                        .split('|')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !ver.is_empty() {
                        return Some(ver);
                    }
                }
            }
        }
    }

    None
}

/// Run a command and return trimmed stdout, or None on failure.
fn run_cmd(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Summary of what setup actions are needed, without performing them.
#[derive(Debug, Clone)]
pub struct SetupPlan {
    /// uv needs to be installed.
    pub needs_uv: bool,
    /// Python packages that will be added/synced.
    pub packages: Vec<String>,
    /// PyTorch wheel source URL.
    pub torch_index_url: &'static str,
    /// GPU detected.
    pub has_gpu: bool,
}

impl SetupPlan {
    /// Build a setup plan from the detected environment and required modules.
    pub fn from_env(env: &PythonEnv, required_modules: &[String]) -> Self {
        SetupPlan {
            needs_uv: !env.uv_available,
            packages: required_modules.to_vec(),
            torch_index_url: env.pytorch_index_url(),
            has_gpu: env.has_gpu,
        }
    }

    /// Print the plan to stdout in a human-readable format.
    pub fn print_summary(&self) {
        if self.needs_uv {
            println!("  • uv not found — will need installation");
        }
        if !self.packages.is_empty() {
            println!("  • Python packages: {}", self.packages.join(", "));
        }
        println!("  • PyTorch source: {}", self.torch_index_url);
        if self.has_gpu {
            if self.torch_index_url.contains("cpu") {
                println!("  ⚠️ WARNING: GPU detected, but CUDA version is unknown or unsupported/mismatched.");
                println!("  ⚠️ Falling back to CPU-only PyTorch wheels! Ensure NVIDIA driver and CUDA toolkit are installed.");
            } else {
                println!("  • GPU detected — CUDA wheels will be used");
            }
        } else {
            println!("  • No GPU detected — CPU-only wheels will be used");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pytorch_index_cuda_130() {
        let env = PythonEnv {
            uv_available: true,
            uv_version: None,
            python_version: None,
            cuda_version: Some("13.0".to_string()),
            has_gpu: true,
        };
        assert!(env.pytorch_index_url().contains("cu130"));
    }

    #[test]
    fn pytorch_index_cuda_124() {
        let env = PythonEnv {
            uv_available: true,
            uv_version: None,
            python_version: None,
            cuda_version: Some("12.4".to_string()),
            has_gpu: true,
        };
        assert!(env.pytorch_index_url().contains("cu124"));
    }

    #[test]
    fn pytorch_index_cuda_118() {
        let env = PythonEnv {
            uv_available: true,
            uv_version: None,
            python_version: None,
            cuda_version: Some("11.8".to_string()),
            has_gpu: true,
        };
        assert!(env.pytorch_index_url().contains("cu118"));
    }

    #[test]
    fn pytorch_index_no_cuda() {
        let env = PythonEnv {
            uv_available: true,
            uv_version: None,
            python_version: None,
            cuda_version: None,
            has_gpu: false,
        };
        assert!(env.pytorch_index_url().contains("cpu"));
    }
}
