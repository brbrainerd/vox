import { describe, it, expect } from 'vitest';
import { sha256Png, buildManifest, type ManifestEntry } from './screenshotManifest';

describe('screenshotManifest', () => {
  it('sha256Png is deterministic and 64 hex chars', () => {
    const a = sha256Png(Buffer.from('fake-png-bytes'));
    const b = sha256Png(Buffer.from('fake-png-bytes'));
    expect(a).toBe(b);
    expect(a).toMatch(/^[0-9a-f]{64}$/);
  });
  it('different bytes produce different hashes', () => {
    expect(sha256Png(Buffer.from('a'))).not.toBe(sha256Png(Buffer.from('b')));
  });
  it('buildManifest sorts entries by viewKey and stamps schema_version', () => {
    const entries: ManifestEntry[] = [
      { viewKey: 'settings', file: 'settings.png', sha256: 'x'.repeat(64), bytes: 10, width: 2880, height: 1800, captureMs: 120 },
      { viewKey: 'agents', file: 'agents.png', sha256: 'y'.repeat(64), bytes: 20, width: 2880, height: 1800, captureMs: 90 },
    ];
    const m = buildManifest(entries, 42);
    expect(m.schema_version).toBe(1);
    expect(m.total_capture_ms).toBe(42);
    expect(m.surfaces.map((s) => s.viewKey)).toEqual(['agents', 'settings']);
  });
});
