import { describe, it, expect } from 'vitest';
import {
  HUD_TILE_KINDS,
  HUD_DENSITIES,
  HUD_TILE_LABELS,
  defaultHudTiles,
  validateHudTilesConfig,
  filterKpisByTiles,
  resolveVisibleHudTiles,
  toggleHudTile,
  reorderHudTile,
} from './useHudTiles';

describe('pending_approvals HUD tile', () => {
  it('is part of the HUD tile SSOT with a label', () => {
    expect(HUD_TILE_KINDS).toContain('pending_approvals');
    expect(HUD_TILE_LABELS.pending_approvals).toBe('Pending approvals');
  });

  it('appears in the strip by default and DROPS when disabled', () => {
    const cfg = defaultHudTiles();
    expect(resolveVisibleHudTiles(cfg)).toContain('pending_approvals');
    const disabled = toggleHudTile(cfg, 'pending_approvals', false);
    expect(resolveVisibleHudTiles(disabled)).not.toContain('pending_approvals');
  });
});

describe('useHudTiles', () => {
  it('defaultHudTiles() returns every catalog kind in SSOT order', () => {
    // Asserted against HUD_TILE_KINDS rather than a literal list: the catalog
    // grows, and a hardcoded copy here just fails on every addition without
    // testing anything the SSOT doesn't already state.
    const config = defaultHudTiles();
    expect(config.tiles.map((t) => t.kind)).toEqual([...HUD_TILE_KINDS]);
  });

  it('enables the original v1 tiles by default and leaves later additions off', () => {
    const enabled = defaultHudTiles().tiles.filter((t) => t.enabled).map((t) => t.kind);
    expect(enabled).toEqual([
      'active_agents',
      'queue_depth',
      'budget_burn',
      'mesh_peers',
      'active_model',
      'openrouter_spend',
      'pending_approvals',
    ]);
  });

  it('validateHudTilesConfig rejects unknown tile id', () => {
    expect(() =>
      validateHudTilesConfig({
        version: 1,
        tiles: [{ id: 'not-a-real-tile', kind: 'active_agents', enabled: true }],
      }),
    ).toThrow(/unknown tile id/i);
  });

  it('filterKpisByTiles only renders enabled tiles', () => {
    const config = {
      version: 1 as const,
      tiles: [
        { id: 'active_agents', kind: 'active_agents' as const, enabled: true },
        { id: 'queue_depth', kind: 'queue_depth' as const, enabled: false },
        { id: 'budget_burn', kind: 'budget_burn' as const, enabled: true },
        { id: 'mesh_peers', kind: 'mesh_peers' as const, enabled: false },
        { id: 'active_model', kind: 'active_model' as const, enabled: true },
        { id: 'openrouter_spend', kind: 'openrouter_spend' as const, enabled: false },
      ],
    };
    expect(filterKpisByTiles(config)).toEqual([
      'active_agents',
      'budget_burn',
      'active_model',
    ]);
    expect(resolveVisibleHudTiles(config)).toEqual(filterKpisByTiles(config));
  });

  it('toggleHudTile updates enabled flag for matching id', () => {
    const config = defaultHudTiles();
    const next = toggleHudTile(config, 'queue_depth', false);
    expect(next.tiles.find((t) => t.id === 'queue_depth')?.enabled).toBe(false);
    expect(next.tiles.find((t) => t.id === 'active_agents')?.enabled).toBe(true);
  });

  it('reorderHudTile moves tile from fromIndex to toIndex', () => {
    const config = defaultHudTiles();
    const next = reorderHudTile(config, 0, 2);
    expect(next.tiles.map((t) => t.kind).slice(0, 3)).toEqual([
      'queue_depth',
      'budget_burn',
      'active_agents',
    ]);
    // The move is a permutation: nothing added, nothing dropped.
    expect([...next.tiles].map((t) => t.kind).sort()).toEqual([...HUD_TILE_KINDS].sort());
  });
});

describe('v2 config: bar options and migration', () => {
  it('defaults carry the v2 options block', () => {
    const cfg = defaultHudTiles();
    expect(cfg.version).toBe(2);
    expect(cfg.options).toEqual({
      density: 'labeled',
      showFreshness: true,
      spendPollSeconds: 60,
    });
  });

  it('migrates a persisted v1 config forward, preserving tile order and enablement', () => {
    const v1 = {
      version: 1,
      tiles: [
        { id: 'queue_depth', kind: 'queue_depth', enabled: false },
        { id: 'active_agents', kind: 'active_agents', enabled: true },
      ],
    };
    const cfg = validateHudTilesConfig(v1);
    expect(cfg.version).toBe(2);
    expect(cfg.tiles.slice(0, 2)).toEqual(v1.tiles);
    expect(cfg.options.density).toBe('labeled');
  });

  it('appends tile kinds added after the persisted config was written, disabled', () => {
    // A v1 config predates vram_total/session_spend/build_version. Dropping
    // them would make new tiles permanently unreachable for existing users.
    const cfg = validateHudTilesConfig({
      version: 1,
      tiles: [{ id: 'active_agents', kind: 'active_agents', enabled: true }],
    });
    const vram = cfg.tiles.find((t) => t.kind === 'vram_total');
    expect(vram).toEqual({ id: 'vram_total', kind: 'vram_total', enabled: false });
  });

  it('rejects an unknown density rather than silently rendering the default', () => {
    expect(() =>
      validateHudTilesConfig({ version: 2, tiles: [], options: { density: 'huge', showFreshness: true, spendPollSeconds: 60 } }),
    ).toThrow(/density/);
  });

  it('clamps spendPollSeconds to the contract range', () => {
    const lo = validateHudTilesConfig({ version: 2, tiles: [], options: { density: 'labeled', showFreshness: true, spendPollSeconds: 1 } });
    const hi = validateHudTilesConfig({ version: 2, tiles: [], options: { density: 'labeled', showFreshness: true, spendPollSeconds: 99999 } });
    expect(lo.options.spendPollSeconds).toBe(10);
    expect(hi.options.spendPollSeconds).toBe(3600);
  });

  it('exposes the three new tile kinds with labels', () => {
    for (const k of ['vram_total', 'session_spend', 'build_version'] as const) {
      expect(HUD_TILE_KINDS).toContain(k);
      expect(HUD_TILE_LABELS[k]).toBeTruthy();
    }
  });
});

describe('config hygiene', () => {
  it('collapses a repeated tile kind so the bar cannot render duplicate React keys', () => {
    const cfg = validateHudTilesConfig({
      version: 2,
      tiles: [
        { id: 'queue_depth', kind: 'queue_depth', enabled: true },
        { id: 'queue_depth', kind: 'queue_depth', enabled: false },
        { id: 'active_agents', kind: 'active_agents', enabled: true },
      ],
      options: { density: 'labeled', showFreshness: true, spendPollSeconds: 60 },
    });
    const kinds = cfg.tiles.map((t) => t.kind);
    expect(new Set(kinds).size).toBe(kinds.length);
    // First occurrence wins, so the user's enabled:true is what survives.
    expect(cfg.tiles.find((t) => t.kind === 'queue_depth')?.enabled).toBe(true);
    expect(kinds[0]).toBe('queue_depth');
  });

  it('offers only densities that render differently', () => {
    // A third mode existed briefly and was indistinguishable from 'compact' —
    // Segment branches solely on whether the label renders.
    expect([...HUD_DENSITIES]).toEqual(['labeled', 'compact']);
  });
});
