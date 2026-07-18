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
