# Homebrew tap for Vox

```bash
brew install vox-foundation/vox/voxlang
```

**Use the fully-qualified name.** Two reasons, both verified 2026-09-04 on
macOS 26.5 with Homebrew 6.0.21:

1. `brew install vox` installs **the wrong software**. `vox` resolves to a cask
   in homebrew-cask — the VOX music player — and brew reports success while
   putting `VOX.app` in `/Applications`. The formula here is named `voxlang`
   precisely to avoid that collision; the installed command is still `vox`.
2. The fully-qualified form needs **no `brew trust` step**. Homebrew 6 refuses
   to load a formula from an untrusted third-party tap, but naming the tap or
   the fully-qualified formula on the command line is an accepted grant — the
   first install succeeds and records the trust itself. `brew install voxlang`
   (unqualified) does *not* qualify and fails until you run
   `brew trust vox-foundation/vox`.

Measured, with an empty trust store:

| Command | Result |
|---|---|
| `brew install --dry-run voxlang` | exit 1 — `Refusing to load formula … from untrusted tap` |
| `brew install --dry-run vox-foundation/vox/voxlang` | exit 0 — `Trusted formula vox-foundation/vox/voxlang` |

## Why install this way on macOS

`brew` fetches over curl, and curl does not set `com.apple.quarantine` —
LaunchServices applies that on behalf of browsers. A tarball downloaded from the
GitHub Releases *page* in a browser **is** quarantined, and because the macOS
binaries are ad-hoc (linker) signed rather than notarized, macOS kills them.
Installing through this tap avoids that path entirely.

Two details worth stating precisely, because they are easy to over-read:

- The installed binary is still **Gatekeeper-rejected** (`spctl -a -t exec` →
  `rejected`, with or without quarantine). It runs because the *quarantine
  xattr is absent*, not because Gatekeeper approved it. Gatekeeper is consulted
  on the first launch of quarantined code via LaunchServices; a plain `execve`
  of an unquarantined file never reaches it. The kernel's own requirement — a
  valid signature over the executable pages, which on arm64 is why ld64 emits an
  ad-hoc signature at all — *is* satisfied (`codesign --verify` → `valid on
  disk`, exit 0).
- A quarantined copy fails differently depending on how it is launched.
  Double-clicked or `open`ed, you get the "developer cannot be verified" dialog
  and a Privacy & Security override. Run from a shell or a script, it is killed
  with **no dialog and no diagnostic** — exit 137.

`curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh` is
unaffected for the same reason, and remains the other supported path.

## Why this formula is not in homebrew-core

homebrew-core requires a formula to *build from source* or install portable,
platform-independent output (`Acceptable-Formulae.md`, "Requirements"). This
formula ships prebuilt per-triple binaries, so it is ineligible as written.

That is a **choice, not a bar**: Vox is Apache-2.0, so nothing about its licence
excludes it, and a source-building `voxlang` formula would be eligible in
principle. We do not ship one because building Vox means a 136-crate Cargo
workspace on a pinned Rust 1.96.0 toolchain — a multi-minute build on every
user's machine, and a toolchain requirement homebrew-core would have to carry.

## Maintaining the formula

`Formula/voxlang.rb` is generated from a release's `checksums.txt`. The
`version`, both `url`s and both `sha256`s are pinned to one release and go stale
the moment another ships — they must be rewritten per release, not hand-edited.

The canonical copy lives in the main repo at `Formula/voxlang.rb`
(`vox-foundation/vox`); this tap is a publishing target for it.
