//! `vox compile` — umbrella entry for native installers, scripts, and workspace builds.
//!
//! Delegates to `super::bundle`, script compilation, Tauri packaging hints, and optional archive emit.

use crate::cli_args::CompileArgs;
use crate::commands::bundle;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
#[cfg(feature = "script-execution")]
use tokio::fs;
use vox_cli_core::cli_args::{BundleMode, CompileKind};
use vox_codegen::codegen_rust::RustAppShell;
use vox_config::project_manifest::ProjectManifest;

#[cfg(feature = "script-execution")]
use crate::commands::runtime::run::script;

fn rust_app_shell_for_compile_app(kind: CompileKind) -> RustAppShell {
    match kind {
        CompileKind::Desktop | CompileKind::MobileAndroid | CompileKind::MobileIos => {
            RustAppShell::TauriApp
        }
        CompileKind::NativeBinary => RustAppShell::AxumLocalServer,
        _ => RustAppShell::AxumLocalServer,
    }
}

/// Run `vox compile` / `vox fabrica compile`.
pub async fn run(args: &CompileArgs) -> Result<()> {
    if args.workspace {
        return run_workspace(args).await;
    }
    run_single_workspace_member(args).await
}

async fn run_single_workspace_member(args: &CompileArgs) -> Result<()> {
    let file = args
        .file
        .as_ref()
        .context("compile: pass a `.vox` file or use `--workspace`")?;

    let manifest_dir = manifest_dir_for_entry(file);
    let vox_toml = manifest_dir.join("Vox.toml");
    let proj = ProjectManifest::load(&vox_toml)?;
    print_bundle_preflight_notes(args.kind, &proj);

    match args.kind {
        CompileKind::Script => {
            #[cfg(feature = "script-execution")]
            {
                bundle::run(
                    file,
                    &args.out_dir,
                    args.triple.as_deref(),
                    args.release,
                    BundleMode::Script,
                rust_app_shell_for_compile_app(args.kind),
            )
                .await?;
            }
            #[cfg(not(feature = "script-execution"))]
            {
                anyhow::bail!(
                    "`vox compile --target script` requires `--features script-execution`"
                );
            }
        }
        CompileKind::NativeBinary => {
            bundle::run(
                file,
                &args.out_dir,
                args.triple.as_deref(),
                args.release,
                BundleMode::App,
                rust_app_shell_for_compile_app(args.kind),
            )
            .await?;
        }
        CompileKind::Desktop | CompileKind::MobileAndroid | CompileKind::MobileIos => {
            bundle::run(
                file,
                &args.out_dir,
                args.triple.as_deref(),
                args.release,
                BundleMode::App,
                rust_app_shell_for_compile_app(args.kind),
            )
            .await?;
            emit_tauri_and_assets(&proj, &manifest_dir, args, file)?;
            if matches!(
                args.kind,
                CompileKind::MobileAndroid | CompileKind::MobileIos
            ) {
                println!(
                    "Mobile compile: Tauri mobile installers require Android SDK / Xcode — see docs/src/architecture/vox-application-packaging-ssot-2026.md"
                );
            }
        }
        CompileKind::Server => {
            anyhow::bail!(
                "compile --target server: use `vox deploy` for OCI/server packaging (see docs/src/reference/vox-portability-ssot.md)"
            );
        }
        CompileKind::Wasi => {
            #[cfg(feature = "script-execution")]
            {
                let opts = script::ScriptOpts {
                    sandbox: false,
                    allow_mcp: false,
                    no_cache: false,
                    isolation: Some("wasm".into()),
                    trust_class: None,
                    wasi_dirs: Vec::new(),
                    target_triple: args.triple.clone(),
                };
                let (artifact_path, backend) = script::compile(file, &opts).await?;
                fs::create_dir_all(&args.out_dir).await?;
                let app_name = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "script".into());
                let bin_name = if backend.cache_label().contains("wasi") {
                    format!("{app_name}.wasm")
                } else {
                    format!("{app_name}.wasm")
                };
                let dest = args.out_dir.join(bin_name);
                fs::copy(&artifact_path, &dest)
                    .await
                    .context("copy WASI artifact to out_dir")?;
                println!("✓ WASI artifact: {}", dest.display());
            }
            #[cfg(not(feature = "script-execution"))]
            {
                anyhow::bail!("`vox compile --target wasi` requires `--features script-execution`");
            }
        }
    }

    maybe_archive_dist_binary(args, file)?;
    Ok(())
}

fn manifest_dir_for_entry(file: &Path) -> PathBuf {
    let mut dir = file
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    loop {
        if dir.join("Vox.toml").is_file() {
            return dir;
        }
        if !dir.pop() {
            return file
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf();
        }
    }
}

async fn run_workspace(args: &CompileArgs) -> Result<()> {
    let root = std::env::current_dir()?;
    let manifest_path = root.join("Vox.toml");
    let proj = ProjectManifest::load(&manifest_path)
        .with_context(|| format!("read workspace manifest {}", manifest_path.display()))?;
    let paths = proj.member_manifest_paths();
    if paths.is_empty() {
        anyhow::bail!(
            "workspace compile: add `[workspace]` members = [\"pkg/a\", ...] to {}",
            manifest_path.display()
        );
    }

    for mpath in paths {
        if !mpath.is_file() {
            eprintln!("skip missing member manifest {}", mpath.display());
            continue;
        }
        let member_dir = mpath.parent().context("member Vox.toml parent")?;
        let main_vox = find_default_entry_vox(member_dir)?;
        let mut sub = args.clone();
        sub.workspace = false;
        sub.file = Some(main_vox);
        run_single_workspace_member(&sub).await?;
    }
    Ok(())
}

fn find_default_entry_vox(dir: &Path) -> Result<PathBuf> {
    let candidates = [dir.join("src/main.vox"), dir.join("main.vox")];
    for p in candidates {
        if p.is_file() {
            return Ok(p);
        }
    }
    anyhow::bail!("member {} has no src/main.vox or main.vox", dir.display())
}

fn print_bundle_preflight_notes(kind: CompileKind, proj: &ProjectManifest) {
    let label = match kind {
        CompileKind::Desktop => "desktop",
        CompileKind::MobileAndroid => "mobile-android",
        CompileKind::MobileIos => "mobile-ios",
        _ => return,
    };
    match &proj.bundle {
        None => {
            eprintln!(
                "note: `{label}` packaging: add `[bundle]` to Vox.toml — see `contracts/manifest/vox-bundle.v1.schema.json`"
            );
        }
        Some(b) => {
            let id = b.identifier.as_deref().unwrap_or("");
            if id.is_empty() || id == "com.vox.generated" {
                eprintln!(
                    "warning: `[bundle].identifier` is unset or still the placeholder — set a reverse-DNS id before app-store signing"
                );
            }
        }
    }
}

fn bundle_identifier_display<'a>(
    proj: &'a ProjectManifest,
    fallback_dir: &'a Path,
) -> (&'a str, &'a str) {
    let id = proj
        .bundle
        .as_ref()
        .and_then(|b| b.identifier.as_deref())
        .unwrap_or("com.vox.generated");
    let name = proj
        .bundle
        .as_ref()
        .and_then(|b| b.display_name.as_deref())
        .unwrap_or_else(|| {
            fallback_dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("vox-app")
        });
    (id, name)
}

fn required_capabilities_for_packaging(
    vox_file: &Path,
) -> Result<vox_compiler::required_capabilities::RequiredRuntimeCapabilities> {
    let src = std::fs::read_to_string(vox_file)
        .with_context(|| format!("read compile entry {}", vox_file.display()))?;
    let tokens = vox_compiler::lexer::cursor::lex(&src);
    let module = vox_compiler::parser::parse(tokens).map_err(|errors| {
        anyhow::anyhow!(
            "parse compile entry {} failed with {} error(s)",
            vox_file.display(),
            errors.len()
        )
    })?;
    let _ = vox_compiler::typeck::typecheck_module(&module, &src);
    let hir = vox_compiler::hir::lower_module(&module);
    Ok(vox_codegen::projection_bundle::project_bundle_from_hir(&hir).capabilities)
}

fn emit_tauri_and_assets(
    proj: &ProjectManifest,
    manifest_dir: &Path,
    _args: &CompileArgs,
    vox_file: &Path,
) -> Result<()> {
    let generated =
        crate::fs_utils::run_target_dir_for_workspace(Some(manifest_dir)).join("generated");
    let (id, title) = bundle_identifier_display(proj, manifest_dir);
    let params = vox_tauri_codegen::TauriEmitParams {
        identifier: id,
        display_name: title,
        frontend_dist_relative: "../public",
    };
    let contracts_repo = vox_tauri_codegen::find_contracts_repo_root(manifest_dir);
    let required = required_capabilities_for_packaging(vox_file)?;
    vox_tauri_codegen::emit_tauri_packaging_hints(
        &generated,
        &params,
        contracts_repo.as_deref(),
        Some(&required),
    )
    .context("emit Tauri packaging hints")?;

    let src_tauri_conf = generated.join("src-tauri").join("tauri.conf.json");
    vox_tauri_codegen::write_tauri_desktop_config(&src_tauri_conf, &params).with_context(|| {
        format!(
            "write generated Tauri config {}",
            src_tauri_conf.display()
        )
    })?;

    if let Some(ref b) = proj.bundle {
        let assets = vox_codegen::assets::AssetManifest::from_bundle_fragment(
            manifest_dir,
            b.assets.as_ref().and_then(|a| a.icons.as_deref()),
            b.assets.as_ref().and_then(|a| a.splash.as_deref()),
            b.assets.as_ref().and_then(|a| a.ml_models.as_ref()),
            b.assets.as_ref().and_then(|a| a.fonts.as_ref()),
            b.assets.as_ref().and_then(|a| a.lazy),
        );
        assets.validate_preflight().context("bundle.assets")?;
        let stage = generated.join("packaged-assets");
        assets.stage_under(&stage).context("stage assets")?;
    }

    Ok(())
}

fn maybe_archive_dist_binary(args: &CompileArgs, file: &Path) -> Result<()> {
    if !args.archive {
        return Ok(());
    }
    let triple = match args.triple.as_deref() {
        Some(t) => t,
        None => crate::utils::install_policy::host_triple_for_release_binary_install()
            .context("archive requires `--triple` or a supported host triple")?,
    };

    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("app");
    let ext = if triple.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let bin_name = format!("{stem}{ext}");
    let bin_path = args.out_dir.join(&bin_name);
    if !bin_path.is_file() {
        anyhow::bail!(
            "archive: binary not found at {} (run bundle first)",
            bin_path.display()
        );
    }

    let version = env!("CARGO_PKG_VERSION");
    let archive_base = crate::utils::release_artifacts::artifact_filename(stem, version, triple);
    let archive_path = args.out_dir.join(&archive_base);
    if crate::utils::release_artifacts::is_windows_target(triple) {
        crate::utils::release_artifacts::package_zip(&bin_path, &archive_path, &bin_name)?;
    } else {
        crate::utils::release_artifacts::package_tar_gz(&bin_path, &archive_path, &bin_name)?;
    }
    let digest = crate::utils::release_artifacts::sha256_file(&archive_path)?;
    let chk = args.out_dir.join("checksums-compile.txt");
    let line = crate::utils::release_artifacts::checksum_line(&digest, &archive_base);
    std::fs::write(&chk, line).context("write checksums-compile.txt")?;
    println!("  archive: {}", archive_path.display());
    println!("  checksums-compile.txt");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_codegen::codegen_rust::RustAppShell;

    #[test]
    fn rust_app_shell_desktop_and_mobile_use_tauri() {
        assert_eq!(
            rust_app_shell_for_compile_app(CompileKind::Desktop),
            RustAppShell::TauriApp
        );
        assert_eq!(
            rust_app_shell_for_compile_app(CompileKind::MobileAndroid),
            RustAppShell::TauriApp
        );
        assert_eq!(
            rust_app_shell_for_compile_app(CompileKind::MobileIos),
            RustAppShell::TauriApp
        );
    }

    #[test]
    fn rust_app_shell_native_binary_stays_axum() {
        assert_eq!(
            rust_app_shell_for_compile_app(CompileKind::NativeBinary),
            RustAppShell::AxumLocalServer
        );
    }

    #[test]
    fn required_runtime_capability_ids_for_file_maps_uses_net() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("main.vox");
        std::fs::write(
            &file,
            r#"
@query fn ping() uses net to int { return 1 }
"#,
        )
        .unwrap();

        let caps = required_capabilities_for_packaging(&file).unwrap();
        assert_eq!(caps.capability_ids, vec!["net.http"]);
    }

    #[test]
    fn manifest_dir_for_src_main_uses_package_root() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("packages/desktop");
        std::fs::create_dir_all(pkg.join("src")).unwrap();
        std::fs::write(pkg.join("Vox.toml"), "[package]\nname = \"desktop\"\n").unwrap();
        let entry = pkg.join("src/main.vox");
        std::fs::write(&entry, "component App() { view: text() { \"ok\" } }\n").unwrap();

        assert_eq!(manifest_dir_for_entry(&entry), pkg);
    }
}
