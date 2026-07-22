import React, { useEffect } from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { DocReader } from '../surfaces/DocReader/DocReader';
import type { ActiveDoc } from '../../hooks/useDocViewer';

export interface DocViewerDrawerProps {
  doc: ActiveDoc | null;
  onClose: () => void;
}

export function DocViewerDrawer({ doc, onClose }: DocViewerDrawerProps) {
  useEffect(() => {
    if (!doc) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [doc, onClose]);

  if (!doc) return null;

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <button
        type="button"
        aria-label="Close doc overlay"
        className="flex-1 bg-black/50"
        onClick={onClose}
      />
      <Glass
        role="dialog"
        aria-label={doc.title}
        aria-modal="true"
        className="flex h-full w-full max-w-2xl flex-col rounded-none border-l border-border-subtle shadow-2xl"
        inset={false}
      >
        <div className="flex items-center justify-between border-b border-border-subtle px-4 py-3">
          <span className="font-display text-[13px] uppercase tracking-[0.14em] text-text-primary">
            {doc.title}
          </span>
          <button
            type="button"
            aria-label="Close doc"
            onClick={onClose}
            className="flex size-7 items-center justify-center rounded-md text-text-muted hover:bg-overlay-hover hover:text-text-primary"
          >
            <Icon.x className="size-4" aria-hidden="true" />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          <DocReader path={doc.path} />
        </div>
      </Glass>
    </div>
  );
}
