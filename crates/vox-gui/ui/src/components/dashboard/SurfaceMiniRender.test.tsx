// @vitest-environment jsdom
import { describe, expect, it, vi, afterEach } from 'vitest';
import React, { useEffect } from 'react';
import { render, screen } from '@testing-library/react';
import { SurfaceMiniRender } from './SurfaceMiniRender';
import { useIsEmbeddedSurface } from './EmbeddedSurfaceContext';

/**
 * A probe surface that uses the SAME embedded-gate pattern every real polling
 * surface uses: one initial fetch, then a repeating poll that is skipped when
 * embedded.
 */
function PollingProbe({ onFetch }: { onFetch: () => void }) {
  const embedded = useIsEmbeddedSurface();
  useEffect(() => {
    onFetch(); // initial fetch (allowed even when embedded)
    if (embedded) return;
    const id = setInterval(onFetch, 1000);
    return () => clearInterval(id);
  }, [embedded, onFetch]);
  return <div>probe (embedded={String(embedded)})</div>;
}

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

  describe('embedded surfaces suppress polling', () => {
    afterEach(() => {
      vi.useRealTimers();
      vi.restoreAllMocks();
    });

    it('provides embedded=true to children so they skip their poll interval', () => {
      vi.useFakeTimers();
      const setIntervalSpy = vi.spyOn(globalThis, 'setInterval');
      const onFetch = vi.fn();
      render(
        <SurfaceMiniRender surfaceKey="probe" label="Probe">
          <PollingProbe onFetch={onFetch} />
        </SurfaceMiniRender>,
      );
      // Initial fetch ran once; NO repeating interval was scheduled.
      expect(onFetch).toHaveBeenCalledTimes(1);
      expect(setIntervalSpy).not.toHaveBeenCalled();
      // Advancing time well past the poll period does NOT trigger more fetches.
      vi.advanceTimersByTime(5000);
      expect(onFetch).toHaveBeenCalledTimes(1);
      expect(screen.getByText('probe (embedded=true)')).toBeTruthy();
    });

    it('the same probe DOES poll when NOT embedded (control)', () => {
      vi.useFakeTimers();
      const setIntervalSpy = vi.spyOn(globalThis, 'setInterval');
      const onFetch = vi.fn();
      render(<PollingProbe onFetch={onFetch} />);
      expect(setIntervalSpy).toHaveBeenCalled();
      vi.advanceTimersByTime(3000);
      // initial + 3 interval ticks.
      expect(onFetch).toHaveBeenCalledTimes(4);
      expect(screen.getByText('probe (embedded=false)')).toBeTruthy();
    });
  });
});
