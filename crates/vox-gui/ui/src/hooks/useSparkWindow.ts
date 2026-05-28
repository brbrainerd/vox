import { useState, useEffect, useRef } from 'react';

/** Maximum samples to keep in the rolling window (one per tick). */
const WINDOW_SAMPLES = 150; // 150 × 2s ticks ≈ 5 minutes

/** Rolling window with a fixed capacity that never re-allocates. */
export function useSparkWindow(value: number, maxSamples: number = WINDOW_SAMPLES): number[] {
  const windowRef = useRef<number[]>([]);

  useEffect(() => {
    const w = windowRef.current;
    if (w.length >= maxSamples) w.shift();
    w.push(value);
  }, [value, maxSamples]);

  return windowRef.current.slice();
}

/**
 * Aggregate multiple metrics into 5-minute rolling sparkline data.
 * Returns the current window and a helper to add a new data point.
 */
export function use5MinWindow(initial: number[] = []): [number[], (v: number) => void] {
  const [window, setWindow] = useState<number[]>(() => {
    // Seed with the supplied initial array (may come from persisted state).
    return initial.slice(-WINDOW_SAMPLES);
  });

  const push = (v: number) => {
    setWindow(prev => {
      const next = [...prev, v];
      return next.length > WINDOW_SAMPLES ? next.slice(-WINDOW_SAMPLES) : next;
    });
  };

  return [window, push];
}

/**
 * Persist the rolling window to localStorage so sparklines survive a page
 * reload / Tauri window close.  Key should be unique per metric.
 */
export function usePersistedSparkWindow(
  key: string,
  liveValue: number
): number[] {
  const STORAGE_KEY = `vox.spark.${key}`;
  const MAX = WINDOW_SAMPLES;

  const [window, setWindow] = useState<number[]>(() => {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      return raw ? JSON.parse(raw) : [];
    } catch {
      return [];
    }
  });

  useEffect(() => {
    setWindow(prev => {
      const next = [...prev, liveValue];
      const trimmed = next.length > MAX ? next.slice(-MAX) : next;
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(trimmed));
      } catch {
        // Quota exceeded — silently discard.
      }
      return trimmed;
    });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [liveValue]);

  return window;
}
