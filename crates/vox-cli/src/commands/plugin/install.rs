//! `vox plugin install` — install a plugin from catalog, local path, or URL.
//!
//! # Modes
//! - `--path <dir>` : copy from local directory (Plugin.toml + siblings)
//! - `--url <url>`  : fetch a .zip, unpack to temp, install-from-path (TODO: not yet implemented)
//! - `<id>`         : look up default-source in catalog and resolve (TODO: github/local source)

use super::list::plugins_root;
use anyhow::{Context, Result, bail};
use std::path::Path;

/// Install a plugin.
///
/// Exactly one of `id` (catalog install), `path` (local dir), or `url` must be provided.
pub async fn run(
    id: Option<&str>,
    path: Option<&Path>,
    url: Option<&str>,
    yes: bool,
) -> Result<()> {
    match (id, path, url) {
        (_, Some(dir), None) => install_from_path(dir, yes),
        (_, None, Some(u)) => install_from_url(u, yes).await,
        (Some(plugin_id), None, None) => install_from_catalog(plugin_id, yes).await,
        (None, None, None) => bail!("Specify a plugin id, --path <dir>, or --url <url>"),
        (Some(_), Some(_), _) | (_, Some(_), Some(_)) => {
            bail!("Only one of id, --path, or --url may be specified at a time")
        }
    }
}

/// Copy plugin files from `src_dir` (must contain Plugin.toml) into the install root.
fn install_from_path(src_dir: &Path, yes: bool) -> Result<()> {
    let plugin_toml_path = src_dir.join("Plugin.toml");
    if !plugin_toml_path.exists() {
        bail!("No Plugin.toml found in {}", src_dir.display());
    }

    // Parse Plugin.toml to discover id + version.
    let raw = std::fs::read_to_string(&plugin_toml_path)
        .with_context(|| format!("reading {}", plugin_toml_path.display()))?;
    let head: PluginHead =
        toml::from_str(&raw).with_context(|| format!("parsing {}", plugin_toml_path.display()))?;
    let id = &head.plugin.id;
    let version = &head.plugin.version;

    let root = plugins_root();
    let dest = root.join(id).join(version);

    if !yes {
        eprint!(
            "Install plugin '{}' v{} from {} to {}? [y/N] ",
            id,
            version,
            src_dir.display(),
            dest.display()
        );
        use std::io::BufRead;
        let mut line = String::new();
        std::io::BufReader::new(std::io::stdin()).read_line(&mut line)?;
        if !line.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    std::fs::create_dir_all(&dest)
        .with_context(|| format!("creating install dir {}", dest.display()))?;

    // Copy all files from src_dir into dest.
    let mut copied = 0usize;
    for entry in std::fs::read_dir(src_dir)? {
        let entry = entry?;
        let from = entry.path();
        if from.is_file() {
            let to = dest.join(entry.file_name());
            std::fs::copy(&from, &to)
                .with_context(|| format!("copying {} -> {}", from.display(), to.display()))?;
            copied += 1;
        }
    }

    println!(
        "✓ Installed plugin '{}' v{} ({} files) → {}",
        id,
        version,
        copied,
        dest.display()
    );
    Ok(())
}

/// Fetch a .zip from `url`, unpack to a temp dir, then install-from-path.
async fn install_from_url(url: &str, yes: bool) -> Result<()> {
    if !url.starts_with("https://") {
        bail!("Only HTTPS URLs are supported (got: {})", url);
    }

    if !yes {
        eprint!("Fetch and install plugin from {}? [y/N] ", url);
        use std::io::BufRead;
        let mut line = String::new();
        std::io::BufReader::new(std::io::stdin()).read_line(&mut line)?;
        if !line.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    println!("Fetching {} …", url);
    let client = vox_http_client::client();
    let bytes = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {}", url))?
        .error_for_status()
        .with_context(|| format!("HTTP error fetching {}", url))?
        .bytes()
        .await
        .context("reading response bytes")?;

    // Create a unique temp directory under the system temp dir.
    let tmp_base = std::env::temp_dir().join(format!("vox-plugin-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp_base).context("creating temp dir")?;

    extract_plugin_zip(&bytes, &tmp_base)?;

    let result = install_from_path(&tmp_base, true);
    // Best-effort cleanup.
    let _ = std::fs::remove_dir_all(&tmp_base);
    result
}

/// Extract a plugin archive, refusing anything that escapes `dest` or is not a
/// plain file or directory.
///
/// `ZipArchive::extract` materialises symlinks and applies no size cap. This is
/// the one extraction path in the codebase whose output is `dlopen`'d, and
/// unlike voxup's tar path it has no checksum gate in front of it.
fn extract_plugin_zip(data: &[u8], dest: &Path) -> Result<()> {
    const MAX_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;
    const MAX_ENTRIES: usize = 10_000;

    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).context("open plugin zip")?;
    if archive.len() > MAX_ENTRIES {
        bail!("plugin archive has more than {MAX_ENTRIES} entries; refusing to extract");
    }

    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("read zip entry")?;
        let enclosed = entry
            .enclosed_name()
            .with_context(|| format!("entry {:?} escapes destination", entry.name()))?;
        let outpath = dest.join(&enclosed);
        if !outpath.starts_with(dest) {
            bail!("entry {:?} escapes destination", entry.name());
        }

        total = total.saturating_add(entry.size());
        if total > MAX_UNCOMPRESSED_BYTES {
            bail!("plugin archive expands beyond {MAX_UNCOMPRESSED_BYTES} bytes");
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&outpath)
                .with_context(|| format!("create dir {}", outpath.display()))?;
            continue;
        }
        // Anything that is neither a plain file nor a directory is refused
        // rather than materialised.
        if entry.is_symlink() {
            bail!(
                "plugin archive contains a symlink entry {:?}; refusing",
                entry.name()
            );
        }
        if let Some(parent) = outpath.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        let mut out = std::fs::File::create(&outpath)
            .with_context(|| format!("create {}", outpath.display()))?;
        std::io::copy(&mut entry, &mut out)
            .with_context(|| format!("write {}", outpath.display()))?;
    }
    Ok(())
}

/// Opt-in switch for the workspace-local plugin source.
///
/// This was previously an opt-*out* env var (the negated, `VOX_NO_`-prefixed
/// form of this same name). Because
/// `workspace_local_plugin_source` walks up eight levels from the CURRENT
/// WORKING DIRECTORY, an opt-out default meant any directory the user happened
/// to be inside could supply a cdylib for a catalog plugin id, bypassing every
/// integrity check. Contributors who want the local source set this explicitly;
/// `--path <dir>` remains the documented alternative.
pub(crate) const LOCAL_FALLBACK_ENV: &str = "VOX_LOCAL_PLUGIN_FALLBACK";

fn local_fallback_enabled() -> bool {
    matches!(
        std::env::var(LOCAL_FALLBACK_ENV).as_deref(),
        Ok("1") | Ok("true")
    )
}

/// Resolve `id` in the catalog, parse default-source, and install.
async fn install_from_catalog(id: &str, yes: bool) -> Result<()> {
    let catalog = vox_plugin_catalog::all_plugins();
    let entry = catalog
        .iter()
        .find(|p| p.id == id)
        .with_context(|| format!("Plugin '{}' not found in catalog", id))?;

    let source = &entry.default_source;

    // Workspace-local fallback: if the caller is running from a vox checkout
    // that contains `crates/vox-plugin-<id>/Plugin.toml`, install from there
    // and skip the GitHub download entirely. Saves the contributor a `--path`
    // copy-paste step and avoids needing GitHub release artifacts during
    // local development. Catalog `local:` entries already do this; this
    // extends the same treatment to `github:` defaults when a matching
    // workspace crate is on disk. Opt-in only — see LOCAL_FALLBACK_ENV.
    if !source.starts_with("local:") && local_fallback_enabled() {
        if let Some(local) = vox_plugin_host::workspace_local_plugin_source(id) {
            println!(
                "ℹ {}=1 — installing plugin '{}' from the local workspace source at {} \
                 instead of the catalog default ('{}'). This path performs NO \
                 integrity verification.",
                LOCAL_FALLBACK_ENV,
                id,
                local.display(),
                source
            );
            return install_from_path(&local, yes);
        }
    }

    // Resolve source to a URL or local path.
    if let Some(rel) = source.strip_prefix("local:") {
        let local_path = std::path::Path::new(rel);
        install_from_path(local_path, yes)
    } else if let Some(gh) = source.strip_prefix("github:") {
        // github:owner/repo → conventional release asset URL.
        let triple = vox_plugin_host::current_target_triple_key();
        let version = "latest";
        let url = format!(
            "https://github.com/{}/releases/{}/download/{}-{}-{}.zip",
            gh, version, id, version, triple
        );
        install_from_url(&url, yes).await
    } else {
        bail!(
            "Unsupported default-source format for plugin '{}': '{}'. \
             Use --path or --url to install manually.",
            id,
            source
        );
    }
}

// ── TOML parsing helpers ──────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct PluginHead {
    plugin: PluginMeta,
}

#[derive(serde::Deserialize)]
struct PluginMeta {
    id: String,
    version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialises every test that mutates `LOCAL_FALLBACK_ENV`.
    ///
    /// `std::env::set_var`/`remove_var` change process-global state, and cargo
    /// runs the tests in this binary in parallel — without this, Task 6's
    /// `catalog_install_refuses_an_unpinned_entry_before_downloading` (which
    /// removes the var) races the two tests below (which set and remove it),
    /// and all three are intermittently wrong. Recover from poisoning rather
    /// than cascading a panic into unrelated tests.
    pub(super) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// The workspace-local fallback must be OPT-IN. As an opt-out it let any
    /// directory the user happened to be inside supply a cdylib for a catalog
    /// plugin id, bypassing every integrity check — the `.`-in-PATH bug class.
    #[test]
    fn local_fallback_is_opt_in_not_opt_out() {
        let src = include_str!("install.rs");
        // Split so this needle cannot match the assertion message below.
        let opt_out = concat!("VOX_NO_", "LOCAL_PLUGIN_FALLBACK");
        assert!(
            !src.contains(opt_out),
            "the workspace-local plugin fallback is still opt-out; it must require \
             {LOCAL_FALLBACK_ENV} to be set before it can bypass verification"
        );
        assert_eq!(LOCAL_FALLBACK_ENV, "VOX_LOCAL_PLUGIN_FALLBACK");
    }

    #[test]
    fn local_fallback_disabled_when_env_unset() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_LOCK serialises every mutator of this variable.
        unsafe { std::env::remove_var(LOCAL_FALLBACK_ENV) };
        assert!(
            !local_fallback_enabled(),
            "fallback must be off unless explicitly enabled"
        );
    }

    #[test]
    fn local_fallback_enabled_when_env_set() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_LOCK serialises every mutator of this variable.
        unsafe { std::env::set_var(LOCAL_FALLBACK_ENV, "1") };
        assert!(local_fallback_enabled());
        unsafe { std::env::remove_var(LOCAL_FALLBACK_ENV) };
    }

    /// Plugin archives are extracted and then dlopen'd. An escaping entry must
    /// be refused, not materialised.
    #[test]
    fn extract_plugin_zip_rejects_escaping_entries() {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            w.start_file("../escaped.txt", opts).expect("start file");
            w.write_all(b"pwned").expect("write");
            w.finish().expect("finish");
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let err = extract_plugin_zip(&buf, dir.path()).expect_err("must reject escaping entry");
        assert!(
            err.to_string().contains("escapes destination"),
            "got: {err}"
        );
        assert!(!dir.path().parent().unwrap().join("escaped.txt").exists());
    }

    #[test]
    fn extract_plugin_zip_accepts_a_normal_entry() {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            w.start_file("Plugin.toml", opts).expect("start file");
            w.write_all(b"[plugin]\n").expect("write");
            w.finish().expect("finish");
        }
        let dir = tempfile::tempdir().expect("tempdir");
        extract_plugin_zip(&buf, dir.path()).expect("normal entry must extract");
        assert!(dir.path().join("Plugin.toml").is_file());
    }
}
