import React, { useEffect, useRef } from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { DocReader } from '../surfaces/DocReader/DocReader';
import type { ActiveDoc } from '../../hooks/useDocViewer';

export interface DocViewerDrawerProps {
  doc: ActiveDoc | null;
  onClose: () => void;
}

const FOCUSABLE_SELECTOR =
  'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function DocViewerDrawer({ doc, onClose }: DocViewerDrawerProps) {
  const panelRef = useRef<HTMLDivElement | null>(null);
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!doc) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [doc, onClose]);

  // Modal focus behavior: move focus into the panel on open, trap Tab/Shift+Tab
  // within it while open, and restore focus to whatever triggered it on close.
  useEffect(() => {
    if (!doc) return;

    previouslyFocusedRef.current = document.activeElement as HTMLElement | null;
    closeButtonRef.current?.focus();

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;
      const panel = panelRef.current;
      if (!panel) return;
      const focusable = Array.from(
        panel.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
      ).filter(el => el.offsetParent !== null || el === document.activeElement);
      if (focusable.length === 0) return;

      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = document.activeElement;

      if (e.shiftKey) {
        if (active === first || !panel.contains(active)) {
          e.preventDefault();
          last.focus();
        }
      } else {
        if (active === last || !panel.contains(active)) {
          e.preventDefault();
          first.focus();
        }
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      previouslyFocusedRef.current?.focus();
      previouslyFocusedRef.current = null;
    };
  }, [doc]);

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
        <div ref={panelRef} className="flex h-full min-h-0 w-full flex-col">
          <div className="flex items-center justify-between border-b border-border-subtle px-4 py-3">
            <span className="font-display text-[13px] uppercase tracking-[0.14em] text-text-primary">
              {doc.title}
            </span>
            <button
              ref={closeButtonRef}
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
        </div>
      </Glass>
    </div>
  );
}
