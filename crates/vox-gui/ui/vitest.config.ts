import { defineConfig } from 'vitest/config';

// Unit tests live under src/ (vitest). The e2e/ Playwright specs are run
// separately via `pnpm test:e2e` and must NOT be collected by vitest.
export default defineConfig({
  test: {
    include: ['src/**/*.{test,spec}.{ts,tsx}', 'e2e/lib/**/*.test.ts', 'e2e/review/**/*.test.ts'],
    // Exclude the Playwright specs (e2e/*.spec.ts) but allow the pure unit-tested
    // e2e/lib helpers above to be collected by vitest.
    exclude: ['e2e/*.spec.ts', 'node_modules/**', 'dist/**'],
    setupFiles: ['src/test-setup.ts'],
    // Vitest's 5s default measures wall-clock, not time spent executing. Each
    // worker carries its own jsdom environment, so on a memory-constrained host
    // a full parallel run spends most of that budget waiting rather than
    // running: files that fail here finish in well under a second when run
    // alone (IsolationPanel 537ms, PoliciesView 632ms). Measured on a 16 GB
    // machine with ~2 GB free, where the same pressure OOM-killed a release
    // cargo build outright. 20s keeps genuine hangs failing fast while
    // surviving a full run on a host that is short on memory.
    testTimeout: 20_000,
  },
});
