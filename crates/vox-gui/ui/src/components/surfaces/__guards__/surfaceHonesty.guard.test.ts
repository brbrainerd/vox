import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { scanSource } from './honestyScan';
import { HIDDEN_ALLOWLIST } from './honestyScan.allowlist';

const ROOT = 'src/components/surfaces';
// Skip tests AND *.unfinished.tsx — the HIDE mechanism moves not-yet-wired markup
// into a sibling `<Name>.unfinished.tsx` that shipped code never imports.
function walk(d: string): string[] {
  return readdirSync(d).flatMap(n => {
    const p = join(d, n);
    return statSync(p).isDirectory() ? walk(p)
      : p.endsWith('.tsx') && !p.endsWith('.test.tsx') && !p.endsWith('.unfinished.tsx') ? [p] : [];
  });
}
const allowed = (f: string, l: number) =>
  HIDDEN_ALLOWLIST.some(a => f.endsWith(a.file) && a.line === l);

describe('surface honesty guard', () => {
  it('no placeholder text or dead handlers in shipped surfaces', () => {
    const violations = walk(ROOT)
      .flatMap(f => scanSource(f, readFileSync(f, 'utf8')))
      .filter(v => !allowed(v.file, v.line));
    expect(violations, JSON.stringify(violations, null, 2)).toHaveLength(0);
  });
});
