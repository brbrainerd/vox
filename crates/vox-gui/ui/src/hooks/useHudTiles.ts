/**
 * HUD tile config — SSOT kinds from contracts/gui/hud-tiles.v1.yaml
 * Persisted under gui.hud.tiles.v1 (shell-persistence.v1.yaml).
 */

export const HUD_TILE_KINDS = [
  'active_agents',
  'queue_depth',
  'budget_burn',
  'mesh_peers',
  'active_model',
  'openrouter_spend',
  'pending_approvals',
] as const;

export type HudTileKind = (typeof HUD_TILE_KINDS)[number];

export const HUD_TILE_LABELS: Record<HudTileKind, string> = {
  active_agents: 'Active agents',
  queue_depth: 'Queue depth',
  budget_burn: 'Budget burn',
  mesh_peers: 'Mesh peers',
  active_model: 'Active model',
  openrouter_spend: 'OpenRouter spend',
  pending_approvals: 'Pending approvals',
};

export interface HudTileEntry {
  id: string;
  kind: HudTileKind;
  enabled: boolean;
}

export interface HudTilesConfig {
  version: 1;
  tiles: HudTileEntry[];
}

const KIND_SET = new Set<string>(HUD_TILE_KINDS);

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

export function defaultHudTiles(): HudTilesConfig {
  return {
    version: 1,
    tiles: HUD_TILE_KINDS.map((kind) => ({
      id: kind,
      kind,
      enabled: true,
    })),
  };
}

export function validateHudTilesConfig(raw: unknown): HudTilesConfig {
  if (!isRecord(raw)) {
    throw new Error('hud tiles config must be an object');
  }
  if (raw.version !== 1) {
    throw new Error('hud tiles config version must be 1');
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

  return { version: 1, tiles };
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
