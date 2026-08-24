import React from 'react';
import { useLabel } from '../../../hooks/useLanguage';
import {
  defaultHudTiles,
  defaultHudOptions,
  HUD_TILE_LABELS,
  SPEND_POLL_SECONDS_MAX,
  SPEND_POLL_SECONDS_MIN,
  reorderHudTile,
  setHudOption,
  toggleHudTile,
  type HudTilesConfig,
} from '../../../hooks/useHudTiles';

const BTN =
  'rounded border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle disabled:opacity-40';

interface HudTilesEditorProps {
  config: HudTilesConfig;
  onChange: (next: HudTilesConfig) => void;
}

export function HudTilesEditor({ config, onChange }: HudTilesEditorProps) {
  const move = (from: number, to: number) => {
    onChange(reorderHudTile(config, from, to));
  };
  const options = config.options ?? defaultHudOptions();

  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-display')}</h2>
      <p className="mt-0.5 text-[11px] text-text-muted">
        Choose which tiles appear in the status bar and their order
      </p>
      <div className="mt-4 space-y-2">
        {config.tiles.map((tile, index) => (
          <div
            key={tile.id}
            className="flex items-center justify-between gap-3 rounded-xl border border-border-subtle bg-overlay-subtle p-3"
          >
            <label className="flex min-w-0 flex-1 items-center gap-3">
              <input
                type="checkbox"
                checked={tile.enabled}
                aria-label={HUD_TILE_LABELS[tile.kind]}
                onChange={(e) => onChange(toggleHudTile(config, tile.id, e.target.checked))}
                className="size-4 rounded border-white/20 bg-black/30 accent-[rgb(var(--brass))]"
              />
              <span className="font-display text-[12px] text-text-secondary">
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
      <div className="mt-4 flex items-center justify-between gap-3 rounded-xl border border-border-subtle bg-overlay-subtle p-3">
        <label htmlFor="hud-spend-poll" className="min-w-0 flex-1">
          <span className="font-display text-[12px] text-text-secondary">Spend refresh</span>
          <span className="mt-0.5 block text-[10px] text-text-muted">
            How often the LLM spend tile re-reads recorded cost ({SPEND_POLL_SECONDS_MIN}–
            {SPEND_POLL_SECONDS_MAX}s). Each refresh is one database read.
          </span>
        </label>
        <div className="flex shrink-0 items-center gap-1">
          <input
            id="hud-spend-poll"
            type="number"
            min={SPEND_POLL_SECONDS_MIN}
            max={SPEND_POLL_SECONDS_MAX}
            defaultValue={options.spendPollSeconds}
            key={options.spendPollSeconds}
            // Committed on blur, not per keystroke: the persisted value is
            // clamped to 10-3600, so clamping mid-edit makes an intermediate
            // "3" snap to 10 and the next digit land as 105 instead of 30.
            onBlur={(e) => {
              const n = Number(e.target.value);
              if (Number.isFinite(n) && n !== options.spendPollSeconds) {
                onChange(setHudOption(config, 'spendPollSeconds', n));
              }
            }}
            className="w-20 rounded border border-border-subtle bg-bg-base px-2 py-1 text-right font-mono text-[11px] text-text-secondary"
          />
          <span className="font-mono text-[10px] text-text-muted">s</span>
        </div>
      </div>

      <p className="mt-3 text-[10px] leading-snug text-text-muted">
        Budget caps themselves (daily, per-session, warn threshold) are edited under
        Runtime — they govern LLM dispatch, not just what this bar displays.
      </p>

      <button
        type="button"
        onClick={() => onChange(defaultHudTiles())}
        className="mt-4 rounded border border-border-subtle bg-overlay-subtle px-3 py-1.5 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle"
      >
        Reset to defaults
      </button>
    </>
  );
}
