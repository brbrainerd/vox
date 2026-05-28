//! Core toolchain checks: Rust, Node, pnpm, mesh, Docker, WASI, …

use tokio::process::Command;

use super::super::common::Check;

pub async fn run(auto_heal: bool, checks: &mut Vec<Check>) {
    let cargo = Command::new("cargo").arg("--version").output().await;
    checks.push(match cargo {
        Ok(o) if o.status.success() => Check {
            name: "Rust / Cargo".to_string(),
            pass: true,
            detail: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        _ => Check {
            name: "Rust / Cargo".to_string(),
            pass: false,
            detail: "not found — install from https://rustup.rs".to_string(),
        },
    });

    let node = Command::new("node").arg("--version").output().await;
    checks.push(match node {
        Ok(o) if o.status.success() => Check {
            name: "Node.js".to_string(),
            pass: true,
            detail: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        _ => Check {
            name: "Node.js".to_string(),
            pass: false,
            detail: "not found — install from https://nodejs.org".to_string(),
        },
    });

    let pnpm_exe = if cfg!(target_os = "windows") {
        "pnpm.cmd"
    } else {
        "pnpm"
    };
    let pnpm = Command::new(pnpm_exe).arg("--version").output().await;
    let pnpm_pass = matches!(&pnpm, Ok(o) if o.status.success());
    let mut pnpm_detail = if pnpm_pass {
        format!("v{}", String::from_utf8_lossy(&pnpm.unwrap().stdout).trim())
    } else {
        "not found — run: npm install -g pnpm".to_string()
    };

    let mut actual_pnpm_pass = pnpm_pass;
    if !pnpm_pass && auto_heal {
        println!("  [auto-heal] Installing pnpm...");
        let npm_exe = if cfg!(target_os = "windows") {
            "npm.cmd"
        } else {
            "npm"
        };
        if Command::new(npm_exe)
            .args(["install", "-g", "pnpm"])
            .status()
            .await
            .is_ok_and(|s| s.success())
        {
            actual_pnpm_pass = true;
            pnpm_detail = "installed successfully via auto-heal".to_string();
        } else {
            pnpm_detail = "auto-heal failed: could not run npm install pnpm".to_string();
        }
    }

    checks.push(Check {
        name: "pnpm".to_string(),
        pass: actual_pnpm_pass,
        detail: pnpm_detail,
    });

    let git = // vox-arch-check: allow git-exec
        Command::new("git").arg("--version").output().await;
    checks.push(match git {
        Ok(o) if o.status.success() => Check {
            name: "Git".to_string(),
            pass: true,
            detail: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        _ => Check {
            name: "Git".to_string(),
            pass: false,
            detail: "not found — install from https://git-scm.com".to_string(),
        },
    });

    let mesh_mode = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshMode)
        .expose()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "lan".to_string());
    let mesh_token_set = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshToken)
        .expose()
        .is_some_and(|v| !v.trim().is_empty());
    let mesh_scope_set = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshScopeId)
        .expose()
        .is_some_and(|v| !v.trim().is_empty());
    checks.push(Check {
        name: "Populi mesh security defaults".to_string(),
        pass: mesh_token_set && mesh_scope_set,
        detail: format!(
            "mode={mesh_mode}, token_set={mesh_token_set}, scope_set={mesh_scope_set} (recommended: `vox populi up`)"
        ),
    });

    let tailscale_ok = Command::new("tailscale")
        .arg("version")
        .output()
        .await
        .is_ok_and(|o| o.status.success());
    let wireguard_ok = Command::new("wg")
        .arg("show")
        .output()
        .await
        .is_ok_and(|o| o.status.success());
    let tunnel_ok = Command::new("cloudflared")
        .arg("--version")
        .output()
        .await
        .is_ok_and(|o| o.status.success())
        || Command::new("ngrok")
            .arg("version")
            .output()
            .await
            .is_ok_and(|o| o.status.success());
    checks.push(Check {
        name: "Overlay networking (optional)".to_string(),
        pass: true,
        detail: format!(
            "tailscale={}, wireguard={}, tunnel={} (use `vox populi status` for deeper diagnostics)",
            if tailscale_ok { "ok" } else { "missing" },
            if wireguard_ok { "ok" } else { "missing" },
            if tunnel_ok { "ok" } else { "missing" }
        ),
    });

    let docker = Command::new("docker").arg("--version").output().await;
    checks.push(match docker {
        Ok(o) if o.status.success() => Check {
            name: "Docker".to_string(),
            pass: true,
            detail: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        _ => Check {
            name: "Docker (optional)".to_string(),
            pass: true,
            detail: "not installed — install from https://docs.docker.com/get-docker/".to_string(),
        },
    });

    let podman = Command::new("podman").arg("--version").output().await;
    checks.push(match podman {
        Ok(o) if o.status.success() => Check {
            name: "Podman".to_string(),
            pass: true,
            detail: format!("{} (rootless)", String::from_utf8_lossy(&o.stdout).trim()),
        },
        _ => Check {
            name: "Podman (optional)".to_string(),
            pass: true,
            detail: "not installed — install from https://podman.io/getting-started/installation"
                .to_string(),
        },
    });

    let wasi_target = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .await;
    let wasi_installed = wasi_target
        .as_ref()
        .map(|o| {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .any(|l| l.trim() == "wasm32-wasip1")
        })
        .unwrap_or(false);

    let mut wasi_detail = if wasi_installed {
        "installed — `vox run --isolation wasm` is fast (warm cache)".to_string()
    } else {
        "not installed — first WASI run will take ~10s to install".to_string()
    };

    if !wasi_installed && auto_heal {
        println!("  [auto-heal] Installing wasm32-wasip1 target...");
        let ok = Command::new("rustup")
            .args(["target", "add", "wasm32-wasip1"])
            .status()
            .await
            .is_ok_and(|s| s.success());
        if ok {
            wasi_detail =
                "installed via auto-heal — `vox run --isolation wasm` is now fast".to_string();
        } else {
            wasi_detail = "auto-heal failed — run: rustup target add wasm32-wasip1".to_string();
        }
    }

    checks.push(Check {
        name: "WASI target (wasm32-wasip1)".to_string(),
        pass: wasi_installed || auto_heal,
        detail: wasi_detail,
    });

    let zig = Command::new("zig").arg("version").output().await;
    checks.push(match zig {
        Ok(o) if o.status.success() => Check {
            name: "Zig (optional)".to_string(),
            pass: true,
            detail: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        _ => Check {
            name: "Zig (optional)".to_string(),
            pass: true,
            detail: "not found — suggested for advanced cross-compilation (https://ziglang.org)"
                .to_string(),
        },
    });

    #[cfg(target_os = "linux")]
    {
        let mold = Command::new("mold").arg("--version").output().await;
        checks.push(match mold {
            Ok(o) if o.status.success() => Check {
                name: "mold linker".to_string(),
                pass: true,
                detail: String::from_utf8_lossy(&o.stdout).trim().to_string(),
            },
            _ => Check {
                name: "mold linker".to_string(),
                pass: false,
                detail: "not found — recommended for fast Linux builds: sudo apt install mold"
                    .to_string(),
            },
        });

        let cranelift = Command::new("rustup")
            .args(["component", "list", "--installed"])
            .output()
            .await;
        let has_cranelift = matches!(&cranelift, Ok(o) if String::from_utf8_lossy(&o.stdout).contains("rustc-codegen-cranelift-preview"));
        checks.push(Check {
            name: "Cranelift codegen".to_string(),
            pass: has_cranelift,
            detail: if has_cranelift {
                "installed".to_string()
            } else {
                "not installed — recommended for fast dev builds: rustup component add rustc-codegen-cranelift-preview".to_string()
            },
        });
    }
}
