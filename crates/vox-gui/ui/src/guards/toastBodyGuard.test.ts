import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

/** F-03 guard: toast bodies AND non-toast error display state must go through
 * sanitizeErrorForToast, never raw String(err) — a raw Tauri TypeError's text
 * contains __TAURI_INTERNALS__. Matches `body: String(` (toast payloads),
 * `setFoo(String(` (state setters rendering raw error text, e.g.
 * setValidateOut/setDispatchResult/setIsolationError/setOutput), and bare
 * `String(e)`/`String(err)`/`String(error)` anywhere (catches the
 * `e.message || String(e)` shape found in NeedsYouSurface.tsx, which the
 * original two patterns missed since String(...) wasn't the first token after
 * `body:`/`setX(`). Requires a capital-letter setter name (`set[A-Z]`) so this
 * doesn't over-match unrelated `set(String(...))`-shaped calls; the bare form
 * is restricted to common catch-variable names, not any String(...) call, so
 * String(cmd.id)-style non-error coercions are unaffected. */
const LEAK_SINK_PATTERN = /(?:body:\s*|\bset[A-Z]\w*\()\s*String\(|\bString\((?:e|err|error)\)/;
/** Escape hatch for genuinely non-user-facing String(err) uses (e.g. a
 * backend telemetry payload) — mark the line `// gui-safe: <reason>`. */
const SAFE_MARKER = /\/\/\s*gui-safe:/;
const SRC_ROOT = join(import.meta.dirname, '..');
// The sanitizer's own implementation legitimately calls String(err) — it IS
// the thing every other site is required to route through.
const EXEMPT_FILES = new Set(['src/lib/backendGuard.ts']);

function* walk(dir: string): Generator<string> {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walk(p);
    else if (/\.(ts|tsx)$/.test(name) && !/\.test\.(ts|tsx)$/.test(name)) yield p;
  }
}

describe('toast body sanitization containment', () => {
  it('no raw `body: String(`, `setFoo(String(`, or bare `String(e|err|error)` anywhere under src/ (unless marked `// gui-safe:`)', () => {
    const offenders: string[] = [];
    for (const file of walk(SRC_ROOT)) {
      const relPath = file.replace(SRC_ROOT, 'src').replace(/\\/g, '/');
      if (EXEMPT_FILES.has(relPath)) continue;
      const lines = readFileSync(file, 'utf8').split('\n');
      for (const line of lines) {
        if (LEAK_SINK_PATTERN.test(line) && !SAFE_MARKER.test(line)) {
          offenders.push(relPath);
          break;
        }
      }
    }
    expect(offenders, `use sanitizeErrorForToast instead (or mark the line // gui-safe: <reason> if it's genuinely not user-facing): ${offenders.join(', ')}`).toEqual([]);
  });
});
