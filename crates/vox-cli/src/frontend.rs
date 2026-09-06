//! Shared frontend scaffold, install, and build logic for run and bundle commands.
//!
//! `pnpm` invocations (`vox run` scaffold) use `pnpm_executable` as the single source of truth
//! for the OS-specific binary name.

use anyhow::{Context, Result};
use std::path::Path;

use crate::config;
use crate::fs_utils;
use crate::templates;
use vox_bounded_fs::read_utf8_path_capped;

/// Basename of the **pnpm** CLI for this OS (`pnpm.cmd` on Windows, `pnpm` elsewhere).
#[must_use]
pub fn pnpm_executable() -> &'static str {
    if cfg!(windows) { "pnpm.cmd" } else { "pnpm" }
}

/// Spawns **`pnpm run dev:ssr-upstream`** by default (unless **`VOX_ORCHESTRATE_VITE=0`**) so Axum can proxy HTML to Vite (see **`VOX_SSR_DEV_URL`**).
pub struct OrchestratedViteGuard(Option<std::process::Child>);

impl OrchestratedViteGuard {
    /// No child process (default when frontend is skipped or orchestration is off).
    #[must_use]
    pub fn disabled() -> Self {
        Self(None)
    }

    /// Unless **`VOX_ORCHESTRATE_VITE=0`**, start Vite on port **3001**.
    ///
    /// Returns an optional **`VOX_SSR_DEV_URL`** pair for the **`cargo run`** child when unset
    /// (Rust 2024 avoids mutating process environment via `set_var` here).
    pub fn maybe_spawn(app_dir: &Path) -> Result<(Self, Option<(String, String)>)> {
        if std::env::var("VOX_ORCHESTRATE_VITE").ok().as_deref() == Some("0") {
            return Ok((Self(None), None));
        }
        let pkg_path = app_dir.join("package.json");
        let pkg_raw = std::fs::read_to_string(&pkg_path)
            .with_context(|| format!("failed to read {}", pkg_path.display()))?;
        let pkg_json: serde_json::Value = serde_json::from_str(&pkg_raw)
            .with_context(|| format!("failed to parse {}", pkg_path.display()))?;
        let has_script = |name: &str| {
            pkg_json
                .get("scripts")
                .and_then(|scripts| scripts.get(name))
                .and_then(|v| v.as_str())
                .is_some()
        };
        let (script, url) = if has_script("dev:ssr-upstream") {
            ("dev:ssr-upstream", "http://127.0.0.1:3001".to_string())
        } else if has_script("dev") {
            (
                "dev",
                format!("http://127.0.0.1:{}", crate::config::default_port()),
            )
        } else {
            anyhow::bail!(
                "{} has no `dev:ssr-upstream` or `dev` script",
                pkg_path.display()
            );
        };
        let pnpm = pnpm_executable();
        println!(
            "  Spawning preview server (pnpm run {script}) in {} (opt-out via VOX_ORCHESTRATE_VITE=0)...",
            app_dir.display(),
        );
        let child = std::process::Command::new(pnpm)
            .args(["run", script])
            .current_dir(app_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .with_context(|| {
                format!(
                    "failed to spawn pnpm dev:ssr-upstream in {}",
                    app_dir.display()
                )
            })?;
        std::thread::sleep(vox_config::timeouts::D_2S);
        let inject = if std::env::var("VOX_SSR_DEV_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .is_some()
        {
            None
        } else {
            println!("  Passing VOX_SSR_DEV_URL={url} to generated Axum (GET proxy for non-/api)");
            Some(("VOX_SSR_DEV_URL".to_string(), url.to_string()))
        };
        Ok((Self(Some(child)), inject))
    }
}

impl Drop for OrchestratedViteGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Scaffold a Vite + React project around generated TS components.
///
/// When `tanstack_start` is true, writes a TanStack Start layout (`src/routes/*`, `router.tsx`,
/// `routeTree.gen.ts`) instead of `index.html` + `main.tsx`. Controlled by `Vox.toml` `[web] tanstack_start`
/// or **`VOX_WEB_TANSTACK_START=1`**.
pub fn scaffold_react_app(
    app_dir: &Path,
    generated_ts_dir: &Path,
    tanstack_start: bool,
) -> Result<()> {
    let src_dir = app_dir.join("src");
    let generated_dir = src_dir.join("generated");

    std::fs::create_dir_all(&generated_dir)
        .context("Failed to create app/src/generated directory")?;

    let port = config::default_port();
    // TanStack Start always uses file-based `src/routes/*` + seeded `routeTree.gen.ts`.
    // Compiler output is `routes.manifest.ts` + components (no programmatic `VoxTanStackRouter.tsx`).
    let file_route_tsr_pregen = tanstack_start;
    let pkg = templates::package_json(tanstack_start, file_route_tsr_pregen);
    let vite = templates::vite_config(port, tanstack_start);

    std::fs::write(app_dir.join("package.json"), pkg).context("Failed to write package.json")?;
    std::fs::write(
        app_dir.join("pnpm-workspace.yaml"),
        templates::pnpm_workspace_yaml(),
    )
    .context("Failed to write pnpm-workspace.yaml")?;
    std::fs::write(app_dir.join("vite.config.ts"), vite)
        .context("Failed to write vite.config.ts")?;
    std::fs::write(app_dir.join("tsconfig.json"), templates::tsconfig_json())
        .context("Failed to write tsconfig.json")?;
    std::fs::write(
        app_dir.join("components.json"),
        templates::components_json_shadcn_client(),
    )
    .context("Failed to write components.json")?;
    std::fs::write(src_dir.join("index.css"), templates::index_css())
        .context("Failed to write index.css")?;

    let manifest_present = generated_ts_dir.join("routes.manifest.ts").is_file();

    if tanstack_start {
        if manifest_present {
            std::fs::write(
                src_dir.join("vox-manifest-route-adapter.tsx"),
                templates::vox_manifest_route_adapter_tsx(),
            )
            .context("Failed to write vox-manifest-route-adapter.tsx (Start + manifest)")?;
        }
        std::fs::create_dir_all(src_dir.join("routes")).context("Failed to create src/routes")?;
        std::fs::write(
            src_dir.join("routes/__root.tsx"),
            templates::tanstack_start_root_tsx(),
        )
        .context("Failed to write routes/__root.tsx")?;
        let has_app = generated_ts_dir.join("App.tsx").is_file();
        let component_name = fs_utils::find_component_name(generated_ts_dir)?;
        let index_tsx = if has_app {
            templates::tanstack_start_index_for_app().to_string()
        } else {
            templates::tanstack_start_index_for_component(&component_name)
        };
        std::fs::write(src_dir.join("routes/index.tsx"), index_tsx)
            .context("Failed to write routes/index.tsx")?;
        std::fs::write(
            src_dir.join("routeTree.gen.ts"),
            templates::tanstack_start_route_tree_gen(),
        )
        .context("Failed to write routeTree.gen.ts")?;
        std::fs::write(
            src_dir.join("router.tsx"),
            templates::tanstack_start_router_tsx(),
        )
        .context("Failed to write router.tsx")?;
    } else {
        std::fs::write(app_dir.join("index.html"), templates::index_html())
            .context("Failed to write index.html")?;
        if manifest_present {
            std::fs::write(
                src_dir.join("vox-manifest-route-adapter.tsx"),
                templates::vox_manifest_route_adapter_tsx(),
            )
            .context("Failed to write vox-manifest-route-adapter.tsx")?;
            std::fs::write(
                src_dir.join("vox-manifest-router.tsx"),
                templates::vox_spa_manifest_router_tsx(),
            )
            .context("Failed to write vox-manifest-router.tsx")?;
            std::fs::write(
                src_dir.join("main.tsx"),
                templates::main_tsx_manifest_entry(),
            )
            .context("Failed to write main.tsx (manifest router)")?;
        } else {
            let component_name = fs_utils::find_component_name(generated_ts_dir)?;
            std::fs::write(
                src_dir.join("main.tsx"),
                templates::main_tsx(&component_name),
            )
            .context("Failed to write main.tsx")?;
        }
    }

    for entry in
        std::fs::read_dir(generated_ts_dir).context("Failed to read generated TS directory")?
    {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|e| e == "tsx" || e == "ts" || e == "css")
        {
            let dest = generated_dir.join(path.file_name().unwrap());
            std::fs::copy(&path, &dest)
                .with_context(|| format!("Failed to copy {} to generated/", path.display()))?;
        }
    }

    Ok(())
}

/// Format captured subprocess streams so a failed `pnpm` invocation surfaces
/// the real error instead of a one-line "pnpm install failed".
pub fn format_process_output(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}")
}

/// Run `pnpm` in `app_dir`, capturing both streams. On failure the returned
/// error includes the exit status and the captured output.
pub fn run_pnpm(app_dir: &Path, args: &[&str], fail_label: &str) -> Result<()> {
    let pnpm = pnpm_executable();
    let output = std::process::Command::new(pnpm)
        .args(args)
        .current_dir(app_dir)
        .output()
        .with_context(|| {
            format!(
                "Failed to run {pnpm} {}. Is Node.js and pnpm installed?",
                args.join(" ")
            )
        })?;
    if !output.status.success() {
        let text = format_process_output(&output);
        eprint!("{text}");
        anyhow::bail!("{fail_label} (exit {}):\n{text}", output.status);
    }
    Ok(())
}

/// Run pnpm install and build in the scaffolded project.
pub fn npm_install_and_build(app_dir: &Path) -> Result<()> {
    if !app_dir.join("node_modules").exists() {
        println!("  Installing pnpm dependencies...");
        run_pnpm(
            app_dir,
            &["install", "--prefer-offline"],
            "pnpm install failed",
        )?;
    }

    try_pnpm_routes_gen(app_dir)?;

    println!("  Building frontend assets...");
    run_pnpm(app_dir, &["run", "build"], "Frontend build failed")?;

    Ok(())
}

/// When the app uses TanStack file routes (not programmatic `voxRouteTree` re-export), run
/// **`pnpm run routes:gen`** so `routeTree.gen.ts` matches `src/routes/**` (TanStack Router CLI).
fn try_pnpm_routes_gen(app_dir: &Path) -> Result<()> {
    let route_tree = app_dir.join("src").join("routeTree.gen.ts");
    if !route_tree.is_file() {
        return Ok(());
    }
    let rt = read_utf8_path_capped(&route_tree)
        .with_context(|| format!("read {}", route_tree.display()))?;
    if rt.contains("voxRouteTree") {
        return Ok(());
    }
    let pkg_path = app_dir.join("package.json");
    if !pkg_path.is_file() {
        return Ok(());
    }
    let pkg =
        read_utf8_path_capped(&pkg_path).with_context(|| format!("read {}", pkg_path.display()))?;
    if !pkg.contains("\"routes:gen\"") {
        return Ok(());
    }
    println!("  Regenerating TanStack route tree (pnpm run routes:gen)...");
    run_pnpm(
        app_dir,
        &["run", "routes:gen"],
        "pnpm run routes:gen failed",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_process_output_includes_both_streams() {
        let probed = std::process::Command::new(env!("CARGO"))
            .arg("--version")
            .output()
            .expect("cargo --version");
        let output = std::process::Output {
            status: probed.status,
            stdout: b"OUT-PAYLOAD".to_vec(),
            stderr: b"ERR-PAYLOAD".to_vec(),
        };
        let text = format_process_output(&output);
        assert!(text.contains("OUT-PAYLOAD"), "{text}");
        assert!(text.contains("ERR-PAYLOAD"), "{text}");
        assert!(text.contains("--- stdout ---"), "{text}");
        assert!(text.contains("--- stderr ---"), "{text}");
    }
}

/// Copy built static assets from Vite output to the backend's public directory.
pub fn copy_built_assets(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        anyhow::bail!("Built assets not found at {}", from.display());
    }
    if to.exists() {
        std::fs::remove_dir_all(to).ok();
    }
    std::fs::create_dir_all(to)?;
    fs_utils::copy_dir_recursive(from, to).with_context(|| {
        format!(
            "Failed to copy assets from {} to {}",
            from.display(),
            to.display()
        )
    })?;
    Ok(())
}
