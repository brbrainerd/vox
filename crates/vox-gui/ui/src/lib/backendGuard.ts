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

/**
 * Toast bodies must never leak raw IPC internals (F-03: a caught rejection's
 * String(err) rendering __TAURI_INTERNALS__ verbatim in a user-visible toast).
 * Distinct from the unhandledrejection filter — this runs on *caught*
 * exceptions the app chooses to display. \binvoke\b does not match
 * invoke_mcp_tool (underscore is a word char); prose like "failed to invoke X"
 * degrades to the generic message, which is acceptable.
 */
const LEAK_PATTERN = /__TAURI_INTERNALS__|\binvoke\b/;

export function sanitizeErrorForToast(err: unknown): string {
  if (err instanceof BackendUnavailableError) return err.message;
  const text = String(err);
  return LEAK_PATTERN.test(text) ? 'An unexpected error occurred.' : text;
}

/**
 * Matches `BudgetGuardError::Exceeded`'s `Display` impl
 * (`crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/budget_guard.rs`):
 * `"{scope:?} budget of ${cap_usd:.2} exceeded (spent ${spent_usd:.2})"`, which
 * renders as e.g. `"Daily budget of $5.00 exceeded (spent $5.12)"` or
 * `"Session budget of $2.00 exceeded (spent $2.01)"`. Dispatch-error catch
 * blocks use this to special-case budget-exceeded failures into a distinct
 * toast instead of the generic error toast — see App.tsx's two
 * `submit_orchestrator_task` / `chat_send_message` catch blocks, the two
 * dispatch mechanisms the budget guard is wired into server-side.
 */
const BUDGET_EXCEEDED_PATTERN = /^(Daily|Session) budget of \$/;

export function isBudgetExceededError(text: string): boolean {
  return BUDGET_EXCEEDED_PATTERN.test(text);
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
