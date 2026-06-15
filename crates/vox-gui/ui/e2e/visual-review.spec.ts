import { test, expect } from '@playwright/test';
import { readFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { installTauriMock } from './lib/tauriMock';
import { sha256Png, buildManifest, writeManifest, type ManifestEntry } from './lib/screenshotManifest';
import { SURFACE_REGISTRY } from '../src/generated/surfaceRegistry.generated';

// ESM-safe __dirname (the ui package is "type": "module").
const __dirname = dirname(fileURLToPath(import.meta.url));
const OUT_DIR = join(__dirname, 'screens');
const VIEWS = Array.from(new Set(
  SURFACE_REGISTRY.filter((e) => e.viewKey != null && e.tier !== 'none').map((e) => e.viewKey as string),
)).sort();

test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });

test('visual-review: capture every surface + emit manifest', async ({ browser }, testInfo) => {
  // One test captures every surface sequentially (~30+ contexts), so the default
  // 30s per-test budget is far too small. Scale generously with the surface count.
  test.setTimeout(Math.max(120_000, VIEWS.length * 15_000));
  mkdirSync(OUT_DIR, { recursive: true });
  const entries: ManifestEntry[] = [];
  const sweepStart = Date.now();
  for (const view of VIEWS) {
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
    const page = await context.newPage();
    await page.addInitScript(installTauriMock, view);
    const t0 = Date.now();
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });
    await page.waitForLoadState('networkidle').catch(() => {});
    const file = `${view}.png`;
    await page.screenshot({ path: join(OUT_DIR, file), fullPage: true });
    const captureMs = Date.now() - t0;
    const buf = readFileSync(join(OUT_DIR, file));
    const { width, height } = await page.evaluate(() => ({ width: document.documentElement.scrollWidth, height: document.documentElement.scrollHeight }));
    entries.push({ viewKey: view, file, sha256: sha256Png(buf), bytes: buf.length, width, height, captureMs });
    await context.close();
  }
  const manifest = buildManifest(entries, Date.now() - sweepStart, sweepStart);
  const manifestPath = writeManifest(OUT_DIR, manifest);
  testInfo.attachments.push({ name: 'manifest.json', path: manifestPath, contentType: 'application/json' });
  expect(manifest.surfaces.length).toBeGreaterThan(0);
});
