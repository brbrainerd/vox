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
