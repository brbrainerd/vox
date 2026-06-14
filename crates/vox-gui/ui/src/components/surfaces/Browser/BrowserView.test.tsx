// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(null) }));
vi.mock('../../../transport', () => ({
  listenAgentEvents: () => Promise.resolve(() => {}),
  listenBrowserFrames: () => Promise.resolve(() => {}),
  listenPreviewAvailable: () => Promise.resolve(() => {}),
}));

import { BrowserView, mapClickToViewport } from './BrowserView';

describe('BrowserView component', () => {
  it('renders the view tabs as a tablist with aria-selected', () => {
    render(<BrowserView pushToast={() => {}} />);
    const tablist = screen.getByRole('tablist', { name: /browser view/i });
    expect(tablist).toBeDefined();
    const preview = screen.getByRole('tab', { name: /^preview$/i });
    expect(preview.getAttribute('aria-selected')).toBe('true');
  });

  it('every button carries an explicit type="button"', () => {
    render(<BrowserView pushToast={() => {}} />);
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });
});

describe('mapClickToViewport', () => {
  it('maps centered click without letterboxing', () => {
    const got = mapClickToViewport(
      640,
      400,
      { left: 0, top: 0, width: 1280, height: 800 },
      1280,
      800,
    );
    expect(got).toEqual({ x: 640, y: 400 });
  });

  it('accounts for object-contain vertical letterboxing', () => {
    // Container 1280x900 displaying a 1280x800 viewport -> 50px top/bottom pads.
    const got = mapClickToViewport(
      640,
      50 + 400,
      { left: 0, top: 0, width: 1280, height: 900 },
      1280,
      800,
    );
    expect(got).toEqual({ x: 640, y: 400 });
  });

  it('returns null when click is in letterboxed padding', () => {
    const got = mapClickToViewport(
      640,
      10,
      { left: 0, top: 0, width: 1280, height: 900 },
      1280,
      800,
    );
    expect(got).toBeNull();
  });

  it('maps correctly for non-default viewport dimensions', () => {
    // Container 1000x600 displaying an 800x600 frame -> horizontal pillarbox.
    const got = mapClickToViewport(
      500,
      300,
      { left: 0, top: 0, width: 1000, height: 600 },
      800,
      600,
    );
    expect(got).toEqual({ x: 400, y: 300 });
  });
});
