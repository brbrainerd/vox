# Homebrew formula for the Vox CLI.
#
# WHY A BINARY FORMULA, NOT A SOURCE BUILD
# ----------------------------------------
# Building from source means compiling a 136-crate workspace on the user's
# machine — minutes at best, and it needs the pinned 1.96.0 toolchain. The
# release already publishes per-triple binaries; this serves those.
#
# WHY `voxlang`, NOT `vox`
# ------------------------
# `brew install vox` resolves to an unrelated public cask — the VOX music player
# (homebrew-cask, ~3.7.7). Naming this formula `vox` meant a user following the
# install docs silently got a media player in /Applications instead of the
# toolchain, with brew reporting success. The formula is `voxlang` (matching
# voxlang.org); the installed command is still `vox`.
#
# WHY THIS SIDESTEPS GATEKEEPER
# -----------------------------
# `brew` fetches over curl, and curl does not set `com.apple.quarantine` —
# LaunchServices applies that on behalf of browsers. A tarball downloaded from
# the Releases *page* in a browser IS quarantined, and because the binaries are
# only ad-hoc (linker) signed rather than notarized, macOS kills them silently.
# Installing via this formula avoids that path entirely, with no Apple Developer
# Program membership required.
class Voxlang < Formula
  desc "Language toolchain and CLI for the Vox programming language"
  homepage "https://voxlang.org"
  version "0.6.0-rc.4748"
  license "Apache-2.0"

  # Without this, `brew livecheck` falls back to the Git strategy and scrapes ref
  # names for anything version-shaped — it reported the latest version as "6", a
  # single character, which sorts ABOVE 0.6.0-rc.4748 and makes the formula read
  # as perpetually outdated. `brew bump-formula-pr` would act on that garbage.
  livecheck do
    url :stable
    strategy :github_latest
  end

  on_macos do
    on_arm do
      url "https://github.com/vox-foundation/vox/releases/download/v0.6.0-rc.4748/vox-v0.6.0-rc.4748-aarch64-apple-darwin.tar.gz"
      sha256 "91060c1f32ddc1b03b67a41bf824506d8619ab184f6d18a030087d491fa0a456"
    end
    on_intel do
      url "https://github.com/vox-foundation/vox/releases/download/v0.6.0-rc.4748/vox-v0.6.0-rc.4748-x86_64-apple-darwin.tar.gz"
      sha256 "da632656969b441b5b37c047366535a948a432468ad82699de5e6ab7202f5659"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/vox-foundation/vox/releases/download/v0.6.0-rc.4748/vox-v0.6.0-rc.4748-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "9f939b9f5ed0b98663aabdbac50513e309c23b67f82bd14aca8376aeb543fcd8"
    end
  end

  def install
    bin.install "vox"
  end

  test do
    # `--version` prints `vox <semver>+build.<n> (<sha>)`. Asserting the semver
    # prefix rather than the whole string keeps the test stable across builds,
    # since the build number and hash change every commit.
    assert_match "vox #{version.to_s.split("-").first}", shell_output("#{bin}/vox --version")

    # The catalog is clap-derived and needs no config, network, or API key, so it
    # is a real "the binary works" assertion rather than a version-string echo.
    assert_match "recommended", shell_output("#{bin}/vox commands --recommended")
  end
end
