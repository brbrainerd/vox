import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

// Resolve relative to THIS test file (robust regardless of vitest cwd):
// src/__tests__ -> src -> ui -> vox-gui/tauri.conf.json
const here = dirname(fileURLToPath(import.meta.url));
const conf = JSON.parse(
  readFileSync(resolve(here, '../../../tauri.conf.json'), 'utf8'),
);

describe('Axis branding — tauri config', () => {
  it('window title is "Axis"', () => {
    expect(conf.app.windows[0].title).toBe('Axis');
  });
  it('productName and identifier are unchanged (brand-layer only)', () => {
    expect(conf.productName).toBe('Vox');
    expect(conf.identifier).toBe('org.vox-foundation.gui');
  });
});
