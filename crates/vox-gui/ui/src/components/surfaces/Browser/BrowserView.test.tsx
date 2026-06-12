import { describe, it, expect } from 'vitest';
import { mapClickToViewport } from './BrowserView';

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
