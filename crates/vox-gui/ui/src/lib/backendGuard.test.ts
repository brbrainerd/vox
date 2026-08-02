// crates/vox-gui/ui/src/lib/backendGuard.test.ts
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  backendAvailable,
  BackendUnavailableError,
  makeBackendUnavailableRejectionFilter,
  __resetBackendAvailabilityForTests,
  sanitizeErrorForToast,
  isBudgetExceededError,
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
});
