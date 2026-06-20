import React, { useCallback, useEffect, useRef } from 'react';
import { clampSidebarWidth, snapToPreset } from '../../lib/sidebarWidth';

interface SidebarResizerProps {
  /** Called continuously while dragging (clamped px). */
  onResize: (px: number) => void;
  /** Called once on release (snapped px) — the value to persist. */
  onCommit: (px: number) => void;
  /** Reset to the default preset on double-click. */
  onReset: () => void;
}

export function SidebarResizer({ onResize, onCommit, onReset }: SidebarResizerProps) {
  const dragging = useRef(false);
  const latest = useRef(212);

  const onPointerMove = useCallback((e: PointerEvent) => {
    if (!dragging.current) return;
    const px = clampSidebarWidth(e.clientX);
    latest.current = px;
    onResize(px);
  }, [onResize]);

  const stop = useCallback(() => {
    if (!dragging.current) return;
    dragging.current = false;
    document.body.style.cursor = '';
    onCommit(snapToPreset(latest.current));
  }, [onCommit]);

  useEffect(() => {
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', stop);
    return () => {
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', stop);
    };
  }, [onPointerMove, stop]);

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize sidebar"
      onPointerDown={() => { dragging.current = true; document.body.style.cursor = 'col-resize'; }}
      onDoubleClick={onReset}
      className="absolute top-0 right-0 z-20 h-full w-1.5 -mr-0.5 cursor-col-resize bg-transparent hover:bg-brass/30 transition-colors"
    />
  );
}
