// crates/vox-gui/ui/src/components/ui/BackendBanner.test.tsx
// @vitest-environment jsdom
import { describe, it, expect, afterEach, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import React from 'react';
import { BackendBanner } from './BackendBanner';
import { __resetBackendAvailabilityForTests } from '../../lib/backendGuard';

beforeEach(() => {
  // test-setup.ts stubs globalThis.__TAURI_INTERNALS__ for the suite at
  // large; this suite asserts no-backend behavior, so remove it first.
  delete (globalThis as any).__TAURI_INTERNALS__;
  delete (window as any).__TAURI_INTERNALS__;
  __resetBackendAvailabilityForTests();
});
afterEach(() => {
  cleanup();
  (globalThis as any).__TAURI_INTERNALS__ = {};
  __resetBackendAvailabilityForTests();
});

describe('BackendBanner', () => {
  it('renders in no-backend mode and dismisses on click', () => {
    render(<BackendBanner />);
    expect(screen.getByRole('status', { name: /browser preview/i })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(screen.queryByRole('status', { name: /browser preview/i })).toBeNull();
  });
  it('renders nothing when the backend is present', () => {
    (window as any).__TAURI_INTERNALS__ = {};
    __resetBackendAvailabilityForTests();
    render(<BackendBanner />);
    expect(screen.queryByRole('status', { name: /browser preview/i })).toBeNull();
  });
});
