import { describe, it, expect } from 'vitest';
import {
  HUD_TILE_KINDS,
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
  it('defaultHudTiles() returns all 7 kinds in order', () => {
    const config = defaultHudTiles();
    expect(config.tiles.map((t) => t.kind)).toEqual([
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
    expect(next.tiles.map((t) => t.kind)).toEqual([
      'queue_depth',
      'budget_burn',
      'active_agents',
      'mesh_peers',
      'active_model',
      'openrouter_spend',
      'pending_approvals',
    ]);
  });
});
