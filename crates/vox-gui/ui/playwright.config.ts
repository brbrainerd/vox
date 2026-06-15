import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  // Only Playwright specs end in .spec.ts; e2e/lib/*.test.ts are vitest unit tests
  // (they import 'vitest') and must NOT be collected by the Playwright runner.
  testMatch: '**/*.spec.ts',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  // `open: 'never'` keeps the HTML artifact without blocking headless/CI runs on a report server.
  reporter: [['html', { open: 'never' }], ['line']],
  use: {
    baseURL: 'http://localhost:1420',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'pnpm run dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
