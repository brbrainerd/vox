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

## Task 1: Uninstall (D1, D2) — **the most dangerous task in the seven plans**

Two unrecoverable, silent losses are one naive implementation away. Read this
before writing any code.

**`~/.zshrc`.** On a pristine macOS account voxup *creates* it (`shell.rs:26-57`,
via `fs::write`, truncating) containing only its own PATH line. Weeks later it is
the user's shell rc, holding their aliases and init. "We created it, so delete it"
destroys that — not in Trash, not in git. "Delete from the marker to EOF" is also
wrong: the snippet is appended (`shell.rs:165`), so on a repeat install the marker
may not be last.

**`~/.vox/.vox-master-key`** — 32 bytes, the vault's fallback decryption key,
sitting inside the directory an uninstaller is aimed at. `login.toml` and the model
catalog are beside it.

- [x] Add `voxup uninstall` operating on an **explicit allowlist**: `~/.vox/bin`,
      `~/.vox/toolchains`, `~/.vox/run`. **Never `~/.vox` itself. Never
      `remove_dir_all` on any path outside the list.**
- [x] **Refuse to run if `HOME` is unset or empty.** `paths.rs:141` falls back to
      `PathBuf::from(".")`, so an uninstall with no HOME targets `./.vox` relative to
      whatever CWD the agent happens to be in.
- [x] Profile edits: remove **only** the exact contiguous two-line block
      (`# Added by voxup` + its one PATH line), leaving every other byte identical.
      Write to a temp file and `fs::rename`; never `fs::write` in place. Back up to
      `<profile>.voxup-backup-<ISO8601>` first and print the path.
- [x] If the marker is absent but `~/.vox/bin` appears in the profile, **do not edit** —
      print the file and line and tell the user to remove it by hand. `try_append`'s
      idempotency check is a substring test, so a user's own hand-written PATH line
      never got a marker.
- [x] `--dry-run` prints the exact diff, and is the **default when stdin is not a TTY**.
- [x] **Do not touch `~/.cargo/bin/vox` unless provenance is provable** — verify it is a
      hardlink to `~/.vox/bin/vox` (same inode, `st_nlink > 1`). That directory
      currently holds `vox`, `vox-compilerd`, `vox-ml-cli`, `vox-orchestrator-d`
      installed by `cargo install`, not by voxup. Never glob `~/.cargo/bin/vox*`.
- [x] Toolchain pruning must resolve the active version by reading
      `~/.vox/toolchains/active` and **fail closed** (delete nothing) if that file is
      missing or unparseable.
- [x] Prune old toolchains on install — keep the active one plus N.
- [x] Test on a temp `HOME`. **The assertion is not "the filesystem is clean"** — that
      phrasing would drive an implementation toward `remove_dir_all`. Assert instead:
      the allowlisted paths are gone, **and** `.vox-master-key`, `login.toml`, `cache/`
      and every pre-seeded user file are byte-identical, **and** `~/.vox` still exists.
- [x] Guard the test itself: fail immediately if the resolved home equals the outer
      process's `$HOME` or is not under the temp dir. On macOS `dirs` falls back to
      `getpwuid` when `HOME` is empty, so a botched setup silently operates on the
      real home directory.
- [x] Profile test fixture: the block **in the middle** of a file with user content on
      both sides; compare full file bytes before and after.

## Task 2: Optional PATH (D3)
- [x] Add `--no-modify-path`. Every packaging system and CI image expects it.
- [x] Disclose the `~/.cargo/bin/vox` hardlink in the install output, or stop writing it.

## Task 3: Bundle configuration (D4, D5, D6)
- [x] **Verify** whether `tauri build` honours `bundle.active` before changing it. Do not assume in either direction.
- [x] Set `category`, `shortDescription`, `publisher`; add `fileAssociations` for `.vox`.
- [ ] Assert on the produced bundle, not the config: build one and inspect the generated `.desktop`/`Info.plist`. **RESUME:** contract test asserts the fragments Tauri would emit; a full `tauri build` of `vox-gui` still needs the sidecar (`cargo build -p vox-cli --release` + copy) and was not run in this isolated clone.

## Task 4: Two Homebrew identities
- [x] `/Applications` requires a **cask**, not a formula — a formula installs to the Cellar. Axis and the CLI must be two identities regardless of anything else.
- [x] Keep `voxlang` as the formula token; `vox` collides with the VOX music-player cask.
- [x] Resolve the **three-way** contradiction about tap publication by **fixing the
      documentation to match the code** in owned files (`Formula/README.md`,
      `Formula/TAP_README.md`). `installation.md` is unowned — request filed rather
      than editing it. Tap dispatch left as the P4 no-op.

## Task 5: MSI identity
- [x] `wix/main.wxs:63,65` has `Product Name = vox-cli` and `Manufacturer = Bert Brainerd`. Both must become the product identity.
- [x] MSI compares only the **first three** version fields; a build number in the fourth makes every build look like the same version for upgrade detection.
- [x] `ADDLOCAL` sets `Preselected=1`, which skips the whole Condition table — so any auto-detection is a convenience for interactive installs only, and the explicit path must be self-sufficient.

## Task 6: Honour install tiers (P1 request)
- [x] `install.rs` reads `tier` only to log it. Either drive the install from it using `resolve_bundle` from P1, or delete the tier concept.
- [x] `distribution_parity.rs` must assert **installer behaviour**, not YAML self-consistency.

## Verification
- [x] Install → uninstall → assert the filesystem is clean, on a temp `HOME`.
- [x] `cargo test -p voxup --all-targets` with real counts.
- [x] Never execute a downloaded binary; never set `com.apple.quarantine`.

## Cross-plan inbox

Rows filed here by other plans. Each must be executable with no conversation.

| From | Request |
|---|---|
| _(none yet)_ | |

## Cross-plan requests

| To | Request |
|---|---|
| P1 | `bundle_resolved()` for Task 6 — extend the existing function, do not add a third spelling |
| P4 | Remove or implement `release-installers.yml:138`'s tap no-op (P4 owns workflows) |
| P4 | Linux release signing + `checksums.txt.asc`; macOS/Windows are signed, Linux is not |
| — | `docs/src/reference/installation.md` is unowned; Task 4's doc fix needs an owner. The code-settled fact to write: Homebrew tap is **Not published**; do not tell users to `brew install`. Formula/README.md in this PR already matches the code. |

## Added by critique

- [ ] **`vox-gui` cannot be built in a fresh worktree** until the sidecar exists.
      Run `cargo build -p vox-cli --release` and copy `target/release/vox` to the
      triple-suffixed sidecar path first. A `tauri-build` "resource path doesn't exist"
      error is **this**, not your config change. **RESUME:** sidecar + `tauri build` not run here.
- [x] `vox upgrade --rollback` — Task 1 covers uninstall but not rollback. (`voxup rollback`)
- [x] GUI auto-update: `tauri.conf.json` has no `updater` block. (`createUpdaterArtifacts: false`; no live updater endpoints.)
