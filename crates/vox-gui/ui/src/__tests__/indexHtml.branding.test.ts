import { describe, it, expect } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

// src/__tests__ -> src -> ui
const here = dirname(fileURLToPath(import.meta.url));
const html = readFileSync(resolve(here, '../../index.html'), 'utf8');

describe('Axis branding — index.html', () => {
  it('document title is Axis', () => {
    expect(html).toMatch(/<title>Axis<\/title>/);
  });
  it('links the favicon', () => {
    expect(html).toMatch(/rel="icon"[^>]*href="\/favicon\.svg"/);
  });
  it('ships the favicon asset', () => {
    expect(existsSync(resolve(here, '../../public/favicon.svg'))).toBe(true);
  });
});
