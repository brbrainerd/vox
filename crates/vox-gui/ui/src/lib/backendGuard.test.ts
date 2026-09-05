// crates/vox-gui/ui/src/lib/backendGuard.test.ts
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  backendAvailable,
  BackendUnavailableError,
  makeBackendUnavailableRejectionFilter,
  __resetBackendAvailabilityForTests,
  sanitizeErrorForToast,
  isBudgetExceededError,
  isRateLimitedError,
  isContextExceededError,
  stripRateLimitedPrefix,
} from './backendGuard';

// Phase A test-setup.ts stubs __TAURI_INTERNALS__ globally for suites that mock
// @tauri-apps/api; this suite asserts unavailable-mode behavior, so it must undo
// that stub before every test, not just clean up after.
beforeEach(() => {
  delete (globalThis as any).__TAURI_INTERNALS__;
  delete (globalThis as any).window;
  __resetBackendAvailabilityForTests();
});

afterEach(() => {
  delete (globalThis as any).__TAURI_INTERNALS__;
  delete (globalThis as any).window;
  __resetBackendAvailabilityForTests();
});

describe('backendAvailable', () => {
  it('is false with no __TAURI_INTERNALS__ anywhere (node env)', () => {
    expect(backendAvailable()).toBe(false);
  });
  it('is true when globalThis has __TAURI_INTERNALS__ (node-env test stub)', () => {
    (globalThis as any).__TAURI_INTERNALS__ = {};
    expect(backendAvailable()).toBe(true);
  });
  it('is true when a window with __TAURI_INTERNALS__ exists (jsdom/Tauri)', () => {
    (globalThis as any).window = { __TAURI_INTERNALS__: {} };
    expect(backendAvailable()).toBe(true);
  });
  it('memoizes per app load', () => {
    expect(backendAvailable()).toBe(false);
    (globalThis as any).__TAURI_INTERNALS__ = {};
    expect(backendAvailable()).toBe(false);
  });
});

describe('BackendUnavailableError', () => {
  it('carries the command and an honest message', () => {
    const e = new BackendUnavailableError('chat_list_sessions');
    expect(e.command).toBe('chat_list_sessions');
    expect(e.message).toContain('desktop backend');
    expect(e.message).toContain('chat_list_sessions');
    expect(e).toBeInstanceOf(Error);
  });
});

describe('makeBackendUnavailableRejectionFilter', () => {
  it('preventDefaults BackendUnavailableError rejections', () => {
    const filter = makeBackendUnavailableRejectionFilter();
    const ev = { reason: new BackendUnavailableError('x'), preventDefault: vi.fn() };
    filter(ev as unknown as PromiseRejectionEvent);
    expect(ev.preventDefault).toHaveBeenCalledOnce();
  });
  it('preventDefaults raw __TAURI_INTERNALS__ TypeErrors ONLY when backend unavailable', () => {
    // 33 files import invoke directly and 7 import listen — their raw
    // TypeErrors must not surface uncaught in browser mode.
    const filter = makeBackendUnavailableRejectionFilter();
    const raw = {
      reason: new TypeError("can't access property \"invoke\", window.__TAURI_INTERNALS__ is undefined"),
      preventDefault: vi.fn(),
    };
    filter(raw as unknown as PromiseRejectionEvent);
    expect(raw.preventDefault).toHaveBeenCalledOnce();
    // With a backend present, the same TypeError is a REAL bug — pass through.
    (globalThis as any).__TAURI_INTERNALS__ = {};
    __resetBackendAvailabilityForTests();
    const filter2 = makeBackendUnavailableRejectionFilter();
    const raw2 = { reason: new TypeError('x __TAURI_INTERNALS__ y'), preventDefault: vi.fn() };
    filter2(raw2 as unknown as PromiseRejectionEvent);
    expect(raw2.preventDefault).not.toHaveBeenCalled();
  });
  it('passes unrelated rejections through', () => {
    const filter = makeBackendUnavailableRejectionFilter();
    const ev = { reason: new TypeError('boom'), preventDefault: vi.fn() };
    filter(ev as unknown as PromiseRejectionEvent);
    expect(ev.preventDefault).not.toHaveBeenCalled();
  });
});

describe('sanitizeErrorForToast', () => {
  it('returns the honest message for BackendUnavailableError', () => {
    const err = new BackendUnavailableError('chat_list_sessions');
    expect(sanitizeErrorForToast(err)).toBe(err.message);
  });

  it('does not leak __TAURI_INTERNALS__ or raw invoke internals', () => {
    const err = new TypeError(`can't access property "invoke", window.__TAURI_INTERNALS__ is undefined`);
    expect(sanitizeErrorForToast(err)).not.toMatch(/__TAURI_INTERNALS__/);
    expect(sanitizeErrorForToast(err)).not.toMatch(/invoke/);
  });

  it('passes through ordinary error text unchanged', () => {
    expect(sanitizeErrorForToast(new Error('Network timeout'))).toBe('Error: Network timeout');
  });
});

// Task 5 (free-tier onboarding plan): App.tsx's dispatch-error catch blocks
// use this to special-case `BudgetGuardError::Exceeded`'s Display-impl text
// (crates/vox-orchestrator-mcp/.../budget_guard.rs) into a distinct
// "Budget limit reached" toast instead of the generic dispatch-failure toast.
describe('isBudgetExceededError', () => {
  it('matches the Daily-scope BudgetGuardError::Exceeded message', () => {
    expect(isBudgetExceededError('Daily budget of $5.00 exceeded (spent $5.12)')).toBe(true);
  });

  it('matches the Session-scope BudgetGuardError::Exceeded message', () => {
    expect(isBudgetExceededError('Session budget of $2.00 exceeded (spent $2.01)')).toBe(true);
  });

  it('does not match unrelated backend errors', () => {
    expect(isBudgetExceededError('Network timeout')).toBe(false);
    expect(isBudgetExceededError('Error: request failed with status 500')).toBe(false);
  });

  it('does not match a message that merely mentions "budget" mid-sentence', () => {
    expect(isBudgetExceededError('Your daily budget of $5.00 exceeded (spent $5.12)')).toBe(false);
  });

  // Bug fix (2026-08): every real dispatch call site wraps the raw
  // BudgetGuardError string as `format!("LLM error: {e}")` before it reaches
  // the GUI (crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs and
  // siblings) — this is the shape that actually arrives in production, and
  // the one that would have caught the original bug (detectors anchored
  // against the raw unwrapped string never matched real dispatch failures).
  it('matches the Daily-scope message wrapped in the "LLM error: " prefix', () => {
    expect(isBudgetExceededError('LLM error: Daily budget of $5.00 exceeded (spent $5.12)')).toBe(true);
  });

  it('matches the Session-scope message wrapped in the "LLM error: " prefix', () => {
    expect(isBudgetExceededError('LLM error: Session budget of $2.00 exceeded (spent $2.01)')).toBe(true);
  });
});

// Task 12b (free-tier onboarding plan): App.tsx's dispatch-error catch blocks
// use this to special-case the `RATE_LIMITED_PREFIX` marker (prepended by both
// live-dispatch funnels — `chat.rs`'s and `infer.rs`'s `RATE_LIMITED_PREFIX`,
// both `"RATE_LIMITED: "`) into a distinct "Free tier limit reached" toast
// instead of the generic dispatch-failure toast.
describe('isRateLimitedError', () => {
  it('matches a message carrying the RATE_LIMITED_PREFIX marker', () => {
    expect(isRateLimitedError('RATE_LIMITED: OpenRouter rate limit exceeded, try again in 24h')).toBe(true);
  });

  it('does not match unrelated backend errors', () => {
    expect(isRateLimitedError('Network timeout')).toBe(false);
    expect(isRateLimitedError('Error: request failed with status 500')).toBe(false);
  });

  it('does not match a message that merely mentions rate limiting mid-sentence', () => {
    expect(isRateLimitedError('The provider returned a RATE_LIMITED: response')).toBe(false);
  });

  // Bug fix (2026-08): both live-dispatch funnels' RATE_LIMITED_PREFIX-marked
  // errors reach the GUI wrapped as `format!("LLM error: {e}")` at every real
  // call site (see isBudgetExceededError's wrapped-case tests above for the
  // same root cause) — this is the shape production actually sends.
  it('matches a message carrying both the "LLM error: " wrapper and the RATE_LIMITED_PREFIX marker', () => {
    expect(isRateLimitedError('LLM error: RATE_LIMITED: OpenRouter rate limit exceeded, try again in 24h')).toBe(
      true,
    );
  });
});

describe('stripRateLimitedPrefix', () => {
  it('removes the marker, leaving the human-readable message', () => {
    expect(stripRateLimitedPrefix('RATE_LIMITED: OpenRouter rate limit exceeded, try again in 24h')).toBe(
      'OpenRouter rate limit exceeded, try again in 24h',
    );
  });

  it('is a no-op when the prefix is absent', () => {
    expect(stripRateLimitedPrefix('Network timeout')).toBe('Network timeout');
  });

  // Bug fix (2026-08): must strip BOTH wrapper layers, producing clean
  // user-facing text rather than "LLM error: <message-with-marker-still-attached>".
  it('strips both the "LLM error: " wrapper and the RATE_LIMITED_PREFIX marker', () => {
    expect(stripRateLimitedPrefix('LLM error: RATE_LIMITED: OpenRouter rate limit exceeded, try again in 24h')).toBe(
      'OpenRouter rate limit exceeded, try again in 24h',
    );
  });
});

// Task C1 (chat-harness-unification plan): mirrors the Rust-side
// `classify_turn_error`'s `ContextExceeded` case
// (`crates/vox-gui/src/commands/chat_turn.rs`), which matches
// `vox_actor_runtime::llm::CONTEXT_EXCEEDED_PREFIX`.
describe('isContextExceededError', () => {
  it('matches a message carrying the CONTEXT_LENGTH_EXCEEDED marker', () => {
    expect(isContextExceededError('CONTEXT_LENGTH_EXCEEDED: 200000 > 128000')).toBe(true);
  });

  it('matches the "LLM error: "-wrapped form production actually sends', () => {
    expect(isContextExceededError('LLM error: CONTEXT_LENGTH_EXCEEDED: 200000 > 128000')).toBe(true);
  });

  it('does not match unrelated backend errors', () => {
    expect(isContextExceededError('Network timeout')).toBe(false);
  });
});
