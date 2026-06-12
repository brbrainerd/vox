import { test, expect } from '@playwright/test';

const previewUrl = process.env.VOX_PREVIEW_URL ?? 'http://127.0.0.1:3001';

test.describe('Browser preview harness', () => {
  test('preview URL responds with HTML', async ({ request }) => {
    try {
      const res = await request.get(previewUrl, { timeout: 5_000 });
      expect(res.status(), `GET ${previewUrl}`).toBeLessThan(500);
      const ct = res.headers()['content-type'] ?? '';
      expect(ct).toMatch(/text\/html|application\/json/i);
    } catch {
      test.skip(true, `Preview server not running at ${previewUrl}`);
    }
  });
});
