//! Corpus mix + primary-source sync for Mens training (single prep path for CLI / schola / pipeline).
//!
//! Relative `--data-dir` / `--output-dir` / resume paths are anchored in
//! [`crate::training::contract::normalize_workspace_relative_path`] before mix and validation run.

use std::path::{Path, PathBuf};

use crate::corpus::{self, MixConfigSchema, MixRunOptions};

/// Relative path from workspace root to mix configuration.
pub const MIX_CONFIG_REL: &str = "mens/config/mix.yaml";

/// Whether two paths refer to the same existing file (handles relative vs absolute and `\\?\` on Windows).
fn same_existing_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if !a.is_file() || !b.is_file() {
        return false;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

/// `VOX_TRAIN_SKIP_CORPUS_MIX=1|true` skips mix entirely (operators / tests).
#[must_use]
pub fn corpus_mix_skip_from_env() -> bool {
    std::env::var("VOX_TRAIN_SKIP_CORPUS_MIX")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Resolve mix YAML path: prefer workspace root; fall back to `cwd/mens/config/mix.yaml` (pipeline legacy).
pub fn resolve_mix_config_path(workspace_root: Option<&Path>) -> PathBuf {
    if let Some(root) = workspace_root {
        root.join(MIX_CONFIG_REL)
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(MIX_CONFIG_REL)
    }
}

/// When the active `data_dir/train.jsonl` differs from mix’s primary source path, copy train into the
/// primary path so `run_mix` sees the latest pairs (workspace-relative primary paths).
pub fn sync_mix_primary_with_train_jsonl(
    workspace_root: Option<&Path>,
    data_dir: &Path,
    mix_yaml: &Path,
) -> anyhow::Result<()> {
    let Some(ws) = workspace_root else {
        return Ok(());
    };
    let Ok(cfg) = MixConfigSchema::load(mix_yaml) else {
        return Ok(());
    };
    let Some(primary) = cfg.sources.first() else {
        return Ok(());
    };
    let primary_resolved = ws.join(&primary.path);
    let train_jsonl = data_dir.join(super::preflight::PRIMARY_TRAIN_FILE);
    if same_existing_file(&primary_resolved, &train_jsonl) {
        tracing::debug!(
            path = %train_jsonl.display(),
            "mix primary path matches data_dir train.jsonl; skip redundant copy"
        );
        return Ok(());
    }
    if !train_jsonl.is_file() {
        tracing::warn!(
            active_train_jsonl = %train_jsonl.display(),
            "active train.jsonl missing; skip mix primary sync"
        );
        return Ok(());
    }
    if let Some(parent) = primary_resolved.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&train_jsonl, &primary_resolved).map_err(|e| {
        anyhow::anyhow!(
            "mix primary sync failed ({} -> {}): {e}",
            train_jsonl.display(),
            primary_resolved.display()
        )
    })?;
    Ok(())
}

/// Run corpus mix (when not skipped) and return the mixed JSONL path for [`super::preflight::validate_train_preflight`].
///
/// Mix output is always resolved relative to **`workspace_root`** when known (SSOT); avoids CWD drift.
pub fn refresh_train_contract_override_from_mix(
    workspace_root: Option<&Path>,
    data_dir: &Path,
    skip_mix: bool,
    sync_primary_with_data_dir_train: bool,
    explicit_mix_yaml: Option<&Path>,
) -> anyhow::Result<Option<PathBuf>> {
    if skip_mix {
        return Ok(None);
    }
    let mix_yaml = explicit_mix_yaml
        .map(Path::to_path_buf)
        .unwrap_or_else(|| resolve_mix_config_path(workspace_root));
    if !mix_yaml.is_file() {
        return Ok(None);
    }
    if sync_primary_with_data_dir_train {
        sync_mix_primary_with_train_jsonl(workspace_root, data_dir, &mix_yaml)?;
    }
    let path_base_for_mix = workspace_root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    if let Err(e) = corpus::run_mix_with_options(
        &mix_yaml,
        Some(path_base_for_mix.as_path()),
        MixRunOptions::default(),
    ) {
        tracing::warn!(error = %e, mix_yaml = %mix_yaml.display(), "corpus mix failed; continuing with existing train files");
        return Ok(None);
    }
    let Ok(mix_cfg) = MixConfigSchema::load(&mix_yaml) else {
        return Ok(None);
    };
    let mix_output = match workspace_root {
        Some(ws) => ws.join(&mix_cfg.output),
        None => path_base_for_mix.join(&mix_cfg.output),
    };

    // Prefer data_dir/train.jsonl after copy so training reads the same file mix just produced even if
    // cwd/workspace differed historically or target/ is cleaned between preflight and load.
    let mut use_train_jsonl = false;
    if let Some(ws) = workspace_root {
        match copy_mix_output_to_train_jsonl(ws, data_dir, &mix_yaml) {
            Ok(true) => use_train_jsonl = true,
            Ok(false) => {}
            Err(e) => tracing::warn!(
                error = %e,
                data_dir = %data_dir.display(),
                "copy mix output to data_dir train.jsonl failed; using mix output path"
            ),
        }
    }
    let train_jsonl = data_dir.join(super::preflight::PRIMARY_TRAIN_FILE);
    if use_train_jsonl && train_jsonl.is_file() {
        return Ok(Some(train_jsonl));
    }
    if mix_output.is_file() {
        Ok(Some(mix_output))
    } else {
        Ok(None)
    }
}

/// After pipeline (or other steps) already produced a mixed file, copy mix output to `data_dir/train.jsonl`.
pub fn copy_mix_output_to_train_jsonl(
    workspace_root: &Path,
    data_dir: &Path,
    mix_yaml: &Path,
) -> anyhow::Result<bool> {
    if !mix_yaml.is_file() {
        return Ok(false);
    }
    let mix_cfg = MixConfigSchema::load(mix_yaml)?;
    let mixed_path = workspace_root.join(&mix_cfg.output);
    let final_train_path = data_dir.join(super::preflight::PRIMARY_TRAIN_FILE);
    if !mixed_path.is_file() {
        return Ok(false);
    }
    if same_existing_file(&mixed_path, &final_train_path) {
        return Ok(true);
    }
    if let Some(parent) = final_train_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&mixed_path, &final_train_path)?;
    Ok(true)
}

/// Re-materialize training JSONL if it disappeared after preflight (e.g. long HF download while `cargo clean`
/// removed `target/`, or the mixed file lived only under `target/`).
pub fn recover_train_input_path_after_prefetch(
    workspace_root: Option<&Path>,
    data_dir: &Path,
    mix_yaml: &Path,
    skip_mix: bool,
    previously_resolved: &Path,
) -> anyhow::Result<PathBuf> {
    if previously_resolved.is_file() {
        return Ok(previously_resolved.to_path_buf());
    }
    tracing::warn!(
        path = %previously_resolved.display(),
        "training JSONL missing before kernel load; attempting recovery from mix output or re-mix"
    );
    let primary = data_dir.join(super::preflight::PRIMARY_TRAIN_FILE);

    if let Some(ws) = workspace_root
        && mix_yaml.is_file()
    {
        match copy_mix_output_to_train_jsonl(ws, data_dir, mix_yaml) {
            Ok(true) if primary.is_file() => return Ok(primary),
            Ok(true) => {}
            Ok(false) => {}
            Err(e) => tracing::warn!(
                error = %e,
                "recovery: copy mix output to data_dir train.jsonl failed"
            ),
        }
    }

    if !skip_mix && mix_yaml.is_file() {
        if let Ok(cfg) = crate::corpus::mix::MixConfigSchema::load(mix_yaml) {
            let path_base = workspace_root
                .map(Path::to_path_buf)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            for src in &cfg.sources {
                let p = path_base.join(&src.path);
                if !p.exists() {
                    if src.path == "mens/data/mix_sources/docs.jsonl" {
                        eprintln!("  🔄 Generating missing components: docs.jsonl");
                        let c = crate::corpus::extract_docs::ExtractDocsConfig {
                            root: path_base.join("docs"),
                            ..Default::default()
                        };
                        if let Ok(pairs) = crate::corpus::extract_docs::walk_and_extract_docs(&c) {
                            let _ = crate::corpus::extract_docs::write_docs_to_jsonl(&pairs, &p);
                        }
                    } else if src.path == "mens/data/mix_sources/rust_source.jsonl" {
                        eprintln!("  🔄 Generating missing components: rust_source.jsonl");
                        let c = crate::corpus::extract_rs::ExtractRsConfig {
                            root: path_base.join("crates"),
                            ..Default::default()
                        };
                        if let Ok(pairs) = crate::corpus::extract_rs::walk_and_extract(&c) {
                            let _ = crate::corpus::extract_rs::write_to_jsonl(&pairs, &p);
                        }
                    }
                }
            }
        }
        match refresh_train_contract_override_from_mix(
            workspace_root,
            data_dir,
            false,
            true,
            Some(mix_yaml),
        ) {
            Ok(Some(p)) if p.is_file() => return Ok(p),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "recovery: corpus mix re-run failed"),
        }
    }

    if primary.is_file() {
        return Ok(primary);
    }

    anyhow::bail!(
        "Training data `{}` is missing before training. If `--data-dir` is under `target/`, avoid `cargo clean` between preflight and load, or use a data directory outside `target/`.",
        previously_resolved.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn sync_copies_train_into_primary_when_paths_differ() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        let data = ws.join("target/dogfood");
        std::fs::create_dir_all(&data).expect("data dir");
        let mix_dir = ws.join("mens/config");
        std::fs::create_dir_all(&mix_dir).expect("mix dir");
        let primary = ws.join("mens/data/mix_sources/primary.jsonl");
        std::fs::create_dir_all(primary.parent().unwrap()).expect("parent");
        let train = data.join("train.jsonl");
        std::fs::write(&train, r#"{"a":1}"#).expect("write train");

        let mix_yaml = mix_dir.join("mix.yaml");
        let mut f = std::fs::File::create(&mix_yaml).expect("mix file");
        writeln!(
            f,
            "output: mens/data/mixed_out.jsonl\nsources:\n  - path: mens/data/mix_sources/primary.jsonl\n    weight: 1.0"
        )
        .expect("write mix");

        sync_mix_primary_with_train_jsonl(Some(ws), &data, &mix_yaml).expect("sync");
        let got = std::fs::read_to_string(&primary).expect("read primary");
        assert!(got.contains("\"a\":1"));
    }

    #[test]
    fn sync_skips_when_absolute_paths_differ_but_same_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        std::fs::create_dir_all(ws.join("target/dogfood")).expect("dirs");
        std::fs::create_dir_all(ws.join("mens/config")).expect("mix dir");
        let train = ws.join("target/dogfood/train.jsonl");
        std::fs::write(&train, r#"{"x":1}"#).expect("train");

        let mix_yaml = ws.join("mens/config/mix.yaml");
        let mut f = std::fs::File::create(&mix_yaml).expect("mix");
        writeln!(
            f,
            "output: target/dogfood/train_mixed.jsonl\nsources:\n  - path: target/dogfood/train.jsonl\n    weight: 1.0"
        )
        .expect("write");

        let data = ws.join("target/dogfood");
        sync_mix_primary_with_train_jsonl(Some(ws), &data, &mix_yaml).expect("no self-copy");

        let got = std::fs::read_to_string(&train).expect("read");
        assert!(got.contains("\"x\":1"));
    }

    #[test]
    fn sync_skips_when_data_dir_is_relative_to_cwd_matching_primary() {
        static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = CWD_LOCK.lock().expect("cwd test lock");

        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        std::fs::create_dir_all(ws.join("target/dogfood")).expect("dirs");
        std::fs::create_dir_all(ws.join("mens/config")).expect("mix dir");
        let train = ws.join("target/dogfood/train.jsonl");
        std::fs::write(&train, r#"{"y":2}"#).expect("train");

        let mix_yaml = ws.join("mens/config/mix.yaml");
        let mut f = std::fs::File::create(&mix_yaml).expect("mix");
        writeln!(
            f,
            "output: target/dogfood/train_mixed.jsonl\nsources:\n  - path: target/dogfood/train.jsonl\n    weight: 1.0"
        )
        .expect("write");

        let prev = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(ws).expect("cd tmp");
        let r = sync_mix_primary_with_train_jsonl(Some(ws), Path::new("target/dogfood"), &mix_yaml);
        std::env::set_current_dir(prev).expect("restore cwd");
        r.expect("relative data-dir must not self-copy on Windows");

        let got = std::fs::read_to_string(&train).expect("read");
        assert!(got.contains("\"y\":2"));
    }

    #[test]
    fn test_mix_config_paths_valid() {
        let ws = crate::training::contract::find_workspace_root().unwrap();
        let mix_yaml = resolve_mix_config_path(Some(&ws));
        let config = crate::corpus::MixConfigSchema::load(&mix_yaml).unwrap();
        for source in config.sources {
            if !source.optional {
                assert!(
                    ws.join(&source.path).is_file()
                        || source.path.contains("validated_mixed.jsonl"),
                    "Source path {} must exist if not optional",
                    source.path
                );
            }
        }
    }
}
