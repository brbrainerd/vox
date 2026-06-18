/**
 * Visual audit report generator.
 *
 * Reads all *.png files from e2e/screens/ and writes a dark-themed browsable
 * HTML grid to e2e/screens/audit-report.html. Run after any screenshot sweep:
 *
 *   pnpm exec playwright test screenshots-audit-report.spec.ts --project=chromium
 *
 * Then open:
 *   start crates\vox-gui\ui\e2e\screens\audit-report.html
 */
import { test, expect } from '@playwright/test';
import { readdirSync, writeFileSync, existsSync } from 'node:fs';
import { join, basename, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SCREENS_DIR = join(__dirname, 'screens');
const OUT_PATH = join(SCREENS_DIR, 'audit-report.html');

type StateVariant = 'base' | 'empty' | 'error' | 'special';

function classifyPng(name: string): { surface: string; variant: StateVariant } {
  if (name.startsWith('_')) return { surface: name, variant: 'special' };
  if (name.endsWith('-empty')) return { surface: name.replace(/-empty$/, ''), variant: 'empty' };
  if (name.endsWith('-error')) return { surface: name.replace(/-error$/, ''), variant: 'error' };
  return { surface: name, variant: 'base' };
}

const VARIANT_BADGE: Record<StateVariant, { bg: string; color: string; label: string }> = {
  base:    { bg: '#1f6feb22', color: '#1f6feb', label: 'base' },
  empty:   { bg: '#388bfd22', color: '#58a6ff', label: 'empty' },
  error:   { bg: '#f8514922', color: '#f85149', label: 'error' },
  special: { bg: '#8b949e22', color: '#8b949e', label: 'special' },
};

test('generate visual audit report', async () => {
  if (!existsSync(SCREENS_DIR)) {
    throw new Error(`screens/ not found at ${SCREENS_DIR}. Run screenshot specs first.`);
  }
  const pngs = readdirSync(SCREENS_DIR).filter((f) => f.endsWith('.png')).sort();
  expect(pngs.length, 'No PNGs in e2e/screens/ — run screenshot specs first').toBeGreaterThan(0);

  const groups = new Map<string, { file: string; variant: StateVariant }[]>();
  for (const file of pngs) {
    const { surface, variant } = classifyPng(basename(file, '.png'));
    if (!groups.has(surface)) groups.set(surface, []);
    groups.get(surface)!.push({ file, variant });
  }

  const cards = Array.from(groups.entries())
    .sort(([a], [b]) => a.localeCompare(b))
    .flatMap(([, entries]) =>
      entries.map(({ file, variant }) => {
        const { bg, color, label } = VARIANT_BADGE[variant];
        const name = basename(file, '.png');
        return `
<figure>
  <span class="badge" style="background:${bg};color:${color};border:1px solid ${color}44">${label}</span>
  <a href="${file}" target="_blank"><img src="${file}" loading="lazy" alt="${name}"></a>
  <figcaption>${name}</figcaption>
</figure>`;
      }),
    )
    .join('\n');

  const html = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Vox GUI Visual Audit</title>
  <style>
    *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
    body { background: #0d1117; color: #e6edf3; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; padding: 1.5rem; }
    header { margin-bottom: 1.5rem; border-bottom: 1px solid #21262d; padding-bottom: 1rem; }
    h1 { font-size: 1.4rem; color: #58a6ff; margin-bottom: 0.35rem; }
    .meta { font-size: 0.82rem; color: #8b949e; }
    .legend { display: flex; gap: 0.75rem; margin-top: 0.75rem; flex-wrap: wrap; }
    .legend-item { display: flex; align-items: center; gap: 0.35rem; font-size: 0.78rem; color: #8b949e; }
    .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 0.875rem; }
    figure { background: #161b22; border: 1px solid #30363d; border-radius: 8px; overflow: hidden; transition: border-color 0.15s; }
    figure:hover { border-color: #58a6ff55; }
    figure a { display: block; }
    figure img { width: 100%; display: block; border-bottom: 1px solid #21262d; }
    figcaption { padding: 6px 10px 8px; font-size: 12px; font-weight: 500; color: #c9d1d9; }
    .badge { font-size: 10px; font-weight: 600; padding: 2px 7px; border-radius: 20px; margin: 7px 10px 0; display: inline-block; }
  </style>
</head>
<body>
  <header>
    <h1>Vox GUI Visual Audit</h1>
    <p class="meta">${pngs.length} screenshots · ${groups.size} surfaces · ${new Date().toISOString()}</p>
    <div class="legend">
      <span class="legend-item"><span class="badge" style="background:#1f6feb22;color:#1f6feb;border:1px solid #1f6feb44">base</span> default populated state</span>
      <span class="legend-item"><span class="badge" style="background:#388bfd22;color:#58a6ff;border:1px solid #388bfd44">empty</span> all lists empty</span>
      <span class="legend-item"><span class="badge" style="background:#f8514922;color:#f85149;border:1px solid #f8514944">error</span> IPC failures injected</span>
      <span class="legend-item"><span class="badge" style="background:#8b949e22;color:#8b949e;border:1px solid #8b949e44">special</span> sidebar / palette</span>
    </div>
  </header>
  <div class="grid">${cards}</div>
</body>
</html>`;

  writeFileSync(OUT_PATH, html, 'utf-8');
  console.log(`\n✅ Audit report: ${OUT_PATH}`);
  console.log(`   Open with: start "${OUT_PATH}"`);
});
