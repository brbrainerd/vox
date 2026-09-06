import React, { useEffect } from 'react';

export interface SecretaryToastProps {
  /** The task intent text extracted from the chat message. */
  intent: string;
  /** The client-side proposal ID (no task exists until confirmed). */
  itemId: string;
  /** Called when the proposal should be dismissed without submitting a task. */
  onDismiss: () => void;
  /**
   * Called when the user explicitly confirms the proposal. This is the only
   * path that results in a task being submitted (Task 0.2: propose-only —
   * the secretary never auto-dispatches).
   */
  onConfirm: () => void;
}

const AUTO_DISMISS_MS = 5_000;
const MAX_INTENT_CHARS = 80;

/** Dismissable toast shown when the secretary proposes a task from chat. Requires
 * explicit user confirmation before any task is submitted. */
export function SecretaryToast({ intent, itemId: _itemId, onDismiss, onConfirm }: SecretaryToastProps) {
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
      className="flex items-center gap-2 rounded-lg border border-border-subtle bg-bg-base/95 px-3 py-2 shadow-lg backdrop-blur-xs"
    >
      {/* Secretary icon */}
      <span className="shrink-0 text-[10px] text-text-muted" aria-hidden>📋</span>

      <div className="min-w-0 flex-1">
        <p className="text-[10px] text-text-muted">Secretary suggests a task</p>
        <p
          data-testid="secretary-toast-intent"
          className="truncate text-[11px] text-text-secondary"
        >
          {displayed}
        </p>
      </div>

      {/* Confirm button — the only action that submits a task */}
      <button
        type="button"
        aria-label="Confirm and add task"
        onClick={onConfirm}
        className="shrink-0 rounded-sm px-2 py-0.5 text-[10px] text-brass hover:bg-overlay-subtle transition"
      >
        Add task
      </button>

      {/* Dismiss button */}
      <button
        type="button"
        aria-label="Dismiss secretary toast"
        onClick={onDismiss}
        className="shrink-0 rounded-sm p-0.5 text-text-muted hover:text-text-secondary transition"
      >
        ✕
      </button>
    </div>
  );
}

