# Homebrew formula

`voxlang.rb` is the canonical formula for the Vox CLI. It is published to
`vox-foundation/homebrew-vox` (the `vox-foundation/vox` tap); this copy is the
source of truth.

**The formula is `voxlang`, not `vox`.** `brew install vox` resolves to an
unrelated cask in homebrew-cask — the VOX music player — and brew reports
success while installing a media player instead of the toolchain. Verified
2026-09-04. The installed command is still `vox`.

**Homebrew 6 requires `brew trust vox-foundation/vox`** before it will load any
third-party tap; `brew install` fails outright without it. That is a property of
taps in general, not a judgement on this one.

**This step is permanent for this tap, and that is now a settled fact.** An
earlier note here claimed it would disappear once `voxlang` reached
homebrew-core. That was wrong: homebrew-core does not accept binary-only
formulae, and this formula installs a prebuilt binary rather than building from
source. There is no core submission to wait for, so `brew trust` is simply part
of the install and the instruction must stay.

(Corrected 2026-09-04 after auditing Homebrew's own formula_auditor: the stricter
`--new` rules that a core submission faces are gated behind `@core_tap` and never
fire in a third-party tap, and core's binary-formula policy is separate from the
audit anyway.)

## Why a formula at all

`brew` fetches over curl, and curl does not set `com.apple.quarantine` —
LaunchServices applies that on behalf of browsers. A tarball downloaded from the
GitHub Releases *page* in a browser **is** quarantined, and because the macOS
binaries are only ad-hoc (linker) signed rather than notarized, the OS kills them
with no useful message. Installing through Homebrew avoids that path entirely and
needs no Apple Developer Program membership.

## Verified locally (2026-09-04, macOS 26.5 aarch64)

Against the existing `v0.6.0-rc.4748` assets, via a throwaway local tap:

| Step | Result |
|---|---|
| `brew style` | clean |
| `brew audit --strict` | exit 0 |
| `brew install` | exit 0 |
| `brew test` | exit 0 — `--version` and `commands --recommended` both assert |
| `xattr` on the installed binary | `com.apple.provenance` only — **no quarantine** |
| `codesign -dv` | `adhoc, linker-signed`, and it runs |

## Publishing

The tap exists and is published. To update it after a release:

1. Copy `voxlang.rb` into its `Formula/` directory.
2. Replace the `echo "Simulating Homebrew Tap update..."` placeholder in
   `.github/workflows/release-installers.yml` with a `repository_dispatch` to it.
3. Have that dispatch rewrite `version`, both `url`s and both `sha256`s from the
   release's `checksums.txt` — the values here are pinned to one release and go
   stale the moment another ships.

Until then `curl https://voxlang.org/voxup | sh` is the supported macOS path, and
it is unaffected by quarantine for the same reason.
