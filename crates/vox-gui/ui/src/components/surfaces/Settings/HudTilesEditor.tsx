import React from 'react';
import {
  defaultHudTiles,
  HUD_TILE_LABELS,
  reorderHudTile,
  toggleHudTile,
  type HudTilesConfig,
} from '../../../hooks/useHudTiles';

const BTN =
  'rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5 disabled:opacity-40';

interface HudTilesEditorProps {
  config: HudTilesConfig;
  onChange: (next: HudTilesConfig) => void;
}

export function HudTilesEditor({ config, onChange }: HudTilesEditorProps) {
  const move = (from: number, to: number) => {
    onChange(reorderHudTile(config, from, to));
  };

  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Display</h2>
      <p className="mt-0.5 text-[11px] text-zinc-500">
        Choose which KPI tiles appear in the top HUD and their order
      </p>
      <div className="mt-4 space-y-2">
        {config.tiles.map((tile, index) => (
          <div
            key={tile.id}
            className="flex items-center justify-between gap-3 rounded-xl border border-white/5 bg-white/[0.02] p-3"
          >
            <label className="flex min-w-0 flex-1 items-center gap-3">
              <input
                type="checkbox"
                checked={tile.enabled}
                aria-label={HUD_TILE_LABELS[tile.kind]}
                onChange={(e) => onChange(toggleHudTile(config, tile.id, e.target.checked))}
                className="size-4 rounded border-white/20 bg-black/30 accent-[rgb(var(--brass))]"
              />
              <span className="font-display text-[12px] text-zinc-200">
                {HUD_TILE_LABELS[tile.kind]}
              </span>
            </label>
            <div className="flex shrink-0 items-center gap-1">
              <button
                type="button"
                className={BTN}
                disabled={index === 0}
                onClick={() => move(index, index - 1)}
                aria-label={`Move ${HUD_TILE_LABELS[tile.kind]} up`}
                title="Move up"
              >
                ↑
              </button>
              <button
                type="button"
                className={BTN}
                disabled={index === config.tiles.length - 1}
                onClick={() => move(index, index + 1)}
                aria-label={`Move ${HUD_TILE_LABELS[tile.kind]} down`}
                title="Move down"
              >
                ↓
              </button>
            </div>
          </div>
        ))}
      </div>
      <button
        type="button"
        onClick={() => onChange(defaultHudTiles())}
        className="mt-4 rounded border border-white/10 bg-white/[0.02] px-3 py-1.5 font-mono text-[10px] text-zinc-300 hover:bg-white/5"
      >
        Reset to defaults
      </button>
    </>
  );
}
