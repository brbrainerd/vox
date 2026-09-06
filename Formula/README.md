# Homebrew identities

Two identities, because they install to different places:

| Identity | Kind | Token | Destination |
|---|---|---|---|
| CLI | **formula** | `voxlang` | Cellar (`$(brew --prefix)/bin/vox`) |
| Axis (GUI) | **cask** | `axis` | `/Applications` |

A formula **cannot** put an app in `/Applications`. Axis therefore cannot share
the `voxlang` formula regardless of anything else.

**The formula is `voxlang`, not `vox`.** `brew install vox` resolves to an
unrelated cask in homebrew-cask — the VOX music player — and brew reports
success while installing a media player instead of the toolchain. Verified
2026-09-04. The installed command is still `vox`.

## Publication status (settled by the code)

**Nothing publishes this formula or any cask.**
`.github/workflows/release-installers.yml` still runs
`echo "Simulating Homebrew Tap update..."` — there is no `repository_dispatch`,
no push to `vox-foundation/homebrew-vox`, and no PR against any tap.

`Formula/voxlang.rb` in this repo is the source of truth for the CLI formula.
It is not automatically copied anywhere. `curl https://voxlang.org/voxup | sh`
is the supported macOS path until a tap dispatch exists (that dispatch is P4's
file; this plan will not implement it).

Do not tell users to `brew install voxlang` or `brew install --cask axis` as if
those commands were live. They are not.

## Why a formula at all (when it *is* published)

`brew` fetches over curl, and curl does not set `com.apple.quarantine` —
LaunchServices applies that on behalf of browsers. A tarball downloaded from the
GitHub Releases *page* in a browser **is** quarantined, and because the macOS
binaries are only ad-hoc (linker) signed rather than notarized, the OS kills them
with no useful message. Installing through Homebrew avoids that path entirely and
needs no Apple Developer Program membership.

**Homebrew 6 requires `brew trust vox-foundation/vox`** before it will load an
unqualified third-party formula. The fully-qualified form
`brew install vox-foundation/vox/voxlang` records trust itself. That is a
property of taps in general, not a judgement on this one, and it will remain
true even after a tap exists: homebrew-core does not accept binary-only
formulae, and this formula installs a prebuilt binary rather than building from
source.

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
