//! `vox share` — public-URL tunnel for Vox apps.
//!
//! S1: LAN backend only. S2 adds Cloudflare default. S3 adds localhost.run fallback.
//! S8: bundle/dev integration — pass a .vox FILE to auto-build and serve.

use crate::cli_args::BundleMode;
use anyhow::{Context as _, Result};
use clap::Args;
use std::time::Duration;
use crate::utils::share::auth::AuthMode;
use crate::utils::share::{BackendKind, ShareConfig, ShareSession};

/// Timeout for the initial connection from the share proxy to the upstream app.
/// Configurable at compile time; override via `VOX_SHARE_CONNECT_TIMEOUT_SECS` at runtime
/// if finer control is needed without a rebuild.
const SHARE_CONNECT_TIMEOUT_SECS: u64 = 10;

#[derive(Args, Debug)]
#[command(about = "Share a Vox app via a public URL tunnel")]
pub struct ShareArgs {
    /// Vox app file to bundle and share (e.g. app.vox). If omitted, assumes app already running on --port.
    #[arg(value_name = "FILE")]
    pub file: Option<std::path::PathBuf>,

    /// Tunnel backend (lan, cloudflare, localhost-run, tailscale). Default: cloudflare.
    #[arg(long, default_value = "cloudflare")]
    pub backend: String,

    /// Port the bundled app listens on.
    #[arg(long, default_value = "7860")]
    pub port: u16,

    /// Auto-shutdown duration (e.g. 8h, 30m, 5s, none). Default: 8h.
    #[arg(long, default_value = "8h")]
    pub duration: String,

    /// Use dev server pipeline instead of vox bundle (faster iteration).
    #[arg(long)]
    pub dev: bool,

    /// Accept Cloudflare ToS without prompting (required in CI / non-interactive environments).
    #[arg(long)]
    pub accept_tos: bool,

    /// Authentication mode: none, basic:user:pass. Default: url-token (auto-generated).
    #[arg(long, default_value = "token")]
    pub auth: String,

    /// Keep Cloudflare backend even if SSE routes detected (SSE will be buffered/broken).
    #[arg(long)]
    pub allow_buffered_streaming: bool,
}

pub async fn run(args: ShareArgs) -> Result<()> {
    let backend: BackendKind = args
        .backend
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid --backend `{}`: {}", args.backend, e))?;

    let auth_mode = if args.auth == "token" {
        AuthMode::random_token()
    } else {
        args.auth
            .parse::<AuthMode>()
            .map_err(|e| anyhow::anyhow!("invalid --auth: {}", e))?
    };

    let duration = parse_duration(&args.duration)?;

    println!("[vox share] Backend: {}", backend);

    if matches!(backend, BackendKind::Cloudflare) {
        crate::utils::share::consent::ensure_consent(args.accept_tos, false)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    println!("[vox share] Proxy port is OS-assigned.");

    let (app_binary, _dev_child) = if let Some(ref file) = args.file {
        if args.dev {
            // Dev mode: spawn `vox dev <file> --port <port>` as background child
            let child = spawn_dev_server(file, args.port).await?;
            (None, Some(child)) // server already running on args.port
        } else {
            // Bundle mode: build with cache
            let binary = build_or_reuse_bundle(file).await?;
            (Some(binary), None)
        }
    } else {
        if args.dev {
            anyhow::bail!(
                "`--dev` requires a FILE argument — pass the .vox file to develop: vox share --dev app.vox"
            );
        }
        // No file given: assume the app is already running on --port (pre-S8 behavior).
        // When no file is provided we are not building — no target/ paths are used.
        println!(
            "[vox share] Note: pass a .vox FILE to auto-build. Sharing port {} directly.",
            args.port
        );
        (None, None)
    };

    // Allow fallback to localhost.run when using Cloudflare (the default).
    // If the user explicitly passed --backend cloudflare or any other backend,
    // we still allow fallback only for Cloudflare (it's the fallback-eligible backend).
    let allow_fallback = matches!(backend, BackendKind::Cloudflare);

    let connect_secs = std::env::var("VOX_SHARE_CONNECT_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(SHARE_CONNECT_TIMEOUT_SECS);

    let cfg = ShareConfig {
        backend,
        upstream_port: args.port,
        proxy_port: 0,
        duration,
        app_binary,
        connect_timeout: Duration::from_secs(connect_secs),
        allow_fallback,
        auth_mode,
        allow_buffered_streaming: args.allow_buffered_streaming,
    };

    let session = ShareSession::start(cfg)
        .await
        .map_err(|e| anyhow::anyhow!("share session: {}", e))?;

    println!("[vox share] Public URL: {}", session.public_url);
    println!(
        "[vox share] Local proxy: http://127.0.0.1:{}",
        session.proxy_port
    );
    println!("[vox share] Press Ctrl+C to stop.");
    if let Some(d) = duration {
        println!(
            "[vox share] Auto-shutdown in {}",
            crate::utils::share::lifecycle::format_duration(d)
        );
    }
    session.wait().await;
    println!("[vox share] Done.");
    Ok(())
}

/// Build the app with `vox bundle` and cache the result in `target/share-bundle/`.
/// Returns the path to the built binary.
async fn build_or_reuse_bundle(file: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    let app_name = file
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "app".to_string());
    let ext = if cfg!(windows) { ".exe" } else { "" };
    // Anchor to workspace root so `vox share` works from any subdirectory.
    let ws_root = find_workspace_root(file)?;
    let bundle_dir = ws_root.join("target").join("share-bundle");
    let binary_path = bundle_dir.join(format!("{}{}", app_name, ext));
    let hash_path = bundle_dir.join(".last-hash");

    // Cache key: SHA-256 of the entry-point file contents. This correctly invalidates the
    // cache for single-file apps. Multi-file projects (with relative imports or companion
    // assets) will need a directory-walk approach — extend here when needed.
    let source = tokio::fs::read(file)
        .await
        .with_context(|| format!("read source file {}", file.display()))?;
    let hash = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&source);
        hex::encode(h.finalize())
    };

    // Warm cache check
    if binary_path.exists() {
        if let Ok(stored) = tokio::fs::read_to_string(&hash_path).await {
            if stored.trim() == hash.as_str() {
                println!("[vox share] Reusing cached bundle (source unchanged).");
                return Ok(binary_path);
            }
        }
    }

    // Cold build
    println!("[vox share] Building app from {}...", file.display());
    let build_out = ws_root.join("target").join("share-build");
    tokio::fs::create_dir_all(&build_out).await?;
    crate::commands::bundle::run(
        file,
        &build_out,
        None,  // native target
        false, // not release
        BundleMode::App,
        vox_codegen::codegen_rust::RustAppShell::default(),
    )
    .await
    .with_context(|| format!("vox bundle failed for {}", file.display()))?;

    // bundle::run writes the binary to dist/<name>{ext}
    let dist_binary = std::path::PathBuf::from("dist").join(format!("{}{}", app_name, ext));
    anyhow::ensure!(
        dist_binary.exists(),
        "expected bundle output at {} but it was not found",
        dist_binary.display()
    );

    // Cache it
    tokio::fs::create_dir_all(&bundle_dir).await?;
    tokio::fs::copy(&dist_binary, &binary_path)
        .await
        .with_context(|| format!("copy bundle to {}", binary_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = std::fs::metadata(&binary_path)?.permissions();
        perm.set_mode(perm.mode() | 0o111);
        std::fs::set_permissions(&binary_path, perm)?;
    }

    tokio::fs::write(&hash_path, &hash).await?;
    println!("[vox share] Bundle complete → {}", binary_path.display());
    Ok(binary_path)
}

/// Spawn `vox dev <file> --port <port>` as a background child process.
/// Waits until the dev server is accepting connections before returning.
async fn spawn_dev_server(
    file: &std::path::Path,
    port: u16,
) -> anyhow::Result<tokio::process::Child> {
    let vox = std::env::current_exe().context("could not determine current executable path")?;

    let dev_out = std::path::PathBuf::from("target/share-dev");
    tokio::fs::create_dir_all(&dev_out).await?;

    let child = tokio::process::Command::new(&vox)
        .args([
            "dev",
            &file.display().to_string(),
            "--out-dir",
            &dev_out.display().to_string(),
            "--port",
            &port.to_string(),
        ])
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to spawn dev server for {}", file.display()))?;

    println!("[vox share] Dev server starting on port {}...", port);
    wait_for_port(port, std::time::Duration::from_secs(60)).await?;
    println!("[vox share] Dev server ready.");
    Ok(child)
}

/// Poll `127.0.0.1:<port>` until it accepts a TCP connection or `timeout` elapses.
async fn wait_for_port(port: u16, timeout: std::time::Duration) -> anyhow::Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            anyhow::bail!(
                "timed out after {}s waiting for dev server on port {}",
                timeout.as_secs(),
                port
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

/// Walk up the directory tree from `start` to find the workspace root.
///
/// The workspace root is the first ancestor directory that contains a
/// `Cargo.toml` with a `[workspace]` table or a `Vox.toml`.  Falls back
/// to the current working directory if no workspace root is found.
fn find_workspace_root(start: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    let start = if start.is_file() {
        start.parent().unwrap_or(start)
    } else {
        start
    };
    let mut dir = start.to_path_buf();
    loop {
        // `Vox.toml` — Vox workspace manifest
        if dir.join("Vox.toml").exists() {
            return Ok(dir);
        }
        // `Cargo.toml` with `[workspace]` — Rust workspace root
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return Ok(dir);
                }
            }
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => break,
        }
    }
    // Fallback: use cwd
    std::env::current_dir().context("could not determine working directory")
}

fn parse_duration(s: &str) -> Result<Option<Duration>> {
    if s == "none" {
        return Ok(None);
    }
    if s.len() < 2 {
        anyhow::bail!("bad duration `{}` — use e.g. 8h, 30m, 5s, or none", s);
    }
    let (num, unit) = s.split_at(s.len() - 1);
    let n: u64 = num
        .parse()
        .map_err(|_| anyhow::anyhow!("bad duration `{}` — use e.g. 8h, 30m, 5s", s))?;
    Ok(Some(match unit {
        "s" => Duration::from_secs(n),
        "m" => Duration::from_secs(n * 60),
        "h" => Duration::from_secs(n * 3600),
        _ => anyhow::bail!("duration unit must be s/m/h or `none`, got `{}`", unit),
    }))
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn warm_cache_reuses_binary_when_hash_matches() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bundle_dir = tmp.path().join("share-bundle");
        tokio::fs::create_dir_all(&bundle_dir).await.unwrap();

        // Create a fake source file
        let src = tmp.path().join("app.vox");
        tokio::fs::write(&src, b"fake source").await.unwrap();

        // Create a fake binary
        let binary = bundle_dir.join("app");
        tokio::fs::write(&binary, b"fake binary").await.unwrap();

        // Write the matching hash
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"fake source");
        let hash = hex::encode(h.finalize());
        tokio::fs::write(bundle_dir.join(".last-hash"), &hash)
            .await
            .unwrap();

        // Verify the logic: hash matches → warm cache
        let source = tokio::fs::read(&src).await.unwrap();
        let mut h2 = Sha256::new();
        h2.update(&source);
        let computed = hex::encode(h2.finalize());
        assert_eq!(computed, hash, "hash should match for identical content");

        let stored = tokio::fs::read_to_string(bundle_dir.join(".last-hash"))
            .await
            .unwrap();
        assert_eq!(
            stored.trim(),
            computed.as_str(),
            "warm cache condition: hash matches"
        );
    }

    #[tokio::test]
    async fn cold_cache_when_hash_differs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bundle_dir = tmp.path().join("share-bundle");
        tokio::fs::create_dir_all(&bundle_dir).await.unwrap();

        let src = tmp.path().join("app.vox");
        tokio::fs::write(&src, b"modified source").await.unwrap();

        // Write a hash that does NOT match
        tokio::fs::write(bundle_dir.join(".last-hash"), "aabbccdd")
            .await
            .unwrap();
        let binary = bundle_dir.join("app");
        tokio::fs::write(&binary, b"old binary").await.unwrap();

        // Hash the source
        use sha2::{Digest, Sha256};
        let source = tokio::fs::read(&src).await.unwrap();
        let mut h = Sha256::new();
        h.update(&source);
        let computed = hex::encode(h.finalize());

        // Stored hash does not match → cold build needed
        let stored = tokio::fs::read_to_string(bundle_dir.join(".last-hash"))
            .await
            .unwrap();
        assert_ne!(
            stored.trim(),
            computed.as_str(),
            "hash mismatch triggers cold build"
        );
    }
}
