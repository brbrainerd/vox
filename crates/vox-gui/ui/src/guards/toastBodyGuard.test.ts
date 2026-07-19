import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

/** F-03 guard: toast bodies must go through sanitizeErrorForToast, never raw
 * String(err) — a raw Tauri TypeError's text contains __TAURI_INTERNALS__. */
const SRC_ROOT = join(import.meta.dirname, '..');

function* walk(dir: string): Generator<string> {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walk(p);
    else if (/\.(ts|tsx)$/.test(name) && !/\.test\.(ts|tsx)$/.test(name)) yield p;
  }
}

describe('toast body sanitization containment', () => {
  it('no raw `body: String(` anywhere under src/', () => {
    const offenders: string[] = [];
    for (const file of walk(SRC_ROOT)) {
      const src = readFileSync(file, 'utf8');
      if (/body:\s*String\(/.test(src)) offenders.push(file.replace(SRC_ROOT, 'src'));
    }
    expect(offenders, `use sanitizeErrorForToast instead: ${offenders.join(', ')}`).toEqual([]);
  });
});
