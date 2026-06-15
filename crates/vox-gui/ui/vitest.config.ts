import { defineConfig } from 'vitest/config';

// Unit tests live under src/ (vitest). The e2e/ Playwright specs are run
// separately via `pnpm test:e2e` and must NOT be collected by vitest.
export default defineConfig({
  test: {
    include: ['src/**/*.{test,spec}.{ts,tsx}', 'e2e/lib/**/*.test.ts'],
    // Exclude the Playwright specs (e2e/*.spec.ts) but allow the pure unit-tested
    // e2e/lib helpers above to be collected by vitest.
    exclude: ['e2e/*.spec.ts', 'node_modules/**', 'dist/**'],
    setupFiles: ['src/test-setup.ts'],
  },
});
