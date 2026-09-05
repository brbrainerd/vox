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
    allow_unverified: bool,
) -> Result<()> {
    match (id, path, url) {
        (_, Some(dir), None) => install_from_path(dir, yes),
        (_, None, Some(u)) => install_from_url(u, yes, None, allow_unverified).await,
        (Some(plugin_id), None, None) => {
            install_from_catalog(plugin_id, yes, allow_unverified).await
        }
        (None, None, None) => bail!("Specify a plugin id, --path <dir>, or --url <url>"),
        (Some(_), Some(_), _) | (_, Some(_), Some(_)) => {
            bail!("Only one of id, --path, or --url may be specified at a time")
        }
    }
}

/// Reject an id or version that would escape the install root once joined.
///
/// The extraction hardening bounds what an ARCHIVE can contain, but the install
/// destination is built from the archive's declared metadata, which no zip
/// entry check ever sees. A plugin declaring `id = "../../.config/autostart"`
/// passes every extraction gate and still writes outside the root.
fn validate_path_component(kind: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        bail!("plugin {kind} is empty");
    }
    if value == "." || value == ".." {
        bail!("plugin {kind} {value:?} is not a usable directory name");
    }
    // Allowlist, not a denylist: this rejects `/`, `\`, a `C:` drive prefix and
    // every separator a future platform might add, without enumerating them.
    if let Some(bad) = value
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.'))
    {
        bail!("plugin {kind} {value:?} contains {bad:?}; allowed: A-Z a-z 0-9 . - _");
    }
    Ok(())
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
    // Both strings come from the archive's OWN Plugin.toml and are about to
    // become path components, so they are attacker-controlled on every install
    // path (catalog, --path, --url). Validate here rather than at each caller:
    // this is the one function they all route through.
    validate_path_component("id", id)?;
    validate_path_component("version", version)?;

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

    // A code plugin is only installed if its cdylib landed. This loop copies
    // top-level files; a workspace source directory contains Cargo.toml and
    // Plugin.toml but no built artifact, so without this check the command
    // printed "✓ Installed" over a directory the host loader cannot dlopen —
    // and `vox plugin list` then reported it as installed.
    let declared = head.plugin.payload.artifacts();
    if !declared.is_empty() {
        let triple = vox_plugin_types::current_target_triple().ok_or_else(|| {
            anyhow::anyhow!(
                "plugin '{id}' ships code artifacts, but this platform is not a supported \
                 plugin target (expected one of {:?})",
                vox_plugin_types::PLUGIN_TARGET_TRIPLES
            )
        })?;
        let filename = declared.get(triple).ok_or_else(|| {
            anyhow::anyhow!(
                "plugin '{id}' declares no artifact for '{triple}' (declares: {:?}). \
                 Add a '{triple}' entry to the artifacts map in Plugin.toml.",
                declared.keys().collect::<Vec<_>>()
            )
        })?;
        if !dest.join(filename).is_file() {
            // Remove the half-installed directory so `vox plugin list` cannot
            // report a plugin that will fail to load. Also drop the now-empty
            // `<id>/` parent (remove_dir only succeeds when it is empty, so a
            // second installed version is never disturbed).
            let _ = std::fs::remove_dir_all(&dest);
            if let Some(parent) = dest.parent() {
                let _ = std::fs::remove_dir(parent);
            }
            bail!(
                "plugin '{id}' v{version}: artifact '{filename}' for '{triple}' was not found in \
                 {}.\nBuild it first, then install from the directory containing it:\n  \
                 cargo build -p {} --release",
                src_dir.display(),
                id_to_crate_name(id),
            );
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

/// Plugin ids are the crate name minus the `vox-plugin-` prefix
/// (`populi-mesh` ⇄ `vox-plugin-populi-mesh`), which is the form `cargo build -p`
/// wants in the remediation above.
fn id_to_crate_name(id: &str) -> String {
    if id.starts_with("vox-plugin-") {
        id.to_string()
    } else {
        format!("vox-plugin-{id}")
    }
}

/// Fetch a .zip from `url`, unpack to a temp dir, then install-from-path.
async fn install_from_url(
    url: &str,
    yes: bool,
    expected_sha256: Option<&str>,
    allow_unverified: bool,
) -> Result<()> {
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

    verify_plugin_archive(&bytes, expected_sha256, allow_unverified, url)?;

    // Create a unique temp directory under the system temp dir.
    let tmp_base = std::env::temp_dir().join(format!("vox-plugin-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp_base).context("creating temp dir")?;

    // Cleanup must cover the extract failure paths too, not just install:
    // extract_plugin_zip bails on the size cap, on an escaping entry, and on a
    // symlink entry; an early return there would strand tmp_base and any
    // partially-written file on disk.
    let result =
        extract_plugin_zip(&bytes, &tmp_base).and_then(|()| install_from_path(&tmp_base, true));
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
    extract_plugin_zip_capped(data, dest, MAX_UNCOMPRESSED_BYTES)
}

/// Same as [`extract_plugin_zip`], with the uncompressed-size cap as a
/// parameter so tests can exercise the cap without building a 512 MiB
/// fixture.
fn extract_plugin_zip_capped(data: &[u8], dest: &Path, max_uncompressed_bytes: u64) -> Result<()> {
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

        // Cheap pre-check on the declared size to skip wasted work, but this
        // is NOT the authoritative bound: `entry.size()` is metadata from the
        // central directory and need not match what DEFLATE actually
        // inflates to. The real bound is on `io::copy`'s output below.
        if entry.size() > max_uncompressed_bytes.saturating_sub(total) {
            bail!("plugin archive expands beyond {max_uncompressed_bytes} bytes");
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
        // Bound the COPY itself, not just the declared metadata: a crafted
        // entry can declare a small size and inflate far past it. `take`
        // physically caps what `io::copy` can pull from `entry`, so this is
        // authoritative regardless of what the entry claims. +1 lets us tell
        // "exactly at the cap" apart from "would exceed it". If we bail
        // mid-copy, `outpath` is left holding a truncated file; the URL
        // caller removes its temp dir on the failure path too, so nothing
        // survives to be dlopen'd.
        use std::io::Read;
        let remaining = max_uncompressed_bytes.saturating_sub(total);
        let mut limited = (&mut entry).take(remaining.saturating_add(1));
        let written = std::io::copy(&mut limited, &mut out)
            .with_context(|| format!("write {}", outpath.display()))?;
        if written > remaining {
            bail!("plugin archive expands beyond {max_uncompressed_bytes} bytes");
        }
        total = total.saturating_add(written);
    }
    Ok(())
}

/// The repo whose own GitHub Release first-party plugins ship as assets of.
/// `install_from_catalog` compares a `github:` source against this to decide
/// whether the dynamic, checksums.txt-verified path below applies instead of
/// requiring a hand-pinned version+sha256.
const FIRST_PARTY_PLUGIN_REPO: &str = "vox-foundation/vox";

/// Override for the release TAG a first-party plugin is fetched from.
///
/// The plugin's version (and therefore its asset filename) always comes from
/// this binary's own `CARGO_PKG_VERSION`. The release it lives in is normally
/// `v<that version>` -- but a prerelease verification run publishes to e.g.
/// `v0.6.0-rc.4735` while the binary still reports `0.6.0`. Without this
/// override the install path cannot be exercised before the final release
/// exists, which would leave a fail-closed security path shipping unproven.
/// Verification-only; never needed for a normal install.
pub(crate) const PLUGIN_RELEASE_TAG_ENV: &str = "VOX_PLUGIN_RELEASE_TAG";

/// Parse a `checksums.txt` body (`sha256sum` output: `<hash>  <filename>`)
/// into filename -> lowercase hex sha256.
///
// vox:defactored-from voxup 2026-08-24 (voxup::download::parse_checksums, ~13 lines)
fn parse_checksums(text: &str) -> std::collections::HashMap<String, String> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let (hash, rest) = line.split_once("  ")?;
            let name = rest.trim().to_string();
            let hash = hash.trim().to_lowercase();
            if hash.len() != 64 {
                return None;
            }
            Some((name, hash))
        })
        .collect()
}

/// Asset URL, checksums URL, and asset filename for a first-party plugin
/// published under release tag `tag`. Pure and deterministic.
fn first_party_plugin_urls_tagged(
    id: &str,
    version: &str,
    triple: &str,
    tag: &str,
) -> (String, String, String) {
    let asset_name = format!("{id}-v{version}-{triple}.zip");
    let base = format!("https://github.com/{FIRST_PARTY_PLUGIN_REPO}/releases/download/{tag}");
    (
        format!("{base}/{asset_name}"),
        format!("{base}/checksums.txt"),
        asset_name,
    )
}

/// Same, for the normal case where the release tag is `v<version>`.
fn first_party_plugin_urls(id: &str, version: &str, triple: &str) -> (String, String, String) {
    first_party_plugin_urls_tagged(id, version, triple, &format!("v{version}"))
}

/// Fetch `checksums.txt` and return the hash recorded for `asset_name`.
///
/// Fail-closed: a network error, malformed body, or missing entry is always
/// an `Err`. Whether that is fatal is the caller's decision, made solely
/// from the explicit `allow_unverified` flag -- never inferred here.
async fn fetch_first_party_checksum(checksums_url: &str, asset_name: &str) -> Result<String> {
    let client = vox_http_client::client();
    let text = client
        .get(checksums_url)
        .send()
        .await
        .with_context(|| format!("GET {checksums_url}"))?
        .error_for_status()
        .with_context(|| format!("HTTP error fetching {checksums_url}"))?
        .text()
        .await
        .context("reading checksums.txt")?;

    parse_checksums(&text).remove(asset_name).with_context(|| {
        format!(
            "no checksums.txt entry found for {asset_name} at {checksums_url}. \
             If this plugin's build was skipped for this release (e.g. the CUDA \
             job is allowed to fail), no asset was published for your platform."
        )
    })
}

/// Install a first-party plugin shipped as a release asset of vox's own repo.
///
/// Neither version nor sha256 is pinned in catalog.toml: the plugin ships in
/// the SAME release as this running binary, so its version is this binary's
/// own `CARGO_PKG_VERSION` and its expected hash comes from that release's
/// `checksums.txt` -- the mechanism `voxup` already uses to verify the `vox`
/// binary itself. This stays inside the trust boundary the running binary is
/// already in; it does not widen it to arbitrary `github:` sources.
async fn install_first_party_plugin(
    id: &str,
    triple: &str,
    yes: bool,
    allow_unverified: bool,
) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let (asset_url, checksums_url, asset_name) = match std::env::var(PLUGIN_RELEASE_TAG_ENV) {
        Ok(tag) if !tag.trim().is_empty() => {
            eprintln!(
                "ℹ {PLUGIN_RELEASE_TAG_ENV}={tag} -- fetching {id} from that release \
                 instead of v{version}."
            );
            first_party_plugin_urls_tagged(id, version, triple, tag.trim())
        }
        _ => first_party_plugin_urls(id, version, triple),
    };

    // Always ATTEMPT the fetch, even with --allow-unverified: the flag means
    // "install despite a failed check", not "skip looking". A user who opts
    // out still gets told what the recorded hash was, or why there wasn't one.
    let expected_sha256 = match fetch_first_party_checksum(&checksums_url, &asset_name).await {
        Ok(hash) => Some(hash),
        Err(e) if allow_unverified => {
            eprintln!(
                "⚠ could not obtain a recorded checksum ({e:#}); continuing because --allow-unverified was passed."
            );
            None
        }
        Err(e) => return Err(e),
    };

    install_from_url(
        &asset_url,
        yes,
        expected_sha256.as_deref(),
        allow_unverified,
    )
    .await
}

/// Verify a downloaded plugin archive and return its lowercase hex SHA-256.
///
/// Fail-closed: with no `expected` hash this REFUSES unless `allow_unverified`.
/// The archive is `dlopen`'d as native code after installation, so an unverified
/// download is arbitrary code execution — see spec finding F9.
///
// vox:defactored-from voxup 2026-08-21 (voxup::download::verify_sha256, ~10 lines)
fn verify_plugin_archive(
    data: &[u8],
    expected: Option<&str>,
    allow_unverified: bool,
    source: &str,
) -> Result<String> {
    use sha2::{Digest, Sha256};
    let actual = hex::encode(Sha256::digest(data));

    match expected {
        Some(want) => {
            let want = want.trim().to_lowercase();
            if want != actual {
                bail!(
                    "plugin checksum mismatch for {source}\n  expected: {want}\n  actual:   {actual}"
                );
            }
            Ok(actual)
        }
        None if allow_unverified => {
            eprintln!(
                "⚠ Installing {source} with no sha256 to check against. Its contents \
                 will be loaded as native code. Actual sha256: {actual}"
            );
            Ok(actual)
        }
        None => bail!(
            "refusing to install {source}: no sha256 recorded for this plugin.\n  \
             Add a `sha256` to its entry in crates/vox-plugin-catalog/catalog.toml, \
             or pass --allow-unverified to accept the risk explicitly.\n  \
             Actual sha256 of the fetched archive: {actual}"
        ),
    }
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
async fn install_from_catalog(id: &str, yes: bool, allow_unverified: bool) -> Result<()> {
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
        // Same threat as the workspace-local fallback, same gate: `rel` is
        // CWD-relative, so any directory the user happens to be inside can
        // supply a cdylib for a catalog plugin id. A `local:` entry only ever
        // resolves inside a repo checkout anyway, which is exactly the
        // developer scenario this env var exists for.
        if !local_fallback_enabled() {
            bail!(
                "refusing to install {id} from its local source {source:?}: it resolves 
                   relative to the current directory, so any directory you happen to be in 
                   could supply the native code this loads.
                   Set {}=1 to opt in, or pass --path <dir> to name the directory explicitly.",
                LOCAL_FALLBACK_ENV
            );
        }
        let local_path = std::path::Path::new(rel);
        install_from_path(local_path, yes)
    } else if let Some(gh) = source.strip_prefix("github:") {
        let triple = vox_plugin_host::current_target_triple_key();

        // First-party plugins shipped inside vox's own release derive both
        // version and checksum dynamically -- see install_first_party_plugin.
        if gh == FIRST_PARTY_PLUGIN_REPO {
            return install_first_party_plugin(id, triple, yes, allow_unverified).await;
        }

        // Third-party sources keep the pinned-hash model, unchanged: no
        // dynamic lookup for a repo this binary shares no release with.
        // Pinned, not `latest`: the bytes behind a floating asset change, so no
        // recorded hash could ever match it.
        let version = entry.version.as_deref().with_context(|| {
            format!(
                "plugin '{id}' has a github: source but no pinned `version` in \
                 catalog.toml; an unpinned release asset cannot be checksummed"
            )
        })?;
        let url = format!(
            "https://github.com/{gh}/releases/download/v{version}/{id}-v{version}-{triple}.zip"
        );
        install_from_url(&url, yes, entry.sha256.as_deref(), allow_unverified).await
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
    #[serde(default)]
    payload: PluginPayload,
}

/// Mirrors the two manifest shapes the host loader accepts
/// (`vox_plugin_host::load_code_plugin`): a `kind = "code"` payload puts the map
/// at `[plugin.payload.artifacts]`, while a composite payload nests it at
/// `[plugin.payload.code.artifacts]`. Skill-only plugins have neither.
#[derive(serde::Deserialize, Default)]
struct PluginPayload {
    /// `kind = "code"` — artifacts directly on the payload.
    #[serde(default)]
    artifacts: std::collections::BTreeMap<String, String>,
    /// Composite payload — artifacts nested under `code`.
    #[serde(default)]
    code: Option<PluginCodePayload>,
}

impl PluginPayload {
    /// Platform key (`<os>-<arch>`, see `vox_plugin_types::PLUGIN_TARGET_TRIPLES`)
    /// → cdylib filename, from whichever shape this manifest uses.
    fn artifacts(&self) -> &std::collections::BTreeMap<String, String> {
        match self.code.as_ref() {
            Some(code) if !code.artifacts.is_empty() => &code.artifacts,
            _ => &self.artifacts,
        }
    }
}

#[derive(serde::Deserialize)]
struct PluginCodePayload {
    #[serde(default)]
    artifacts: std::collections::BTreeMap<String, String>,
}

#[cfg(test)]
mod artifact_guard_tests {
    use super::*;

    fn manifest(artifacts_table: &str) -> String {
        format!(
            r#"
[plugin]
id = "demo"
version = "0.1.0"

{artifacts_table}
"#
        )
    }

    /// Both manifest shapes the host loader accepts must be understood here, or
    /// the guard silently skips whichever one it cannot see.
    #[test]
    fn artifacts_are_read_from_both_manifest_shapes() {
        let code_kind =
            manifest("[plugin.payload.artifacts]\n\"macos-aarch64\" = \"libdemo.dylib\"");
        let head: PluginHead = toml::from_str(&code_kind).expect("code-kind manifest");
        assert_eq!(
            head.plugin
                .payload
                .artifacts()
                .get("macos-aarch64")
                .map(String::as_str),
            Some("libdemo.dylib"),
        );

        let composite =
            manifest("[plugin.payload.code.artifacts]\n\"macos-aarch64\" = \"libdemo.dylib\"");
        let head: PluginHead = toml::from_str(&composite).expect("composite manifest");
        assert_eq!(
            head.plugin
                .payload
                .artifacts()
                .get("macos-aarch64")
                .map(String::as_str),
            Some("libdemo.dylib"),
        );
    }

    /// A skill-only plugin declares no artifacts and must remain installable.
    #[test]
    fn skill_only_plugins_declare_no_artifacts() {
        let head: PluginHead = toml::from_str(&manifest("")).expect("skill manifest");
        assert!(head.plugin.payload.artifacts().is_empty());
    }

    /// The remediation must name the crate `cargo build -p` actually accepts.
    #[test]
    fn crate_name_remediation_is_buildable() {
        assert_eq!(id_to_crate_name("populi-mesh"), "vox-plugin-populi-mesh");
        assert_eq!(
            id_to_crate_name("vox-plugin-populi-mesh"),
            "vox-plugin-populi-mesh"
        );
    }
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
    #[allow(unsafe_code)] // `set_var`/`remove_var` are unsafe on Rust 2024; ENV_LOCK serialises this test's mutators.
    fn local_fallback_disabled_when_env_unset() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var(LOCAL_FALLBACK_ENV) };
        assert!(
            !local_fallback_enabled(),
            "fallback must be off unless explicitly enabled"
        );
    }

    #[test]
    #[allow(unsafe_code)] // `set_var`/`remove_var` are unsafe on Rust 2024; ENV_LOCK serialises this test's mutators.
    fn local_fallback_enabled_when_env_set() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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

    /// The cap must bound bytes actually written, not the entry's declared
    /// (and forgeable) uncompressed-size metadata. Build a real zip, then
    /// patch the uncompressed-size field in both the local file header and
    /// the central directory record down to a small lie — decompression
    /// itself doesn't depend on that field, so `io::copy` still produces the
    /// full payload. A metadata-only check would pass this straight through;
    /// the byte-bounded copy must catch it.
    #[test]
    fn extract_plugin_zip_bounds_actual_bytes_not_declared_size() {
        use std::io::Write;
        let payload = vec![0u8; 5_000]; // highly compressible, tiny once deflated
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            w.start_file("bomb.bin", opts).expect("start file");
            w.write_all(&payload).expect("write");
            w.finish().expect("finish");
        }

        // Patch the uncompressed-size field in the local file header (offset
        // 22 after the PK\x03\x04 signature) and the central directory
        // record (offset 24 after PK\x01\x02) down to a small lie. Anchored
        // on the header signatures rather than a blind byte-pattern search,
        // so this can't accidentally corrupt the compressed data stream.
        let declared_size: [u8; 4] = 5_000u32.to_le_bytes();
        let lie: [u8; 4] = 10u32.to_le_bytes();
        let mut patched = 0;
        let local_sig = [0x50, 0x4B, 0x03, 0x04];
        let central_sig = [0x50, 0x4B, 0x01, 0x02];
        let mut i = 0;
        while i + 4 <= buf.len() {
            let (sig_len, size_off) = if buf[i..i + 4] == local_sig {
                (4, 22)
            } else if buf[i..i + 4] == central_sig {
                (4, 24)
            } else {
                i += 1;
                continue;
            };
            let field = i + size_off;
            assert_eq!(
                buf[field..field + 4],
                declared_size,
                "unexpected header layout"
            );
            buf[field..field + 4].copy_from_slice(&lie);
            patched += 1;
            i += sig_len;
        }
        assert_eq!(
            patched, 2,
            "expected to forge exactly the local and central uncompressed-size fields, patched {patched}"
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let err = extract_plugin_zip_capped(&buf, dir.path(), 1_000)
            .expect_err("must reject an entry that decompresses past the cap");
        assert!(err.to_string().contains("expands beyond"), "got: {err}");
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

    const PAYLOAD: &[u8] = b"pretend this is a plugin zip";

    fn payload_hash() -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(PAYLOAD))
    }

    #[test]
    fn matching_hash_is_accepted_and_returned() {
        let want = payload_hash();
        let got = verify_plugin_archive(PAYLOAD, Some(&want), false, "test://x").unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn mismatched_hash_is_rejected() {
        let err = verify_plugin_archive(PAYLOAD, Some(&"a".repeat(64)), false, "test://x")
            .expect_err("mismatched hash must fail");
        assert!(err.to_string().contains("checksum mismatch"), "got: {err}");
    }

    /// The core property: with no expected hash, installation is REFUSED.
    /// The escape this phase's extraction hardening cannot see: it lives in the
    /// archive's declared metadata, not in any zip entry.
    /// Wiring test, not a unit test: the validator existing is worthless if
    /// `install_from_path` does not CALL it. Removing either call site must
    /// break this, which a direct `validate_path_component` test cannot detect.
    #[test]
    fn install_from_path_refuses_a_plugin_whose_declared_id_escapes() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("Plugin.toml"),
            "[plugin]
id = \"../../../../evil\"
version = \"0.1.0\"
",
        )
        .expect("write Plugin.toml");
        let err = install_from_path(dir.path(), true)
            .expect_err("a declared id that escapes the install root must be refused");
        let m = err.to_string();
        assert!(m.contains("id"), "error must name the offending field: {m}");
    }

    #[test]
    fn install_from_path_refuses_a_plugin_whose_declared_version_escapes() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("Plugin.toml"),
            "[plugin]
id = \"ok-plugin\"
version = \"../../..\"
",
        )
        .expect("write Plugin.toml");
        let err = install_from_path(dir.path(), true)
            .expect_err("a declared version that escapes the install root must be refused");
        assert!(
            err.to_string().contains("version"),
            "error must name the offending field: {err}"
        );
    }

    /// The sibling of the workspace-local fallback: a `local:` catalog source is
    /// CWD-relative, so it must sit behind the same opt-in.
    #[tokio::test]
    #[allow(unsafe_code)] // `remove_var` is unsafe on Rust 2024; ENV_LOCK serialises this test's mutators.
    #[allow(clippy::await_holding_lock)] // `#[tokio::test]` is single-threaded, so holding the guard across the await is sound.
    async fn catalog_local_source_is_refused_without_the_opt_in() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var(LOCAL_FALLBACK_ENV) };
        // nvml-probe, not mens-candle-metal: the GPU plugins now ship as
        // first-party release assets (github:vox-foundation/vox), so they no
        // longer exercise the `local:` refusal this test exists to guard --
        // and routing them here would make a live network call from a unit
        // test. nvml-probe is still a `local:` entry in catalog.toml.
        let err = install_from_catalog("nvml-probe", true, false)
            .await
            .expect_err("a CWD-relative local: source must not install by default");
        let m = err.to_string();
        assert!(
            m.contains(LOCAL_FALLBACK_ENV),
            "error must name the opt-in switch: {m}"
        );
    }

    #[test]
    fn a_traversing_id_is_refused_before_it_becomes_a_path() {
        for bad in [
            "../../../../../.config/autostart",
            "..",
            ".",
            "a/b",
            r"a\b",
            "C:evil",
            "",
        ] {
            assert!(
                validate_path_component("id", bad).is_err(),
                "must reject id {bad:?} — it is joined onto the install root"
            );
        }
    }

    #[test]
    fn a_traversing_version_is_refused_too() {
        assert!(validate_path_component("version", "../../etc").is_err());
        // ...while the version strings real plugins actually use still pass.
        for ok in ["0.1.0", "1.2.3-beta.1", "2026_08_22"] {
            validate_path_component("version", ok)
                .unwrap_or_else(|e| panic!("legitimate version {ok:?} must pass: {e}"));
        }
    }

    #[test]
    fn a_normal_id_still_passes() {
        for ok in ["oratio", "mens-candle-cuda", "skill_git", "v0"] {
            validate_path_component("id", ok)
                .unwrap_or_else(|e| panic!("legitimate id {ok:?} must pass: {e}"));
        }
    }

    #[test]
    fn missing_hash_is_refused_by_default() {
        let err = verify_plugin_archive(PAYLOAD, None, false, "https://example/p.zip")
            .expect_err("an unverifiable plugin must not install");
        let m = err.to_string();
        assert!(m.contains("no sha256"), "error must say why: {m}");
        assert!(
            m.contains("--allow-unverified"),
            "error must name the override: {m}"
        );
    }

    #[test]
    fn missing_hash_is_allowed_with_the_explicit_override() {
        let got = verify_plugin_archive(PAYLOAD, None, true, "https://example/p.zip").unwrap();
        assert_eq!(got, payload_hash());
    }

    /// A catalog install must fail BEFORE any network call when the entry is
    /// unpinned — an unpinned `latest` asset cannot be checksummed at all.
    #[tokio::test]
    #[allow(unsafe_code)] // `remove_var` is unsafe on Rust 2024; ENV_LOCK serialises this test's mutators.
    #[allow(clippy::await_holding_lock)] // `#[tokio::test]` is single-threaded, so holding the guard across the await is sound.
    async fn catalog_install_refuses_an_unpinned_entry_before_downloading() {
        // ENV_LOCK (defined above) serialises this against the fallback tests,
        // which set and remove the same process-global variable.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var(LOCAL_FALLBACK_ENV) };
        let err = install_from_catalog("oratio", true, false)
            .await
            .expect_err("unpinned catalog entry must not install");
        let m = err.to_string();
        assert!(
            m.contains("no pinned `version`") || m.contains("no sha256"),
            "expected a pre-network refusal, got: {m}"
        );
    }

    #[test]
    fn parse_checksums_extracts_name_to_hash_pairs() {
        let text = "\
aaaa000000000000000000000000000000000000000000000000000000001111  mens-candle-metal-v0.6.0-macos-aarch64.zip
bbbb000000000000000000000000000000000000000000000000000000002222  checksums.txt
";
        let got = parse_checksums(text);
        assert_eq!(
            got.get("mens-candle-metal-v0.6.0-macos-aarch64.zip")
                .map(String::as_str),
            Some("aaaa000000000000000000000000000000000000000000000000000000001111")
        );
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn parse_checksums_skips_short_and_blank_lines() {
        let text = "\n   \nnothash  file.zip\n";
        let got = parse_checksums(text);
        assert!(
            got.is_empty(),
            "malformed/short hash lines must be skipped, got {got:?}"
        );
    }

    #[test]
    fn first_party_plugin_urls_are_built_from_the_running_binary_version() {
        let (asset_url, checksums_url, asset_name) =
            first_party_plugin_urls("mens-candle-metal", "0.6.0", "macos-aarch64");
        assert_eq!(asset_name, "mens-candle-metal-v0.6.0-macos-aarch64.zip");
        assert_eq!(
            asset_url,
            "https://github.com/vox-foundation/vox/releases/download/v0.6.0/mens-candle-metal-v0.6.0-macos-aarch64.zip"
        );
        assert_eq!(
            checksums_url,
            "https://github.com/vox-foundation/vox/releases/download/v0.6.0/checksums.txt"
        );
    }

    /// The release tag and the plugin's own version are separate inputs: a
    /// prerelease (v0.6.0-rc.4735) ships a plugin whose version is still
    /// 0.6.0, so the asset NAME and the release TAG must be allowed to
    /// differ. Without this the security-critical install path cannot be
    /// exercised on any rc tag.
    #[test]
    fn a_release_tag_override_changes_only_the_tag_not_the_asset_name() {
        let (asset_url, checksums_url, asset_name) = first_party_plugin_urls_tagged(
            "mens-candle-metal",
            "0.6.0",
            "macos-aarch64",
            "v0.6.0-rc.4735",
        );
        assert_eq!(asset_name, "mens-candle-metal-v0.6.0-macos-aarch64.zip");
        assert!(
            asset_url.contains("/download/v0.6.0-rc.4735/"),
            "got {asset_url}"
        );
        assert!(
            asset_url.ends_with("mens-candle-metal-v0.6.0-macos-aarch64.zip"),
            "got {asset_url}"
        );
        assert_eq!(
            checksums_url,
            "https://github.com/vox-foundation/vox/releases/download/v0.6.0-rc.4735/checksums.txt"
        );
    }

    /// Regression guard: an earlier version of this resolver interpolated the
    /// literal string `"latest"` instead of the running binary's version,
    /// silently pointing every install at whatever the newest GitHub release
    /// happened to be. That bug is fixed, but nothing else in this file
    /// stops it from coming back -- this test is that guard.
    #[test]
    fn first_party_plugin_urls_never_contain_the_literal_latest() {
        let (asset_url, checksums_url, _) =
            first_party_plugin_urls("mens-candle-cuda", "0.6.0", "linux-x86_64");
        assert!(!asset_url.contains("latest"), "got {asset_url}");
        assert!(!checksums_url.contains("latest"), "got {checksums_url}");
    }
}
