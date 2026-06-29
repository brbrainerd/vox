// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import React from 'react';
import { HudTilesEditor } from './HudTilesEditor';
import {
  defaultHudTiles,
  HUD_TILE_KINDS,
  HUD_TILE_LABELS,
} from '../../../hooks/useHudTiles';
import { LanguageProvider } from '../../../hooks/useLanguage';

describe('HudTilesEditor', () => {
  beforeEach(() => cleanup());

  it('lists all 7 tile kinds from HUD_TILE_KINDS', () => {
    const config = defaultHudTiles();
    render(<LanguageProvider><HudTilesEditor config={config} onChange={vi.fn()} /></LanguageProvider>);
    for (const kind of HUD_TILE_KINDS) {
      expect(screen.getByLabelText(HUD_TILE_LABELS[kind])).toBeTruthy();
    }
  });

  it('toggle disables a tile and calls onChange with updated config', () => {
    const config = defaultHudTiles();
    const onChange = vi.fn();
    render(<LanguageProvider><HudTilesEditor config={config} onChange={onChange} /></LanguageProvider>);
    const checkbox = screen.getByLabelText(HUD_TILE_LABELS.queue_depth) as HTMLInputElement;
    expect(checkbox.checked).toBe(true);
    fireEvent.click(checkbox);
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        tiles: expect.arrayContaining([
          expect.objectContaining({ id: 'queue_depth', enabled: false }),
        ]),
      }),
    );
  });

  it('reset to defaults restores defaultHudTiles()', () => {
    const config = {
      version: 1 as const,
      tiles: defaultHudTiles().tiles.map((t) =>
        t.id === 'active_agents' ? { ...t, enabled: false } : t,
      ),
    };
    const onChange = vi.fn();
    render(<LanguageProvider><HudTilesEditor config={config} onChange={onChange} /></LanguageProvider>);
    fireEvent.click(screen.getByRole('button', { name: /reset to defaults/i }));
    expect(onChange).toHaveBeenCalledWith(defaultHudTiles());
  });
});
