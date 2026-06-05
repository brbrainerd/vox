import { defineConfig, devices } from '@playwright/test';

// Screenshot-sweep config: targets the already-running Vite dev server on the
// project's real port (1420, the Tauri default — the committed playwright.config
// uses 5173 which this project does not use). No webServer block; the dev server
// is managed externally during the visual audit.
export default defineConfig({
  testDir: './e2e',
  testMatch: 'screenshots.spec.ts',
  fullyParallel: true,
  reporter: 'line',
  timeout: 60000,
  use: {
    baseURL: 'http://localhost:1420',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
});
