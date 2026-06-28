/**
 * Loads the VG-1 build-time GUI content manifest (gui-content-manifest.json),
 * exposed by the Tauri command `vox_content_manifest` (modeled on
 * `vox_docs_index`). Defaults to [] when the command is absent or errors so the
 * Omnibar degrades honestly before VG-1 lands.
 *
 * See docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md §2.1.
 */
import { useEffect, useState } from 'react';
import { voxTransport } from '../transport';

export interface ContentManifestEntry {
  viewKey: string;
  label: string;
  route: string;
  headings: string[];
  copy: string[];
  commands: string[];
  docs: string[];
}

export function useContentManifest(): ContentManifestEntry[] {
  const [rows, setRows] = useState<ContentManifestEntry[]>([]);
  useEffect(() => {
    let cancelled = false;
    // .then(onSuccess, onError) (see useMemoryStatus) consumes a rejected
    // transport promise directly. Promise.resolve().then(() => fn()) also folds a
    // synchronous throw (voxContentManifest missing pre-VG-1) into onError.
    Promise.resolve()
      .then(() => voxTransport.voxContentManifest())
      .then(
        (loaded) => {
          if (!cancelled) setRows(Array.isArray(loaded) ? loaded : []);
        },
        () => {
          if (!cancelled) setRows([]);
        },
      );
    return () => {
      cancelled = true;
    };
  }, []);
  return rows;
}
