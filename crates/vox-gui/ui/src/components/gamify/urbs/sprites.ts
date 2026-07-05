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
