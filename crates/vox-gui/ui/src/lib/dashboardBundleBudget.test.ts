import { describe, it, expect } from 'vitest';
import {
  DASHBOARD_CHUNK_GZIP_BUDGET_BYTES,
  measureDashboardChunkGzipBytes,
} from './dashboardBundleBudget';

/**
 * Bundle budget gate for Phase 2.1 — dashboard chart/grid dependencies.
 *
 * Measurement (see contracts/budgets/gui-dashboard-chunk.v1.yaml):
 * - Two Vite production builds (baseline vs full dashboard chunk entry)
 * - react / react-dom externalized (incremental lazy chunk beyond app shell)
 * - es2022 target, esbuild minify (matches vite.config.ts production defaults)
 * - Incremental gzip = full chunk gzip − baseline chunk gzip (layout-only helpers)
 * - Measurement runs in a child Node process (Vitest-safe)
 *
 * Budget: dashboard chart/grid gzip delta < 128 KiB (131_072 bytes).
 */
describe('dashboard bundle budget', () => {
  it('dashboard chart/grid gzip delta stays under 128 KiB (react externalized)', () => {
    const gzipBytes = measureDashboardChunkGzipBytes();

    expect(
      gzipBytes,
      `dashboard chart/grid gzip delta ${gzipBytes} B exceeds budget ${DASHBOARD_CHUNK_GZIP_BUDGET_BYTES} B`,
    ).toBeLessThan(DASHBOARD_CHUNK_GZIP_BUDGET_BYTES);
  }, 180_000);
});
