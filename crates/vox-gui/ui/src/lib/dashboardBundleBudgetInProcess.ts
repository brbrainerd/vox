import { build } from 'vite';
import react from '@vitejs/plugin-react';
import { gzipSync } from 'node:zlib';
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

const DASHBOARD_CHUNK_ENTRY = resolve(__dirname, '../test-fixtures/dashboard-chunk-entry.ts');
const DASHBOARD_BASELINE_ENTRY = resolve(
  __dirname,
  '../test-fixtures/dashboard-chunk-baseline-entry.ts',
);

const REACT_EXTERNALS = ['react', 'react-dom', 'react/jsx-runtime'];

function collectJsFiles(dir: string): string[] {
  const entries = readdirSync(dir, { withFileTypes: true });
  const files: string[] = [];

  for (const entry of entries) {
    const path = resolve(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectJsFiles(path));
      continue;
    }
    if (entry.isFile() && entry.name.endsWith('.js')) {
      files.push(path);
    }
  }

  return files;
}

function gzipBytesForJsTree(dir: string): number {
  return collectJsFiles(dir).reduce((total, filePath) => {
    const raw = readFileSync(filePath);
    return total + gzipSync(raw).length;
  }, 0);
}

async function buildEntryGzipBytes(entry: string): Promise<number> {
  const outDir = mkdtempSync(resolve(tmpdir(), 'vox-gui-dashboard-budget-'));

  try {
    await build({
      configFile: false,
      plugins: [react()],
      build: {
        outDir,
        emptyOutDir: true,
        target: 'es2022',
        minify: 'esbuild',
        cssCodeSplit: false,
        reportCompressedSize: false,
        rollupOptions: {
          input: entry,
          external: REACT_EXTERNALS,
          output: {
            format: 'es',
            entryFileNames: 'chunk.js',
          },
        },
        write: true,
      },
      logLevel: 'error',
    });

    if (!statSync(outDir).isDirectory()) {
      throw new Error(`dashboard bundle budget build did not emit output directory: ${outDir}`);
    }

    const jsFiles = collectJsFiles(outDir);
    if (jsFiles.length === 0) {
      throw new Error(`dashboard bundle budget build emitted no JS files under ${outDir}`);
    }

    return gzipBytesForJsTree(outDir);
  } finally {
    rmSync(outDir, { recursive: true, force: true });
  }
}

/**
 * Incremental gzip bytes attributable to dashboard chart/grid deps.
 *
 * Builds two production-minified ES2022 chunks (react externalized):
 * - baseline: layout/grid helpers without recharts or @dnd-kit
 * - full: recharts + @dnd-kit imports mirrored by dashboard chart/grid widgets
 *
 * Returns full − baseline so shared dashboard layout code is not double-counted.
 */
export async function measureDashboardChunkGzipBytesInProcess(): Promise<number> {
  const baselineGzip = await buildEntryGzipBytes(DASHBOARD_BASELINE_ENTRY);
  const fullGzip = await buildEntryGzipBytes(DASHBOARD_CHUNK_ENTRY);

  const delta = fullGzip - baselineGzip;
  if (delta < 0) {
    throw new Error(
      `dashboard bundle baseline (${baselineGzip} B gzip) exceeded full chunk (${fullGzip} B gzip)`,
    );
  }

  return delta;
}
