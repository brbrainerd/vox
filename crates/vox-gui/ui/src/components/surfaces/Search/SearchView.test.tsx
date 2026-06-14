// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/react';
import React from 'react';

const invokeMock = vi.fn(() => Promise.resolve(null));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { SearchView } from './SearchView';

describe('SearchView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
    try { localStorage.clear(); } catch { /* ignore */ }
  });

  it('renders the Unified Search heading', () => {
    render(<SearchView pushToast={vi.fn()} />);
    expect(screen.getByText('Unified Search')).toBeTruthy();
  });

  it('every button carries an explicit type="button"', () => {
    render(<SearchView pushToast={vi.fn()} />);
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('scope chips expose aria-pressed', () => {
    render(<SearchView pushToast={vi.fn()} />);
    const pressed = screen.getAllByRole('button').filter(b => b.hasAttribute('aria-pressed'));
    expect(pressed.length).toBeGreaterThan(0);
  });

  it('the main search input and path filter are labeled', () => {
    render(<SearchView pushToast={vi.fn()} />);
    expect(screen.getByLabelText('Search query')).toBeTruthy();
    expect(screen.getByLabelText('Filter results by path glob')).toBeTruthy();
  });
});
