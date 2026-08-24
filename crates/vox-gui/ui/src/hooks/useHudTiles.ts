/**
 * Status-bar tile config — SSOT kinds from contracts/gui/hud-tiles.v1.yaml.
 * Persisted under gui.hud.tiles.v1 (shell-persistence.v1.yaml); v1 payloads
 * are migrated forward on read, so an existing user's tile order and
 * enablement survive while newly added kinds arrive disabled.
 */

export const HUD_TILE_KINDS = [
  'active_agents',
  'queue_depth',
  'budget_burn',
  'mesh_peers',
  'active_model',
  'openrouter_spend',
  'pending_approvals',
  'vram_total',
  'session_spend',
  'build_version',
] as const;

export type HudTileKind = (typeof HUD_TILE_KINDS)[number];

export const HUD_TILE_LABELS: Record<HudTileKind, string> = {
  active_agents: 'Active agents',
  queue_depth: 'Queue depth',
  budget_burn: 'Budget burn',
  mesh_peers: 'Mesh peers',
  active_model: 'Active model',
  openrouter_spend: 'LLM spend',
  pending_approvals: 'Pending approvals',
  vram_total: 'Mesh VRAM',
  session_spend: 'Session spend',
  build_version: 'Build version',
};

/** Bar-level display options (contract `options_shape`). */
// Two modes, not three: `Segment` only branches on whether the label renders,
// so a third value would be indistinguishable from `compact` on screen.
export const HUD_DENSITIES = ['labeled', 'compact'] as const;
export type HudDensity = (typeof HUD_DENSITIES)[number];

export const SPEND_POLL_SECONDS_MIN = 10;
export const SPEND_POLL_SECONDS_MAX = 3600;

export interface HudOptions {
  density: HudDensity;
  showFreshness: boolean;
  spendPollSeconds: number;
}

export function defaultHudOptions(): HudOptions {
  return { density: 'labeled', showFreshness: true, spendPollSeconds: 60 };
}

export interface HudTileEntry {
  id: string;
  kind: HudTileKind;
  enabled: boolean;
}

export interface HudTilesConfig {
  version: 2;
  tiles: HudTileEntry[];
  options: HudOptions;
}

const KIND_SET = new Set<string>(HUD_TILE_KINDS);

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** Kinds added after v1; they default to off so the bar doesn't silently grow. */
const OFF_BY_DEFAULT: ReadonlySet<string> = new Set([
  'vram_total',
  'session_spend',
  'build_version',
]);

export function defaultHudTiles(): HudTilesConfig {
  return {
    version: 2,
    tiles: HUD_TILE_KINDS.map((kind) => ({
      id: kind,
      kind,
      enabled: !OFF_BY_DEFAULT.has(kind),
    })),
    options: defaultHudOptions(),
  };
}

const DENSITY_SET: ReadonlySet<string> = new Set(HUD_DENSITIES);

function clamp(n: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, n));
}

/**
 * Validate the `options` block. Absent (v1) => defaults. An unknown density is
 * rejected rather than coerced: silently substituting the default would make a
 * typo in a hand-edited config look like the setting simply didn't work.
 * `spendPollSeconds` IS clamped — an out-of-range number is unambiguous about
 * intent (poll faster/slower), so the contract bound is applied rather than
 * failing the whole config over one field.
 */
function validateHudOptions(raw: unknown): HudOptions {
  if (raw === undefined) return defaultHudOptions();
  if (!isRecord(raw)) throw new Error('options must be an object');
  const d = raw.density ?? 'labeled';
  if (typeof d !== 'string' || !DENSITY_SET.has(d)) {
    throw new Error(`unknown density: ${String(d)}`);
  }
  const showFreshness = raw.showFreshness ?? true;
  if (typeof showFreshness !== 'boolean') {
    throw new Error('options.showFreshness must be a boolean');
  }
  const rawPoll = raw.spendPollSeconds ?? 60;
  if (typeof rawPoll !== 'number' || !Number.isFinite(rawPoll)) {
    throw new Error('options.spendPollSeconds must be a finite number');
  }
  return {
    density: d as HudDensity,
    showFreshness,
    spendPollSeconds: clamp(Math.round(rawPoll), SPEND_POLL_SECONDS_MIN, SPEND_POLL_SECONDS_MAX),
  };
}

export function validateHudTilesConfig(raw: unknown): HudTilesConfig {
  if (!isRecord(raw)) {
    throw new Error('hud tiles config must be an object');
  }
  if (raw.version !== 1 && raw.version !== 2) {
    throw new Error('hud tiles config version must be 1 or 2');
  }
  if (!Array.isArray(raw.tiles)) {
    throw new Error('hud tiles config tiles must be an array');
  }

  const tiles: HudTileEntry[] = raw.tiles.map((entry, index) => {
    const path = `tiles[${index}]`;
    if (!isRecord(entry)) {
      throw new Error(`${path}: tile must be an object`);
    }
    const id = entry.id;
    const kind = entry.kind;
    const enabled = entry.enabled;
    if (typeof id !== 'string' || id.length === 0) {
      throw new Error(`${path}: id must be a non-empty string`);
    }
    if (!KIND_SET.has(id)) {
      throw new Error(`unknown tile id: ${id}`);
    }
    if (typeof kind !== 'string' || !KIND_SET.has(kind)) {
      throw new Error(`unknown tile kind: ${String(kind)}`);
    }
    if (typeof enabled !== 'boolean') {
      throw new Error(`${path}: enabled must be a boolean`);
    }
    return { id, kind: kind as HudTileKind, enabled };
  });

  // Drop repeats before migrating. A hand-edited config can name the same kind
  // twice; the bar keys its segments by kind, so duplicates would render two
  // identical tiles under one React key. First occurrence wins, preserving the
  // user's ordering.
  const seen = new Set<string>();
  const deduped = tiles.filter((t) => (seen.has(t.kind) ? false : (seen.add(t.kind), true)));

  // Forward-migration: a persisted config predates any kind added since it was
  // written. Appending the missing kinds (disabled) keeps the user's order and
  // enablement intact while leaving new tiles reachable from the editors —
  // without this, a v1 config would pin the bar to the v1 catalog forever.
  for (const kind of HUD_TILE_KINDS) {
    if (!seen.has(kind)) deduped.push({ id: kind, kind, enabled: false });
  }

  return { version: 2, tiles: deduped, options: validateHudOptions(raw.options) };
}

export function setHudOption<K extends keyof HudOptions>(
  config: HudTilesConfig,
  key: K,
  value: HudOptions[K],
): HudTilesConfig {
  return { ...config, options: { ...config.options, [key]: value } };
}

export function resolveVisibleHudTiles(config: HudTilesConfig): HudTileKind[] {
  return config.tiles.filter((t) => t.enabled).map((t) => t.kind);
}

/** Returns enabled tile kinds in config order (alias for HUD KPI filtering). */
export function filterKpisByTiles(config: HudTilesConfig): HudTileKind[] {
  return resolveVisibleHudTiles(config);
}

export function toggleHudTile(
  config: HudTilesConfig,
  id: string,
  enabled: boolean,
): HudTilesConfig {
  return {
    ...config,
    tiles: config.tiles.map((t) => (t.id === id ? { ...t, enabled } : t)),
  };
}

export function reorderHudTile(
  config: HudTilesConfig,
  fromIndex: number,
  toIndex: number,
): HudTilesConfig {
  if (fromIndex === toIndex) return config;
  if (fromIndex < 0 || fromIndex >= config.tiles.length) return config;
  if (toIndex < 0 || toIndex >= config.tiles.length) return config;
  const tiles = config.tiles.slice();
  const [moved] = tiles.splice(fromIndex, 1);
  tiles.splice(toIndex, 0, moved);
  return { ...config, tiles };
}
