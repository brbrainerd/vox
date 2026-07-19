import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

/** F-03 guard: toast bodies AND non-toast error display state must go through
 * sanitizeErrorForToast, never raw String(err) — a raw Tauri TypeError's text
 * contains __TAURI_INTERNALS__. Matches both `body: String(` (toast payloads)
 * and `setFoo(String(` (state setters rendering raw error text, e.g.
 * setValidateOut/setDispatchResult/setIsolationError/setOutput). Requires a
 * capital-letter setter name (`set[A-Z]`) so this doesn't over-match unrelated
 * `set(String(...))`-shaped calls. */
const LEAK_SINK_PATTERN = /(?:body:\s*|\bset[A-Z]\w*\()\s*String\(/;
const SRC_ROOT = join(import.meta.dirname, '..');

function* walk(dir: string): Generator<string> {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walk(p);
    else if (/\.(ts|tsx)$/.test(name) && !/\.test\.(ts|tsx)$/.test(name)) yield p;
  }
}

describe('toast body sanitization containment', () => {
  it('no raw `body: String(` or `setFoo(String(` anywhere under src/', () => {
    const offenders: string[] = [];
    for (const file of walk(SRC_ROOT)) {
      const src = readFileSync(file, 'utf8');
      if (LEAK_SINK_PATTERN.test(src)) offenders.push(file.replace(SRC_ROOT, 'src'));
    }
    expect(offenders, `use sanitizeErrorForToast instead: ${offenders.join(', ')}`).toEqual([]);
  });
});
