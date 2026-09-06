import { execFileSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const UI_ROOT = resolve(__dirname, '../..');

/** 128 KiB — mirrors contracts/budgets/gui-dashboard-chunk.v1.yaml */
export const DASHBOARD_CHUNK_GZIP_BUDGET_BYTES = 128 * 1024;

/**
 * Incremental gzip bytes attributable to dashboard chart/grid deps.
 * Spawns a clean Node process so Vitest's transform pipeline does not skew Vite output.
 */
export function measureDashboardChunkGzipBytes(): number {
  const stdout = execFileSync(
    process.execPath,
    [
      '--experimental-strip-types',
      '--input-type=module',
      '-e',
      "import { measureDashboardChunkGzipBytesInProcess } from './src/lib/dashboardBundleBudgetInProcess.ts'; console.log(await measureDashboardChunkGzipBytesInProcess());",
    ],
    {
      cwd: UI_ROOT,
      encoding: 'utf8',
      env: { ...process.env, NODE_ENV: 'production' },
    },
  );

  const bytes = Number(stdout.trim());
  if (!Number.isFinite(bytes) || bytes <= 0) {
    throw new Error(`invalid dashboard bundle measurement output: ${stdout}`);
  }

  return bytes;
}
