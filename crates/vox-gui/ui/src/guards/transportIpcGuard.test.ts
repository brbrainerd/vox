import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/**
 * Phase A guard: inside transport.ts, only the marked region may touch raw
 * Tauri IPC. (Direct imports in components are separate tracked debt:
 * ipcBoundaries.test.ts allowlist for `invoke`; `listen` imports are
 * covered user-visibly by backendGuard's rejection filter.)
 */
const SRC = readFileSync(join(import.meta.dirname, '../transport.ts'), 'utf8');

describe('transport raw-IPC containment', () => {
  const begin = SRC.indexOf('// __VOX_RAW_IPC_BEGIN__');
  const end = SRC.indexOf('// __VOX_RAW_IPC_END__');

  it('has exactly one marked raw-IPC region', () => {
    expect(begin).toBeGreaterThan(-1);
    expect(end).toBeGreaterThan(begin);
    expect(SRC.indexOf('// __VOX_RAW_IPC_BEGIN__', begin + 1)).toBe(-1);
  });

  it('no raw invoke( / invoke< / listen( / listen< outside the marked region', () => {
    const outside = SRC.slice(0, begin) + SRC.slice(end);
    const offenders = [...outside.matchAll(/(?<![A-Za-z_$.])(invoke|listen)\s*[(<]/g)].map((m) => m[0]);
    expect(offenders, `raw IPC outside safe wrappers: ${JSON.stringify(offenders)}`).toEqual([]);
  });
});
