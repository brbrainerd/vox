import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';

export interface ManifestEntry {
  viewKey: string;
  file: string;
  sha256: string;
  bytes: number;
  width: number;
  height: number;
  captureMs: number;
}
export interface Manifest {
  schema_version: 1;
  generated_at_unix_ms: number | null;
  total_capture_ms: number;
  surfaces: ManifestEntry[];
}
export function sha256Png(buf: Buffer): string {
  return createHash('sha256').update(buf).digest('hex');
}
export function buildManifest(entries: ManifestEntry[], totalCaptureMs: number, nowMs: number | null = null): Manifest {
  const surfaces = [...entries].sort((a, b) => a.viewKey.localeCompare(b.viewKey));
  return { schema_version: 1, generated_at_unix_ms: nowMs, total_capture_ms: totalCaptureMs, surfaces };
}
export function writeManifest(dir: string, manifest: Manifest): string {
  const path = join(dir, 'manifest.json');
  writeFileSync(path, JSON.stringify(manifest, null, 2) + '\n', 'utf8');
  return path;
}
