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
  return `${band}#${s.layout.grid.w}x${s.layout.grid.h}#${diag}#${tasks}#${hk}`;
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
