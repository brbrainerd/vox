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

/**
 * Every real dispatch call site that can surface a `BudgetGuardError` or a
 * `RATE_LIMITED_PREFIX`-marked error to the GUI wraps it first via
 * `format!("LLM error: {e}")` (confirmed at all 10 real call sites: e.g.
 * `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs:580,608,639,658`,
 * `chat_tools/ghost_text.rs`, `chat_tools/inline_edit.rs`, `chat_tools/plan.rs`,
 * `compiler_tools.rs`, `db_tools.rs`, `scientia_tools/assist.rs`). The pattern
 * detectors below were previously anchored against the *raw* unwrapped string,
 * which never actually reaches the GUI in production — this strips that one
 * known wrapper layer first so detection works against the real wire shape.
 * A no-op when the prefix isn't present, so unwrapped strings (e.g. direct
 * test fixtures) still match too.
 */
const LLM_ERROR_WRAPPER_PREFIX = 'LLM error: ';

function unwrapLlmErrorPrefix(text: string): string {
  return text.startsWith(LLM_ERROR_WRAPPER_PREFIX) ? text.slice(LLM_ERROR_WRAPPER_PREFIX.length) : text;
}

export function isBudgetExceededError(text: string): boolean {
  return BUDGET_EXCEEDED_PATTERN.test(unwrapLlmErrorPrefix(text));
}

/**
 * Task 12b (free-tier onboarding plan): matches the `RATE_LIMITED_PREFIX` marker
 * both live-dispatch funnels prepend to their terminal-failure error string when
 * the underlying provider failure is a rate limit (e.g. OpenRouter's free-tier
 * 50/day cap) —
 * `crates/vox-actor-runtime/src/llm/chat.rs::RATE_LIMITED_PREFIX` (the
 * `llm_chat`/`try_run_agent_turn` funnel) and
 * `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs::RATE_LIMITED_PREFIX`
 * (the `mcp_infer_tool_completion` funnel — call_llm, ghost_text, inline_edit,
 * plan, ...) both use the exact marker string `"RATE_LIMITED: "`. Previously
 * this class was only ever surfaced by `vox doctor`'s diagnostic path; this is
 * what lets App.tsx's dispatch-error catch blocks give it a distinct toast too,
 * same pattern as `isBudgetExceededError` above.
 */
const RATE_LIMITED_PREFIX = 'RATE_LIMITED: ';

export function isRateLimitedError(text: string): boolean {
  return unwrapLlmErrorPrefix(text).startsWith(RATE_LIMITED_PREFIX);
}

/**
 * Mirrors `vox_actor_runtime::llm::CONTEXT_EXCEEDED_PREFIX`
 * (`crates/vox-actor-runtime/src/llm/chat.rs`) — the marker `llm_chat`
 * prepends when the underlying provider failure is a context-window
 * overflow. Same detection shape as `isRateLimitedError`/`isBudgetExceededError`.
 */
const CONTEXT_EXCEEDED_PREFIX = 'CONTEXT_LENGTH_EXCEEDED: ';

export function isContextExceededError(text: string): boolean {
  return unwrapLlmErrorPrefix(text).startsWith(CONTEXT_EXCEEDED_PREFIX);
}

/** Strips both the `LLM error: ` wrapper (if present) and the detection-only
 *  `RATE_LIMITED_PREFIX` marker, leaving the human-readable egress error
 *  message (e.g. "OpenRouter rate limit exceeded, try again in 24h") to show
 *  the user. No-op for either layer that isn't present. */
export function stripRateLimitedPrefix(text: string): string {
  const unwrapped = unwrapLlmErrorPrefix(text);
  return unwrapped.startsWith(RATE_LIMITED_PREFIX) ? unwrapped.slice(RATE_LIMITED_PREFIX.length) : unwrapped;
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
