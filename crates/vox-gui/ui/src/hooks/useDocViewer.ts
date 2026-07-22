import { useCallback, useState } from 'react';

export interface ActiveDoc {
  path: string;
  title: string;
}

function titleFromPath(path: string): string {
  return path.split('/').pop()?.replace(/\.md$/i, '') ?? path;
}

/**
 * Doc-viewer state: at most one open doc, presented as a drawer (see
 * DocViewerDrawer.tsx). Deliberately NOT persisted across reloads (unlike
 * useActiveView) — a doc reference is a transient "I clicked a link" action,
 * not a navigation destination worth restoring on next launch. No-stacking is
 * safe today because DocReader renders raw text with no clickable in-doc
 * links (confirmed by reading DocReader.tsx while writing this plan) —
 * revisit if DocReader ever gains markdown link rendering.
 */
export function useDocViewer() {
  const [activeDoc, setActiveDoc] = useState<ActiveDoc | null>(null);

  const openDoc = useCallback((path: string, title?: string) => {
    setActiveDoc({ path, title: title ?? titleFromPath(path) });
  }, []);

  const closeDoc = useCallback(() => {
    setActiveDoc(null);
  }, []);

  return { activeDoc, openDoc, closeDoc };
}
