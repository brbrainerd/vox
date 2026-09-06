# Homebrew tap for Vox (not published)

This file is the tap README **if** a tap is ever stood up. Today nothing
publishes it: the release job's tap step is
`echo "Simulating Homebrew Tap update..."` and there is no dispatch to
`vox-foundation/homebrew-vox`.

When that changes (P4 owns the workflow), the two identities are:

```bash
# CLI → Cellar. Token is voxlang: `brew install vox` is the VOX music player.
brew install vox-foundation/vox/voxlang

# Axis → /Applications. A formula cannot do this; it must be a cask.
brew install --cask vox-foundation/vox/axis
```

**Use the fully-qualified name.** Two reasons, both verified 2026-09-04 on
macOS 26.5 with Homebrew 6.0.21:

1. `brew install vox` installs **the wrong software**. `vox` resolves to a cask
   in homebrew-cask — the VOX music player — and brew reports success while
   putting `VOX.app` in `/Applications`. The formula here is named `voxlang`
   precisely to avoid that collision; the installed command is still `vox`.
2. The fully-qualified form needs **no `brew trust` step**. Homebrew 6 refuses
   to load a formula from an untrusted third-party tap, but naming the tap or
   the fully-qualified formula on the command line is an accepted grant.

Until the tap actually publishes, the supported macOS path is:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh
```

## Why this formula is not in homebrew-core

homebrew-core requires a formula to *build from source* or install portable,
platform-independent output (`Acceptable-Formulae.md`, "Requirements"). This
formula ships prebuilt per-triple binaries, so it is ineligible as written.
