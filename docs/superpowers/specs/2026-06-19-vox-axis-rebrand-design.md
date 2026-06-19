# Vox Axis / "Axis" Rebrand — Design Spec (2026-06-19)

**Status:** Approved (brainstorming, 2026-06-19). Brand-layer only.
**Owner surface:** the Vox GUI (Tauri app, crate `vox-gui`).

## 1. Brand model (locked decisions)

| Element | Value | Where it appears |
|---|---|---|
| **Full brand** | **Vox Axis** | About/splash, footer, package/marketing prose, docs |
| **Short name** | **Axis** | Window title bar, in-app brand mark, icon wordmark |
| **Primary launch** | `vox axis` | clap subcommand alias of the existing `vox gui` |
| **Convenience launch** | `axis` (optional shim) | a generated launcher next to the `vox` binary |
| **Conceptual framing** | a **plugin** — "Axis is the Vox GUI" | n/a (positioning only) |

### 1.1 Spelling convention (anti-drift rule)
- **Display / human-facing prose:** two words — **"Vox Axis"** (full) or **"Axis"** (short).
- **Identifiers / commands / filenames / env vars:** single token — **`axis`** / **`VoxAxis`**.
- When referencing the program in **comments and docs**, write it as **"Vox Axis (Axis)"** on first mention in a file, then "Axis" — this is the canonical phrasing that keeps future LLM edits from re-forking the name.

## 2. Scope — what changes and what does NOT

**Changes (identity layer only):**
1. GUI window title → `"Axis"` (`tauri.conf.json`).
2. In-app sidebar brand mark `V` / `VOX` → `A` / `AXIS`.
3. Footer/about brand line → spells out **"Vox Axis"**.
4. New `vox axis` subcommand (clap `visible_alias` of `Gui`).
5. App icon set regenerated with the Axis mark.
6. Comments / help-text / a short docs note adopt the "Vox Axis (Axis)" phrasing.
7. (Optional) `axis` launcher shim generated at install time.

**Explicitly NOT changed (split-brain guards):**
- `productName: "Vox"` and `identifier: "org.vox-foundation.gui"` in `tauri.conf.json` — these drive the **installed binary/bundle name and OS app identity**; changing them ripples through installers, CI artifacts, and update channels. Leave them.
- The `vox-gui` crate name, the `vox` binary, the `vox-gui.exe` binary name, and every Rust/TS code identifier. No import or API churn.
- The gamification "Imperator" rank titles (`crates/vox-gamify/src/profile.rs`) — unrelated to product branding; leave them.

## 3. Touchpoint map (verified against the tree, 2026-06-19)

| # | Surface | File:line | Change |
|---|---|---|---|
| 1 | Window title | `crates/vox-gui/tauri.conf.json:14` | `"title": "Vox"` → `"Axis"` (keep `productName`/`identifier`) |
| 2 | Brand glyph | `crates/vox-gui/ui/src/components/layout/Sidebar.tsx:175` | `>V<` → `>A<` |
| 3 | Brand wordmark | `crates/vox-gui/ui/src/components/layout/Sidebar.tsx:178` | `>VOX<` → `>AXIS<` |
| 4 | Footer brand line | `crates/vox-gui/ui/src/components/layout/Sidebar.tsx:310` | prepend `Vox Axis · ` to the build line |
| 5 | `vox axis` subcommand | `crates/vox-cli/src/lib.rs:431-435` | add `#[command(visible_alias = "axis")]` to the `Gui` variant |
| 6 | Launch log + help | `crates/vox-cli/src/commands/gui.rs:7`, `lib.rs:429-430` | brand-phrasing the doc comment + tracing line |
| 7 | Icon set | `crates/vox-gui/icons/*` | regenerate from a 1024px Axis source |
| 8 | Docs note | `docs/src/...` (new, small) | one short "Axis is the Vox GUI" reference |

## 4. Asset generation — decision

**Generated in Claude Code BEFORE the Antigravity handoff** (not by Gemini Flash).
Rationale: Gemini 3.5 Flash cannot reliably author binary image assets; image/asset
work is a known hallucination/failure surface for it. The icon set is produced once,
committed, and the Flash-executed plan only **references and verifies** the committed
files. Toolchain: hand-authored SVG → 1024px PNG → `tauri icon` fan-out (full `.ico` /
`.icns` / `.png` / `Square*Logo` / android / ios set).

**Visual direction (for the asset author):** the mark reads as an **"A" / axis**
motif — a clean geometric "A" doubling as a pair of crossed axes (x/y axis lines),
in the existing GUI palette (brass `#b08d57`-family → zinc, matching the current
`from-brass via-amber-600 to-zinc-900` glyph gradient). Square, legible at 32px.

## 5. Execution model (the 4-step pipeline this spec feeds)

1. **Brainstorm** (done) → this spec.
2. **Write plan** (this session) → `docs/superpowers/plans/2026-06-19-vox-axis-rebrand.md`
   + `…-GEMINI-FLASH-HANDOFF.md` (the outgoing copy-paste block).
3. **Critique & harden** (next session) → tighten the plan for Gemini Flash 3.5 /
   Antigravity, confirm pre-flight gates, finalize the handoff block.
4. **Hand off** → paste the handoff block into Antigravity; Flash executes; on
   completion Flash emits a **handback block** that, pasted back into Claude Code,
   drives the append to `docs/superpowers/antigravity-handoff-ledger.md`.

## 6. Verification surface
- Rust (CLI): `cargo test -p vox-cli`, `cargo clippy -p vox-cli -- -D warnings`.
- GUI Rust: `cargo clippy -p vox-gui --lib -- -D warnings` (never `--all-targets` — Tauri build script).
- GUI config test: a `vox-gui` Rust test parsing `tauri.conf.json` asserting `title == "Axis"`.
- GUI TS: `npx vitest run <path>`, `npx tsc --noEmit`; `// @vitest-environment jsdom` first line of component tests.
- Icon set: presence + non-empty check on regenerated files.

## 7. Out of scope / follow-ups
- Renaming the installed binary/bundle to "Axis" (would need `productName` change + installer work).
- Marketing site / store listings.
- A bespoke splash-screen window (current Tauri config has none; About lives in the sidebar footer).
