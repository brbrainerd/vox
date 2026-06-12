import { useEffect, useState } from 'react';
import { LIVE_EVENT_FRESH_MS } from '../config/constants';

export type FreshnessTone = 'live' | 'poll' | 'stale';

export interface FreshnessOptions {
  freshMs?: number;
  usesPolling?: boolean;
}

/** Pure freshness classifier (HUD live indicator, panel staleness badges). */
export function freshnessTone(
  lastAt: number | null | undefined,
  options?: FreshnessOptions,
): FreshnessTone {
  const freshMs = options?.freshMs ?? LIVE_EVENT_FRESH_MS;
  const usesPolling = options?.usesPolling ?? false;
  if (lastAt == null) return usesPolling ? 'poll' : 'stale';
  const age = Date.now() - lastAt;
  if (age <= freshMs) return usesPolling ? 'poll' : 'live';
  return 'stale';
}

/**
 * Re-evaluates freshness on an interval so UI transitions from live → stale without
 * requiring unrelated state updates.
 */
export function useFreshness(
  lastAt: number | null | undefined,
  options?: FreshnessOptions,
): FreshnessTone {
  const freshMs = options?.freshMs ?? LIVE_EVENT_FRESH_MS;
  const usesPolling = options?.usesPolling ?? false;
  const [tone, setTone] = useState<FreshnessTone>(() =>
    freshnessTone(lastAt, { freshMs, usesPolling }),
  );

  useEffect(() => {
    const tick = () => setTone(freshnessTone(lastAt, { freshMs, usesPolling }));
    tick();
    const id = window.setInterval(tick, Math.min(freshMs, 5000));
    return () => window.clearInterval(id);
  }, [lastAt, freshMs, usesPolling]);

  return tone;
}
