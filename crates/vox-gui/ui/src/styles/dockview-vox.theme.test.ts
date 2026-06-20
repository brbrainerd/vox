import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const css = readFileSync(fileURLToPath(new URL('./dockview-vox.css', import.meta.url)), 'utf8');

describe('dockview-vox theme', () => {
  it('sets the active tab and drop-target to the brass accent token', () => {
    expect(css).toMatch(/--dv-activegroup-visiblepanel-tab-background-color/);
    expect(css).toMatch(/var\(--brass\)|rgb\(var\(--brass\)/);
  });
});
