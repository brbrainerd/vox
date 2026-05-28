//! Toolchain upgrade: release lane (checksum-verified binary via `self_update`) and repo lane
//! (`commands::repo_upgrade`). Does not touch `Vox.toml` / `vox.lock`. See `vox upgrade` in CLI docs.

use crate::cli_args::{UpgradeLane, UpgradeReleaseProvider, UpgradeToolchainArgs};
use anyhow::{Result, anyhow};
use self_update::{
    ArchiveKind, Compression, Download, Extract, Move, TempDir,
    backends::{github, gitlab},
    update::{Release, ReleaseAsset, ReleaseUpdate},
};
use self_update::{get_target, version};
use semver::Version;
use std::path::PathBuf;
use crate::utils::install_policy::{DEFAULT_RELEASE_GITHUB_OWNER, DEFAULT_RELEASE_GITHUB_REPO};

/// Blocking entry (call from `spawn_blocking`).
pub fn run_toolchain_upgrade(args: &UpgradeToolchainArgs, json_output: bool) -> Result<()> {
    if args.lane == UpgradeLane::Repo {
        return crate::commands::repo_upgrade::run_repo_upgrade(args, json_output);
    }

    let triple = get_target().to_string();
    let current_str = env!("CARGO_PKG_VERSION");
    let current_ver = Version::parse(current_str)
        .map_err(|e| anyhow!("internal: CARGO_PKG_VERSION semver: {e}"))?;

    let provider = resolve_provider(args)?;
    let channel = normalize_channel(&args.channel)?;
    let allow_pre = args.allow_prerelease || matches!(channel, Channel::Next);

    let auth = AuthTokens {
        github: optional_env(&["GITHUB_TOKEN", "GH_TOKEN", "VOX_GITHUB_TOKEN"]),
        gitlab: optional_env(&["GITLAB_TOKEN", "VOX_GITLAB_TOKEN"]),
    };

    let candidate = resolve_candidate(
        &provider,
        &triple,
        current_str,
        &current_ver,
        args,
        channel,
        allow_pre,
        &auth,
    )
    .map_err(map_self_update)?;

    let Some(candidate) = candidate else {
        return emit_up_to_date(json_output, current_str, &triple, provider.describe());
    };

    if !args.apply {
        return emit_check_only(
            json_output,
            current_str,
            &candidate,
            &triple,
            provider.describe(),
        );
    }

    install_candidate(&candidate, &auth, json_output, current_str, &triple)?;

    if json_output {
        println!(
            "{}",
            serde_json::json!({
                "toolchain_upgrade": {
                    "status": "installed",
                    "current_version": current_str,
                    "installed_version": candidate.release.version,
                    "asset": candidate.asset.name,
                    "target_triple": triple,
                    "source": provider.describe(),
                    "manifest_graph_touched": false,
                }
            })
        );
    } else {
        println!(
            "Installed vox {} for {} (was {}).",
            candidate.release.version, triple, current_str
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channel {
    Stable,
    Next,
}

fn normalize_channel(s: &str) -> Result<Channel> {
    match s.trim().to_ascii_lowercase().as_str() {
        "stable" => Ok(Channel::Stable),
        "next" => Ok(Channel::Next),
        other => Err(anyhow!(
            "unknown upgrade channel `{other}` (use `stable` or `next`)"
        )),
    }
}

#[derive(Clone, Debug)]
enum Provider {
    Github {
        owner: String,
        repo: String,
        api_base: Option<String>,
    },
    Gitlab {
        host: String,
        owner: String,
        repo: String,
    },
    Http {
        base: String,
        tag: String,
    },
}

impl Provider {
    fn describe(&self) -> serde_json::Value {
        match self {
            Provider::Github { owner, repo, .. } => {
                serde_json::json!({"kind": "github", "owner": owner, "repo": repo})
            }
            Provider::Gitlab { host, owner, repo } => {
                serde_json::json!({"kind": "gitlab", "host": host, "owner": owner, "repo": repo})
            }
            Provider::Http { base, tag } => {
                serde_json::json!({"kind": "http", "base": base, "tag": tag})
            }
        }
    }
}

#[derive(Clone, Debug)]
struct AuthTokens {
    github: Option<String>,
    gitlab: Option<String>,
}

#[derive(Clone, Debug)]
struct Candidate {
    release: Release,
    asset: ReleaseAsset,
    checksums_url: String,
}

fn resolve_provider(args: &UpgradeToolchainArgs) -> Result<Provider> {
    let from_env = std::env::var("VOX_UPGRADE_PROVIDER")
        .ok()
        .map(|s| s.to_ascii_lowercase());
    let src = args.provider.or_else(|| {
        from_env.as_deref().and_then(|e| match e {
            "gitlab" => Some(UpgradeReleaseProvider::Gitlab),
            "http" => Some(UpgradeReleaseProvider::Http),
            "github" => Some(UpgradeReleaseProvider::Github),
            _ => None,
        })
    });

    let repo_arg = args
        .repo
        .clone()
        .or_else(|| std::env::var("VOX_UPGRADE_REPO").ok());

    match src.unwrap_or(UpgradeReleaseProvider::Github) {
        UpgradeReleaseProvider::Http => {
            let base = args
                .base_url
                .clone()
                .or_else(|| std::env::var("VOX_UPGRADE_BASE_URL").ok())
                .ok_or_else(|| {
                    anyhow!("`--provider http` requires `--base-url` or `VOX_UPGRADE_BASE_URL`")
                })?;
            let tag = args
                .version
                .clone()
                .or_else(|| std::env::var("VOX_UPGRADE_VERSION").ok())
                .ok_or_else(|| {
                    anyhow!("`--provider http` requires `--version` (release tag) or `VOX_UPGRADE_VERSION`")
                })?;
            Ok(Provider::Http {
                base: base.trim_end_matches('/').to_string(),
                tag: normalize_tag(&tag),
            })
        }
        UpgradeReleaseProvider::Gitlab => {
            let (owner, repo) = parse_owner_repo(
                &repo_arg,
                DEFAULT_RELEASE_GITHUB_OWNER,
                DEFAULT_RELEASE_GITHUB_REPO,
            )?;
            let host = args
                .gitlab_host
                .clone()
                .or_else(|| std::env::var("VOX_UPGRADE_GITLAB_HOST").ok())
                .unwrap_or_else(|| "https://gitlab.com".to_string());
            Ok(Provider::Gitlab {
                host: host.trim_end_matches('/').to_string(),
                owner,
                repo,
            })
        }
        UpgradeReleaseProvider::Github => {
            let (owner, repo) = parse_owner_repo(
                &repo_arg,
                DEFAULT_RELEASE_GITHUB_OWNER,
                DEFAULT_RELEASE_GITHUB_REPO,
            )?;
            let api_base = args
                .github_api_url
                .clone()
                .or_else(|| std::env::var("VOX_UPGRADE_GITHUB_API_URL").ok());
            Ok(Provider::Github {
                owner,
                repo,
                api_base,
            })
        }
    }
}

fn parse_owner_repo(
    repo: &Option<String>,
    def_owner: &str,
    def_repo: &str,
) -> Result<(String, String)> {
    let s = repo
        .clone()
        .unwrap_or_else(|| format!("{def_owner}/{def_repo}"));
    let mut it = s.splitn(2, '/');
    let owner = it
        .next()
        .filter(|o| !o.is_empty())
        .ok_or_else(|| anyhow!("invalid repo `{s}` (expected owner/repo)"))?
        .to_string();
    let repo_name = it
        .next()
        .filter(|r| !r.is_empty())
        .ok_or_else(|| anyhow!("invalid repo `{s}` (expected owner/repo)"))?
        .to_string();
    Ok((owner, repo_name))
}

fn optional_env(keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Ok(v) = std::env::var(k) {
            let t = v.trim().to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    None
}

fn normalize_tag(v: &str) -> String {
    let v = v.trim();
    if v.starts_with('v') {
        v.to_string()
    } else {
        format!("v{v}")
    }
}

fn tag_prefix_for_urls(version_field: &str) -> String {
    // `Release.version` from self_update is tag without leading `v`.
    format!("v{version_field}")
}

fn map_self_update(e: self_update::errors::Error) -> anyhow::Error {
    anyhow!("toolchain upgrade: {e}")
}

fn resolve_candidate(
    provider: &Provider,
    triple: &str,
    current_str: &str,
    current_ver: &Version,
    args: &UpgradeToolchainArgs,
    channel: Channel,
    allow_pre: bool,
    auth: &AuthTokens,
) -> Result<Option<Candidate>, self_update::errors::Error> {
    let allow_breaking = args.allow_breaking;

    if let Some(ver) = &args.version {
        let tag = normalize_tag(ver);
        return Ok(Some(fetch_pinned(provider, triple, &tag, auth)?));
    }

    match provider {
        Provider::Http { .. } => Ok(None),
        Provider::Github {
            owner,
            repo,
            api_base,
        } => {
            let mut list_b = github::ReleaseList::configure();
            list_b.repo_owner(owner).repo_name(repo).with_target(triple);
            if let Some(t) = &auth.github {
                list_b.auth_token(t);
            }
            if let Some(u) = api_base {
                list_b.with_url(u);
            }
            let mut releases = list_b.build()?.fetch()?;
            releases.sort_by(|a, b| cmp_release_versions(b, a));
            pick_from_releases_github(
                releases,
                triple,
                current_str,
                current_ver,
                channel,
                allow_pre,
                allow_breaking,
                owner,
                repo,
            )
        }
        Provider::Gitlab { host, owner, repo } => {
            let mut list_b = gitlab::ReleaseList::configure();
            list_b.with_host(host);
            list_b.repo_owner(owner).repo_name(repo).with_target(triple);
            if let Some(t) = &auth.gitlab {
                list_b.auth_token(t);
            }
            let mut releases = list_b.build()?.fetch()?;
            releases.sort_by(|a, b| cmp_release_versions(b, a));
            pick_from_releases_gitlab(
                releases,
                triple,
                current_str,
                current_ver,
                channel,
                allow_pre,
                allow_breaking,
            )
        }
    }
}

fn cmp_release_versions(a: &Release, b: &Release) -> std::cmp::Ordering {
    let va = Version::parse(&a.version).ok();
    let vb = Version::parse(&b.version).ok();
    match (va, vb) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => std::cmp::Ordering::Equal,
    }
}

fn release_eligible(
    rel: &Release,
    triple: &str,
    current_str: &str,
    current_ver: &Version,
    allow_pre: bool,
    allow_breaking: bool,
) -> Option<ReleaseAsset> {
    let ver_parsed = Version::parse(&rel.version).ok()?;
    if ver_parsed <= *current_ver {
        return None;
    }
    if !ver_parsed.pre.is_empty() && !allow_pre {
        return None;
    }
    let compatible = version::bump_is_compatible(current_str, &rel.version).unwrap_or(false);
    if !compatible && !allow_breaking {
        return None;
    }
    rel.asset_for(triple, Some("vox-"))
}

fn pick_from_releases_github(
    releases: Vec<Release>,
    triple: &str,
    current_str: &str,
    current_ver: &Version,
    _channel: Channel,
    allow_pre: bool,
    allow_breaking: bool,
    owner: &str,
    repo: &str,
) -> Result<Option<Candidate>, self_update::errors::Error> {
    for rel in releases {
        let Some(asset) = release_eligible(
            &rel,
            triple,
            current_str,
            current_ver,
            allow_pre,
            allow_breaking,
        ) else {
            continue;
        };
        let tag = tag_prefix_for_urls(&rel.version);
        let checksums_url = github_checksums_url(owner, repo, &tag);
        return Ok(Some(Candidate {
            release: rel,
            asset,
            checksums_url,
        }));
    }
    Ok(None)
}

fn pick_from_releases_gitlab(
    releases: Vec<Release>,
    triple: &str,
    current_str: &str,
    current_ver: &Version,
    _channel: Channel,
    allow_pre: bool,
    allow_breaking: bool,
) -> Result<Option<Candidate>, self_update::errors::Error> {
    for rel in releases {
        let Some(asset) = release_eligible(
            &rel,
            triple,
            current_str,
            current_ver,
            allow_pre,
            allow_breaking,
        ) else {
            continue;
        };
        let checksums_url = gitlab_checksums_url(&rel)?;
        return Ok(Some(Candidate {
            release: rel,
            asset,
            checksums_url,
        }));
    }
    Ok(None)
}

fn github_checksums_url(owner: &str, repo: &str, tag: &str) -> String {
    format!("https://github.com/{owner}/{repo}/releases/download/{tag}/checksums.txt")
}

fn gitlab_checksums_url(release: &Release) -> Result<String, self_update::errors::Error> {
    release
        .assets
        .iter()
        .find(|a| {
            a.name == "checksums.txt"
                || a.name.ends_with("/checksums.txt")
                || a.name.contains("checksums.txt")
        })
        .map(|a| a.download_url.clone())
        .ok_or_else(|| {
            self_update::errors::Error::Release(
                "No checksums.txt link in GitLab release assets".into(),
            )
        })
}

fn fetch_pinned(
    provider: &Provider,
    triple: &str,
    tag: &str,
    auth: &AuthTokens,
) -> Result<Candidate, self_update::errors::Error> {
    let release = match provider {
        Provider::Github {
            owner,
            repo,
            api_base,
        } => {
            let mut b = github::Update::configure();
            b.repo_owner(owner).repo_name(repo);
            b.current_version("0.0.0")
                .bin_name("vox")
                .bin_path_in_archive(bin_inside_archive())
                .show_output(false)
                .no_confirm(true)
                .show_download_progress(false);
            if let Some(t) = &auth.github {
                b.auth_token(t);
            }
            if let Some(u) = api_base {
                b.with_url(u);
            }
            let u = b.build()?;
            ReleaseUpdate::get_release_version(u.as_ref(), tag)?
        }
        Provider::Gitlab { host, owner, repo } => {
            let mut b = gitlab::Update::configure();
            b.with_host(host);
            b.repo_owner(owner).repo_name(repo);
            b.current_version("0.0.0")
                .bin_name("vox")
                .bin_path_in_archive(bin_inside_archive())
                .show_output(false)
                .no_confirm(true)
                .show_download_progress(false);
            if let Some(t) = &auth.gitlab {
                b.auth_token(t);
            }
            let u = b.build()?;
            ReleaseUpdate::get_release_version(u.as_ref(), tag)?
        }
        Provider::Http { base, .. } => {
            let ext = archive_ext();
            let basename = format!("vox-{tag}-{triple}.{ext}");
            let asset_url = format!("{base}/download/{tag}/{basename}");
            let checksums_url = format!("{base}/download/{tag}/checksums.txt");
            let asset = ReleaseAsset {
                name: basename.clone(),
                download_url: asset_url.clone(),
            };
            return Ok(Candidate {
                release: Release {
                    name: tag.to_string(),
                    version: tag.trim_start_matches('v').to_string(),
                    date: String::new(),
                    body: None,
                    assets: vec![asset.clone()],
                },
                asset,
                checksums_url,
            });
        }
    };

    let asset = release
        .asset_for(triple, Some("vox-"))
        .ok_or_else(|| self_update::errors::Error::Release("no vox asset for target".into()))?;

    let checksums_url = match provider {
        Provider::Github { owner, repo, .. } => github_checksums_url(owner, repo, tag),
        Provider::Gitlab { .. } => gitlab_checksums_url(&release)?,
        Provider::Http { .. } => unreachable!(),
    };

    Ok(Candidate {
        release,
        asset,
        checksums_url,
    })
}

fn bin_inside_archive() -> &'static str {
    if cfg!(target_os = "windows") {
        "vox.exe"
    } else {
        "vox"
    }
}

fn archive_ext() -> &'static str {
    if cfg!(target_os = "windows") {
        "zip"
    } else {
        "tar.gz"
    }
}

fn archive_kind_for_asset_name(name: &str) -> ArchiveKind {
    if name.ends_with(".zip") {
        ArchiveKind::Zip
    } else {
        ArchiveKind::Tar(Some(Compression::Gz))
    }
}

fn install_bin_dir() -> Result<PathBuf> {
    if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
        return Ok(PathBuf::from(cargo_home).join("bin"));
    }
    if cfg!(target_os = "windows") {
        let user = std::env::var("USERPROFILE")
            .map_err(|_| anyhow!("USERPROFILE is not set (needed to locate ~/.cargo/bin)"))?;
        Ok(PathBuf::from(user).join(".cargo").join("bin"))
    } else {
        let home = std::env::var("HOME")
            .map_err(|_| anyhow!("HOME is not set (needed to locate ~/.cargo/bin)"))?;
        Ok(PathBuf::from(home).join(".cargo").join("bin"))
    }
}

fn verify_checksum(asset_bytes: &[u8], checksums_txt: &str, asset_name: &str) -> Result<()> {
    crate::utils::checksum_manifest::verify_checksum(asset_bytes, checksums_txt, asset_name)
        .map_err(|e| anyhow!(e))
}

fn download_bytes(url: &str, auth: &AuthTokens) -> Result<Vec<u8>> {
    let mut d = Download::from_url(url);
    d.set_header(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/octet-stream"),
    );
    if url.contains("github.com") {
        if let Some(ref t) = auth.github {
            d.set_header(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("token {t}"))
                    .map_err(|e| anyhow!("Authorization header: {e}"))?,
            );
        }
    } else if url.contains("gitlab") {
        if let Some(ref t) = auth.gitlab {
            d.set_header(
                reqwest::header::HeaderName::from_static("PRIVATE-TOKEN"),
                reqwest::header::HeaderValue::from_str(t)
                    .map_err(|e| anyhow!("PRIVATE-TOKEN header: {e}"))?,
            );
        }
    }
    let mut buf = Vec::new();
    d.download_to(&mut buf).map_err(map_self_update)?;
    Ok(buf)
}

fn download_checksum_manifest(url: &str, auth: &AuthTokens) -> Result<String> {
    let bytes = download_bytes(url, auth)?;
    String::from_utf8(bytes).map_err(|e| anyhow!("checksums.txt is not UTF-8: {e}"))
}

fn install_candidate(
    candidate: &Candidate,
    auth: &AuthTokens,
    json_output: bool,
    _current_str: &str,
    target_triple: &str,
) -> Result<()> {
    let asset_bytes = download_bytes(&candidate.asset.download_url, auth)?;
    let checksum_txt = download_checksum_manifest(&candidate.checksums_url, auth)?;
    verify_checksum(&asset_bytes, &checksum_txt, &candidate.asset.name)?;

    let tmp = TempDir::new().map_err(|e| anyhow!("temp dir: {e}"))?;
    let archive_path = tmp.path().join(&candidate.asset.name);
    std::fs::write(&archive_path, &asset_bytes).map_err(|e| anyhow!(e))?;

    let bin_in = bin_inside_archive();
    let mut ex = Extract::from_source(&archive_path);
    ex.archive(archive_kind_for_asset_name(&candidate.asset.name));
    ex.extract_file(tmp.path(), bin_in)
        .map_err(map_self_update)?;
    let extracted = tmp.path().join(bin_in);

    let dest_dir = install_bin_dir()?;
    std::fs::create_dir_all(&dest_dir).map_err(|e| anyhow!(e))?;
    let dest = dest_dir.join(bin_in);

    if let Ok(cur) = std::env::current_exe() {
        if cur != dest && !json_output {
            eprintln!(
                "note: running executable is `{}`, install target is `{}` (PATH may pick a different copy).",
                cur.display(),
                dest.display()
            );
        }
    }

    Move::from_source(extracted.as_path())
        .to_dest(dest.as_path())
        .map_err(map_self_update)?;

    maybe_install_openclaw_sidecar(
        candidate,
        auth,
        target_triple,
        &checksum_txt,
        &dest_dir,
        json_output,
    )?;

    run_bootstrap_environment_check(candidate, auth, target_triple, &checksum_txt, json_output)?;

    Ok(())
}

fn maybe_install_openclaw_sidecar(
    candidate: &Candidate,
    auth: &AuthTokens,
    target_triple: &str,
    checksum_txt: &str,
    dest_dir: &std::path::Path,
    json_output: bool,
) -> Result<()> {
    if std::env::var(crate::utils::install_policy::VOX_OPENCLAW_SIDECAR_DISABLE_ENV)
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    {
        return Ok(());
    }
    let ext = if cfg!(target_os = "windows") {
        ".zip"
    } else {
        ".tar.gz"
    };
    let sidecar_asset = find_sidecar_asset(checksum_txt, target_triple, ext)
        .ok_or_else(|| anyhow!("OpenClaw sidecar asset not found in release checksums. Set VOX_OPENCLAW_SIDECAR_DISABLE_ENV=1 to skip."))?;

    let mut base = candidate.asset.download_url.clone();
    if let Some(idx) = base.rfind('/') {
        base.truncate(idx + 1);
    }
    let sidecar_url = format!("{base}{sidecar_asset}");
    let sidecar_bytes = download_bytes(&sidecar_url, auth)
        .map_err(|e| anyhow!("Failed to download OpenClaw sidecar: {e}"))?;

    verify_checksum(&sidecar_bytes, checksum_txt, &sidecar_asset)
        .map_err(|e| anyhow!("OpenClaw sidecar checksum verification failed: {e}"))?;

    let tmp = TempDir::new().map_err(|e| anyhow!("temp dir: {e}"))?;
    let archive_path = tmp.path().join(&sidecar_asset);
    std::fs::write(&archive_path, &sidecar_bytes).map_err(|e| anyhow!(e))?;
    let sidecar_bin = if cfg!(target_os = "windows") {
        format!("{}.exe", crate::utils::install_policy::OPENCLAW_SIDECAR_BIN_BASENAME)
    } else {
        crate::utils::install_policy::OPENCLAW_SIDECAR_BIN_BASENAME.to_string()
    };
    let mut ex = Extract::from_source(&archive_path);
    ex.archive(archive_kind_for_asset_name(&sidecar_asset));
    ex.extract_file(tmp.path(), &sidecar_bin)
        .map_err(|e| anyhow!("Failed to extract OpenClaw sidecar: {e}"))?;

    let extracted = tmp.path().join(&sidecar_bin);
    let dest = dest_dir.join(&sidecar_bin);
    Move::from_source(extracted.as_path())
        .to_dest(dest.as_path())
        .map_err(|e| anyhow!("Failed to move OpenClaw sidecar to destination: {e}"))?;

    if !json_output {
        eprintln!("Installed OpenClaw sidecar: {}", dest.display());
    }
    Ok(())
}

fn find_sidecar_asset(checksum_txt: &str, target_triple: &str, ext: &str) -> Option<String> {
    for line in checksum_txt.lines() {
        let mut parts = line.split_whitespace();
        let Some(_hash) = parts.next() else {
            continue;
        };
        let Some(path) = parts.next() else {
            continue;
        };
        let file = path.rsplit('/').next().unwrap_or(path).to_string();
        if !file.contains(target_triple) || !file.ends_with(ext) {
            continue;
        }
        if crate::utils::install_policy::OPENCLAW_SIDECAR_ASSET_PREFIXES
            .iter()
            .any(|prefix| file.starts_with(prefix))
        {
            return Some(file);
        }
    }
    None
}

fn run_bootstrap_environment_check(
    candidate: &Candidate,
    auth: &AuthTokens,
    target_triple: &str,
    checksum_txt: &str,
    json_output: bool,
) -> Result<()> {
    let ext = if cfg!(target_os = "windows") {
        ".zip"
    } else {
        ".tar.gz"
    };

    let bootstrap_asset = find_bootstrap_asset(checksum_txt, target_triple, ext);
    let Some(bootstrap_asset) = bootstrap_asset else {
        return Ok(());
    };

    let mut base = candidate.asset.download_url.clone();
    if let Some(idx) = base.rfind('/') {
        base.truncate(idx + 1);
    }
    let bg_url = format!("{base}{bootstrap_asset}");
    let bg_bytes = match download_bytes(&bg_url, auth) {
        Ok(b) => b,
        Err(e) => {
            if !json_output {
                eprintln!("note: could not download vox-bootstrap to perform env check: {e}");
            }
            return Ok(());
        }
    };

    if verify_checksum(&bg_bytes, checksum_txt, &bootstrap_asset).is_err() {
        if !json_output {
            eprintln!("note: vox-bootstrap checksum verification failed, skipping env check");
        }
        return Ok(());
    }

    let tmp = TempDir::new().map_err(|e| anyhow!("temp dir: {e}"))?;
    let archive_path = tmp.path().join(&bootstrap_asset);
    std::fs::write(&archive_path, &bg_bytes).map_err(|e| anyhow!(e))?;
    let bg_bin = if cfg!(target_os = "windows") {
        "vox-bootstrap.exe"
    } else {
        "vox-bootstrap"
    };

    let mut ex = Extract::from_source(&archive_path);
    ex.archive(archive_kind_for_asset_name(&bootstrap_asset));
    if ex.extract_file(tmp.path(), bg_bin).is_err() {
        return Ok(());
    }

    let extracted = tmp.path().join(bg_bin);

    let output = std::process::Command::new(&extracted)
        .args(["plan"])
        .output();

    if let Ok(out) = output {
        if !out.status.success() {
            if !json_output {
                let heal_script = if cfg!(target_os = "windows") {
                    ".\\scripts\\install.ps1 -Apply"
                } else {
                    "./scripts/install.sh --apply"
                };
                eprintln!("\n{}", "=".repeat(60));
                eprintln!("WARNING: ENVIRONMENTAL DRIFT DETECTED");
                eprintln!("{}", "=".repeat(60));
                eprintln!("The newly installed version of Vox requires system dependencies");
                eprintln!("that are either missing or outdated on your machine.");
                eprintln!("\nPlease run the following command to self-heal your environment:");
                eprintln!("    {}", heal_script);
                eprintln!("{}\n", "=".repeat(60));
            }
        }
    }

    Ok(())
}

fn find_bootstrap_asset(checksum_txt: &str, target_triple: &str, ext: &str) -> Option<String> {
    for line in checksum_txt.lines() {
        let mut parts = line.split_whitespace();
        let Some(_hash) = parts.next() else {
            continue;
        };
        let Some(path) = parts.next() else {
            continue;
        };
        let file = path.rsplit('/').next().unwrap_or(path).to_string();
        if !file.contains(target_triple) || !file.ends_with(ext) {
            continue;
        }
        if file.starts_with("vox-bootstrap-") {
            return Some(file);
        }
    }
    None
}

fn emit_up_to_date(
    json_output: bool,
    current: &str,
    triple: &str,
    source: serde_json::Value,
) -> Result<()> {
    if json_output {
        println!(
            "{}",
            serde_json::json!({
                "toolchain_upgrade": {
                    "status": "up_to_date",
                    "current_version": current,
                    "target_triple": triple,
                    "source": source,
                    "manifest_graph_touched": false,
                    "hint": "No newer eligible release for this channel/policy, or use `--provider http` with `--version` for static mirrors.",
                }
            })
        );
    } else {
        println!("Toolchain is up to date (v{current}, target {triple}).");
        println!("Use `--apply` after reviewing release notes to install an upgrade.");
    }
    Ok(())
}

fn emit_check_only(
    json_output: bool,
    current: &str,
    c: &Candidate,
    triple: &str,
    source: serde_json::Value,
) -> Result<()> {
    if json_output {
        println!(
            "{}",
            serde_json::json!({
                "toolchain_upgrade": {
                    "status": "update_available",
                    "current_version": current,
                    "candidate_version": c.release.version,
                    "asset": c.asset.name,
                    "target_triple": triple,
                    "source": source,
                    "manifest_graph_touched": false,
                    "hint": "Re-run with `--apply` to download (checksum-verified) and install into CARGO_HOME/bin.",
                }
            })
        );
    } else {
        println!(
            "Update available: v{} → v{} ({}) — {}",
            current, c.release.version, triple, c.asset.name
        );
        println!("Run: vox upgrade --apply");
    }
    Ok(())
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn stable_channel_skips_prerelease() {
        let rel = Release {
            name: "x".into(),
            version: "0.2.0-beta.1".into(),
            date: String::new(),
            body: None,
            assets: vec![ReleaseAsset {
                name: format!("vox-v0.2.0-beta.1-{}-fake.tar.gz", get_target()),
                download_url: "http://x".into(),
            }],
        };
        assert!(
            release_eligible(
                &rel,
                get_target(),
                "0.1.0",
                &Version::new(0, 1, 0),
                false,
                true
            )
            .is_none()
        );
        assert!(
            release_eligible(
                &rel,
                get_target(),
                "0.1.0",
                &Version::new(0, 1, 0),
                true,
                true
            )
            .is_some()
        );
    }

    #[test]
    fn breaking_major_skipped_without_flag() {
        let rel = Release {
            name: "x".into(),
            version: "1.0.0".into(),
            date: String::new(),
            body: None,
            assets: vec![ReleaseAsset {
                name: format!("vox-v1.0.0-{}-fake.tar.gz", get_target()),
                download_url: "http://x".into(),
            }],
        };
        assert!(
            release_eligible(
                &rel,
                get_target(),
                "0.9.0",
                &Version::new(0, 9, 0),
                false,
                false
            )
            .is_none()
        );
        assert!(
            release_eligible(
                &rel,
                get_target(),
                "0.9.0",
                &Version::new(0, 9, 0),
                false,
                true
            )
            .is_some()
        );
    }

    #[test]
    fn sidecar_asset_discovery_matches_prefix_and_target() {
        let txt = "abc123  openclaw-gateway-v1.2.3-x86_64-unknown-linux-gnu.tar.gz\n";
        let found = find_sidecar_asset(txt, "x86_64-unknown-linux-gnu", ".tar.gz");
        assert_eq!(
            found.as_deref(),
            Some("openclaw-gateway-v1.2.3-x86_64-unknown-linux-gnu.tar.gz")
        );
    }
}
