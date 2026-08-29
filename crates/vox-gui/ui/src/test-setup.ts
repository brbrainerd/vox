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

// Node >= 22 ships a built-in `localStorage` on globalThis that is disabled
// unless the process was started with a storage path. Under vitest's jsdom
// environment that built-in shadows the working Storage jsdom provides, so
// `window.localStorage` reads back `undefined` and every module touching it
// (useLocalStorage, shellPersistence, the chat model pick) throws "Cannot read
// properties of undefined" at import time — roughly a third of this suite.
//
// Repair: prefer jsdom's own Storage when it is present and usable; otherwise
// install a minimal in-memory Storage. Both `window` and `globalThis` are
// pinned to the same object so the two access paths cannot diverge.
// `test-setup.localstorage.test.ts` is the regression guard for this block.
function installTestStorage(): void {
  const win = globalThis as unknown as { window?: Window & typeof globalThis };
  const jsdomStorage = win.window?.localStorage;
  const usable = (() => {
    try {
      if (!jsdomStorage) return false;
      jsdomStorage.setItem('__vox_probe__', '1');
      jsdomStorage.removeItem('__vox_probe__');
      return true;
    } catch {
      return false;
    }
  })();

  const store: Storage = usable
    ? jsdomStorage!
    : (() => {
        const map = new Map<string, string>();
        return {
          get length() {
            return map.size;
          },
          clear: () => map.clear(),
          getItem: (k: string) => (map.has(String(k)) ? map.get(String(k))! : null),
          key: (i: number) => Array.from(map.keys())[i] ?? null,
          removeItem: (k: string) => void map.delete(String(k)),
          setItem: (k: string, v: string) => void map.set(String(k), String(v)),
        } as Storage;
      })();

  for (const target of [globalThis, win.window].filter(Boolean) as object[]) {
    Object.defineProperty(target, 'localStorage', {
      value: store,
      configurable: true,
      writable: true,
    });
  }
}
installTestStorage();

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
