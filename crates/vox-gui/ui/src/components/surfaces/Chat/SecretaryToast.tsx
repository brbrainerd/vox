import React, { useEffect } from 'react';

export interface SecretaryToastProps {
  /** The task intent text extracted from the chat message. */
  intent: string;
  /** The hopper item ID (for future cancel support). */
  itemId: string;
  /** Called when the toast should be dismissed. */
  onDismiss: () => void;
  /** Called when the user clicks "View task". */
  onViewTask: () => void;
}

const AUTO_DISMISS_MS = 5_000;
const MAX_INTENT_CHARS = 80;

/** Dismissable toast shown when the secretary auto-submits a task from chat. */
export function SecretaryToast({ intent, itemId: _itemId, onDismiss, onViewTask }: SecretaryToastProps) {
  // Auto-dismiss after 5 seconds.
  useEffect(() => {
    const t = setTimeout(onDismiss, AUTO_DISMISS_MS);
    return () => clearTimeout(t);
  }, [onDismiss]);

  const displayed =
    intent.length > MAX_INTENT_CHARS
      ? intent.slice(0, MAX_INTENT_CHARS) + '...'
      : intent;

  return (
    <div
      role="status"
      aria-live="polite"
      className="flex items-center gap-2 rounded-lg border border-border-subtle bg-bg-base/95 px-3 py-2 shadow-lg backdrop-blur-sm"
    >
      {/* Secretary icon */}
      <span className="shrink-0 text-[10px] text-text-muted" aria-hidden>📋</span>

      <div className="min-w-0 flex-1">
        <p className="text-[10px] text-text-muted">Task added by secretary</p>
        <p
          data-testid="secretary-toast-intent"
          className="truncate text-[11px] text-text-secondary"
        >
          {displayed}
        </p>
      </div>

      {/* View task button */}
      <button
        type="button"
        aria-label="View task in Tasks panel"
        onClick={onViewTask}
        className="shrink-0 rounded px-2 py-0.5 text-[10px] text-brass hover:bg-overlay-subtle transition"
      >
        View task
      </button>

      {/* Dismiss button */}
      <button
        type="button"
        aria-label="Dismiss secretary toast"
        onClick={onDismiss}
        className="shrink-0 rounded p-0.5 text-text-muted hover:text-text-secondary transition"
      >
        ✕
      </button>
    </div>
  );
}

