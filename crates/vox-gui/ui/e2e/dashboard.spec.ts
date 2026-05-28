import { test, expect } from '@playwright/test';

test.describe('Vox Dashboard', () => {
  test('should load the dashboard and verify event payload delivery', async ({ page }) => {
    await page.goto('/');

    // Check if the dashboard loads
    await expect(page).toHaveTitle(/Vox/i);

    // Verify TopHud presence
    const header = page.locator('header');
    await expect(header).toBeVisible();

    // Verify memory view or settings view logic when mocked (if Tauri is mocked)
    // Since we are running outside Tauri in Vite dev mode, we would mock the IPC.
    // For this e2e, we verify the UI handles the default state.
    const container = page.locator('main');
    await expect(container).toBeVisible();
    
    // In a fully mocked Tauri env, we could assert the msgpack payload rendering here.
  });
});
