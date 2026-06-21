import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const read = (p: string) => readFileSync(resolve(here, p), 'utf8');

// The brand accent is already a shared, themeable token (`--brass`, switched in
// index.css). The mark must consume tokens (currentColor + the bg-base token), never
// hardcode a hex — otherwise it would (a) drift from the palette and (b) break theming.
describe('brand surfaces use tokens, not hardcoded hexes', () => {
  it('AxisMark uses currentColor + the bg-base token, with no literal hex color', () => {
    const src = read('./AxisMark.tsx');
    expect(src).toMatch(/currentColor/);
    expect(src).toMatch(/fill-bg-base/);
    // no literal 6-digit hex colors in the component
    expect(src).not.toMatch(/#[0-9a-fA-F]{6}/);
  });
});
