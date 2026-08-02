let seq = 0;

/** Monotonic string id for UI-only entities (toasts, stream items when backend omits id). */
export function nextId(prefix = 'ui'): string {
  seq += 1;
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now()}-${seq}`;
}

/** Prefix for GUI workflow run ids. */
export function nextGuiRunId(): string {
  return nextId('gui');
}

/**
 * Session id for background dispatches (/spawn, Deploy skill) that must not
 * borrow the active chat session's identity: submit_orchestrator_task never
 * writes to the orchestrator's chat_history context store, only
 * vox_chat_message does, so reusing the chat session id would silently
 * desync that store from the GUI transcript.
 */
export function newBackgroundSessionId(): string {
  return `bg-task-${nextGuiRunId()}`;
}
