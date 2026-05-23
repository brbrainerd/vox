import { test, expect } from '@playwright/test';
import { writeFileSync, mkdirSync } from 'node:fs';

test.setTimeout(120_000);

test('baseline: capture current vox-lang.org state', async ({ page, request }) => {
  // Direct response headers (no redirect follow, short timeout)
  const headResp = await request.get('https://vox-lang.org/', { maxRedirects: 0, timeout: 10_000 });
  const head = `Status: ${headResp.status()}\nHeaders: ${JSON.stringify(headResp.headers(), null, 2)}`;

  // Page render — wait for DOM ready, not full network idle (pagefind keeps requesting)
  await page.goto('https://vox-lang.org/', { waitUntil: 'domcontentloaded', timeout: 60_000 });
  const title = await page.title();

  // Starlight uses <starlight-toc> + <nav class="sidebar"> structures.
  // Try several selectors; use textContent (sync) rather than innerText to avoid layout waits.
  const sidebarHandle = page
    .locator('starlight-sidebar, nav[aria-label*="Main" i], aside nav, nav.sidebar, [class*="sidebar" i]')
    .first();
  const sidebarText = await sidebarHandle.textContent({ timeout: 5_000 }).catch(() => '(no sidebar matched)');
  const h1 = await page.locator('h1').first().textContent({ timeout: 5_000 }).catch(() => '(no h1)');

  const snapshot = [
    '=== HEAD ===', head,
    '=== TITLE ===', title,
    '=== H1 ===', h1 ?? '(null)',
    '=== SIDEBAR (truncated to 4000 chars) ===',
    (sidebarText ?? '').slice(0, 4000),
  ].join('\n\n');

  mkdirSync('test-results', { recursive: true });
  writeFileSync('test-results/baseline-snapshot.txt', snapshot, 'utf8');

  // Sanity: site is live today
  expect(headResp.status()).toBe(200);
  expect(title.toLowerCase()).toContain('vox');
});
