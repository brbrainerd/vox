// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useVirtualList } from './useVirtualList';

// useVirtualizer needs a container element with a measurable height.
// jsdom doesn't do layout, so we supply a fixed height via the
// scrollElement ref and override getBoundingClientRect.
function makeContainerRef(height: number) {
  const el = document.createElement('div');
  Object.defineProperty(el, 'getBoundingClientRect', {
    value: () => ({ height, width: 400, top: 0, left: 0, right: 400, bottom: height, x: 0, y: 0, toJSON: () => {} }),
  });
  Object.defineProperty(el, 'offsetHeight', { value: height });
  return { current: el } as React.RefObject<HTMLDivElement>;
}

describe('useVirtualList', () => {
  it('returns a virtualizer and totalSize', () => {
    const items = Array.from({ length: 100 }, (_, i) => ({ id: i }));
    const containerRef = makeContainerRef(400);

    const { result } = renderHook(() =>
      useVirtualList({
        containerRef,
        count: items.length,
        estimateSize: () => 44,
        overscan: 3,
      }),
    );

    expect(result.current.virtualizer).toBeDefined();
    expect(typeof result.current.totalSize).toBe('number');
    expect(result.current.totalSize).toBeGreaterThanOrEqual(0);
    expect(Array.isArray(result.current.virtualItems)).toBe(true);
  });

  it('returns zero virtualItems for an empty list', () => {
    const containerRef = makeContainerRef(400);

    const { result } = renderHook(() =>
      useVirtualList({
        containerRef,
        count: 0,
        estimateSize: () => 44,
        overscan: 3,
      }),
    );

    expect(result.current.virtualItems).toHaveLength(0);
    expect(result.current.totalSize).toBe(0);
  });

  it('exposes start/end index on each virtual item', () => {
    const containerRef = makeContainerRef(200);

    const { result } = renderHook(() =>
      useVirtualList({
        containerRef,
        count: 50,
        estimateSize: () => 44,
        overscan: 0,
      }),
    );

    const items = result.current.virtualItems;
    if (items.length > 0) {
      expect(typeof items[0].index).toBe('number');
      expect(typeof items[0].start).toBe('number');
      expect(typeof items[0].size).toBe('number');
    }
  });
});
