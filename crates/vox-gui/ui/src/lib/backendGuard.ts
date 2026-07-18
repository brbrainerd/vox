// crates/vox-gui/ui/src/lib/backendGuard.ts
/**
 * Single source of truth for "is the Tauri desktop backend present?".
 *
 * In a plain browser there is no `window.__TAURI_INTERNALS__` and every raw
 * `invoke`/`listen` from @tauri-apps/api throws
 * `TypeError: can't access property "invoke", window.__TAURI_INTERNALS__ is undefined`.
 * transport.ts routes its IPC through safeInvoke/safeListen (typed rejection);
 * the 33 files importing `invoke` directly and 7 importing `listen`
 * (ipcBoundaries allowlist debt) are covered user-visibly by the rejection
 * filter's raw-TypeError branch below.
 *
 * Detection is env-agnostic (window OR globalThis) so node-env vitest suites
 * can stub `globalThis.__TAURI_INTERNALS__` without fabricating a window.
 */

let cached: boolean | null = null;

export function backendAvailable(): boolean {
  if (cached === null) {
    const host = (typeof window !== 'undefined' ? window : globalThis) as unknown as Record<string, unknown>;
    cached = '__TAURI_INTERNALS__' in host;
  }
  return cached;
}

/** Test-only: memoization would leak across vitest cases. */
export function __resetBackendAvailabilityForTests(): void {
  cached = null;
}

export class BackendUnavailableError extends Error {
  readonly command: string;
  constructor(command: string) {
    super(
      `Axis is running without its desktop backend — '${command}' unavailable. ` +
        `(Browser preview mode: data surfaces show empty states.)`,
    );
    this.name = 'BackendUnavailableError';
    this.command = command;
  }
}

const logged = new Set<string>();

/**
 * 'unhandledrejection' filter: in browser (no-backend) mode, swallow
 * (a) BackendUnavailableError and (b) raw __TAURI_INTERNALS__ TypeErrors from
 * direct-import call sites, logging once per distinct command/message.
 * With a backend present, (b) passes through — it would be a real bug.
 */
export function makeBackendUnavailableRejectionFilter(): (ev: PromiseRejectionEvent) => void {
  return (ev) => {
    const r = ev.reason;
    const isTyped = r instanceof BackendUnavailableError;
    const isRawNoBackend =
      !backendAvailable() && r instanceof TypeError && /__TAURI_INTERNALS__/.test(r.message);
    if (isTyped || isRawNoBackend) {
      ev.preventDefault();
      const key = isTyped ? (r as BackendUnavailableError).command : r.message;
      if (!logged.has(key)) {
        logged.add(key);
        console.debug('[backendGuard] suppressed (browser mode):', key);
      }
    }
  };
}

export function installBackendUnavailableRejectionFilter(): void {
  if (typeof window === 'undefined') return;
  window.addEventListener('unhandledrejection', makeBackendUnavailableRejectionFilter());
}
