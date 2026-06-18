// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import React from 'react';
import { Sparkline } from './Sparkline';

describe('Sparkline', () => {
  it('returns null when data is empty', () => {
    const { container } = render(<Sparkline data={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders a single <svg> with a stroke path and a circle when data is non-empty', () => {
    const { container } = render(<Sparkline data={[1, 2, 3, 4]} />);
    const svg = container.querySelector('svg');
    expect(svg).not.toBeNull();
    expect(container.querySelectorAll('path').length).toBeGreaterThan(0);
    expect(container.querySelector('circle')).not.toBeNull();
  });

  // B1 — bug fix: two Sparklines with identical data must use distinct
  // gradient <defs> ids. The old deterministic hash collided, causing the
  // second sparkline to render in the first's color.
  it('uses a unique gradient id per Sparkline instance, even with identical data', () => {
    const { container } = render(
      <>
        <Sparkline data={[1, 2, 3, 4]} color="#ff0000" />
        <Sparkline data={[1, 2, 3, 4]} color="#00ff00" />
      </>,
    );
    const grads = Array.from(container.querySelectorAll('linearGradient'));
    expect(grads.length).toBe(2);
    const ids = grads.map((g) => g.getAttribute('id'));
    expect(ids[0]).toBeTruthy();
    expect(ids[1]).toBeTruthy();
    expect(ids[0]).not.toBe(ids[1]);
  });

  // B23 — bug fix: when every value is the same (range = 0), the line
  // collapsed to a flat line at the bottom with the dot still visible.
  // The new behavior renders only the dot (no line/area path), so the
  // user sees a single "stable" marker instead of a misleading flat line.
  it('renders only a dot when all data values are identical', () => {
    const { container } = render(<Sparkline data={[5, 5, 5, 5]} />);
    // A constant series has no meaningful line to draw — only the dot.
    expect(container.querySelectorAll('path').length).toBe(0);
    expect(container.querySelector('circle')).not.toBeNull();
  });

  it('still renders a path and circle for non-constant data', () => {
    const { container } = render(<Sparkline data={[1, 2, 3, 4]} />);
    expect(container.querySelectorAll('path').length).toBeGreaterThan(0);
    expect(container.querySelector('circle')).not.toBeNull();
  });
});
