import { describe, it, expect, vi, afterEach } from 'vitest';
import { freshnessTone } from './useFreshness';

describe('freshnessTone', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('returns stale when no timestamp and not polling', () => {
    expect(freshnessTone(null)).toBe('stale');
  });

  it('returns poll when no timestamp but polling fallback', () => {
    expect(freshnessTone(null, { usesPolling: true })).toBe('poll');
  });

  it('returns live for recent events', () => {
    vi.spyOn(Date, 'now').mockReturnValue(10_000);
    expect(freshnessTone(9_500, { freshMs: 1000 })).toBe('live');
  });

  it('returns stale for old events', () => {
    vi.spyOn(Date, 'now').mockReturnValue(20_000);
    expect(freshnessTone(5_000, { freshMs: 1000 })).toBe('stale');
  });
});
