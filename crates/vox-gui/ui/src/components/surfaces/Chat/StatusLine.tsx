import React from 'react';

interface StatusLineProps {
  phase: string;
  elapsedMs: number;
}

/**
 * The single collapsed status line shown in the chat feed while a task is
 * in flight — replaces the old CHECKPOINT/TASK/PHASE/COST event-row spam.
 * Full detail for the same task remains available in the Flow panel.
 */
export function StatusLine({ phase, elapsedMs }: StatusLineProps) {
  const seconds = Math.floor(elapsedMs / 1000);
  return (
    <div
      className="flex items-center gap-2 self-start rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-[11px] text-text-muted"
      data-testid="chat-status-line"
      role="status"
      aria-live="polite"
    >
      <span className="size-1.5 animate-pulse rounded-full bg-brass" aria-hidden="true" />
      <span className="font-mono">{phase} · {seconds}s</span>
    </div>
  );
}
