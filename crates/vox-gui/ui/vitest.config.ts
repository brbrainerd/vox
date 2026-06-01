import { defineConfig } from 'vitest/config';

// Unit tests live under src/ (vitest). The e2e/ Playwright specs are run
// separately via `pnpm test:e2e` and must NOT be collected by vitest.
export default defineConfig({
  test: {
    include: ['src/**/*.{test,spec}.ts'],
    exclude: ['e2e/**', 'node_modules/**', 'dist/**'],
  },
});
