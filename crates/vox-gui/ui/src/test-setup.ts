import { expect, afterEach } from 'vitest';
import * as matchers from '@testing-library/jest-dom/matchers';
import { cleanup } from '@testing-library/react';

expect.extend(matchers);

// Automatically clean up the DOM after each test so renders don't accumulate.
afterEach(() => {
  cleanup();
});

// Phase A backendGuard: tests exercise transport against mocked
// @tauri-apps/api — make detection succeed in BOTH node and jsdom envs.
// Suites asserting no-backend behavior delete this key and call
// __resetBackendAvailabilityForTests() in their own beforeEach.
(globalThis as any).__TAURI_INTERNALS__ ??= {};

// jsdom has no layout engine and doesn't implement ResizeObserver. Libraries
// that measure/react to element size (e.g. dockview) need at least a no-op
// stub present or they throw at mount time in tests.
if (typeof (globalThis as any).ResizeObserver === 'undefined') {
  class ResizeObserverStub {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  (globalThis as any).ResizeObserver = ResizeObserverStub;
}

// ── localStorage ────────────────────────────────────────────────────────────
// Node >= 22 ships an experimental built-in `localStorage` that is DISABLED
// unless the process is started with `--localstorage-file`. It is still defined
// as an own property of globalThis, valued `undefined`. Vitest's jsdom
// environment copies jsdom's window properties onto globalThis only where they
// are not already present — Node's descriptor wins, so jsdom's perfectly good
// Storage never lands and `window.localStorage` reads back `undefined`.
//
// The failure is silent until a test touches storage, and then it is a bare
// "Cannot read properties of undefined (reading 'clear')" a long way from the
// cause. Repairing it here rather than in each suite keeps every test file on
// one working Storage; see test-setup.localstorage.test.ts for the guard that
// fails if this stops working.
//
// The replacement is a real `Storage` instance backed by prototype methods, NOT
// a plain object with own-property methods. Suites legitimately do
// `vi.spyOn(Storage.prototype, 'setItem')` to simulate quota/permission
// failures (see useLocalStorage.test.ts); own properties would shadow the
// prototype and silently neuter those spies, turning a real error-handling
// test green for the wrong reason.
{
  const g = globalThis as any;
  const needsRepair = (name: string) => {
    const v = g[name];
    return v == null || typeof v.getItem !== 'function';
  };

  // DOM environments only. Suites that run under the plain `node` environment
  // have no window, don't touch storage, and expose Node's OWN built-in
  // `Storage` class — whose prototype has a non-configurable `length`, so
  // attempting the repair there throws at setup and fails the whole file.
  const inDom = typeof window !== 'undefined' && typeof document !== 'undefined';

  if (
    inDom &&
    typeof g.Storage === 'function' &&
    (needsRepair('localStorage') || needsRepair('sessionStorage'))
  ) {
    const backing = new WeakMap<object, Map<string, string>>();
    const mapFor = (self: object) => {
      let m = backing.get(self);
      if (!m) backing.set(self, (m = new Map()));
      return m;
    };

    // jsdom's Storage.prototype methods depend on an internal slot we cannot
    // fabricate, so they are replaced with a WeakMap-backed implementation.
    // Defined on the prototype precisely so spies can intercept them.
    Object.assign(g.Storage.prototype, {
      getItem(this: object, k: string) {
        const m = mapFor(this);
        return m.has(String(k)) ? m.get(String(k))! : null;
      },
      setItem(this: object, k: string, v: string) {
        mapFor(this).set(String(k), String(v));
      },
      removeItem(this: object, k: string) {
        mapFor(this).delete(String(k));
      },
      clear(this: object) {
        mapFor(this).clear();
      },
      key(this: object, i: number) {
        return Array.from(mapFor(this).keys())[i] ?? null;
      },
    });
    const lengthDesc = Object.getOwnPropertyDescriptor(g.Storage.prototype, 'length');
    if (!lengthDesc || lengthDesc.configurable) {
      Object.defineProperty(g.Storage.prototype, 'length', {
        get(this: object) {
          return mapFor(this).size;
        },
        configurable: true,
      });
    }

    for (const name of ['localStorage', 'sessionStorage']) {
      if (!needsRepair(name)) continue;
      const instance = Object.create(g.Storage.prototype);
      // configurable so a suite can still replace or stub it wholesale.
      Object.defineProperty(g, name, { value: instance, writable: true, configurable: true });
    }
  }
}
