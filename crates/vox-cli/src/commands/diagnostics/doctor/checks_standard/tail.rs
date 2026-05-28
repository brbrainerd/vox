//! Manifest, LSP, config, keys, Ollama, registration, …

use tokio::process::Command;

use vox_bounded_fs::read_utf8_path_capped_async;

use super::super::common::{self, AuthRegistriesOnly, Check};
use super::super::provider_policy::{ProviderPolicyEngine, ProviderSupportLevel};

pub async fn run(auto_heal: bool, checks: &mut Vec<Check>) {
    let kube = Command::new("kubectl")
        .arg("version")
        .arg("--client")
        .output()
        .await;
    checks.push(match kube {
        Ok(o) if o.status.success() => Check {
            name: "kubectl (optional)".to_string(),
            pass: true,
            detail: "found — ready for Kubernetes deployments".to_string(),
        },
        _ => Check {
            name: "kubectl (optional)".to_string(),
            pass: true,
            detail: "not found — required for 'vox deploy --target k8s'".to_string(),
        },
    });

    let mut has_manifest = tokio::fs::try_exists("Vox.toml").await.unwrap_or(false);
    let mut manifest_detail = if has_manifest {
        "found in current directory".to_string()
    } else {
        "not found — run: vox init".to_string()
    };

    if !has_manifest && auto_heal {
        println!("  [auto-heal] Scaffolding Vox.toml via vox init...");
        let manifest = vox_package::VoxManifest::scaffold("vox-app", "application");
        if let Ok(s) = manifest.to_toml_string() {
            if tokio::fs::write("Vox.toml", s).await.is_ok() {
                has_manifest = true;
                manifest_detail = "Vox.toml scaffolded via auto-heal".to_string();
            }
        }
    }

    checks.push(Check {
        name: "Vox.toml".to_string(),
        pass: has_manifest,
        detail: manifest_detail,
    });

    let lsp_binary_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(if cfg!(windows) { "vox-lsp.exe" } else { "vox-lsp" })));

    let mut lsp_bin = match lsp_binary_path.as_ref() {
        Some(p) => tokio::fs::try_exists(p).await.unwrap_or(false),
        None => false,
    };
    let mut lsp_detail = if lsp_bin {
        "found in PATH".to_string()
    } else {
        "not built — run: cargo build -p vox-lsp --release".to_string()
    };

    if !lsp_bin && auto_heal {
        println!("  [auto-heal] Building vox-lsp...");
        if Command::new("cargo")
            .args(["build", "-p", "vox-lsp", "--release"])
            .status()
            .await
            .is_ok_and(|s| s.success())
        {
            lsp_bin = true;
            lsp_detail = "built successfully via auto-heal".to_string();
        } else {
            lsp_detail = "auto-heal failed to build vox-lsp".to_string();
        }
    }

    checks.push(Check {
        name: "vox-lsp binary".to_string(),
        pass: lsp_bin,
        detail: lsp_detail,
    });

    let config_dir: Option<std::path::PathBuf> = common::user_home_dir().map(|h| h.join(".vox"));
    let config_path = config_dir.as_ref().map(|d| d.join("config.toml"));
    let mut has_config = match config_path.as_ref() {
        Some(p) => tokio::fs::try_exists(p).await.unwrap_or(false),
        None => false,
    };
    let mut config_detail = if has_config {
        "found in ~/.vox/config.toml".to_string()
    } else {
        "not found — run: vox login".to_string()
    };

    if !has_config
        && auto_heal
        && let Some(dir) = &config_dir
    {
        println!("  [auto-heal] Creating default Vox configuration...");
        let _ = tokio::fs::create_dir_all(dir).await;
        let default_config = "[registry]\nurl = \"https://raw.githubusercontent.com/vox-foundation/vox/main/registry\"\n";
        if tokio::fs::write(dir.join("config.toml"), default_config)
            .await
            .is_ok()
        {
            has_config = true;
            config_detail = "config.toml created via auto-heal".to_string();
        }
    }

    checks.push(Check {
        name: "Vox Config".to_string(),
        pass: has_config,
        detail: config_detail,
    });

    let google_key = common::resolved_google_key().await;
    checks.push(match &google_key {
        Some(k) if k.starts_with("AIza") => Check {
            name: "Google AI Studio Key".to_string(),
            pass: true,
            detail: format!(
                "configured (free Gemini models available) — {}",
                common::redact_key(k)
            ),
        },
        Some(k) => Check {
            name: "Google AI Studio Key".to_string(),
            pass: true,
            detail: format!("configured — {}", common::redact_key(k)),
        },
        None => Check {
            name: "Google AI Studio Key".to_string(),
            pass: false,
            detail: "not found — run: vox login --registry google YOUR_KEY\n                          get a free key at: https://aistudio.google.com/apikey".to_string(),
        },
    });

    let or_key = common::resolved_openrouter_key().await;
    checks.push(match &or_key {
        Some(k) => Check {
            name: "OpenRouter Key (optional)".to_string(),
            pass: true,
            detail: format!(
                "configured (free :free models + paid SOTA available) — {}",
                common::redact_key(k)
            ),
        },
        None => Check {
            name: "OpenRouter Key (optional)".to_string(),
            pass: true,
            detail: "not configured — get a free key at https://openrouter.ai/keys".to_string(),
        },
    });

    let engine = ProviderPolicyEngine::new();
    let auth_path = common::vox_dot_dir().join("auth.json");
    if tokio::fs::try_exists(&auth_path).await.unwrap_or(false) {
        if let Ok(content) = read_utf8_path_capped_async(&auth_path).await {
            if let Ok(config) = serde_json::from_str::<AuthRegistriesOnly>(&content) {
                for (reg, _) in config.registries {
                    let policy = engine.policy_for(&reg);
                    let (pass, detail) = match policy {
                        Some(p) => {
                            let status = format!(
                                "{:?} / Quota Truth: {:?}",
                                p.support_level, p.quota_truth_level
                            );
                            (
                                !matches!(
                                    p.support_level,
                                    ProviderSupportLevel::UnsupportedInitially
                                ),
                                status,
                            )
                        }
                        None => (
                            true,
                            "No explicit policy defined (Basic support)".to_string(),
                        ),
                    };
                    checks.push(Check {
                        name: format!("Provider Policy: {}", reg),
                        pass,
                        detail,
                    });
                }
            }
        }
    }

    use vox_config::InferenceProfile;

    let profile = vox_config::inference_profile_from_env();
    let ollama_probe_skipped = matches!(
        profile,
        InferenceProfile::MobileLitert
            | InferenceProfile::MobileCoreml
            | InferenceProfile::CloudOpenAiCompatible
    );
    let ollama_detail = if ollama_probe_skipped {
        format!(
            "TCP probe skipped for vox_populi::inference_PROFILE={profile:?} (not desktop/lan Ollama); see docs/src/architecture/mobile-edge-ai-ssot.md"
        )
    } else {
        let ollama_reachable = std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], 11434)),
            std::time::Duration::from_millis(300),
        )
        .is_ok();
        if ollama_reachable {
            "running on localhost:11434 (local inference available)".to_string()
        } else {
            "not running — install from https://ollama.com if you want local models (or set vox_populi::inference_PROFILE if this host should not use loopback Ollama)".to_string()
        }
    };
    checks.push(Check {
        name: "Ollama Local (optional)".to_string(),
        pass: true,
        detail: ollama_detail,
    });

    let current_exe = std::env::current_exe().unwrap_or_default();
    let exe_path_str = current_exe.to_string_lossy();
    let in_vox_repo = tokio::fs::try_exists("Cargo.toml").await.unwrap_or(false)
        && tokio::fs::try_exists("crates/vox-cli")
            .await
            .unwrap_or(false);
    let is_local_dev = exe_path_str.contains("target")
        && (exe_path_str.contains("debug") || exe_path_str.contains("release"));
    let is_installed = exe_path_str.contains(".vox") && exe_path_str.contains("bin");
    let binary_source_pass = !in_vox_repo || is_local_dev;
    let binary_source_detail = if is_local_dev {
        format!("{} (local dev build)", exe_path_str)
    } else if is_installed && in_vox_repo {
        format!(
            "{} — using installed binary while in repo; prefer: export PATH=\"$(./scripts/dev-path.sh):$PATH\" or cargo run -p vox-cli -- ...",
            exe_path_str
        )
    } else {
        format!("{}", exe_path_str)
    };
    checks.push(Check {
        name: "Vox binary source".to_string(),
        pass: binary_source_pass,
        detail: binary_source_detail,
    });

    let update_hint = if exe_path_str.starts_with("/usr/bin/") || exe_path_str.starts_with("/bin/")
    {
        "sudo apt update && sudo apt install --only-upgrade vox"
    } else if exe_path_str.contains("WinGet") || exe_path_str.contains("WindowsApps") {
        "winget upgrade vox"
    } else if exe_path_str.contains(".cargo") {
        "cargo install vox-cli"
    } else {
        "vox update (or redownload from GitHub Releases)"
    };

    checks.push(Check {
        name: "App Updates".to_string(),
        pass: true,
        detail: format!("to update natively, run: {}", update_hint),
    });

    let vox_dir = common::user_home_dir().map(|h| h.join(".vox"));
    let db_check = match vox_dir.as_ref() {
        Some(d) => {
            tokio::fs::create_dir_all(d).await.is_ok() && {
                let test_file = d.join(".doctor_write_test");
                let ok = tokio::fs::write(&test_file, b"ok").await.is_ok();
                let _ = tokio::fs::remove_file(&test_file).await;
                ok
            }
        }
        None => false,
    };
    checks.push(Check {
        name: "VoxDB directory".to_string(),
        pass: db_check,
        detail: if db_check {
            format!(
                "{} (writable)",
                vox_dir
                    .as_ref()
                    .map(|d| d.display().to_string())
                    .unwrap_or_default()
            )
        } else {
            "~/.vox/ not writable — check permissions".to_string()
        },
    });

    let mut reg_pass = false;
    let mut reg_detail = "not registered — run: vox setup".to_string();
    if let Ok(db) = vox_db::Codex::connect_default().await {
        let key = "project.vox-workspace.path".to_string();
        if let Ok(path) = db.get_object_metadata("vox-workspace", &key).await {
            reg_pass = true;
            reg_detail = format!("registered at {}", path);
        } else if let Ok(path) = db.get_object_metadata("vox-workspace", "path").await {
            reg_pass = true;
            reg_detail = format!("registered at {}", path);
        } else if let Ok(Some(val)) = db.get_user_preference_value_by_key(&key).await {
            reg_pass = true;
            reg_detail = format!("registered at {}", val);
        }
    }

    checks.push(Check {
        name: "Workspace Registration".to_string(),
        pass: reg_pass,
        detail: reg_detail,
    });

    v0_named_export_doctor_check(checks).await;
}

/// When **`VOX_WEB_TS_OUT`** is set, ensures each local `@v0` component has a matching **named** export in that directory.
async fn v0_named_export_doctor_check(checks: &mut Vec<Check>) {
    let Ok(ts_out) = std::env::var("VOX_WEB_TS_OUT") else {
        checks.push(Check::pass(
            "@v0 TSX named exports (optional)",
            "skipped — set VOX_WEB_TS_OUT to the directory where `vox build` writes `*.tsx` (same path as the build output) to verify @v0 named exports",
        ));
        return;
    };
    let root = std::path::PathBuf::from(ts_out);
    if !root.is_dir() {
        checks.push(Check::fail(
            "@v0 TSX named exports",
            format!("VOX_WEB_TS_OUT={} is not a directory", root.display()),
        ));
        return;
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let names = crate::v0_tsx_normalize::scan_v0_component_names_from_vox_sources(&cwd);
    if names.is_empty() {
        checks.push(Check::pass(
            "@v0 TSX named exports",
            format!(
                "no @v0 declarations under {} — nothing to verify",
                cwd.display()
            ),
        ));
        return;
    }
    let mut failures: Vec<String> = Vec::new();
    for name in &names {
        let p = root.join(format!("{name}.tsx"));
        if !p.is_file() {
            failures.push(format!("{name}.tsx missing under {}", root.display()));
            continue;
        }
        let content = match read_utf8_path_capped_async(&p).await {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{}: {e}", p.display()));
                continue;
            }
        };
        if let Some(msg) = crate::v0_tsx_normalize::v0_named_export_violation(&content, name) {
            failures.push(msg);
        }
    }
    if failures.is_empty() {
        checks.push(Check::pass(
            "@v0 TSX named exports",
            format!(
                "{} — {} @v0 component(s) under {} satisfy the named-export contract",
                root.display(),
                names.len(),
                cwd.display()
            ),
        ));
    } else {
        checks.push(Check::fail("@v0 TSX named exports", failures.join("; ")));
    }
}
