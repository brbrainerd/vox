# P5 — Desktop Integration & Installer UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Steps use `- [ ]` checkboxes.
>
> **Read [`2026-09-05-00-INDEX.md`](2026-09-05-00-INDEX.md) first** for file-ownership rules and global constraints.

**Goal:** an optional, configurable app entry and PATH install on all three OSes, and an uninstall path that actually exists.

**Spec:** [`../specs/2026-09-04-distribution-and-plugin-architecture.md`](../specs/2026-09-04-distribution-and-plugin-architecture.md) §10, §3

**You own:** `crates/vox-gui/tauri.conf.json`, `crates/vox-gui/icons/`, `crates/voxup/src/`, `Formula/`, `crates/vox-cli/wix/`

## Global constraints

See the index. Non-negotiable everywhere: assert on the artifact never the exit code (`cmd > /tmp/x.log 2>&1; echo $?`); `cargo test -p X` needs `--all-targets` or it can report "0 passed" when tests live in a bin target; guards must run on macOS (no `grep -oP`); never execute a downloaded binary or set `com.apple.quarantine`.

## Measured starting state

`tauri.conf.json` is 36 lines and its entire `bundle` block is **two keys** (`icon`, `externalBin`). Absent: `targets`, `active`, `category`, `shortDescription`, `fileAssociations`, and every `macOS`/`linux`/`windows` sub-key.

| # | Finding |
|---|---|
| D1 | **No uninstall exists anywhere** — no `voxup uninstall`, no brew hook, no deb `postrm`. The MSI's PATH feature is the only cleanup in the product. |
| D2 | `~/.vox/toolchains/vox-<version>/` accumulates one full extraction per install, **forever**. |
| D3 | PATH modification is unconditional — no `--no-modify-path` — and writes an undisclosed second copy at `~/.cargo/bin/vox`. |
| D4 | `bundle.active` defaults to **false** and is unset. Whether `tauri build` honours it is UNVERIFIED — verify before assuming. |
| D5 | No `category`/`shortDescription` → uncategorised, description-less Linux launcher entry; no macOS `LSApplicationCategoryType`. |
| D6 | Nothing registers `.vox` with any OS. |
| D7 | The GUI bundle embeds a **full second copy** of the `vox` CLI; installing only Axis leaves no `vox` on PATH, installing both gives two copies that can diverge — a skew the code already anticipates with `VersionMismatch`. |

---

## Task 1: Uninstall (D1, D2)
- [ ] Add `voxup uninstall`: removes binaries, prunes `~/.vox/toolchains`, reverts the `# Added by voxup` profile blocks, and reports what it will not remove (user data).
- [ ] Prune old toolchains on install — keep the active one plus N.
- [ ] Test on a simulated pristine home (`HOME` pointed at a temp dir), asserting the filesystem afterwards.

## Task 2: Optional PATH (D3)
- [ ] Add `--no-modify-path`. Every packaging system and CI image expects it.
- [ ] Disclose the `~/.cargo/bin/vox` hardlink in the install output, or stop writing it.

## Task 3: Bundle configuration (D4, D5, D6)
- [ ] **Verify** whether `tauri build` honours `bundle.active` before changing it. Do not assume in either direction.
- [ ] Set `category`, `shortDescription`, `publisher`; add `fileAssociations` for `.vox`.
- [ ] Assert on the produced bundle, not the config: build one and inspect the generated `.desktop`/`Info.plist`.

## Task 4: Two Homebrew identities
- [ ] `/Applications` requires a **cask**, not a formula — a formula installs to the Cellar. Axis and the CLI must be two identities regardless of anything else.
- [ ] Keep `voxlang` as the formula token; `vox` collides with the VOX music-player cask.
- [ ] Resolve the **three-way** contradiction about tap publication: `installation.md:151` says "Not published", `Formula/README.md` says published *and* contradicts itself, and `release-installers.yml:138` is still `echo "Simulating Homebrew Tap update..."`. The code settles it — nothing publishes.

## Task 5: MSI identity
- [ ] `wix/main.wxs:63,65` has `Product Name = vox-cli` and `Manufacturer = Bert Brainerd`. Both must become the product identity.
- [ ] MSI compares only the **first three** version fields; a build number in the fourth makes every build look like the same version for upgrade detection.
- [ ] `ADDLOCAL` sets `Preselected=1`, which skips the whole Condition table — so any auto-detection is a convenience for interactive installs only, and the explicit path must be self-sufficient.

## Task 6: Honour install tiers (P1 request)
- [ ] `install.rs` reads `tier` only to log it. Either drive the install from it using `resolve_bundle` from P1, or delete the tier concept.
- [ ] `distribution_parity.rs` must assert **installer behaviour**, not YAML self-consistency.

## Verification
- [ ] Install → uninstall → assert the filesystem is clean, on a temp `HOME`.
- [ ] `cargo test -p voxup --all-targets` with real counts.
- [ ] Never execute a downloaded binary; never set `com.apple.quarantine`.
