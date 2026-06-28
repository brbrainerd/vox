// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SurfaceMiniRender } from './SurfaceMiniRender';

describe('SurfaceMiniRender', () => {
  it('mounts the provided surface node inside a compact, inert frame', () => {
    render(
      <SurfaceMiniRender surfaceKey="demo" label="Demo">
        <div data-testid="demo-surface-body">live demo content</div>
      </SurfaceMiniRender>,
    );
    // The real child is mounted (no fabrication, no placeholder).
    expect(screen.getByTestId('demo-surface-body')).toBeTruthy();
    // The frame is marked compact + inert for hit-testing.
    const frame = screen.getByTestId('surface-mini-demo');
    expect(frame.getAttribute('data-compact')).toBe('true');
    expect(frame.getAttribute('aria-hidden')).toBe('true');
    // The header shows the surface label so the user knows what they are watching.
    expect(screen.getByText('Demo')).toBeTruthy();
  });
});
