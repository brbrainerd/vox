# Vox Urbs Visualizer Rebuild Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild `LudusSandbox` into Vox Urbs — a Roman-styled, pan/zoomable, LOD isometric city of the whole workspace fed exclusively by real telemetry (spec: `docs/superpowers/specs/2026-07-02-vox-urbs-visualizer-rebuild-design.md`).

**Architecture:** Pure-math modules (camera, treemap layout, LOD, pathfinding) under `crates/vox-gui/ui/src/components/gamify/urbs/`, a procedural sprite atlas + offscreen-buffer renderer, DOM overlay citizens driven by direct Zustand subscriptions, and two new thin Rust taps (`workspace_town_scan`, `harness_ci_fleet_status`/`vcs_town_status`). `LudusSandbox.tsx` becomes a thin integration shell (name kept — call sites in `GamifyView` and tests stay valid).

**Tech Stack:** React 18 + TypeScript + vitest (run from `crates/vox-gui/ui` with **pnpm**, never npm), Canvas 2D, zustand/vanilla, Tauri commands in Rust (`cargo test -p vox-gui`). No new JS dependencies.

**Verified codebase facts (don't re-derive):**
- `projectIso/unprojectIso/getZIndex` exist in `crates/vox-gui/ui/src/lib/projection.ts` (signature: `projectIso(x, y, z, tileWidth, tileHeight, offsetX, offsetY, heightScale=20)`).
- `useLudusStore` is a **vanilla** zustand store (`crates/vox-gui/ui/src/components/gamify/store.ts`) — React reads via `useStore(useLudusStore, sel)`, imperative via `useLudusStore.getState()`.
- The energy persist-back bug is **already fixed** (`get_ludus_profile_impl` regens + upserts, round-trip test exists in `crates/vox-gui/src/commands/gamify.rs`). Do NOT re-add.
- `HudPanels.tsx` is no longer a null stub — it renders treasury/energy/speed from props; the problem is the **mock props** at the call site.
- Treasury source: `voxTransport.getLlmSpend()` (`get_llm_spend` command) — reuse the `useLlmSpend` hook.
- Orchestrator queue source: `hopper_list` command (registered in `main.rs:161`).
- Open-file action: `invoke('open_locator', { locator: { kind: 'file', value: absPath } })` (`crates/vox-gui/src/commands/search.rs:407`).
- Windows child processes MUST use `quiet_command`/`quiet_tokio_command` from `crates/vox-gui/src/commands/process_util.rs`.
- Commands register in the `generate_handler![...]` list in `crates/vox-gui/src/main.rs` (~line 160-205).
- There is **no MCP server-list command** (only `invoke_mcp_tool`), and `get_orchestrator_status` serializes the closed `GuiOrchestratorStatus` struct (orchestrator.rs:153-172) which can never carry an MCP field — so AQVAE renders **unconditionally unlit** until a dedicated command exists. Do NOT probe the status JSON; that branch is dead by construction. `DueActionDto` has only `action_id` (no file path), so there is **no per-building FSRS glow** — due actions surface on the SENATVS panel only. Both deviations are recorded in the spec.
- `hopper_list` returns inbox **plus assigned (in-flight)** items; each `HopperTaskDto` carries a `state` field — filter on it for an honest queued count. Never present the raw array length as "queued".
- vitest has **no global test environment** (`vitest.config.ts` sets none): every `.tsx` test using render/screen needs a first-line `// @vitest-environment jsdom` pragma, like every existing DOM test in the repo.
- `LudusSandbox` has **two call sites**: `GamifyView.tsx` and `Dashboard.tsx:454` (`<LudusSandbox files={buildingFiles} />`) — both must be updated when the props change.
- `AgentEventKind::BuildStage` exists (crates/vox-orchestrator/src/events.rs:616, stages lex/parse/hir/typecheck/codegen) and arrives on the already-consumed `vox://agent-events` channel — **verify its serde tag** in events.rs before matching on it in TS.
- Workspace root convention in vox-gui commands: `std::env::current_dir()`.
- CI runner rows come from `gh api repos/<slug>/actions/runners` (same source `vox-cli`'s `runner_scale.rs` uses).

**Commit style:** `feat(vox-gui): …` / `test(vox-gui): …`, ending with the Co-Authored-By line per repo convention. The pre-commit hook runs a line-endings check; all new files are UTF-8/LF.

---

## Task order & parallelism

Tasks 1→3→4→5→6→9→10 are the TS spine (sequential). Task 2 (Rust scan) and Task 7 (Rust taps) are file-disjoint from the TS track and from each other — **[PARALLEL-SAFE]** with any TS task. Task 8 needs 5 and 7. Task 9 needs 5. Task 10 needs everything.

---

### Task 1: Camera module + canvas sizing (kills cut-off / centering / no-pan/zoom)

**Files:**
- Create: `crates/vox-gui/ui/src/components/gamify/urbs/camera.ts`
- Test: `crates/vox-gui/ui/src/components/gamify/urbs/camera.test.ts`

- [ ] **Step 1: Write the failing tests**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/camera.test.ts
import { describe, it, expect } from 'vitest';
import {
  worldToScreen, screenToWorld, zoomAt, clampCamera, fitBounds,
  type Camera, type WorldBounds,
} from './camera';

const BOUNDS: WorldBounds = { minX: 0, minY: 0, maxX: 2000, maxY: 1200 };
const VP = { w: 800, h: 500 };

describe('camera math', () => {
  it('screen↔world round-trips at arbitrary zoom', () => {
    const cam: Camera = { x: -137, y: 42, zoom: 1.73 };
    const w = screenToWorld(cam, 400, 250);
    const s = worldToScreen(cam, w.wx, w.wy);
    expect(s.sx).toBeCloseTo(400, 6);
    expect(s.sy).toBeCloseTo(250, 6);
  });

  it('zoomAt keeps the world point under the cursor fixed', () => {
    const cam: Camera = { x: 0, y: 0, zoom: 1 };
    const before = screenToWorld(cam, 600, 100);
    const zoomed = zoomAt(cam, 600, 100, 1.5, 0.2, 4);
    const after = screenToWorld(zoomed, 600, 100);
    expect(after.wx).toBeCloseTo(before.wx, 6);
    expect(after.wy).toBeCloseTo(before.wy, 6);
  });

  it('zoomAt clamps zoom to [min,max]', () => {
    const cam: Camera = { x: 0, y: 0, zoom: 3.9 };
    expect(zoomAt(cam, 0, 0, 2, 0.2, 4).zoom).toBe(4);
    expect(zoomAt({ ...cam, zoom: 0.25 }, 0, 0, 0.1, 0.2, 4).zoom).toBe(0.2);
  });

  it('clampCamera keeps the world covering the viewport', () => {
    // Panned absurdly far right/down: world must still touch the viewport.
    const cam = clampCamera({ x: 99999, y: 99999, zoom: 1 }, BOUNDS, VP.w, VP.h);
    expect(cam.x).toBeLessThanOrEqual(VP.w);
    expect(cam.y).toBeLessThanOrEqual(VP.h);
    const cam2 = clampCamera({ x: -99999, y: -99999, zoom: 1 }, BOUNDS, VP.w, VP.h);
    // Left/top edge: world max corner may not scroll past viewport origin.
    expect(worldToScreen(cam2, BOUNDS.maxX, BOUNDS.maxY).sx).toBeGreaterThanOrEqual(0);
    expect(worldToScreen(cam2, BOUNDS.maxX, BOUNDS.maxY).sy).toBeGreaterThanOrEqual(0);
  });

  it('fitBounds centers the world in the viewport with padding', () => {
    const cam = fitBounds(BOUNDS, VP.w, VP.h, 20);
    const tl = worldToScreen(cam, BOUNDS.minX, BOUNDS.minY);
    const br = worldToScreen(cam, BOUNDS.maxX, BOUNDS.maxY);
    // Fully visible…
    expect(tl.sx).toBeGreaterThanOrEqual(0);
    expect(br.sx).toBeLessThanOrEqual(VP.w);
    // …and horizontally centered (world is wider than tall for this aspect).
    expect(tl.sx + br.sx).toBeCloseTo(VP.w, 0);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run (from `crates/vox-gui/ui`): `pnpm vitest run src/components/gamify/urbs/camera.test.ts`
Expected: FAIL — `Cannot find module './camera'`.

- [ ] **Step 3: Write the implementation**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/camera.ts
/** Screen = world * zoom + offset. All pure; the component owns the state. */
export interface Camera { x: number; y: number; zoom: number }
export interface WorldBounds { minX: number; minY: number; maxX: number; maxY: number }

export function worldToScreen(cam: Camera, wx: number, wy: number): { sx: number; sy: number } {
  return { sx: wx * cam.zoom + cam.x, sy: wy * cam.zoom + cam.y };
}

export function screenToWorld(cam: Camera, sx: number, sy: number): { wx: number; wy: number } {
  return { wx: (sx - cam.x) / cam.zoom, wy: (sy - cam.y) / cam.zoom };
}

/** Zoom by `factor` keeping the world point under screen (sx, sy) stationary. */
export function zoomAt(
  cam: Camera, sx: number, sy: number, factor: number, minZoom: number, maxZoom: number,
): Camera {
  const zoom = Math.min(maxZoom, Math.max(minZoom, cam.zoom * factor));
  const { wx, wy } = screenToWorld(cam, sx, sy);
  return { zoom, x: sx - wx * zoom, y: sy - wy * zoom };
}

/** Keep at least part of the world on screen (never lose it off an edge). */
export function clampCamera(cam: Camera, b: WorldBounds, vw: number, vh: number): Camera {
  const x = Math.min(vw - b.minX * cam.zoom, Math.max(-b.maxX * cam.zoom, cam.x));
  const y = Math.min(vh - b.minY * cam.zoom, Math.max(-b.maxY * cam.zoom, cam.y));
  return { ...cam, x, y };
}

/** Camera that fits (and centers) the whole bounds in the viewport. */
export function fitBounds(b: WorldBounds, vw: number, vh: number, pad: number): Camera {
  const w = b.maxX - b.minX;
  const h = b.maxY - b.minY;
  const zoom = Math.min((vw - 2 * pad) / w, (vh - 2 * pad) / h);
  return {
    zoom,
    x: (vw - w * zoom) / 2 - b.minX * zoom,
    y: (vh - h * zoom) / 2 - b.minY * zoom,
  };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `pnpm vitest run src/components/gamify/urbs/camera.test.ts`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/urbs/camera.ts crates/vox-gui/ui/src/components/gamify/urbs/camera.test.ts
git commit -m "feat(vox-gui): urbs camera module — pure pan/zoom/fit math

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

(The DOM wiring — drag/wheel handlers, ResizeObserver, DPR — lands in Task 10 with the shell rewrite; this task is the math it depends on.)

---

### Task 2: `workspace_town_scan` command (Rust) [PARALLEL-SAFE]

**Files:**
- Create: `crates/vox-gui/src/commands/workspace_town.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs` (add `pub mod workspace_town;`)
- Modify: `crates/vox-gui/src/main.rs` (register in `generate_handler![...]`)

- [ ] **Step 1: Write the failing test** (bottom of the new file — module + test together)

```rust
// crates/vox-gui/src/commands/workspace_town.rs
//! Workspace scan for the Vox Urbs town map: crates → files → line counts.
//! Read-only, cached, gitignore-aware. Feeds the treemap layout (see
//! docs/superpowers/specs/2026-07-02-vox-urbs-visualizer-rebuild-design.md).

use std::path::Path;
use std::sync::Mutex;

const MAX_FILES: usize = 20_000;
/// Rescan at most this often; the town layout is not a file watcher.
const CACHE_TTL_MS: i64 = 60_000;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TownFileDto {
    /// Path relative to the workspace root, forward slashes.
    pub path: String,
    pub lines: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TownCrateDto {
    pub name: String,
    /// Crate root relative to the workspace root, forward slashes.
    pub root: String,
    pub files: Vec<TownFileDto>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TownScanDto {
    pub crates: Vec<TownCrateDto>,
    /// Absolute workspace root, forward slashes — lets the UI build absolute
    /// paths for open_locator without a second command.
    pub root: String,
    pub scanned_at_ms: i64,
    pub truncated: bool,
}

/// Group scanned source files under `crates/<name>/…`; everything else under
/// a synthetic "(workspace)" crate. Pure — unit-testable without IO.
pub(crate) fn group_by_crate(files: Vec<TownFileDto>) -> Vec<TownCrateDto> {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<(String, String), Vec<TownFileDto>> = BTreeMap::new();
    for f in files {
        let key = match f.path.strip_prefix("crates/") {
            Some(rest) => match rest.split('/').next() {
                Some(name) if rest.contains('/') => {
                    (name.to_string(), format!("crates/{name}"))
                }
                _ => ("(workspace)".to_string(), String::new()),
            },
            None => ("(workspace)".to_string(), String::new()),
        };
        map.entry(key).or_default().push(f);
    }
    map.into_iter()
        .map(|((name, root), files)| TownCrateDto { name, root, files })
        .collect()
}

pub(crate) fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rs" | "ts" | "tsx" | "vox" | "toml" | "md")
    )
}

fn scan(root: &Path) -> TownScanDto {
    let mut files = Vec::new();
    let mut truncated = false;
    let walker = ignore::WalkBuilder::new(root).hidden(true).build();
    for entry in walker.flatten() {
        if files.len() >= MAX_FILES {
            truncated = true;
            break;
        }
        let path = entry.path();
        if !path.is_file() || !is_source_file(path) {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else { continue };
        let rel = rel.to_string_lossy().replace('\\', "/");
        // Skip build/vendor dirs the ignore crate may still surface.
        if rel.starts_with("target/") || rel.contains("node_modules/") {
            continue;
        }
        let lines = std::fs::read_to_string(path)
            .map(|c| c.lines().count() as u32)
            .unwrap_or(0);
        files.push(TownFileDto { path: rel, lines });
    }
    TownScanDto {
        crates: group_by_crate(files),
        root: root.to_string_lossy().replace('\\', "/"),
        scanned_at_ms: chrono::Utc::now().timestamp_millis(),
        truncated,
    }
}

static CACHE: Mutex<Option<TownScanDto>> = Mutex::new(None);

#[tauri::command]
pub async fn workspace_town_scan() -> Result<TownScanDto, String> {
    let now = chrono::Utc::now().timestamp_millis();
    if let Some(cached) = CACHE.lock().unwrap().clone() {
        if now - cached.scanned_at_ms < CACHE_TTL_MS {
            return Ok(cached);
        }
    }
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let fresh = tokio::task::spawn_blocking(move || scan(&root))
        .await
        .map_err(|e| e.to_string())?;
    *CACHE.lock().unwrap() = Some(fresh.clone());
    Ok(fresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_crate_files_and_workspace_files() {
        let files = vec![
            TownFileDto { path: "crates/vox-db/src/lib.rs".into(), lines: 100 },
            TownFileDto { path: "crates/vox-db/src/store.rs".into(), lines: 50 },
            TownFileDto { path: "crates/vox-cli/src/main.rs".into(), lines: 10 },
            TownFileDto { path: "docs/src/intro.md".into(), lines: 5 },
        ];
        let crates = group_by_crate(files);
        let names: Vec<&str> = crates.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["(workspace)", "vox-cli", "vox-db"]);
        assert_eq!(crates[2].files.len(), 2);
        assert_eq!(crates[2].root, "crates/vox-db");
    }

    #[test]
    fn scan_walks_a_fixture_tree_and_counts_lines() {
        let dir = tempfile::tempdir().unwrap();
        let crate_src = dir.path().join("crates/mini/src");
        std::fs::create_dir_all(&crate_src).unwrap();
        std::fs::write(crate_src.join("lib.rs"), "a\nb\nc\n").unwrap();
        std::fs::write(dir.path().join("ignored.png"), b"\x89PNG").unwrap();

        let result = scan(dir.path());
        assert!(!result.truncated);
        assert!(!result.root.is_empty());
        let mini = result.crates.iter().find(|c| c.name == "mini").unwrap();
        assert_eq!(mini.files.len(), 1);
        assert_eq!(mini.files[0].lines, 3);
        assert_eq!(mini.files[0].path, "crates/mini/src/lib.rs");
    }
}
```

- [ ] **Step 2: Wire the module and run the test to verify it fails first**

Add `pub mod workspace_town;` to `crates/vox-gui/src/commands/mod.rs` (alphabetical position). Then:

Run: `cargo test -p vox-gui workspace_town`
Expected: compile error if `ignore`/`tempfile`/`chrono` are missing from `crates/vox-gui/Cargo.toml`. Check `Cargo.toml`; `chrono` and `tempfile` (dev-dep) are near-certainly present (used elsewhere in the crate — verify with grep before adding). If `ignore` is absent, add `ignore.workspace = true` (it is already a workspace dependency for the search stack; verify with `grep -n '^ignore' Cargo.toml crates/vox-gui/Cargo.toml`). Re-run until the two tests pass.

- [ ] **Step 3: Register the command**

In `crates/vox-gui/src/main.rs`, inside `generate_handler![...]`, after the `commands::gamify::*` entries add:

```rust
            commands::workspace_town::workspace_town_scan,
```

Run: `cargo check -p vox-gui`
Expected: clean.

- [ ] **Step 4: Clippy + commit**

Run: `cargo clippy -p vox-gui -- -D warnings` (vox-gui is excluded from workspace `--all-targets` clippy; per-crate is the required gate before merge).

```bash
git add crates/vox-gui/src/commands/workspace_town.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/src/main.rs crates/vox-gui/Cargo.toml
git commit -m "feat(vox-gui): workspace_town_scan command — cached gitignore-aware crate/file/line scan

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Treemap layout engine (replaces the spiral)

**Files:**
- Create: `crates/vox-gui/ui/src/components/gamify/urbs/types.ts`
- Create: `crates/vox-gui/ui/src/components/gamify/urbs/layout.ts`
- Test: `crates/vox-gui/ui/src/components/gamify/urbs/layout.test.ts`

- [ ] **Step 1: Write the shared types**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/types.ts
/** Mirrors the Rust TownScanDto (crates/vox-gui/src/commands/workspace_town.rs). */
export interface TownFile { path: string; lines: number }
export interface TownCrate { name: string; root: string; files: TownFile[] }
export interface TownScan {
  crates: TownCrate[];
  /** Absolute workspace root (forward slashes) for open_locator path joins. */
  root: string;
  scanned_at_ms: number;
  truncated: boolean;
}

/** Building tier by line-count quartile within its district. */
export type Tier = 0 | 1 | 2 | 3; // hut | villa | insula | temple

export interface BuildingPlot { path: string; x: number; y: number; tier: Tier }

export interface District {
  name: string;
  /** Tile rect, half-open: [x0, x1) × [y0, y1). */
  x0: number; y0: number; x1: number; y1: number;
  landmark: boolean;
  buildings: BuildingPlot[];
}

export interface TownLayout {
  districts: District[];
  byPath: Record<string, BuildingPlot>;
  /** Interior grid size in tiles (landmarks live outside, see margins). */
  grid: { w: number; h: number };
  /** Row-major w×h road mask (district border tiles). */
  roads: boolean[];
  /** Fixed harness landmark anchors in tile coords (may be outside the grid). */
  landmarks: { castrum: Pt; portus: Pt; aqvae: Pt; gate: Pt };
}
export interface Pt { x: number; y: number }
```

- [ ] **Step 2: Write the failing tests**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/layout.test.ts
import { describe, it, expect } from 'vitest';
import { layoutTown, assignNewFile } from './layout';
import type { TownScan } from './types';

function scanFixture(): TownScan {
  const mk = (name: string, n: number): { name: string; root: string; files: { path: string; lines: number }[] } => ({
    name,
    root: `crates/${name}`,
    files: Array.from({ length: n }, (_, i) => ({
      path: `crates/${name}/src/f${i}.rs`,
      lines: (i + 1) * 40,
    })),
  });
  return {
    crates: [mk('alpha', 12), mk('beta', 5), mk('gamma', 30)],
    root: '/ws',
    scanned_at_ms: 0,
    truncated: false,
  };
}

describe('layoutTown', () => {
  it('is deterministic: same input, identical output', () => {
    const a = layoutTown(scanFixture(), new Set(['gamma']));
    const b = layoutTown(scanFixture(), new Set(['gamma']));
    expect(JSON.stringify(a)).toEqual(JSON.stringify(b));
  });

  it('places every file inside its own district rect, no overlaps', () => {
    const layout = layoutTown(scanFixture(), new Set());
    const seen = new Set<string>();
    for (const d of layout.districts) {
      for (const b of d.buildings) {
        expect(b.x).toBeGreaterThanOrEqual(d.x0);
        expect(b.x).toBeLessThan(d.x1);
        expect(b.y).toBeGreaterThanOrEqual(d.y0);
        expect(b.y).toBeLessThan(d.y1);
        const key = `${b.x},${b.y}`;
        expect(seen.has(key)).toBe(false);
        seen.add(key);
      }
    }
    expect(Object.keys(layout.byPath)).toHaveLength(12 + 5 + 30);
  });

  it('district rects do not overlap each other', () => {
    const { districts } = layoutTown(scanFixture(), new Set());
    for (let i = 0; i < districts.length; i++) {
      for (let j = i + 1; j < districts.length; j++) {
        const a = districts[i]; const b = districts[j];
        const disjoint = a.x1 <= b.x0 || b.x1 <= a.x0 || a.y1 <= b.y0 || b.y1 <= a.y0;
        expect(disjoint).toBe(true);
      }
    }
  });

  it('assigns tiers by line-count quartile within the district', () => {
    const layout = layoutTown(scanFixture(), new Set());
    const gamma = layout.districts.find((d) => d.name === 'gamma')!;
    const tiers = gamma.buildings.map((b) => b.tier);
    expect(Math.min(...tiers)).toBe(0);
    expect(Math.max(...tiers)).toBe(3);
    // Highest line count gets the highest tier.
    const biggest = gamma.buildings.find((b) => b.path.endsWith('f29.rs'))!;
    expect(biggest.tier).toBe(3);
  });

  it('marks god-node crates as landmark districts', () => {
    const layout = layoutTown(scanFixture(), new Set(['gamma']));
    expect(layout.districts.find((d) => d.name === 'gamma')!.landmark).toBe(true);
    expect(layout.districts.find((d) => d.name === 'alpha')!.landmark).toBe(false);
  });

  it('roads cover district border tiles and are within the grid', () => {
    const layout = layoutTown(scanFixture(), new Set());
    const { w, h } = layout.grid;
    expect(layout.roads).toHaveLength(w * h);
    const d = layout.districts[0];
    // The tile just outside the district's left edge is road (or grid edge).
    if (d.x0 > 0) expect(layout.roads[d.y0 * w + (d.x0 - 1)]).toBe(true);
  });

  it('never drops files under skewed crate sizes (sliver regression guard)', () => {
    // One 500-file crate beside ten 1-file crates — the shape that broke
    // recursive treemaps with 0/1/2-tile sliver districts.
    const skewed: TownScan = {
      crates: [
        { name: 'giant', root: 'crates/giant', files: Array.from({ length: 500 }, (_, i) => ({ path: `crates/giant/f${i}.rs`, lines: i })) },
        ...Array.from({ length: 10 }, (_, c) => ({
          name: `tiny${c}`, root: `crates/tiny${c}`,
          files: [{ path: `crates/tiny${c}/lib.rs`, lines: 1 }],
        })),
      ],
      root: '/ws', scanned_at_ms: 0, truncated: false,
    };
    const layout = layoutTown(skewed, new Set());
    expect(Object.keys(layout.byPath)).toHaveLength(510);
    for (const d of layout.districts) {
      expect(d.x1 - d.x0).toBeGreaterThanOrEqual(3);
      expect(d.y1 - d.y0).toBeGreaterThanOrEqual(3);
    }
  });

  it('assignNewFile parks a mid-session file on a free tile in its district', () => {
    const layout = layoutTown(scanFixture(), new Set());
    const plot = assignNewFile(layout, 'crates/beta/src/brand_new.rs');
    expect(plot).not.toBeNull();
    const beta = layout.districts.find((d) => d.name === 'beta')!;
    expect(plot!.x).toBeGreaterThanOrEqual(beta.x0);
    expect(plot!.x).toBeLessThan(beta.x1);
    expect(layout.byPath['crates/beta/src/brand_new.rs']).toEqual(plot);
    // Unknown crate → null (renderer ignores it; honesty over invention).
    expect(assignNewFile(layout, 'elsewhere/x.rs')).toBeNull();
  });
});
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `pnpm vitest run src/components/gamify/urbs/layout.test.ts`
Expected: FAIL — `Cannot find module './layout'`.

- [ ] **Step 4: Write the implementation**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/layout.ts
import type { BuildingPlot, District, Pt, Tier, TownLayout, TownScan } from './types';

/** Deterministic 32-bit string hash (FNV-1a) for stable tie-breaking. */
export function hash32(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

interface Rect { x0: number; y0: number; x1: number; y1: number }

/** Side of the square interior a crate needs at ~50% building density.
 *  interiorSide² ≥ 2·files guarantees the checkerboard pass alone can hold
 *  every file — capacity is a construction invariant, not a hope. */
function interiorSide(fileCount: number): number {
  return Math.max(2, Math.ceil(Math.sqrt(fileCount * 2)));
}

/** Deterministic shelf-packing (same idea as the sprite atlas): each district
 *  is a square sized from its OWN file count (+2 for the road border), packed
 *  into rows of ~square overall width. No recursive splitting → no sliver
 *  rects, no crate can ever lose its interior, and a 1-file crate beside a
 *  3000-file district just gets a small square next to a big one. */
function packDistricts(
  crates: { name: string; files: number }[],
): { rects: Map<string, Rect>; side: number } {
  const sized = crates.map((c) => ({ name: c.name, s: interiorSide(c.files) + 2 }));
  const totalArea = sized.reduce((a, d) => a + d.s * d.s, 0);
  const targetW = Math.ceil(Math.sqrt(totalArea * 1.1));
  // Height-desc order (name-hash tiebreak) keeps rows tight; deterministic.
  const order = [...sized].sort((a, b) => b.s - a.s || hash32(a.name) - hash32(b.name));
  const rects = new Map<string, Rect>();
  let x = 0; let y = 0; let rowH = 0; let maxX = 0;
  for (const d of order) {
    if (x > 0 && x + d.s > Math.max(targetW, d.s)) { x = 0; y += rowH; rowH = 0; }
    rects.set(d.name, { x0: x, y0: y, x1: x + d.s, y1: y + d.s });
    x += d.s;
    rowH = Math.max(rowH, d.s);
    maxX = Math.max(maxX, x);
  }
  return { rects, side: Math.max(maxX, y + rowH) };
}

function tierFor(lines: number, sorted: number[]): Tier {
  const idx = sorted.findIndex((v) => v >= lines);
  const q = (idx < 0 ? sorted.length - 1 : idx) / Math.max(1, sorted.length - 1);
  if (q < 0.25) return 0;
  if (q < 0.5) return 1;
  if (q < 0.75) return 2;
  return 3;
}

export function layoutTown(scan: TownScan, godNodes: Set<string>): TownLayout {
  // Deterministic order for placement: size desc, then name-hash.
  const crates = [...scan.crates].sort(
    (a, b) => b.files.length - a.files.length || hash32(a.name) - hash32(b.name),
  );
  const { rects, side } = packDistricts(
    crates.map((c) => ({ name: c.name, files: c.files.length })),
  );

  const districts: District[] = [];
  const byPath: Record<string, BuildingPlot> = {};
  // Everything that is not a district INTERIOR is road: borders, alleys, and
  // the ragged gaps between packed rows. One connected walkable network by
  // construction — A* never strands a citizen between districts.
  const roads = new Array<boolean>(side * side).fill(true);

  for (const crate of crates) {
    const r = rects.get(crate.name)!;
    for (let y = r.y0 + 1; y < r.y1 - 1; y++) {
      for (let x = r.x0 + 1; x < r.x1 - 1; x++) {
        roads[y * side + x] = false; // interior = buildable, not walkable
      }
    }
    const files = [...crate.files].sort((a, b) => hash32(a.path) - hash32(b.path));
    const sortedLines = crate.files.map((f) => f.lines).sort((a, b) => a - b);
    const buildings: BuildingPlot[] = [];
    let i = 0;
    // Checkerboard first (alleys between buildings), other parity as overflow.
    for (const parity of [0, 1]) {
      for (let y = r.y0 + 1; y < r.y1 - 1 && i < files.length; y++) {
        for (let x = r.x0 + 1; x < r.x1 - 1 && i < files.length; x++) {
          if ((x + y) % 2 !== parity) continue;
          const f = files[i++];
          const plot: BuildingPlot = { path: f.path, x, y, tier: tierFor(f.lines, sortedLines) };
          buildings.push(plot);
          byPath[f.path] = plot;
        }
      }
    }
    // interiorSide² ≥ 2·files makes leftovers impossible; assert the invariant
    // so a future sizing change fails loudly instead of dropping files.
    if (i < files.length) {
      throw new Error(`layoutTown: district ${crate.name} under-fit (${i}/${files.length})`);
    }
    districts.push({
      name: crate.name, x0: r.x0, y0: r.y0, x1: r.x1, y1: r.y1,
      landmark: godNodes.has(crate.name), buildings,
    });
  }

  const landmarks: { castrum: Pt; portus: Pt; aqvae: Pt; gate: Pt } = {
    castrum: { x: side + 4, y: 2 },
    portus: { x: -6, y: Math.floor(side / 2) },
    aqvae: { x: -3, y: -3 },
    gate: { x: Math.floor(side / 2), y: side + 3 },
  };
  return { districts, byPath, grid: { w: side, h: side }, roads, landmarks };
}

/** Mid-session file creation: nearest free interior tile in its crate district. */
export function assignNewFile(layout: TownLayout, path: string): BuildingPlot | null {
  const m = path.match(/^crates\/([^/]+)\//);
  const name = m ? m[1] : '(workspace)';
  const d = layout.districts.find((x) => x.name === name);
  if (!d) return null;
  const occupied = new Set(d.buildings.map((b) => `${b.x},${b.y}`));
  for (let y = d.y0 + 1; y < d.y1 - 1; y++) {
    for (let x = d.x0 + 1; x < d.x1 - 1; x++) {
      if (!occupied.has(`${x},${y}`)) {
        const plot: BuildingPlot = { path, x, y, tier: 0 };
        d.buildings.push(plot);
        layout.byPath[path] = plot;
        return plot;
      }
    }
  }
  return null;
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `pnpm vitest run src/components/gamify/urbs/layout.test.ts`
Expected: 8 passed. If the tier test is flaky on quartile edges, the fixture (40-line steps) is strictly monotonic — a failure means a real bug, not test noise.

- [ ] **Step 6: Perf guard**

Append to `layout.test.ts`:

```typescript
it('lays out 5,000 files in under 50ms', () => {
  const big: TownScan = {
    crates: Array.from({ length: 100 }, (_, c) => ({
      name: `crate${c}`, root: `crates/crate${c}`,
      files: Array.from({ length: 50 }, (_, i) => ({ path: `crates/crate${c}/f${i}.rs`, lines: i })),
    })),
    root: '/ws', scanned_at_ms: 0, truncated: false,
  };
  const t0 = performance.now();
  layoutTown(big, new Set());
  expect(performance.now() - t0).toBeLessThan(50);
});
```

(The spec's 5ms target is for coordinate assignment on warmed caches; 50ms cold in CI-jsdom is the enforceable proxy — layout runs once per scan, not per frame.)

Run: `pnpm vitest run src/components/gamify/urbs/layout.test.ts` → 9 passed.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/urbs/types.ts crates/vox-gui/ui/src/components/gamify/urbs/layout.ts crates/vox-gui/ui/src/components/gamify/urbs/layout.test.ts
git commit -m "feat(vox-gui): urbs shelf-packed town layout — deterministic districts, tiers, connected roads, no-drop guarantee

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Procedural Roman sprite atlas

**Files:**
- Create: `crates/vox-gui/ui/src/components/gamify/urbs/sprites.ts`
- Test: `crates/vox-gui/ui/src/components/gamify/urbs/sprites.test.ts`

jsdom has no real 2D canvas, so tests cover the **pure geometry** (atlas packing, keys, anchors); the draw calls take any `CanvasRenderingContext2D`-shaped object and are exercised with a recording stub.

- [ ] **Step 1: Write the failing tests**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/sprites.test.ts
import { describe, it, expect } from 'vitest';
import { planAtlas, drawSprite, SPRITE_KEYS, type SpriteKey } from './sprites';

function stubCtx() {
  const calls: string[] = [];
  const rec = (name: string) => (..._a: unknown[]) => { calls.push(name); };
  return {
    calls,
    ctx: new Proxy({}, {
      get: (_t, prop: string) => {
        if (prop === 'canvas') return { width: 4096, height: 4096 };
        return rec(prop);
      },
      set: () => true,
    }) as unknown as CanvasRenderingContext2D,
  };
}

describe('sprite atlas', () => {
  it('plans a rect for every sprite key with no overlaps', () => {
    const plan = planAtlas(2);
    const keys = Object.keys(plan.rects) as SpriteKey[];
    expect(keys.sort()).toEqual([...SPRITE_KEYS].sort());
    const rs = keys.map((k) => plan.rects[k]);
    for (let i = 0; i < rs.length; i++) {
      for (let j = i + 1; j < rs.length; j++) {
        const a = rs[i]; const b = rs[j];
        const disjoint = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
        expect(disjoint).toBe(true);
      }
    }
    expect(plan.width).toBeGreaterThan(0);
    expect(plan.height).toBeGreaterThan(0);
  });

  it('anchor lies inside each sprite rect', () => {
    const plan = planAtlas(1);
    for (const k of SPRITE_KEYS) {
      const r = plan.rects[k];
      expect(r.ax).toBeGreaterThanOrEqual(0);
      expect(r.ax).toBeLessThanOrEqual(r.w);
      expect(r.ay).toBeGreaterThanOrEqual(0);
      expect(r.ay).toBeLessThanOrEqual(r.h);
    }
  });

  it('every sprite drawer issues at least one path/fill call', () => {
    const plan = planAtlas(1);
    for (const k of SPRITE_KEYS) {
      const { calls, ctx } = stubCtx();
      drawSprite(ctx, k, plan.rects[k]);
      expect(calls.length, `sprite ${k} drew nothing`).toBeGreaterThan(0);
    }
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm vitest run src/components/gamify/urbs/sprites.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Write the implementation**

The Limes palette (from the design system): basalt `#1c1917`/`#292524`/`#44403c`, stone `#57534e`/`#78716c`/`#a8a29e`, gold `#d6b25e`, terracotta `#b45309`/`#d97706`, fire `#ea580c`/`#fbbf24`, weeds `#4d7c0f`.

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/sprites.ts
/** Procedural Roman sprite atlas. No image assets: every sprite is drawn
 *  parametrically once per (scale, DPR) into an offscreen canvas, then stamped
 *  by the world renderer. Anchor (ax, ay) = the tile's ground-center point. */

export const SPRITE_KEYS = [
  'plate',            // district ground plate tile
  'hut', 'villa', 'insula', 'temple',       // building tiers 0-3
  'landmark',         // god-node crate temple (gold pediment)
  'fire-0', 'fire-1', // error flame animation frames
  'weeds',            // warning overlay
  'scaffold',         // active-agent-task overlay
  'castrum', 'tent', 'tent-busy',           // CI fort + runner tents
  'ship',             // orchestrator queue entry
  'arch', 'arch-dry', // aqueduct spans (MCP up/down)
  'gate', 'caravan',  // git gate + PR caravan
] as const;
export type SpriteKey = (typeof SPRITE_KEYS)[number];

export interface SpriteRect { x: number; y: number; w: number; h: number; ax: number; ay: number }
export interface AtlasPlan { width: number; height: number; rects: Record<SpriteKey, SpriteRect> }

const TILE_W = 64;
const TILE_H = 32;

/** Logical size of each sprite (before scale). Height leaves headroom for walls. */
const SIZES: Record<SpriteKey, { w: number; h: number; ay: number }> = {
  plate: { w: TILE_W, h: TILE_H, ay: TILE_H / 2 },
  hut: { w: TILE_W, h: 52, ay: 42 },
  villa: { w: TILE_W, h: 62, ay: 52 },
  insula: { w: TILE_W, h: 78, ay: 68 },
  temple: { w: TILE_W, h: 92, ay: 82 },
  landmark: { w: TILE_W * 2, h: 120, ay: 104 },
  'fire-0': { w: 28, h: 34, ay: 32 },
  'fire-1': { w: 28, h: 34, ay: 32 },
  weeds: { w: 30, h: 18, ay: 9 },
  scaffold: { w: TILE_W, h: 60, ay: 50 },
  castrum: { w: 120, h: 90, ay: 78 },
  tent: { w: 26, h: 22, ay: 20 },
  'tent-busy': { w: 26, h: 22, ay: 20 },
  ship: { w: 44, h: 40, ay: 36 },
  arch: { w: 40, h: 54, ay: 50 },
  'arch-dry': { w: 40, h: 54, ay: 50 },
  gate: { w: 56, h: 64, ay: 56 },
  caravan: { w: 36, h: 26, ay: 22 },
};

/** Shelf-pack the sprites left-to-right into rows (max row width 1024·scale). */
export function planAtlas(scale: number): AtlasPlan {
  const maxW = 1024 * scale;
  const pad = 2 * scale;
  const rects = {} as Record<SpriteKey, SpriteRect>;
  let x = pad; let y = pad; let rowH = 0; let width = 0;
  for (const k of SPRITE_KEYS) {
    const w = SIZES[k].w * scale;
    const h = SIZES[k].h * scale;
    if (x + w + pad > maxW) { x = pad; y += rowH + pad; rowH = 0; }
    rects[k] = { x, y, w, h, ax: w / 2, ay: SIZES[k].ay * scale };
    x += w + pad;
    rowH = Math.max(rowH, h);
    width = Math.max(width, x);
  }
  return { width: Math.ceil(width), height: Math.ceil(y + rowH + pad), rects };
}

// ── Parametric drawers ──────────────────────────────────────────────────────
// Each draws into rect-local coordinates; s = rect.w / logical width.

type Ctx = CanvasRenderingContext2D;

function diamond(ctx: Ctx, cx: number, cy: number, w: number, h: number, fill: string, stroke?: string) {
  ctx.beginPath();
  ctx.moveTo(cx, cy - h / 2);
  ctx.lineTo(cx + w / 2, cy);
  ctx.lineTo(cx, cy + h / 2);
  ctx.lineTo(cx - w / 2, cy);
  ctx.closePath();
  ctx.fillStyle = fill;
  ctx.fill();
  if (stroke) { ctx.strokeStyle = stroke; ctx.stroke(); }
}

/** An iso block: top diamond + two visible walls, wall height `wh`. */
function block(ctx: Ctx, cx: number, groundY: number, w: number, wh: number,
  top: string, left: string, right: string) {
  const h = (w * TILE_H) / TILE_W;
  ctx.fillStyle = right;
  ctx.beginPath();
  ctx.moveTo(cx, groundY); ctx.lineTo(cx + w / 2, groundY - h / 2);
  ctx.lineTo(cx + w / 2, groundY - h / 2 - wh); ctx.lineTo(cx, groundY - wh);
  ctx.closePath(); ctx.fill();
  ctx.fillStyle = left;
  ctx.beginPath();
  ctx.moveTo(cx, groundY); ctx.lineTo(cx - w / 2, groundY - h / 2);
  ctx.lineTo(cx - w / 2, groundY - h / 2 - wh); ctx.lineTo(cx, groundY - wh);
  ctx.closePath(); ctx.fill();
  diamond(ctx, cx, groundY - h / 2 - wh, w, h, top);
}

export function drawSprite(ctx: Ctx, key: SpriteKey, r: SpriteRect): void {
  ctx.save();
  ctx.translate(r.x, r.y);
  const s = r.w / SIZES[key].w;
  ctx.scale(s, s);
  const g = SIZES[key].ay; // ground line in logical units
  const cx = SIZES[key].w / 2;
  switch (key) {
    case 'plate':
      diamond(ctx, cx, TILE_H / 2, TILE_W - 2, TILE_H - 2, '#151210', '#292524');
      break;
    case 'hut':
      block(ctx, cx, g, 34, 14, '#78716c', '#292524', '#3f3a34');
      break;
    case 'villa':
      block(ctx, cx, g, 42, 20, '#d97706', '#292524', '#3f3a34'); // terracotta roof
      break;
    case 'insula':
      block(ctx, cx, g, 46, 34, '#a8a29e', '#292524', '#44403c');
      block(ctx, cx, g - 34, 30, 12, '#78716c', '#1c1917', '#292524'); // upper storey
      break;
    case 'temple': {
      block(ctx, cx, g, 52, 34, '#a8a29e', '#292524', '#44403c');
      // Columns on the right face.
      ctx.fillStyle = '#78716c';
      for (let i = 0; i < 3; i++) ctx.fillRect(cx + 4 + i * 8, g - 30, 3, 24);
      // Pediment.
      diamond(ctx, cx, g - 42, 56, 24, '#d6b25e');
      break;
    }
    case 'landmark': {
      block(ctx, cx, g, 96, 44, '#a8a29e', '#292524', '#44403c');
      ctx.fillStyle = '#78716c';
      for (let i = 0; i < 5; i++) ctx.fillRect(cx + 6 + i * 9, g - 38, 4, 30);
      diamond(ctx, cx, g - 56, 104, 40, '#d6b25e', '#a8834a');
      break;
    }
    case 'fire-0':
    case 'fire-1': {
      const lean = key === 'fire-1' ? 3 : -3;
      ctx.fillStyle = '#ea580c';
      ctx.beginPath();
      ctx.moveTo(6, g); ctx.quadraticCurveTo(10 + lean, g - 26, 14, g - 32);
      ctx.quadraticCurveTo(18 - lean, g - 22, 22, g);
      ctx.closePath(); ctx.fill();
      ctx.fillStyle = '#fbbf24';
      ctx.beginPath();
      ctx.moveTo(10, g); ctx.quadraticCurveTo(14 + lean / 2, g - 14, 14, g - 18);
      ctx.quadraticCurveTo(16, g - 12, 18, g);
      ctx.closePath(); ctx.fill();
      break;
    }
    case 'weeds':
      ctx.strokeStyle = '#4d7c0f';
      ctx.lineWidth = 2;
      for (const dx of [4, 12, 22]) {
        ctx.beginPath();
        ctx.moveTo(dx, g); ctx.quadraticCurveTo(dx - 2, g - 9, dx + 1, g - 12);
        ctx.stroke();
      }
      break;
    case 'scaffold':
      ctx.strokeStyle = '#b45309';
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(6, g - 44); ctx.lineTo(6, g);
      ctx.moveTo(SIZES.scaffold.w - 6, g - 44); ctx.lineTo(SIZES.scaffold.w - 6, g);
      ctx.moveTo(6, g - 38); ctx.lineTo(SIZES.scaffold.w - 6, g - 8);
      ctx.moveTo(6, g - 8); ctx.lineTo(SIZES.scaffold.w - 6, g - 38);
      ctx.stroke();
      break;
    case 'castrum': {
      ctx.strokeStyle = '#78716c'; ctx.lineWidth = 3;
      ctx.strokeRect(8, g - 56, 104, 56);
      ctx.fillStyle = '#44403c';
      for (const [tx, ty] of [[4, g - 60], [104, g - 60], [4, g - 8], [104, g - 8]]) {
        ctx.fillRect(tx, ty, 12, 12);
      }
      ctx.strokeStyle = '#d6b25e'; ctx.lineWidth = 2;
      ctx.beginPath(); ctx.moveTo(60, g - 56); ctx.lineTo(60, g - 74); ctx.stroke();
      ctx.fillStyle = '#d6b25e'; ctx.fillRect(60, g - 74, 10, 7);
      break;
    }
    case 'tent':
    case 'tent-busy':
      ctx.fillStyle = key === 'tent-busy' ? '#7f1d1d' : '#3f3a34';
      ctx.strokeStyle = key === 'tent-busy' ? '#b91c1c' : '#57534e';
      ctx.beginPath();
      ctx.moveTo(2, g); ctx.lineTo(13, g - 16); ctx.lineTo(24, g);
      ctx.closePath(); ctx.fill(); ctx.stroke();
      break;
    case 'ship':
      ctx.fillStyle = '#78716c';
      ctx.beginPath();
      ctx.moveTo(4, g - 8); ctx.quadraticCurveTo(22, g, 40, g - 8);
      ctx.lineTo(36, g); ctx.lineTo(8, g); ctx.closePath(); ctx.fill();
      ctx.strokeStyle = '#a8a29e'; ctx.lineWidth = 2;
      ctx.beginPath(); ctx.moveTo(22, g - 8); ctx.lineTo(22, g - 30); ctx.stroke();
      ctx.fillStyle = '#d6b25e';
      ctx.beginPath();
      ctx.moveTo(22, g - 30); ctx.lineTo(22, g - 14); ctx.lineTo(34, g - 20);
      ctx.closePath(); ctx.fill();
      break;
    case 'arch':
    case 'arch-dry': {
      ctx.strokeStyle = key === 'arch' ? '#78716c' : '#44403c';
      ctx.lineWidth = 3;
      ctx.beginPath(); ctx.moveTo(6, g); ctx.lineTo(6, g - 34); ctx.stroke();
      ctx.beginPath(); ctx.moveTo(34, g); ctx.lineTo(34, g - 34); ctx.stroke();
      ctx.beginPath(); ctx.arc(20, g - 34, 14, Math.PI, 0); ctx.stroke();
      ctx.beginPath(); ctx.moveTo(2, g - 48); ctx.lineTo(38, g - 48); ctx.stroke();
      if (key === 'arch-dry') {
        ctx.strokeStyle = '#7f1d1d'; ctx.lineWidth = 2;
        ctx.beginPath(); ctx.moveTo(10, g - 44); ctx.lineTo(30, g - 24); ctx.stroke();
      }
      break;
    }
    case 'gate':
      block(ctx, cx, g, 44, 30, '#57534e', '#1c1917', '#292524');
      ctx.fillStyle = '#0c0a09';
      ctx.beginPath(); ctx.arc(cx, g - 12, 9, Math.PI, 0);
      ctx.lineTo(cx + 9, g); ctx.lineTo(cx - 9, g); ctx.closePath(); ctx.fill();
      break;
    case 'caravan':
      ctx.fillStyle = '#b45309';
      ctx.fillRect(4, g - 14, 22, 10);
      ctx.fillStyle = '#292524'; ctx.strokeStyle = '#78716c';
      for (const wx of [9, 21]) {
        ctx.beginPath(); ctx.arc(wx, g - 2, 3, 0, Math.PI * 2); ctx.fill(); ctx.stroke();
      }
      break;
  }
  ctx.restore();
}

/** Build the atlas canvas. Browser-only (needs real 2D canvas). */
export function buildAtlas(scale: number): { canvas: HTMLCanvasElement; plan: AtlasPlan } {
  const plan = planAtlas(scale);
  const canvas = document.createElement('canvas');
  canvas.width = plan.width;
  canvas.height = plan.height;
  const ctx = canvas.getContext('2d')!;
  for (const k of SPRITE_KEYS) drawSprite(ctx, k, plan.rects[k]);
  return { canvas, plan };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `pnpm vitest run src/components/gamify/urbs/sprites.test.ts`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/urbs/sprites.ts crates/vox-gui/ui/src/components/gamify/urbs/sprites.test.ts
git commit -m "feat(vox-gui): urbs procedural Roman sprite atlas — parametric buildings, overlays, harness landmarks

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: LOD bands + offscreen world renderer (full-world buffer, render-scale clamp)

**Files:**
- Create: `crates/vox-gui/ui/src/components/gamify/urbs/lod.ts`
- Create: `crates/vox-gui/ui/src/components/gamify/urbs/worldRenderer.ts`
- Test: `crates/vox-gui/ui/src/components/gamify/urbs/lod.test.ts`
- Test: `crates/vox-gui/ui/src/components/gamify/urbs/worldRenderer.test.ts`

- [ ] **Step 1: Write the failing LOD tests**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/lod.test.ts
import { describe, it, expect } from 'vitest';
import { bandForZoom, worldPx, worldBounds, tileFromWorld, LOD_THRESHOLD } from './lod';
import { layoutTown } from './layout';
import type { TownScan } from './types';

const scan: TownScan = {
  crates: [
    { name: 'a', root: 'crates/a', files: Array.from({ length: 8 }, (_, i) => ({ path: `crates/a/${i}.rs`, lines: i })) },
    { name: 'b', root: 'crates/b', files: Array.from({ length: 8 }, (_, i) => ({ path: `crates/b/${i}.rs`, lines: i })) },
  ],
  root: '/ws', scanned_at_ms: 0, truncated: false,
};

describe('lod', () => {
  it('band 0 (aggregate) below threshold, band 1 (buildings) at/above', () => {
    expect(bandForZoom(LOD_THRESHOLD - 0.01)).toBe(0);
    expect(bandForZoom(LOD_THRESHOLD)).toBe(1);
    expect(bandForZoom(3)).toBe(1);
  });

  it('worldPx projects tile → world pixels with non-negative margin origin', () => {
    const layout = layoutTown(scan, new Set());
    const b = worldBounds(layout);
    expect(b.minX).toBe(0);
    expect(b.minY).toBe(0);
    const p = worldPx(layout, 0, 0);
    expect(p.px).toBeGreaterThan(0);
    expect(p.py).toBeGreaterThan(0);
    const q = worldPx(layout, layout.grid.w - 1, layout.grid.h - 1);
    expect(q.px).toBeLessThan(b.maxX);
    expect(q.py).toBeLessThan(b.maxY);
  });

  it('tileFromWorld inverts worldPx for every grid tile', () => {
    const layout = layoutTown(scan, new Set());
    for (const tile of [{ x: 0, y: 0 }, { x: 3, y: 1 }, { x: layout.grid.w - 1, y: layout.grid.h - 1 }]) {
      const { px, py } = worldPx(layout, tile.x, tile.y);
      expect(tileFromWorld(layout, px, py)).toEqual(tile);
    }
  });

});
```

- [ ] **Step 2: Run to verify failure, then implement**

Run: `pnpm vitest run src/components/gamify/urbs/lod.test.ts` → module not found.

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/lod.ts
import { projectIso, unprojectIso } from '../../../lib/projection';
import type { WorldBounds } from './camera';
import type { Pt, TownLayout } from './types';

export const TILE_W = 64;
export const TILE_H = 32;
/** 8-tile margin keeps out-of-grid landmarks (castrum/portus/gate) in-world. */
export const MARGIN_TILES = 8;
/** Below this zoom districts render as aggregate landmarks. */
export const LOD_THRESHOLD = 0.55;

export function bandForZoom(zoom: number): 0 | 1 {
  return zoom < LOD_THRESHOLD ? 0 : 1;
}

/** Iso-project a tile coordinate into world pixel space (origin ≥ 0). */
export function worldPx(layout: TownLayout, x: number, y: number): { px: number; py: number } {
  const offsetX = (layout.grid.h + MARGIN_TILES * 2) * (TILE_W / 2);
  const offsetY = MARGIN_TILES * TILE_H;
  return projectIso(x + MARGIN_TILES, y + MARGIN_TILES, 0, TILE_W, TILE_H, offsetX, offsetY);
}

/** Inverse of worldPx: world pixel → integer tile coordinate. */
export function tileFromWorld(layout: TownLayout, wx: number, wy: number): Pt {
  const offsetX = (layout.grid.h + MARGIN_TILES * 2) * (TILE_W / 2);
  const offsetY = MARGIN_TILES * TILE_H;
  const t = unprojectIso(wx, wy, TILE_W, TILE_H, offsetX, offsetY);
  return { x: Math.round(t.x) - MARGIN_TILES, y: Math.round(t.y) - MARGIN_TILES };
}

export function worldBounds(layout: TownLayout): WorldBounds {
  const n = layout.grid.w + MARGIN_TILES * 2;
  const m = layout.grid.h + MARGIN_TILES * 2;
  return { minX: 0, minY: 0, maxX: ((n + m) * TILE_W) / 2, maxY: ((n + m) * TILE_H) / 2 + 140 };
}

// NOTE (deliberate, spec §4): there is no per-frame viewport culling. The
// full world is painted once into an offscreen buffer and pan/zoom is a
// camera-transformed blit; the buffer's size is bounded by a render-scale
// clamp in worldRenderer. Culling would only help buffer *repaints*, which
// happen on data/LOD changes, not per frame.
```

Run: `pnpm vitest run src/components/gamify/urbs/lod.test.ts` → 3 passed.

- [ ] **Step 3: Write the failing renderer tests** (redraw-key discipline — the buffer must not depend on the camera)

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/worldRenderer.test.ts
import { describe, it, expect } from 'vitest';
import { redrawKey, type WorldState } from './worldRenderer';
import { layoutTown } from './layout';
import type { TownScan } from './types';

const scan: TownScan = {
  crates: [{ name: 'a', root: 'crates/a', files: [{ path: 'crates/a/x.rs', lines: 10 }] }],
  root: '/ws', scanned_at_ms: 0, truncated: false,
};

function state(over: Partial<WorldState> = {}): WorldState {
  return {
    layout: layoutTown(scan, new Set()),
    buildings: {}, agentTasks: {},
    harness: { ci: null, vcs: null, queueLen: null, mcp: null },
    ...over,
  };
}

describe('redrawKey', () => {
  it('is stable when nothing changed', () => {
    const s = state();
    expect(redrawKey(s, 1)).toEqual(redrawKey(s, 1));
  });
  it('changes when diagnostics change (buffer must repaint)', () => {
    const a = redrawKey(state(), 1);
    const b = redrawKey(state({ buildings: { 'crates/a/x.rs': { x: 0, y: 0, warnings: 1, errors: 0 } } }), 1);
    expect(a).not.toEqual(b);
  });
  it('changes across LOD bands but has NO camera or animation-frame input', () => {
    const s = state();
    expect(redrawKey(s, 0)).not.toEqual(redrawKey(s, 1));
    // No camera and no fire-frame parameter exist at all — a compile-level
    // guarantee that pan/zoom and fire animation never repaint the buffer.
    expect(redrawKey(s, 1)).toEqual(redrawKey(s, 1));
  });
});
```

- [ ] **Step 4: Implement the renderer**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/worldRenderer.ts
/** Offscreen world buffer: repainted only when `redrawKey` changes (data or
 *  LOD band), never on camera moves — the onscreen blit applies the camera. */
import type { BuildingState, AgentTask } from '../store';
import type { TownLayout } from './types';
import { bandForZoom, worldPx, worldBounds, TILE_W, TILE_H } from './lod';
import { buildAtlas, type AtlasPlan, type SpriteKey } from './sprites';

export interface HarnessSnapshot {
  /** null = tap unavailable → landmark renders unlit. */
  ci: { runners: { name: string; busy: boolean }[]; queued: number } | null;
  vcs: { branches: { name: string; track: string; isHead: boolean }[]; prs: { number: number; title: string }[] } | null;
  queueLen: number | null;
  /** Always null until a dedicated MCP server-list command exists (spec §7.1). */
  mcp: { name: string; ok: boolean }[] | null;
}

export interface WorldState {
  layout: TownLayout;
  buildings: Record<string, BuildingState>;
  agentTasks: Record<string, AgentTask>;
  harness: HarnessSnapshot;
}

/** Cheap structural key controlling buffer repaints. Camera and animation
 *  frames are deliberately NOT inputs — pan/zoom and fire flicker must never
 *  trigger a whole-world repaint (fires draw in the blit pass instead). */
export function redrawKey(s: WorldState, band: 0 | 1): string {
  const diag = Object.entries(s.buildings)
    .map(([p, b]) => `${p}:${b.warnings}/${b.errors}`).sort().join('|');
  const tasks = Object.values(s.agentTasks).map((t) => t.filePath).sort().join('|');
  const h = s.harness;
  const hk = [
    h.ci ? h.ci.runners.map((r) => +r.busy).join('') : 'x',
    h.vcs ? `${h.vcs.branches.length}/${h.vcs.prs.length}` : 'x',
    h.queueLen ?? 'x',
    h.mcp ? h.mcp.map((m) => +m.ok).join('') : 'x',
  ].join(',');
  return `${band}#${s.layout.grid.w}#${diag}#${tasks}#${hk}`;
}

/** Cap the buffer's long edge — a 7.5k-file world projects to ~11k×5.6k world
 *  px (~250 MB RGBA); rendering at reduced scale and blitting back up bounds
 *  memory while pan/zoom stays a pure blit. */
const MAX_BUFFER_EDGE = 8192;

interface Buffer { canvas: HTMLCanvasElement; key: string; scale: number }

export class WorldRenderer {
  private buffer: Buffer | null = null;
  private atlas: { canvas: HTMLCanvasElement; plan: AtlasPlan } | null = null;

  /** Repaint the buffer iff the key changed. `scale` is buffer px per world
   *  px — the blit must draw it back at world size (see the shell's blit). */
  ensure(s: WorldState, zoom: number): { canvas: HTMLCanvasElement; scale: number } {
    const band = bandForZoom(zoom);
    const key = redrawKey(s, band);
    if (this.buffer && this.buffer.key === key) {
      return { canvas: this.buffer.canvas, scale: this.buffer.scale };
    }
    if (!this.atlas) this.atlas = buildAtlas(2);
    const b = worldBounds(s.layout);
    const scale = Math.min(1, MAX_BUFFER_EDGE / Math.max(b.maxX, b.maxY));
    const canvas = this.buffer?.canvas ?? document.createElement('canvas');
    canvas.width = Math.ceil(b.maxX * scale);
    canvas.height = Math.ceil(b.maxY * scale);
    const ctx = canvas.getContext('2d');
    if (!ctx) return { canvas, scale }; // jsdom/tests: no 2D context, no paint
    ctx.setTransform(scale, 0, 0, scale, 0, 0);
    this.paint(ctx, s, band);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    this.buffer = { canvas, key, scale };
    return { canvas, scale };
  }

  /** Stamp fire sprites for error buildings/districts. Called from the
   *  shell's blit with the camera transform already applied (world coords),
   *  so flames animate WITHOUT touching the buffer. */
  drawFires(ctx: CanvasRenderingContext2D, s: WorldState, zoom: number, frame: 0 | 1): void {
    if (!this.atlas) this.atlas = buildAtlas(2);
    const key: SpriteKey = frame ? 'fire-1' : 'fire-0';
    const { layout } = s;
    if (bandForZoom(zoom) === 0) {
      for (const d of layout.districts) {
        const errs = d.buildings.reduce((n, b) => n + (s.buildings[b.path]?.errors ?? 0), 0);
        if (errs === 0) continue;
        const { px, py } = worldPx(layout, (d.x0 + d.x1) / 2, (d.y0 + d.y1) / 2);
        this.stamp(ctx, key, px, py - 30);
      }
    } else {
      for (const [path, diag] of Object.entries(s.buildings)) {
        if (!diag.errors) continue;
        const plot = layout.byPath[path];
        if (!plot) continue;
        const { px, py } = worldPx(layout, plot.x, plot.y);
        this.stamp(ctx, key, px, py - 20);
      }
    }
  }

  private stamp(ctx: CanvasRenderingContext2D, key: SpriteKey, px: number, py: number) {
    const { canvas, plan } = this.atlas!;
    const r = plan.rects[key];
    ctx.drawImage(canvas, r.x, r.y, r.w, r.h, px - r.ax / 2, py - r.ay / 2, r.w / 2, r.h / 2);
  }

  private paint(ctx: CanvasRenderingContext2D, s: WorldState, band: 0 | 1) {
    const { layout } = s;
    ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);

    // Roads + district plates (both bands).
    for (const d of layout.districts) {
      for (let y = d.y0; y < d.y1; y++) {
        for (let x = d.x0; x < d.x1; x++) {
          const { px, py } = worldPx(layout, x, y);
          if (layout.roads[y * layout.grid.w + x]) {
            ctx.fillStyle = '#231f1c';
            ctx.beginPath();
            ctx.moveTo(px, py - TILE_H / 2); ctx.lineTo(px + TILE_W / 2, py);
            ctx.lineTo(px, py + TILE_H / 2); ctx.lineTo(px - TILE_W / 2, py);
            ctx.closePath(); ctx.fill();
          } else {
            this.stamp(ctx, 'plate', px, py);
          }
        }
      }
    }

    if (band === 0) {
      // Aggregate: one landmark/temple per district at its center; tint by errors.
      for (const d of layout.districts) {
        const cx = (d.x0 + d.x1) / 2;
        const cy = (d.y0 + d.y1) / 2;
        const { px, py } = worldPx(layout, cx, cy);
        this.stamp(ctx, d.landmark ? 'landmark' : 'temple', px, py);
        // Fires are NOT painted here — drawFires stamps them in the blit pass.
        ctx.fillStyle = '#a8a29e';
        ctx.font = '11px serif';
        ctx.textAlign = 'center';
        ctx.fillText(d.name, px, py + TILE_H);
      }
    } else {
      // Buildings, painter's order (y then x), overlays per diagnostics/tasks.
      const active = new Set(Object.values(s.agentTasks).map((t) => t.filePath));
      const plots = layout.districts.flatMap((d) => d.buildings)
        .sort((a, b) => a.y - b.y || a.x - b.x);
      const tierKey: SpriteKey[] = ['hut', 'villa', 'insula', 'temple'];
      for (const p of plots) {
        const { px, py } = worldPx(layout, p.x, p.y);
        this.stamp(ctx, tierKey[p.tier], px, py);
        const diag = s.buildings[p.path];
        if (diag?.warnings) this.stamp(ctx, 'weeds', px + 14, py + 6);
        // Fires are stamped by drawFires in the blit pass, not baked here.
        if (active.has(p.path)) this.stamp(ctx, 'scaffold', px, py);
      }
      for (const d of layout.districts) {
        const { px, py } = worldPx(layout, (d.x0 + d.x1) / 2, d.y1);
        ctx.fillStyle = '#78716c';
        ctx.font = '10px serif';
        ctx.textAlign = 'center';
        ctx.fillText(d.name, px, py + 4);
      }
    }

    this.paintLandmarks(ctx, s);
  }

  private paintLandmarks(ctx: CanvasRenderingContext2D, s: WorldState) {
    const { layout, harness } = s;
    const L = layout.landmarks;
    const unlit = (px: number, py: number, label: string, reason: string) => {
      ctx.globalAlpha = 0.35;
      ctx.fillStyle = '#57534e';
      ctx.font = '10px serif';
      ctx.textAlign = 'center';
      ctx.fillText(`${label} — ${reason}`, px, py + 16);
      ctx.globalAlpha = 1;
    };

    { // CASTRVM (CI fleet)
      const { px, py } = worldPx(layout, L.castrum.x, L.castrum.y);
      this.stamp(ctx, 'castrum', px, py);
      if (harness.ci) {
        harness.ci.runners.slice(0, 6).forEach((r, i) => {
          this.stamp(ctx, r.busy ? 'tent-busy' : 'tent', px - 18 + (i % 3) * 16, py - 8 + Math.floor(i / 3) * 12);
        });
        ctx.fillStyle = '#a8a29e'; ctx.font = '10px serif'; ctx.textAlign = 'center';
        ctx.fillText(`CASTRVM · ${harness.ci.runners.filter((r) => r.busy).length}/${harness.ci.runners.length} busy`, px, py + 20);
      } else unlit(px, py, 'CASTRVM', 'ci unavailable');
    }
    { // PORTVS (orchestrator queue)
      const { px, py } = worldPx(layout, L.portus.x, L.portus.y);
      if (harness.queueLen !== null) {
        for (let i = 0; i < Math.min(harness.queueLen, 5); i++) this.stamp(ctx, 'ship', px - i * 24, py + i * 8);
        ctx.fillStyle = '#a8a29e'; ctx.font = '10px serif'; ctx.textAlign = 'center';
        ctx.fillText(`PORTVS · ${harness.queueLen} queued`, px, py + 26);
      } else unlit(px, py, 'PORTVS', 'orchestrator unavailable');
    }
    { // AQVAE (MCP)
      const { px, py } = worldPx(layout, L.aqvae.x, L.aqvae.y);
      if (harness.mcp) {
        harness.mcp.slice(0, 6).forEach((m, i) => this.stamp(ctx, m.ok ? 'arch' : 'arch-dry', px + i * 22, py + i * 11));
        ctx.fillStyle = '#a8a29e'; ctx.font = '10px serif';
        ctx.fillText(`AQVAE · ${harness.mcp.filter((m) => m.ok).length}/${harness.mcp.length}`, px, py - 30);
      } else unlit(px, py, 'AQVAE', 'no MCP telemetry');
    }
    { // Gate + git
      const { px, py } = worldPx(layout, L.gate.x, L.gate.y);
      this.stamp(ctx, 'gate', px, py);
      if (harness.vcs) {
        harness.vcs.prs.slice(0, 3).forEach((pr, i) => {
          this.stamp(ctx, 'caravan', px + 30 + i * 30, py + 14 + i * 8);
          ctx.fillStyle = '#d97706'; ctx.font = '9px serif'; ctx.textAlign = 'left';
          ctx.fillText(`#${pr.number}`, px + 24 + i * 30, py + 34 + i * 8);
        });
        harness.vcs.branches.slice(0, 4).forEach((b, i) => {
          ctx.strokeStyle = '#57534e'; ctx.lineWidth = 3; ctx.setLineDash([6, 5]);
          ctx.beginPath(); ctx.moveTo(px, py); ctx.lineTo(px + 60 + i * 12, py + 40 + i * 16); ctx.stroke();
          ctx.setLineDash([]);
          ctx.fillStyle = '#a8a29e'; ctx.font = '9px serif';
          // Real ahead/behind from %(upstream:track), e.g. "[ahead 2]".
          ctx.fillText(b.track ? `${b.name} ${b.track}` : b.name, px + 64 + i * 12, py + 52 + i * 16);
        });
      } else unlit(px, py, 'PORTA', 'git unavailable');
    }
  }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `pnpm vitest run src/components/gamify/urbs/lod.test.ts src/components/gamify/urbs/worldRenderer.test.ts`
Expected: 6 passed (renderer's `WorldRenderer` class is browser-only; only `redrawKey` is unit-tested — the class is exercised by Task 10's component tests and manual verification).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/urbs/lod.ts crates/vox-gui/ui/src/components/gamify/urbs/lod.test.ts crates/vox-gui/ui/src/components/gamify/urbs/worldRenderer.ts crates/vox-gui/ui/src/components/gamify/urbs/worldRenderer.test.ts
git commit -m "feat(vox-gui): urbs LOD bands + clamped full-world buffer renderer; fires animate in the blit pass

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: Pathfinding + citizens

**Files:**
- Create: `crates/vox-gui/ui/src/components/gamify/urbs/pathfind.ts`
- Create: `crates/vox-gui/ui/src/components/gamify/urbs/citizens.ts`
- Test: `crates/vox-gui/ui/src/components/gamify/urbs/pathfind.test.ts`
- Test: `crates/vox-gui/ui/src/components/gamify/urbs/citizens.test.ts`

- [ ] **Step 1: Write the failing pathfind tests**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/pathfind.test.ts
import { describe, it, expect } from 'vitest';
import { findPath, nearestRoad } from './pathfind';

// 5×5 grid, road ring around the border, buildings inside.
const W = 5, H = 5;
const roads = Array.from({ length: W * H }, (_, i) => {
  const x = i % W, y = Math.floor(i / W);
  return x === 0 || y === 0 || x === W - 1 || y === H - 1;
});

describe('pathfind', () => {
  it('nearestRoad snaps an interior tile to an adjacent road tile', () => {
    const r = nearestRoad(roads, W, H, { x: 2, y: 2 });
    expect(r).not.toBeNull();
    expect(roads[r!.y * W + r!.x]).toBe(true);
  });

  it('finds a path along roads between two corners', () => {
    const path = findPath(roads, W, H, { x: 0, y: 0 }, { x: 4, y: 4 });
    expect(path).not.toBeNull();
    expect(path![0]).toEqual({ x: 0, y: 0 });
    expect(path![path!.length - 1]).toEqual({ x: 4, y: 4 });
    // Every step is a road tile and adjacent to the previous step.
    for (let i = 0; i < path!.length; i++) {
      expect(roads[path![i].y * W + path![i].x]).toBe(true);
      if (i > 0) {
        const d = Math.abs(path![i].x - path![i - 1].x) + Math.abs(path![i].y - path![i - 1].y);
        expect(d).toBe(1);
      }
    }
  });

  it('returns null when no road route exists', () => {
    const blocked = [...roads];
    for (let y = 0; y < H; y++) blocked[y * W + 2] = false; // cut the ring? column 2 only crosses at y=0 and y=4
    blocked[0 * W + 2] = false;
    blocked[4 * W + 2] = false;
    expect(findPath(blocked, W, H, { x: 0, y: 0 }, { x: 4, y: 4 })).toBeNull();
  });
});
```

- [ ] **Step 2: Implement pathfind**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/pathfind.ts
import type { Pt } from './types';

/** BFS ring-search for the closest road tile to `p` (Chebyshev radius ≤ 6). */
export function nearestRoad(roads: boolean[], w: number, h: number, p: Pt): Pt | null {
  for (let r = 0; r <= 6; r++) {
    for (let dy = -r; dy <= r; dy++) {
      for (let dx = -r; dx <= r; dx++) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) continue;
        const x = p.x + dx, y = p.y + dy;
        if (x >= 0 && y >= 0 && x < w && y < h && roads[y * w + x]) return { x, y };
      }
    }
  }
  return null;
}

/** A* (Manhattan heuristic, 4-neighbour) over the road mask. */
export function findPath(roads: boolean[], w: number, h: number, from: Pt, to: Pt): Pt[] | null {
  const idx = (p: Pt) => p.y * w + p.x;
  if (!roads[idx(from)] || !roads[idx(to)]) return null;
  const open: { p: Pt; g: number; f: number }[] = [{ p: from, g: 0, f: 0 }];
  const came = new Map<number, number>();
  const gScore = new Map<number, number>([[idx(from), 0]]);
  const hcost = (p: Pt) => Math.abs(p.x - to.x) + Math.abs(p.y - to.y);
  while (open.length) {
    open.sort((a, b) => a.f - b.f);
    const cur = open.shift()!;
    if (cur.p.x === to.x && cur.p.y === to.y) {
      const path: Pt[] = [cur.p];
      let k = idx(cur.p);
      while (came.has(k)) {
        k = came.get(k)!;
        path.unshift({ x: k % w, y: Math.floor(k / w) });
      }
      return path;
    }
    for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]] as const) {
      const n = { x: cur.p.x + dx, y: cur.p.y + dy };
      if (n.x < 0 || n.y < 0 || n.x >= w || n.y >= h || !roads[idx(n)]) continue;
      const g = cur.g + 1;
      if (g < (gScore.get(idx(n)) ?? Infinity)) {
        gScore.set(idx(n), g);
        came.set(idx(n), idx(cur.p));
        open.push({ p: n, g, f: g + hcost(n) });
      }
    }
  }
  return null;
}
```

Run: `pnpm vitest run src/components/gamify/urbs/pathfind.test.ts` → 3 passed.

- [ ] **Step 3: Write the failing citizen state-machine tests**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/citizens.test.ts
import { describe, it, expect } from 'vitest';
import { stepCitizen, spawnCitizen, type Citizen } from './citizens';

const path = [{ x: 0, y: 0 }, { x: 1, y: 0 }, { x: 2, y: 0 }];

describe('citizen state machine', () => {
  it('spawns Idle at the given tile', () => {
    const c = spawnCitizen('agent-1', { x: 3, y: 3 });
    expect(c.state).toBe('idle');
    expect(c.pos).toEqual({ x: 3, y: 3 });
  });

  it('commutes along its path at walk speed, then works', () => {
    let c: Citizen = { ...spawnCitizen('a', { x: 0, y: 0 }), state: 'commuting', path, pathIdx: 0 };
    // Walk speed = 2 tiles/s → 1500ms covers 3 tiles.
    for (let t = 0; t < 15; t++) c = stepCitizen(c, 100, 1);
    expect(c.state).toBe('working');
    expect(c.pos.x).toBeCloseTo(2, 1);
  });

  it('speed multiplier scales movement; 0 (pause) freezes it', () => {
    let a: Citizen = { ...spawnCitizen('a', { x: 0, y: 0 }), state: 'commuting', path, pathIdx: 0 };
    let b: Citizen = { ...a };
    a = stepCitizen(a, 250, 1);
    b = stepCitizen(b, 250, 0);
    expect(a.pos.x).toBeGreaterThan(0);
    expect(b.pos.x).toBe(0);
  });

  it('exhausted citizens do not move', () => {
    let c: Citizen = { ...spawnCitizen('a', { x: 0, y: 0 }), state: 'exhausted', path, pathIdx: 0 };
    c = stepCitizen(c, 1000, 1);
    expect(c.pos).toEqual({ x: 0, y: 0 });
  });
});
```

- [ ] **Step 4: Implement citizens**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/citizens.ts
import type { Pt } from './types';

export type CitizenState = 'idle' | 'commuting' | 'working' | 'exhausted';

export interface Citizen {
  id: string;
  pos: { x: number; y: number }; // tile coords, fractional while walking
  state: CitizenState;
  path: Pt[];
  pathIdx: number;
  /** Set when working — the building the citizen stands at. */
  targetPath?: string;
}

const WALK_TILES_PER_SEC = 2;

export function spawnCitizen(id: string, at: Pt): Citizen {
  return { id, pos: { ...at }, state: 'idle', path: [], pathIdx: 0 };
}

/** Advance one citizen by dtMs · speed. Pure — the shell owns the rAF loop. */
export function stepCitizen(c: Citizen, dtMs: number, speed: number): Citizen {
  if (c.state !== 'commuting' || speed <= 0) return c;
  let remaining = (dtMs / 1000) * WALK_TILES_PER_SEC * speed;
  const pos = { ...c.pos };
  let idx = c.pathIdx;
  while (remaining > 0 && idx < c.path.length - 1) {
    const next = c.path[idx + 1];
    const dx = next.x - pos.x;
    const dy = next.y - pos.y;
    const dist = Math.hypot(dx, dy);
    if (dist <= remaining) {
      pos.x = next.x; pos.y = next.y;
      remaining -= dist;
      idx++;
    } else {
      pos.x += (dx / dist) * remaining;
      pos.y += (dy / dist) * remaining;
      remaining = 0;
    }
  }
  const arrived = idx >= c.path.length - 1;
  return { ...c, pos, pathIdx: idx, state: arrived ? 'working' : 'commuting' };
}
```

Run: `pnpm vitest run src/components/gamify/urbs/citizens.test.ts` → 4 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/urbs/pathfind.ts crates/vox-gui/ui/src/components/gamify/urbs/pathfind.test.ts crates/vox-gui/ui/src/components/gamify/urbs/citizens.ts crates/vox-gui/ui/src/components/gamify/urbs/citizens.test.ts
git commit -m "feat(vox-gui): urbs A* road pathfinding + citizen state machine

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: Harness taps — `harness_ci_fleet_status` + `vcs_town_status` (Rust) [PARALLEL-SAFE]

**Files:**
- Create: `crates/vox-gui/src/commands/harness_town.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs` (add `pub mod harness_town;`)
- Modify: `crates/vox-gui/src/main.rs` (register both commands)

- [ ] **Step 1: Write the file with parsers + failing-first tests**

Parsing is pure and fixture-tested; process spawning goes through `quiet_command` and returns `Err` on any failure (the UI renders the landmark unlit — never fake data).

```rust
// crates/vox-gui/src/commands/harness_town.rs
//! Thin harness telemetry taps for the Vox Urbs town map (CASTRVM + PORTA).
//! Read-only, slow-poll (the UI polls at 15-30s), fail-honest: any error is
//! returned as Err and rendered as an "unlit" landmark, never fabricated.

use crate::commands::process_util::quiet_command;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CiRunnerDto {
    pub name: String,
    pub busy: bool,
    pub online: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CiFleetDto {
    pub runners: Vec<CiRunnerDto>,
    pub queued: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VcsBranchDto {
    pub name: String,
    pub is_head: bool,
    /// Git's own upstream summary, e.g. "[ahead 2, behind 1]" — verbatim from
    /// `%(upstream:track)`, empty when no upstream. Never synthesized.
    pub track: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VcsPrDto {
    pub number: u64,
    pub title: String,
    pub head_ref: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VcsTownDto {
    pub branches: Vec<VcsBranchDto>,
    /// Empty (with `prs_available=false`) when `gh` is missing/unauthenticated.
    pub prs: Vec<VcsPrDto>,
    pub prs_available: bool,
}

/// Parse `gh api repos/<slug>/actions/runners` JSON (the same source
/// vox-cli's runner_scale.rs reads).
pub(crate) fn parse_runners(json: &str) -> Result<Vec<CiRunnerDto>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let runners = v["runners"].as_array().ok_or("no .runners array")?;
    Ok(runners
        .iter()
        .map(|r| CiRunnerDto {
            name: r["name"].as_str().unwrap_or("?").to_string(),
            busy: r["busy"].as_bool().unwrap_or(false),
            online: r["status"].as_str() == Some("online"),
        })
        .collect())
}

/// Parse `git for-each-ref refs/heads
/// --format=%(refname:short)%09%(HEAD)%09%(upstream:track)` output.
pub(crate) fn parse_branches(out: &str) -> Vec<VcsBranchDto> {
    out.lines()
        .filter_map(|l| {
            let mut parts = l.split('\t');
            let name = parts.next()?.trim();
            if name.is_empty() {
                return None;
            }
            let is_head = parts.next().map(|h| h.trim() == "*").unwrap_or(false);
            let track = parts.next().unwrap_or("").trim().to_string();
            Some(VcsBranchDto { name: name.to_string(), is_head, track })
        })
        .collect()
}

/// Parse `gh pr list --json number,title,headRefName` output.
pub(crate) fn parse_prs(json: &str) -> Result<Vec<VcsPrDto>, String> {
    let v: Vec<serde_json::Value> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(v.iter()
        .map(|p| VcsPrDto {
            number: p["number"].as_u64().unwrap_or(0),
            title: p["title"].as_str().unwrap_or("").to_string(),
            head_ref: p["headRefName"].as_str().unwrap_or("").to_string(),
        })
        .collect())
}

/// Derive `owner/repo` from a git remote URL (https or ssh).
pub(crate) fn slug_from_remote(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches(".git");
    if let Some(rest) = trimmed.strip_prefix("git@") {
        return rest.split_once(':').map(|(_, s)| s.to_string());
    }
    let no_scheme = trimmed.split("://").nth(1)?;
    let mut seg = no_scheme.splitn(2, '/');
    let _host = seg.next()?;
    Some(seg.next()?.to_string())
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = quiet_command(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{program} exited {:?}", out.status.code()));
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn harness_ci_fleet_status() -> Result<CiFleetDto, String> {
    tokio::task::spawn_blocking(|| {
        let remote = run("git", &["remote", "get-url", "origin"])?;
        let slug = slug_from_remote(&remote).ok_or("cannot parse origin remote")?;
        let runners_json = run("gh", &["api", &format!("repos/{slug}/actions/runners")])?;
        let runners = parse_runners(&runners_json)?;
        let queued_json = run(
            "gh",
            &["api", &format!("repos/{slug}/actions/runs?status=queued&per_page=50")],
        )?;
        let queued = serde_json::from_str::<serde_json::Value>(&queued_json)
            .ok()
            .and_then(|v| v["workflow_runs"].as_array().map(|a| a.len() as u32))
            .unwrap_or(0);
        Ok(CiFleetDto { runners, queued })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn vcs_town_status() -> Result<VcsTownDto, String> {
    tokio::task::spawn_blocking(|| {
        let branch_out = run(
            "git",
            &[
                "for-each-ref",
                "refs/heads",
                "--format=%(refname:short)%09%(HEAD)%09%(upstream:track)",
            ],
        )?;
        let branches = parse_branches(&branch_out);
        // PRs are optional: gh missing/unauthenticated → prs_available=false.
        let (prs, prs_available) =
            match run("gh", &["pr", "list", "--json", "number,title,headRefName"]) {
                Ok(json) => (parse_prs(&json).unwrap_or_default(), true),
                Err(_) => (Vec::new(), false),
            };
        Ok(VcsTownDto { branches, prs, prs_available })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_runner_fixture() {
        let json = r#"{"total_count":2,"runners":[
            {"name":"vox-runner-auto-1","status":"online","busy":true},
            {"name":"vox-runner-auto-2","status":"offline","busy":false}
        ]}"#;
        let rows = parse_runners(json).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[0].busy && rows[0].online);
        assert!(!rows[1].online);
    }

    #[test]
    fn parses_branch_fixture() {
        let out = "main\t \t\nclaude/frosty-fermi\t*\t[ahead 2, behind 1]\n";
        let rows = parse_branches(out);
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].is_head);
        assert_eq!(rows[0].track, "");
        assert!(rows[1].is_head);
        assert_eq!(rows[1].name, "claude/frosty-fermi");
        assert_eq!(rows[1].track, "[ahead 2, behind 1]");
    }

    #[test]
    fn parses_pr_fixture() {
        let json = r#"[{"number":428,"title":"fix guard","headRefName":"fix/guard"}]"#;
        let prs = parse_prs(json).unwrap();
        assert_eq!(prs[0].number, 428);
        assert_eq!(prs[0].head_ref, "fix/guard");
    }

    #[test]
    fn slug_from_https_and_ssh_remotes() {
        assert_eq!(
            slug_from_remote("https://github.com/vox-foundation/vox.git").as_deref(),
            Some("vox-foundation/vox")
        );
        assert_eq!(
            slug_from_remote("git@github.com:vox-foundation/vox.git").as_deref(),
            Some("vox-foundation/vox")
        );
        assert_eq!(slug_from_remote("not a url"), None);
    }

    #[test]
    fn bad_runner_json_is_err_not_empty() {
        assert!(parse_runners("{}").is_err());
        assert!(parse_runners("garbage").is_err());
    }
}
```

- [ ] **Step 2: Wire + test**

Add `pub mod harness_town;` to `crates/vox-gui/src/commands/mod.rs`. Register both commands in `main.rs`'s `generate_handler![...]`:

```rust
            commands::harness_town::harness_ci_fleet_status,
            commands::harness_town::vcs_town_status,
```

Run: `cargo test -p vox-gui harness_town`
Expected: 5 passed.

- [ ] **Step 3: Clippy + commit**

Run: `cargo clippy -p vox-gui -- -D warnings` → clean.

```bash
git add crates/vox-gui/src/commands/harness_town.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/src/main.rs
git commit -m "feat(vox-gui): harness_town taps — CI fleet (gh api) + vcs town status (git/gh), fail-honest

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 8: Harness data hook (TS)

**Files:**
- Create: `crates/vox-gui/ui/src/components/gamify/urbs/harnessData.ts`
- Test: `crates/vox-gui/ui/src/components/gamify/urbs/harnessData.test.ts`

- [ ] **Step 1: Write the failing tests** (mock `invoke`; assert failures → `null` fields, honesty preserved)

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/harnessData.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));

import { fetchHarnessSnapshot } from './harnessData';

beforeEach(() => invokeMock.mockReset());

describe('fetchHarnessSnapshot', () => {
  it('maps successful taps into the snapshot, counting only non-assigned hopper items as queued', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'harness_ci_fleet_status':
          return { runners: [{ name: 'r1', busy: true, online: true }], queued: 2 };
        case 'vcs_town_status':
          return { branches: [{ name: 'main', is_head: true, track: '[ahead 1]' }], prs: [{ number: 1, title: 't', head_ref: 'h' }], prs_available: true };
        case 'hopper_list':
          return [{ id: 'a', state: 'Inbox' }, { id: 'b', state: 'Assigned' }, { id: 'c', state: 'Inbox' }];
        default:
          throw new Error(`unexpected ${cmd}`);
      }
    });
    const s = await fetchHarnessSnapshot();
    expect(s.ci?.runners).toHaveLength(1);
    expect(s.ci?.queued).toBe(2);
    expect(s.vcs?.branches[0]).toEqual({ name: 'main', isHead: true, track: '[ahead 1]' });
    // hopper_list returns inbox + assigned; assigned (in-flight) is NOT queued.
    expect(s.queueLen).toBe(2);
    // No MCP server-list command exists — mcp is unconditionally null (AQVAE unlit).
    expect(s.mcp).toBeNull();
  });

  it('a failing tap yields null for that field only (unlit, not fake)', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'hopper_list') return [];
      throw new Error('unavailable');
    });
    const s = await fetchHarnessSnapshot();
    expect(s.ci).toBeNull();
    expect(s.vcs).toBeNull();
    expect(s.mcp).toBeNull();
    expect(s.queueLen).toBe(0);
  });
});
```

- [ ] **Step 2: Run to verify failure, then implement**

```typescript
// crates/vox-gui/ui/src/components/gamify/urbs/harnessData.ts
import { invoke } from '@tauri-apps/api/core';
import type { HarnessSnapshot } from './worldRenderer';

async function tryInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  try {
    return await invoke<T>(cmd, args);
  } catch {
    return null;
  }
}

interface CiFleetDto { runners: { name: string; busy: boolean; online: boolean }[]; queued: number }
interface VcsTownDto {
  branches: { name: string; is_head: boolean; track: string }[];
  prs: { number: number; title: string; head_ref: string }[];
  prs_available: boolean;
}
interface HopperItemDto { state: string }

/** One poll of every harness tap. Each failure → null (landmark unlit). */
export async function fetchHarnessSnapshot(): Promise<HarnessSnapshot> {
  const [ci, vcs, hopper] = await Promise.all([
    tryInvoke<CiFleetDto>('harness_ci_fleet_status'),
    tryInvoke<VcsTownDto>('vcs_town_status'),
    tryInvoke<HopperItemDto[]>('hopper_list'),
  ]);
  return {
    ci: ci ? { runners: ci.runners.filter((r) => r.online), queued: ci.queued } : null,
    vcs: vcs
      ? {
          branches: vcs.branches.map((b) => ({ name: b.name, isHead: b.is_head, track: b.track })),
          prs: vcs.prs_available ? vcs.prs.map((p) => ({ number: p.number, title: p.title })) : [],
        }
      : null,
    // hopper_list returns inbox PLUS assigned (in-flight); only non-assigned
    // items are honestly "queued" for the PORTVS ship count.
    queueLen: hopper ? hopper.filter((t) => !/assigned/i.test(t.state)).length : null,
    // No MCP server-list command exists, and get_orchestrator_status
    // serializes a closed struct that can never carry one — AQVAE stays
    // unconditionally unlit until a dedicated command lands (spec §7.1).
    mcp: null,
  };
}

export const HARNESS_POLL_MS = 20_000;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `pnpm vitest run src/components/gamify/urbs/harnessData.test.ts` → 2 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/urbs/harnessData.ts crates/vox-gui/ui/src/components/gamify/urbs/harnessData.test.ts
git commit -m "feat(vox-gui): urbs harness snapshot hook — CI/git/queue taps, honest queued count, MCP explicitly unlit

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 9: HUD wiring (real treasury, energy, speed)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/gamify/HudPanels.tsx` (accept nullable values, render `—` when unknown)
- Test: `crates/vox-gui/ui/src/components/gamify/HudPanels.test.tsx` (extend)

- [ ] **Step 1: Write the failing tests** (extend the existing `HudPanels.test.tsx`)

The existing file already imports `render`, `screen`, `it`, `expect`, `vi`, and `HudPanels` — do NOT re-add import lines (duplicate bindings are a SyntaxError, and Step 2 would then fail with a parse error instead of the intended type mismatch). In the same edit, migrate the file's existing case from the old props (`treasuryValue`, bare `energy`) to the new ones so the file parses as one consistent suite. Add these cases:

```tsx
// Add inside crates/vox-gui/ui/src/components/gamify/HudPanels.test.tsx
// (no new imports — the file already has render/screen/it/expect/vi/HudPanels)

it('renders real USD spend and energy fraction', () => {
  render(<HudPanels treasuryUsd={12.4} energy={82} maxEnergy={100} speed={1} onSetSpeed={() => {}} />);
  expect(screen.getByTestId('hud-value').textContent).toContain('$12.40');
  expect(screen.getByTestId('hud-energy').textContent).toBe('82/100');
});

it('renders an em-dash when spend is unknown (tap failed) — never a fake 0', () => {
  render(<HudPanels treasuryUsd={null} energy={82} maxEnergy={100} speed={1} onSetSpeed={() => {}} />);
  expect(screen.getByTestId('hud-value').textContent).toBe('—');
});

it('offers pause (0x), 1x and 3x speeds', () => {
  const spy = vi.fn();
  render(<HudPanels treasuryUsd={1} energy={1} maxEnergy={1} speed={1} onSetSpeed={spy} />);
  screen.getByRole('button', { name: '0x' }).click();
  expect(spy).toHaveBeenCalledWith(0);
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm vitest run src/components/gamify/HudPanels.test.tsx`
Expected: FAIL — prop type mismatch (`treasuryUsd` doesn't exist yet).

- [ ] **Step 3: Update the component** (props change: `treasuryValue: number` → `treasuryUsd: number | null`, `energy: number` → `energy + maxEnergy`, speeds `[1,2,4]` → `[0,1,3]` per the spec's Ⅰx/Ⅲx/pause)

```tsx
// crates/vox-gui/ui/src/components/gamify/HudPanels.tsx  (full replacement)
import React from 'react';
import { Glass } from '../ui/Glass';

interface HudPanelsProps {
  /** Real LLM spend USD (from get_llm_spend); null = unknown → render "—". */
  treasuryUsd: number | null;
  energy: number;
  maxEnergy: number;
  /** Animation speed multiplier: 0 = paused. View-only; not a simulation. */
  speed: number;
  onSetSpeed: (speed: number) => void;
}

export const HudPanels: React.FC<HudPanelsProps> = ({
  treasuryUsd, energy, maxEnergy, speed, onSetSpeed,
}) => {
  return (
    <Glass size="sm" className="flex items-center gap-4 bg-zinc-950/80 pointer-events-auto border border-zinc-800 text-zinc-100 select-none">
      <div className="flex items-center gap-2 border-r border-zinc-800 pr-3">
        <span className="text-zinc-500 text-xs font-semibold uppercase tracking-wider">Aerarivm</span>
        <span data-testid="hud-value" className="text-amber-400 font-bold font-mono">
          {treasuryUsd === null ? '—' : `$${treasuryUsd.toFixed(2)}`}
        </span>
      </div>
      <div className="flex items-center gap-2 border-r border-zinc-800 pr-3">
        <span className="text-zinc-500 text-xs font-semibold uppercase tracking-wider">Energy</span>
        <span data-testid="hud-energy" className="text-emerald-400 font-bold font-mono">
          {energy}/{maxEnergy}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <span className="text-zinc-500 text-xs font-semibold uppercase tracking-wider">Speed</span>
        <div className="flex items-center gap-1">
          {[0, 1, 3].map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => onSetSpeed(s)}
              className={`px-1.5 py-0.5 rounded text-[10px] font-mono transition-colors ${
                speed === s
                  ? 'bg-amber-500/20 border border-amber-500/50 text-amber-300'
                  : 'bg-zinc-900 border border-zinc-800 text-zinc-400 hover:text-zinc-200'
              }`}
            >
              {s}x
            </button>
          ))}
        </div>
      </div>
    </Glass>
  );
};
```

Fix any pre-existing cases in `HudPanels.test.tsx` that used the old props (`treasuryValue` → `treasuryUsd`, add `maxEnergy`).

- [ ] **Step 4: Run tests**

Run: `pnpm vitest run src/components/gamify/HudPanels.test.tsx` → all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/HudPanels.tsx crates/vox-gui/ui/src/components/gamify/HudPanels.test.tsx
git commit -m "feat(vox-gui): HudPanels real props — USD spend (nullable, honest), energy fraction, 0/1/3x speed

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 10: Integration — the `LudusSandbox` shell rewrite

**Files:**
- Rewrite: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx`
- Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` (update to the new surface)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.tsx` (container + props)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` (line ~454 renders `<LudusSandbox files={buildingFiles} />` — the second call site; drop the `files` prop and the now-unused `buildingFiles` computation if nothing else consumes it; check `Dashboard.test.tsx` for `files` assertions)
- Delete code: spiral `assignPlotCoordinates`, mock `file_edited` handler, hardcoded bubble/camera constants (all inside `LudusSandbox.tsx`)

This is the largest task; it wires Tasks 1–9 together. The component stays named `LudusSandbox` so `GamifyView` and the dashboard widget keep working.

- [ ] **Step 1: Write the failing component tests** (replace the body of `LudusSandbox.test.tsx`; mock `invoke` and the transport)

```tsx
// @vitest-environment jsdom
// crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx  (full replacement)
// The pragma above is REQUIRED as line 1: vitest.config.ts sets no global
// environment, so render()/screen crash under the default node env without it.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));
vi.mock('../../transport', () => ({
  listenAgentEvents: vi.fn().mockRejectedValue(new Error('not in tauri')),
  // useLlmSpend (used for the HUD treasury) reads voxTransport from the same
  // module — the mock must provide it or the hook crashes on mount.
  voxTransport: { getLlmSpend: vi.fn().mockRejectedValue(new Error('unavailable')) },
}));

import { LudusSandbox } from './LudusSandbox';

const SCAN = {
  crates: [{ name: 'a', root: 'crates/a', files: [{ path: 'crates/a/x.rs', lines: 10 }] }],
  root: '/ws', scanned_at_ms: 1, truncated: false,
};

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === 'workspace_town_scan') return SCAN;
    throw new Error('unavailable');
  });
});

describe('LudusSandbox (Vox Urbs shell)', () => {
  it('renders the town canvas and loads the workspace scan', async () => {
    render(<LudusSandbox />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('workspace_town_scan'));
    expect(screen.getByTestId('urbs-canvas')).toBeTruthy();
  });

  it('shows SIM PAVSED when the agent event stream is unavailable', async () => {
    render(<LudusSandbox />);
    await waitFor(() => expect(screen.getByText(/SIM PAVSED/i)).toBeTruthy());
  });

  it('shows a scan-failed state (not a fake town) when the scan tap fails', async () => {
    invokeMock.mockImplementation(async () => { throw new Error('nope'); });
    render(<LudusSandbox />);
    await waitFor(() => expect(screen.getByText(/scan unavailable/i)).toBeTruthy());
  });

  it('renders the HUD with real-null treasury (em-dash) when spend tap fails', async () => {
    render(<LudusSandbox />);
    await waitFor(() => expect(screen.getByTestId('hud-value').textContent).toBe('—'));
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm vitest run src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL against the old component (no `urbs-canvas`, old props).

- [ ] **Step 3: Rewrite the shell**

Key structure (complete file):

```tsx
// crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx  (full replacement)
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useStore } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { useLudusStore } from './store';
import { HudPanels } from './HudPanels';
import { useLlmSpend } from '../../hooks/useLlmSpend';
import { listenAgentEvents } from '../../transport';
import { moodFromPhase } from './LudusSandbox.mappers';
import type { MoodType } from './store';
import { layoutTown, assignNewFile } from './urbs/layout';
import type { TownLayout, TownScan } from './urbs/types';
import {
  fitBounds, zoomAt, clampCamera, worldToScreen, screenToWorld, type Camera,
} from './urbs/camera';
import { worldBounds, worldPx, tileFromWorld } from './urbs/lod';
import { WorldRenderer, type HarnessSnapshot } from './urbs/worldRenderer';
import { fetchHarnessSnapshot, HARNESS_POLL_MS } from './urbs/harnessData';
import { nearestRoad, findPath } from './urbs/pathfind';
import { spawnCitizen, stepCitizen, type Citizen } from './urbs/citizens';
import { useIsEmbeddedSurface } from '../dashboard/EmbeddedSurfaceContext';

const MIN_ZOOM = 0.15;
const MAX_ZOOM = 4;
const FIRE_FRAME_MS = 400;

interface Props {
  /** Optional: energy for the HUD (GamifyView passes profile data down). */
  energy?: number;
  maxEnergy?: number;
}

export const LudusSandbox: React.FC<Props> = ({ energy = 0, maxEnergy = 0 }) => {
  const embedded = useIsEmbeddedSurface();
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef(new WorldRenderer());
  const cameraRef = useRef<Camera>({ x: 0, y: 0, zoom: 0.3 });
  const dragRef = useRef<{ sx: number; sy: number; camX: number; camY: number } | null>(null);
  const citizensRef = useRef<Record<string, Citizen>>({});
  const [layout, setLayout] = useState<TownLayout | null>(null);
  const [scanRoot, setScanRoot] = useState('');
  const [scanFailed, setScanFailed] = useState(false);
  const [paused, setPaused] = useState(false); // stream drop → SIM PAVSED
  const [harness, setHarness] = useState<HarnessSnapshot>({ ci: null, vcs: null, queueLen: null, mcp: null });
  // Real spend from the existing hook (same source as the Office cost widget);
  // null = unknown → the HUD renders "—", never a fake 0.
  const { totalUsd: treasuryUsd } = useLlmSpend();
  const [speed, setSpeed] = useState(1);
  const [fireFrame, setFireFrame] = useState<0 | 1>(0);
  const [buildStage, setBuildStage] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ path: string; sx: number; sy: number } | null>(null);
  // Sprite DOM nodes collected via ref callbacks — never a per-frame
  // document.querySelector scan (engine-spec pattern, same as CitizenSprite's
  // own spriteRef).
  const spriteEls = useRef(new Map<string, HTMLElement>());

  const buildings = useStore(useLudusStore, (s) => s.buildings);
  const agentTasks = useStore(useLudusStore, (s) => s.agentTasks);
  const focusedFile = useStore(useLudusStore, (s) => s.focusedFile);

  // ── Workspace scan → layout ──────────────────────────────────────────────
  useEffect(() => {
    let live = true;
    invoke<TownScan>('workspace_town_scan')
      .then((scan) => {
        if (!live) return;
        // Landmark heuristic v1: the 3 biggest crates by file count. (Spec
        // prefers graphify-out god nodes / dependency degree — deliberate
        // simplification; upgrade when a graphify read command exists.)
        const gods = new Set(
          [...scan.crates].sort((a, b) => b.files.length - a.files.length)
            .slice(0, 3).map((c) => c.name),
        );
        setScanRoot(scan.root);
        setLayout(layoutTown(scan, gods));
      })
      .catch(() => { if (live) setScanFailed(true); });
    return () => { live = false; };
  }, []);

  // ── Blit: offscreen buffer → screen with camera transform ───────────────
  const blit = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas || !layout) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    const dpr = window.devicePixelRatio || 1;
    const cam = cameraRef.current;
    const state = { layout, buildings, agentTasks, harness };
    const { canvas: buffer, scale } = rendererRef.current.ensure(state, cam.zoom);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, canvas.width / dpr, canvas.height / dpr);
    ctx.imageSmoothingEnabled = cam.zoom * scale < 1;
    ctx.translate(cam.x, cam.y);
    ctx.scale(cam.zoom, cam.zoom);
    // The buffer may be rendered at reduced scale — draw it back at world size.
    ctx.drawImage(buffer, 0, 0, buffer.width / scale, buffer.height / scale);
    // Fires animate HERE, in the blit pass — a fire tick costs O(#error
    // buildings), never a whole-world buffer repaint.
    rendererRef.current.drawFires(ctx, state, cam.zoom, fireFrame);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
  }, [layout, buildings, agentTasks, harness, fireFrame]);

  useEffect(() => { blit(); }, [blit]);

  // ── Canvas sizing: ResizeObserver + DPR + initial fit (THE cut-off fix) ──
  useEffect(() => {
    const wrap = wrapRef.current;
    const canvas = canvasRef.current;
    if (!wrap || !canvas || !layout) return;
    let first = true;
    const size = () => {
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.round(wrap.clientWidth * dpr);
      canvas.height = Math.round(wrap.clientHeight * dpr);
      if (first) {
        // Initial view: fit + center the whole world. Never a hardcoded camera.
        cameraRef.current = fitBounds(worldBounds(layout), wrap.clientWidth, wrap.clientHeight, 24);
        first = false;
      }
      blit();
    };
    size();
    // jsdom has no ResizeObserver; the guard lets component tests mount.
    const ro = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(size) : null;
    ro?.observe(wrap);
    return () => ro?.disconnect();
  }, [layout, blit]);

  // ── Fire animation frame (slow, only when errors exist) ─────────────────
  useEffect(() => {
    const anyErrors = Object.values(buildings).some((b) => b.errors > 0);
    if (!anyErrors || speed === 0) return;
    const id = setInterval(() => setFireFrame((f) => (f ? 0 : 1)), FIRE_FRAME_MS / Math.max(speed, 1));
    return () => clearInterval(id);
  }, [buildings, speed]);

  // ── Pan / zoom (the actual fix) ──────────────────────────────────────────
  const onPointerDown = (e: React.PointerEvent) => {
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    const cam = cameraRef.current;
    dragRef.current = { sx: e.clientX, sy: e.clientY, camX: cam.x, camY: cam.y };
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = dragRef.current;
    const wrap = wrapRef.current;
    if (!d || !wrap || !layout) return;
    cameraRef.current = clampCamera(
      { ...cameraRef.current, x: d.camX + (e.clientX - d.sx), y: d.camY + (e.clientY - d.sy) },
      worldBounds(layout), wrap.clientWidth, wrap.clientHeight,
    );
    blit();
  };
  const onPointerUp = () => { dragRef.current = null; };
  const onWheel = (e: React.WheelEvent) => {
    const wrap = wrapRef.current;
    if (!wrap || !layout) return;
    const rect = wrap.getBoundingClientRect();
    const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
    cameraRef.current = clampCamera(
      zoomAt(cameraRef.current, e.clientX - rect.left, e.clientY - rect.top, factor, MIN_ZOOM, MAX_ZOOM),
      worldBounds(layout), wrap.clientWidth, wrap.clientHeight,
    );
    blit();
  };
  const fitWorld = () => {
    const wrap = wrapRef.current;
    if (!wrap || !layout) return;
    cameraRef.current = fitBounds(worldBounds(layout), wrap.clientWidth, wrap.clientHeight, 24);
    blit();
  };

  // ── Click → building hit-test → radial menu ─────────────────────────────
  const onClick = (e: React.MouseEvent) => {
    const wrap = wrapRef.current;
    if (!wrap || !layout || dragRef.current) return;
    const rect = wrap.getBoundingClientRect();
    const { wx, wy } = screenToWorld(cameraRef.current, e.clientX - rect.left, e.clientY - rect.top);
    // tileFromWorld is the single inverse of worldPx — no duplicated margin math.
    const { x: tx, y: ty } = tileFromWorld(layout, wx, wy);
    const hit = Object.values(layout.byPath).find((p) => p.x === tx && p.y === ty);
    if (hit) setMenu({ path: hit.path, sx: e.clientX - rect.left, sy: e.clientY - rect.top });
    else setMenu(null);
  };

  // ── Live agent events → citizens + focus (mock injection deleted) ───────
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    listenAgentEvents((event) => {
      const store = useLudusStore.getState();
      const k = event.kind as { type: string; path?: string; agent_id?: string; phase?: string; stage?: string };
      if (k.type === 'file_edited' && typeof k.path === 'string' && layout) {
        if (!layout.byPath[k.path]) assignNewFile(layout, k.path);
      }
      if (k.type === 'task_phase_changed' && k.agent_id && k.phase) {
        store.updateAgent(k.agent_id, { mood: moodFromPhase(k.phase) as MoodType });
      }
      // FABRICA chip: build progress (AgentEventKind::BuildStage). VERIFY the
      // serde tag in crates/vox-orchestrator/src/events.rs before trusting
      // 'build_stage' — match whatever the enum actually serializes as.
      if (k.type === 'build_stage') {
        setBuildStage(typeof k.stage === 'string' ? k.stage : null);
      }
    })
      .then((fn) => { if (!active) fn(); else { unlisten = fn; setPaused(false); } })
      .catch(() => { if (active) setPaused(true); });
    return () => { active = false; unlisten?.(); };
  }, [layout]);

  // ── Citizens: derive from agentTasks, walk in a rAF loop ────────────────
  useEffect(() => {
    if (!layout) return;
    for (const [agentId, task] of Object.entries(agentTasks)) {
      const existing = citizensRef.current[agentId];
      if (existing?.targetPath === task.filePath) continue;
      const plot = layout.byPath[task.filePath] ?? assignNewFile(layout, task.filePath);
      if (!plot) continue;
      const from = existing?.pos ?? { x: layout.landmarks.gate.x, y: layout.grid.h - 1 };
      const start = nearestRoad(layout.roads, layout.grid.w, layout.grid.h, { x: Math.round(from.x), y: Math.round(from.y) });
      const goal = nearestRoad(layout.roads, layout.grid.w, layout.grid.h, { x: plot.x, y: plot.y });
      const path = start && goal ? findPath(layout.roads, layout.grid.w, layout.grid.h, start, goal) : null;
      citizensRef.current[agentId] = {
        ...spawnCitizen(agentId, start ?? { x: plot.x, y: plot.y }),
        state: path ? 'commuting' : 'working',
        path: path ?? [],
        targetPath: task.filePath,
      };
    }
    for (const id of Object.keys(citizensRef.current)) {
      if (!agentTasks[id]) delete citizensRef.current[id];
    }
  }, [agentTasks, layout]);

  useEffect(() => {
    if (!layout || embedded) return;
    let raf = 0;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = now - last;
      last = now;
      for (const [id, c] of Object.entries(citizensRef.current)) {
        const next = stepCitizen(c, dt, speed);
        citizensRef.current[id] = next;
        // Position the DOM sprite directly (engine spec: no React re-render).
        // Elements come from the ref-callback map — no document scans.
        const el = spriteEls.current.get(id);
        if (el) {
          const { px, py } = worldPx(layout, next.pos.x, next.pos.y);
          const s = worldToScreen(cameraRef.current, px, py);
          el.style.transform = `translate(${s.sx}px, ${s.sy}px) translate(-50%, -100%)`;
          el.style.zIndex = String(Math.floor(next.pos.x + next.pos.y));
        }
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [layout, speed, embedded]);

  // ── Auto-focus on task start (user camera wins: any drag cancels) ───────
  useEffect(() => {
    if (!focusedFile || !layout || dragRef.current) return;
    const plot = layout.byPath[focusedFile];
    const wrap = wrapRef.current;
    if (!plot || !wrap) return;
    const { px, py } = worldPx(layout, plot.x, plot.y);
    const zoom = Math.max(cameraRef.current.zoom, 1.2);
    cameraRef.current = clampCamera(
      { zoom, x: wrap.clientWidth / 2 - px * zoom, y: wrap.clientHeight / 2 - py * zoom },
      worldBounds(layout), wrap.clientWidth, wrap.clientHeight,
    );
    blit();
  }, [focusedFile, layout, blit]);

  // ── Slow polls: harness landmarks + treasury (skip when embedded) ───────
  useEffect(() => {
    if (embedded) return;
    let live = true;
    const poll = async () => {
      const snap = await fetchHarnessSnapshot();
      if (live) setHarness(snap);
    };
    poll();
    const id = setInterval(poll, HARNESS_POLL_MS);
    return () => { live = false; clearInterval(id); };
  }, [embedded]);

  // ── Render ───────────────────────────────────────────────────────────────
  if (scanFailed) {
    return (
      <div className="flex h-full w-full items-center justify-center rounded-2xl border border-border-subtle bg-[#0c0a09] text-sm text-text-muted">
        Workspace scan unavailable — the town cannot render.
      </div>
    );
  }
  return (
    <div ref={wrapRef} className="relative h-full w-full overflow-hidden rounded-2xl border border-border-subtle bg-[#0c0a09]">
      <canvas
        ref={canvasRef}
        data-testid="urbs-canvas"
        className="absolute inset-0 cursor-grab active:cursor-grabbing"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onWheel={onWheel}
        onClick={onClick}
      />
      {/* Citizens overlay. Mounted from agentTasks — render-driving state; a
          ref mutation alone never re-renders, so a ref-derived roster would
          leave new citizens without DOM nodes. Positions are applied
          imperatively in the rAF loop via the spriteEls map. CitizenSprite is
          deliberately NOT reused: it self-positions from the store (lazy
          (2,2) registration + its own transform), which would fight the rAF
          wrapper transform. */}
      <div className="pointer-events-none absolute inset-0">
        {Object.keys(agentTasks).map((id) => (
          <div
            key={id}
            ref={(el) => { if (el) spriteEls.current.set(id, el); else spriteEls.current.delete(id); }}
            className="absolute will-change-transform"
          >
            <div className="flex flex-col items-center">
              <span className="text-lg leading-none">🧍</span>
              <span className="rounded bg-bg-base/80 px-1 font-mono text-[9px] text-text-secondary">{id}</span>
            </div>
          </div>
        ))}
      </div>
      {buildStage && (
        <div className="absolute right-2 top-2 rounded border border-border-subtle bg-black/70 px-2 py-1 font-serif text-[10px] tracking-widest text-amber-200">
          FABRICA · {buildStage}
        </div>
      )}
      {paused && (
        <div className="absolute left-1/2 top-2 -translate-x-1/2 rounded border border-amber-700/50 bg-black/70 px-3 py-1 font-serif text-[11px] tracking-widest text-amber-300">
          SIM PAVSED — live stream unavailable
        </div>
      )}
      {menu && (
        <div
          className="absolute z-20 rounded-lg border border-border-subtle bg-bg-base p-1 text-[11px] shadow-lg"
          style={{ left: menu.sx + 8, top: menu.sy + 8 }}
        >
          <div className="max-w-[220px] truncate px-2 py-1 font-mono text-text-muted">{menu.path}</div>
          <button type="button" className="block w-full rounded px-2 py-1 text-left hover:bg-overlay-subtle"
            onClick={() => { invoke('open_locator', { locator: { kind: 'file', value: scanRoot ? `${scanRoot}/${menu.path}` : menu.path } }).catch(() => {}); setMenu(null); }}>
            Open file
          </button>
          <button type="button" className="block w-full rounded px-2 py-1 text-left hover:bg-overlay-subtle"
            onClick={() => { useLudusStore.getState().setFocusedFile(menu.path); setMenu(null); }}>
            Focus
          </button>
          <div className="px-2 py-1 text-text-muted">
            ⚠ {buildings[menu.path]?.warnings ?? 0} · 🔥 {buildings[menu.path]?.errors ?? 0}
          </div>
        </div>
      )}
      <div className="absolute bottom-2 left-2 flex items-center gap-2">
        <HudPanels treasuryUsd={treasuryUsd} energy={energy} maxEnergy={maxEnergy} speed={speed} onSetSpeed={setSpeed} />
        <button type="button" onClick={fitWorld}
          className="pointer-events-auto rounded border border-border-subtle bg-bg-base/80 px-2 py-1 text-[10px] text-text-muted hover:text-text-primary">
          ⌂ fit
        </button>
      </div>
    </div>
  );
};
```

Notes for the implementer:
- `assignPlotCoordinates`, the old offscreen grid painter, the hardcoded bubble, and the mock `file_edited` warning injection are all **gone** — this file replaces them wholesale. Grep for `assignPlotCoordinates` usages (the old export is referenced by `LudusSandbox.test.tsx` only) and remove them.
- `open_locator` gets an absolute path by joining the stored `scanRoot` (the DTO's `root` field exists for exactly this) with the relative building path; the relative fallback only covers a scan that predates the field.
- `CitizenSprite` is deliberately not reused: it lazy-registers agents at (2,2) in the store and imperatively positions its own element, which would fight the rAF wrapper transform. If richer sprites are wanted later, first add a presentation-only mode to `CitizenSprite`.

- [ ] **Step 4: Update BOTH call sites** — fix the container mismatch and mock props

In `crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.tsx`:
- Change the map container `<div className="h-[450px]">` to `<div className="h-[560px]">` (single height owner; the sandbox is now `h-full`).
- Pass profile energy down: `<LudusSandbox files={...}` → `<LudusSandbox energy={profile?.energy ?? 0} maxEnergy={profile?.max_energy ?? 0} />` (the `files` prop is gone — the sandbox scans the workspace itself). Remove the now-unused `buildings`/`buildingFiles` selector at the top of the component if nothing else consumes it.
- The quests + `DueNudge` blocks already rendered by this view ARE the SENATVS board — no new panel.

In `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` (line ~454):
- `<LudusSandbox files={buildingFiles} />` → `<LudusSandbox />` (props default; the embedded widget has no profile energy in scope). Remove the `buildingFiles` computation if it has no other consumer, and fix any `files` assertions in `Dashboard.test.tsx`.

- [ ] **Step 5: Run the full gamify test suite**

Run: `pnpm vitest run src/components/gamify src/components/surfaces/Gamify`
Expected: all pass (update `GamifyView.test.tsx` prop expectations if it asserted on `files`).

- [ ] **Step 6: Type-check + lint the UI**

Run (from `crates/vox-gui/ui`): `pnpm tsc --noEmit && pnpm lint`
Expected: clean.

- [ ] **Step 7: Manual verification (the spec's §12 list)**

Launch the app (per project convention; GUI is pnpm/Tauri). Verify:
1. The town renders **centered and fully visible**; no cut-off at any window size.
2. Drag pans; wheel zooms at the cursor; `⌂ fit` recenters; zooming out past the threshold collapses districts to landmark temples.
3. Edit a file in the workspace → a citizen commutes to that building with scaffolding.
4. Introduce a compile error → fire animates on the right building; fix → fire clears.
5. With `gh` logged out → PORTA shows branches (git still works) but no caravans, CASTRVM renders unlit with its reason — no fake numbers anywhere. AQVAE is **always** unlit (no MCP telemetry exists yet) — confirm it says so rather than showing lit arches.
6. HUD shows real spend (matches the Office budget widget) or `—`; 0x freezes citizens and fire.
7. Run a vox build → the FABRICA chip names the stages as they stream, then clears.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.tsx crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.test.tsx crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.test.tsx
git commit -m "feat(vox-gui): Vox Urbs shell — pan/zoom camera, LOD town, citizens, harness landmarks, honest HUD; deletes spiral + mocks

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Deferred (explicitly NOT in this plan — YAGNI until the core ships)

- Per-building FSRS "neglected district" glow (no file-path data in `DueActionDto`; needs a backend field first).
- A dedicated MCP server-list command (AQVAE stays probe-based until one exists).
- Churn heat tint (needs an edit-recency store; add after the core map proves out).
- "Dispatch refactor subagent" radial action (ships only when an end-to-end orchestrator dispatch command exists).
- Pinch-zoom gesture tuning on touchpads beyond wheel events.
- Git worktree enumeration for PORTA (needs `git worktree list --porcelain` parsing; branches + ahead/behind + PRs ship now).
- The budget-lockout trigger for the citizens' `Exhausted` state (no lockout signal exists in the GUI yet; the state machine itself ships).

## Final acceptance

- All new vitest suites green: `pnpm vitest run src/components/gamify`
- Rust: `cargo test -p vox-gui -- workspace_town harness_town` green (multiple filters must go after `--`; two bare positionals are a cargo error); `cargo clippy -p vox-gui -- -D warnings` clean.
- Manual §12 checklist above verified in the running app.
