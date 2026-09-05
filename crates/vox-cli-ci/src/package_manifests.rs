//! Generate every package manifest from one release, so no channel re-derives
//! the version by hand.
//!
//! # Why this exists
//!
//! Each distribution channel used to carry its own copy of the version:
//!
//! | Channel | Version source before |
//! |---|---|
//! | binary | `[workspace.package] version` + commit count |
//! | Homebrew | `version`, two `url`s and two `sha256`s, hand-pinned |
//! | winget | no manifest at all |
//! | `.deb` / `.msi` | `cargo-deb` / `cargo-wix`, read from `Cargo.toml` |
//!
//! The `.deb` and `.msi` are already correct — they read the manifest. Homebrew
//! was the outlier: pinned to one release, stale the moment another shipped, and
//! nothing detected it. A tap serving a stale binary fails checksum verification
//! on the user's machine, which is the worst place to find out.
//!
//! Everything here derives from a release's `checksums.txt` — the artifact the
//! release workflow already publishes — so the generated manifests cannot
//! disagree with what actually shipped.

use std::collections::BTreeMap;

/// One published asset: its filename and the SHA-256 the release recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub filename: String,
    pub sha256: String,
}

/// The per-triple assets a release publishes for the `vox` CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAssets {
    /// The git tag, e.g. `v0.6.0` or `v0.6.0-rc.4748`.
    pub tag: String,
    pub macos_arm: Asset,
    pub macos_x64: Asset,
    pub linux_x64: Asset,
}

impl ReleaseAssets {
    /// The bare semver, with the leading `v` stripped.
    pub fn version(&self) -> &str {
        self.tag.strip_prefix('v').unwrap_or(&self.tag)
    }

    /// The semver core, dropping any `-rc.N` / prerelease suffix.
    ///
    /// Homebrew's `test do` block asserts on `vox --version` output, which prints
    /// the Cargo package version — that never carries the tag's prerelease
    /// suffix, so comparing against the full tag would fail on every RC.
    pub fn version_core(&self) -> &str {
        let v = self.version();
        v.split('-').next().unwrap_or(v)
    }
}

/// Parse a `checksums.txt` into `filename -> sha256`.
///
/// Format is `sha256sum`'s: `<64 hex>  <basename>` (two spaces). Lines that do
/// not match are ignored rather than erroring, so a future header or a blank
/// line cannot break a release.
pub fn parse_checksums(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    // `str::lines()` strips a trailing \r, so CRLF input needs no special case.
    for line in text.lines() {
        // Split on the FIRST separator and take the rest verbatim. Using
        // `split_whitespace().next()` for the name silently truncated any
        // filename containing a space to its first token — the asset then
        // appeared "missing" while a bogus key sat in the map. sha256sum writes
        // two spaces (or " *" in binary mode); accept a single space too.
        let Some((sum, rest)) = line
            .split_once("  ")
            .or_else(|| line.split_once(" *"))
            .or_else(|| line.split_once(' '))
        else {
            continue;
        };
        let sum = sum.trim();
        let name = rest.trim();
        // A sha256 is exactly 64 hex characters. Anything else is not a checksum
        // line — this is what skips headers and stray prose. Case-insensitive:
        // sha256sum emits lowercase, some tools emit uppercase.
        if name.is_empty() || sum.len() != 64 || !sum.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        // A duplicate filename is a broken release (a re-upload, or two assets
        // colliding). Silently keeping the last one would pin an arbitrary
        // digest, so refuse rather than guess.
        if let Some(prev) = out.get(name)
            && prev != &sum.to_ascii_lowercase()
        {
            return Err(format!(
                "checksums.txt lists '{name}' twice with different digests ({prev} and {sum})"
            ));
        }
        out.insert(name.to_string(), sum.to_ascii_lowercase());
    }
    Ok(out)
}

/// Resolve the three CLI assets a tag must publish.
///
/// Returns the *missing* asset names on failure rather than a generic error, so
/// a release that forgot a target says which one.
pub fn resolve_assets(tag: &str, checksums: &str) -> Result<ReleaseAssets, Vec<String>> {
    let map = parse_checksums(checksums).map_err(|e| vec![e])?;
    let want = |triple: &str| format!("vox-{tag}-{triple}.tar.gz");

    let mut missing = Vec::new();
    let mut take = |triple: &str| -> Option<Asset> {
        let filename = want(triple);
        match map.get(&filename) {
            Some(sha256) => Some(Asset {
                filename,
                sha256: sha256.clone(),
            }),
            None => {
                missing.push(filename);
                None
            }
        }
    };

    let macos_arm = take("aarch64-apple-darwin");
    let macos_x64 = take("x86_64-apple-darwin");
    let linux_x64 = take("x86_64-unknown-linux-gnu");

    match (macos_arm, macos_x64, linux_x64) {
        (Some(macos_arm), Some(macos_x64), Some(linux_x64)) => Ok(ReleaseAssets {
            tag: tag.to_string(),
            macos_arm,
            macos_x64,
            linux_x64,
        }),
        _ => Err(missing),
    }
}

const DOWNLOAD_BASE: &str = "https://github.com/vox-foundation/vox/releases/download";

/// Render `Formula/voxlang.rb` for the given release.
///
/// The formula is named `voxlang`, not `vox`: `brew install vox` resolves to an
/// unrelated cask in homebrew-cask (the VOX music player) and installs a media
/// player while reporting success. The installed command is still `vox`.
pub fn render_homebrew_formula(a: &ReleaseAssets) -> String {
    let tag = &a.tag;
    format!(
        r##"# GENERATED by `vox ci package-manifests` — do not hand-edit.
#
# Regenerate from the release's checksums.txt after every release; the version,
# urls and sha256s below are pinned to one release and are wrong for any other.
#
# The formula is `voxlang`, not `vox`: `brew install vox` resolves to an unrelated
# cask in homebrew-cask (the VOX music player) and installs a media player while
# reporting success. The installed command is still `vox`.
#
# Homebrew 6+ refuses to load an untrusted third-party tap, so users need
# `brew trust vox-foundation/vox` before `brew install voxlang`.
#
# Installing this way sidesteps Gatekeeper: brew fetches over curl, and curl does
# not set com.apple.quarantine — LaunchServices applies that for browsers. So an
# ad-hoc (linker) signed binary runs without an Apple Developer Program
# membership, which a browser download of the same tarball would not.
class Voxlang < Formula
  desc "Language toolchain and CLI for the Vox programming language"
  homepage "https://voxlang.org"
  version "{version}"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "{base}/{tag}/{mac_arm_file}"
      sha256 "{mac_arm_sha}"
    end
    on_intel do
      url "{base}/{tag}/{mac_x64_file}"
      sha256 "{mac_x64_sha}"
    end
  end

  on_linux do
    on_intel do
      url "{base}/{tag}/{linux_file}"
      sha256 "{linux_sha}"
    end
  end

  def install
    bin.install "vox"
  end

  test do
    # Assert the semver core only: `vox --version` prints
    # `vox <semver>+build.<n> (<sha>)`, and the build number and hash change every
    # commit. Matching the full tag would also fail on any -rc release.
    assert_match "vox {version_core}", shell_output("#{{bin}}/vox --version")

    # Clap-derived, needs no config, network or API key — a real "the binary
    # works" assertion rather than a version-string echo.
    assert_match "recommended", shell_output("#{{bin}}/vox commands --recommended")
  end
end
"##,
        version = a.version(),
        version_core = a.version_core(),
        base = DOWNLOAD_BASE,
        tag = tag,
        mac_arm_file = a.macos_arm.filename,
        mac_arm_sha = a.macos_arm.sha256,
        mac_x64_file = a.macos_x64.filename,
        mac_x64_sha = a.macos_x64.sha256,
        linux_file = a.linux_x64.filename,
        linux_sha = a.linux_x64.sha256,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped exactly like the real `checksums.txt`, including the sibling assets
    /// (`vox-ml-cli-`, `voxup-`) whose names overlap the ones we want — a prefix
    /// or substring match would pick the wrong file.
    const FIXTURE: &str = "\
91060c1f32ddc1b03b67a41bf824506d8619ab184f6d18a030087d491fa0a456  vox-v0.6.0-rc.4748-aarch64-apple-darwin.tar.gz
da632656969b441b5b37c047366535a948a432468ad82699de5e6ab7202f5659  vox-v0.6.0-rc.4748-x86_64-apple-darwin.tar.gz
9f939b9f5ed0b98663aabdbac50513e309c23b67f82bd14aca8376aeb543fcd8  vox-v0.6.0-rc.4748-x86_64-unknown-linux-gnu.tar.gz
1111111111111111111111111111111111111111111111111111111111111111  vox-ml-cli-v0.6.0-rc.4748-aarch64-apple-darwin.tar.gz
2222222222222222222222222222222222222222222222222222222222222222  voxup-v0.6.0-rc.4748-aarch64-apple-darwin.tar.gz
";

    #[test]
    fn parses_only_real_checksum_lines() {
        let map = parse_checksums(&format!("# a header\n\n{FIXTURE}")).expect("valid");
        assert_eq!(map.len(), 5, "header and blank line must be ignored");
        assert_eq!(
            map.get("vox-v0.6.0-rc.4748-aarch64-apple-darwin.tar.gz").unwrap(),
            "91060c1f32ddc1b03b67a41bf824506d8619ab184f6d18a030087d491fa0a456"
        );
    }

    #[test]
    fn resolves_the_cli_assets_and_not_their_siblings() {
        let a = resolve_assets("v0.6.0-rc.4748", FIXTURE).expect("all three assets present");
        assert_eq!(
            a.macos_arm.sha256,
            "91060c1f32ddc1b03b67a41bf824506d8619ab184f6d18a030087d491fa0a456"
        );
        // The vox-ml-cli / voxup rows share the tag and triple; picking either
        // would ship the wrong binary under the vox name.
        assert_ne!(a.macos_arm.sha256, "1111111111111111111111111111111111111111111111111111111111111111");
        assert_ne!(a.macos_arm.sha256, "2222222222222222222222222222222222222222222222222222222222222222");
    }

    #[test]
    fn missing_assets_are_named_individually() {
        let partial = FIXTURE
            .lines()
            .filter(|l| !l.contains("x86_64-apple-darwin"))
            .collect::<Vec<_>>()
            .join("\n");
        let err = resolve_assets("v0.6.0-rc.4748", &partial).expect_err("must fail");
        assert_eq!(err, vec!["vox-v0.6.0-rc.4748-x86_64-apple-darwin.tar.gz"]);
    }

    #[test]
    fn version_strips_v_and_prerelease_suffix() {
        let a = resolve_assets("v0.6.0-rc.4748", FIXTURE).unwrap();
        assert_eq!(a.version(), "0.6.0-rc.4748");
        // `vox --version` prints the Cargo version, which never carries -rc.N.
        assert_eq!(a.version_core(), "0.6.0");
    }

    #[test]
    fn formula_pins_every_url_and_sha_to_this_release() {
        let a = resolve_assets("v0.6.0-rc.4748", FIXTURE).unwrap();
        let f = render_homebrew_formula(&a);
        assert!(f.contains("class Voxlang < Formula"), "must not be named Vox — that collides with the VOX cask");
        assert!(f.contains(r#"version "0.6.0-rc.4748""#));
        for sha in [&a.macos_arm.sha256, &a.macos_x64.sha256, &a.linux_x64.sha256] {
            assert!(f.contains(sha.as_str()), "formula must pin {sha}");
        }
        // The test block must assert the core version, or every RC formula fails.
        assert!(f.contains(r#"assert_match "vox 0.6.0""#));
        assert!(!f.contains("assert_match \"vox 0.6.0-rc"), "must not assert the prerelease suffix");
    }

    #[test]
    fn formula_is_marked_generated() {
        let a = resolve_assets("v0.6.0-rc.4748", FIXTURE).unwrap();
        assert!(render_homebrew_formula(&a).starts_with("# GENERATED"));
    }
}
