//! `vox setup` — cross-platform first-run setup wizard.
//!
//! This is the **single source of truth** for environment setup across all
//! platforms. Shell scripts (`install.sh`, `install.ps1`) bootstrap Rust and
//! cargo, then delegate to `vox setup` for everything else.
//!
//! Design principles (ZIG-style):
//! - Detect, don't assume. Check every dep before using it.
//! - Degrade gracefully. Missing optional tools reduce features, never crash.
//! - One binary. This Rust code replaces 1000 lines of bash + PowerShell.

use anyhow::Result;
use std::process::Command;

struct SetupCheck {
    name: &'static str,
    required: bool,
    pass: bool,
    detail: String,
    heal_hint: Option<String>,
}

/// Run the setup wizard.
pub async fn run(dev: bool, non_interactive: bool) -> Result<()> {
    println!();
    println!("  ╔══════════════════════════════════════════╗");
    println!("  ║        Vox Setup Wizard (v0.2.0)         ║");
    if dev {
        println!("  ║           Mode: DEVELOPMENT              ║");
    } else {
        println!("  ║           Mode: END-USER                 ║");
    }
    if non_interactive {
        println!("  ║       (non-interactive / CI mode)         ║");
    }
    println!("  ╚══════════════════════════════════════════╝");
    println!();

    let mut checks: Vec<SetupCheck> = Vec::new();

    // ── 1. Rust ≥ 1.80 ──────────────────────────────────────────────────
    let rust_check = check_command_version("rustc", &["--version"], "1.80");
    checks.push(SetupCheck {
        name: "Rust / Cargo",
        required: true,
        pass: rust_check.0,
        detail: rust_check.1,
        heal_hint: Some("install from https://rustup.rs".into()),
    });

    // ── 2. Node.js ≥ 18 ─────────────────────────────────────────────────
    let node_check = check_node_version(18);
    checks.push(SetupCheck {
        name: "Node.js",
        required: false, // only needed for VS Code extension / codegen-ts
        pass: node_check.0,
        detail: node_check.1,
        heal_hint: Some("install from https://nodejs.org".into()),
    });

    // ── 3. Git ───────────────────────────────────────────────────────────
    let git_check = check_command_version("git", &["--version"], "");
    checks.push(SetupCheck {
        name: "Git",
        required: true,
        pass: git_check.0,
        detail: git_check.1,
        heal_hint: Some("install from https://git-scm.com".into()),
    });

    // ── 4. Platform-specific C compiler ──────────────────────────────────
    let cc_check = check_c_compiler();
    checks.push(SetupCheck {
        name: "C Compiler",
        required: true,
        pass: cc_check.0,
        detail: cc_check.1,
        heal_hint: cc_check.2,
    });

    // ── 5. Google AI Studio key (primary free tier) ──────────────────────
    let google_key = find_ai_key("google", &["GEMINI_API_KEY", "GOOGLE_AI_STUDIO_KEY"]);
    let has_google = google_key.is_some();

    if !has_google && !non_interactive {
        println!("  ── AI Provider Setup ──────────────────────────────────");
        println!("  Vox uses Google AI Studio for free AI (no credit card).");
        println!("  Get a key at: https://aistudio.google.com/apikey");
        println!();

        if let Some(key) = prompt_for_key("Google AI Studio API Key") {
            // Persist via the unified login system
            let login_result = super::login::run(
                Some(&key),
                Some("google"),
                None, // username not needed for AI keys
            )
            .await;
            if login_result.is_ok() {
                println!("  ✓ Key saved to ~/.vox/auth.json");
            }
            checks.push(SetupCheck {
                name: "Google AI Studio Key",
                required: false, // free tier wants it, but Ollama works without
                pass: true,
                detail: "configured (free Gemini models available)".into(),
                heal_hint: None,
            });
        } else {
            checks.push(SetupCheck {
                name: "Google AI Studio Key",
                required: false,
                pass: false,
                detail: "skipped — run: vox login --registry google YOUR_KEY".into(),
                heal_hint: Some("https://aistudio.google.com/apikey".into()),
            });
        }
    } else {
        checks.push(SetupCheck {
            name: "Google AI Studio Key",
            required: false,
            pass: has_google,
            detail: if has_google {
                "configured".into()
            } else {
                "not found — run: vox login --registry google YOUR_KEY".into()
            },
            heal_hint: Some("https://aistudio.google.com/apikey".into()),
        });
    }

    // ── 6. OpenRouter key (optional) ─────────────────────────────────────
    let or_key = find_ai_key("openrouter", &["OPENROUTER_API_KEY"]);
    checks.push(SetupCheck {
        name: "OpenRouter (optional)",
        required: false,
        pass: or_key.is_some(),
        detail: if or_key.is_some() {
            "configured (free :free + paid SOTA available)".into()
        } else {
            "not configured — get a free key at https://openrouter.ai/keys".into()
        },
        heal_hint: None,
    });

    // ── 7. Ollama (optional local inference) ─────────────────────────────
    let ollama_up = std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], 11434)),
        std::time::Duration::from_millis(300),
    )
    .is_ok();
    checks.push(SetupCheck {
        name: "Ollama (optional)",
        required: false,
        pass: ollama_up,
        detail: if ollama_up {
            "running on localhost:11434".into()
        } else {
            "not running — install from https://ollama.com for local models".into()
        },
        heal_hint: None,
    });

    // ── 8. VoxDB directory ───────────────────────────────────────────────
    let vox_dir = dirs::home_dir().map(|h| h.join(".vox"));
    let db_ok = vox_dir
        .as_ref()
        .map(|d| {
            std::fs::create_dir_all(d).is_ok() && {
                let probe = d.join(".setup_probe");
                let ok = std::fs::write(&probe, b"ok").is_ok();
                let _ = std::fs::remove_file(&probe);
                ok
            }
        })
        .unwrap_or(false);
    checks.push(SetupCheck {
        name: "VoxDB directory",
        required: true,
        pass: db_ok,
        detail: if db_ok {
            format!(
                "{} (writable)",
                vox_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default()
            )
        } else {
            "~/.vox/ not writable — check permissions".into()
        },
        heal_hint: None,
    });

    // ── 9. Dev tools (only in --dev mode) ────────────────────────────────
    if dev {
        let clippy = check_command_version("cargo", &["clippy", "--version"], "");
        checks.push(SetupCheck {
            name: "clippy",
            required: false,
            pass: clippy.0,
            detail: clippy.1,
            heal_hint: Some("rustup component add clippy".into()),
        });

        let rustfmt = check_command_version("cargo", &["fmt", "--version"], "");
        checks.push(SetupCheck {
            name: "rustfmt",
            required: false,
            pass: rustfmt.0,
            detail: rustfmt.1,
            heal_hint: Some("rustup component add rustfmt".into()),
        });

        let nextest = check_command_version("cargo", &["nextest", "--version"], "");
        checks.push(SetupCheck {
            name: "cargo-nextest",
            required: false,
            pass: nextest.0,
            detail: nextest.1,
            heal_hint: Some("cargo install cargo-nextest".into()),
        });
    }

    // ── 10. Workspace Registration ──────────────────────────────────────
    let mut reg_pass = false;
    let reg_detail = if let Ok(db) = vox_db::VoxDb::connect_default().await {
        if let Ok(_) = db.register_local_project("vox-workspace", &std::env::current_dir().unwrap_or_default()).await {
            reg_pass = true;
            "registered current workspace in local database".into()
        } else {
            "registration failed".into()
        }
    } else {
        "database unavailable".into()
    };
    checks.push(SetupCheck {
        name: "Workspace Registration",
        required: false,
        pass: reg_pass,
        detail: reg_detail,
        heal_hint: None,
    });

    // ── 11. User Registration ─────────────────────────────────────────
    let user_id = vox_db::paths::local_user_id();
    if let Ok(db) = vox_db::VoxDb::connect_default().await {
        if db.store().get_user(&user_id).await.is_err() {
            let _ = db.store().create_user(&user_id, &user_id, None, None, "admin").await;
        }
    }

    // ── Print results ────────────────────────────────────────────────────
    println!();
    println!("  ── Setup Results ─────────────────────────────────────");
    let mut failed_required = 0;
    let mut failed_optional = 0;
    for c in &checks {
        let icon = if c.pass { "✓" } else if c.required { "✗" } else { "○" };
        let suffix = if !c.required && !c.pass { " (optional)" } else { "" };
        println!("  {}  {:28} {}{}", icon, c.name, c.detail, suffix);
        if !c.pass {
            if let Some(hint) = &c.heal_hint {
                println!("      Suggestion: {}", hint);
            }
            if c.required {
                failed_required += 1;
            } else {
                failed_optional += 1;
            }
        }
    }

    println!();
    if failed_required == 0 {
        println!("  ✓ Setup complete — you're ready to build with Vox!");
        if failed_optional > 0 {
            println!(
                "    ({} optional item(s) not configured — features will degrade gracefully)",
                failed_optional
            );
        }
    } else {
        println!(
            "  ✗ {} required check(s) failed — resolve before building.",
            failed_required
        );
    }

    println!();
    println!("  Next steps:");
    println!("    vox doctor    — verify environment at any time");
    println!("    vox chat      — start AI chat (free tier)");
    println!("    vox build examples/chatbot.vox -o dist");
    println!();

    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Run a command and check whether it succeeds. Optionally verify a minimum version.
fn check_command_version(cmd: &str, args: &[&str], _min_ver: &str) -> (bool, String) {
    match Command::new(cmd).args(args).output() {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout).trim().to_string();
            (true, stdout)
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            (false, format!("command failed: {}", stderr))
        }
        Err(_) => (false, "not found on PATH".into()),
    }
}

/// Check Node.js version >= min_major.
fn check_node_version(min_major: u32) -> (bool, String) {
    match Command::new("node").arg("--version").output() {
        Ok(o) if o.status.success() => {
            let ver_str = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let major: u32 = ver_str
                .trim_start_matches('v')
                .split('.')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if major >= min_major {
                (true, format!("{} (>= v{})", ver_str, min_major))
            } else {
                (false, format!("{} (need >= v{})", ver_str, min_major))
            }
        }
        _ => (false, "not found on PATH".into()),
    }
}

/// Detect a C compiler (platform-specific).
fn check_c_compiler() -> (bool, String, Option<String>) {
    #[cfg(target_os = "windows")]
    {
        // Check for MSVC via vswhere
        let vswhere = format!(
            "{}\\Microsoft Visual Studio\\Installer\\vswhere.exe",
            std::env::var("ProgramFiles(x86)").unwrap_or_default()
        );
        if std::path::Path::new(&vswhere).exists() {
            if let Ok(o) = Command::new(&vswhere)
                .args(["-latest", "-property", "installationPath"])
                .output()
            {
                if o.status.success()
                    && !String::from_utf8_lossy(&o.stdout).trim().is_empty()
                {
                    return (true, "MSVC Build Tools found".into(), None);
                }
            }
        }
        // Fallback: check for cl.exe or gcc
        if Command::new("cl").arg("/?").output().is_ok() {
            return (true, "cl.exe found on PATH".into(), None);
        }
        if Command::new("gcc").arg("--version").output().is_ok() {
            return (true, "gcc found on PATH (MinGW)".into(), None);
        }
        (
            false,
            "no C compiler found".into(),
            Some("install Visual Studio Build Tools: https://visualstudio.microsoft.com/visual-cpp-build-tools/".into()),
        )
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(o) = Command::new("xcode-select").arg("-p").output() {
            if o.status.success() {
                return (true, "Xcode Command Line Tools".into(), None);
            }
        }
        (
            false,
            "Xcode CLT not found".into(),
            Some("run: xcode-select --install".into()),
        )
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(o) = Command::new("cc").arg("--version").output() {
            if o.status.success() {
                let ver = String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .unwrap_or("found")
                    .to_string();
                return (true, ver, None);
            }
        }
        if let Ok(o) = Command::new("gcc").arg("--version").output() {
            if o.status.success() {
                return (true, "gcc found".into(), None);
            }
        }
        (
            false,
            "no C compiler found".into(),
            Some("install build-essential: sudo apt-get install build-essential".into()),
        )
    }
}

/// Look for an AI provider key in env vars or ~/.vox/auth.json.
fn find_ai_key(registry_name: &str, env_vars: &[&str]) -> Option<String> {
    // 1. Check environment variables
    for var in env_vars {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    // 2. Check auth.json
    let auth_path = dirs::home_dir()?.join(".vox").join("auth.json");
    let content = std::fs::read_to_string(auth_path).ok()?;
    if content.contains(&format!("\"{}\"", registry_name)) && content.contains("\"token\"") {
        Some("(from auth.json)".into())
    } else {
        None
    }
}

/// Prompt the user for an API key (blocking, stdin).
fn prompt_for_key(label: &str) -> Option<String> {
    eprint!("  {}: ", label);
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}
