// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';

const invokeMock = vi.fn(() => Promise.resolve(null));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

vi.mock('../../../transport', () => ({
  voxTransport: {
    openLocator: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../../../hooks/useSearchController', () => ({
  useSearchController: vi.fn(() => ({
    state: { query: '', hits: [], loading: false, scopes: ['code'], requestToken: 0 },
    setQuery: vi.fn(),
    setScopes: vi.fn(),
  })),
}));

import { SearchView, pathMatchesGlob } from './SearchView';
import { useSearchController } from '../../../hooks/useSearchController';

describe('SearchView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
    vi.mocked(useSearchController).mockReturnValue({
      state: { query: '', hits: [], loading: false, scopes: ['code'], requestToken: 0 },
      setQuery: vi.fn(),
      setScopes: vi.fn(),
    });
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

  it('shows skeleton while searchState.loading', () => {
    vi.mocked(useSearchController).mockReturnValue({
      state: { query: 'foo', hits: [], loading: true, scopes: ['code'], requestToken: 1 },
      setQuery: vi.fn(),
      setScopes: vi.fn(),
    });
    render(<SearchView pushToast={vi.fn()} />);
    expect(screen.getAllByTestId('search-skeleton').length).toBeGreaterThan(0);
  });

  it('merges SETTINGS_INDEX hits when settings scope chip is selected', async () => {
    vi.mocked(useSearchController).mockReturnValue({
      state: { query: 'openrouter', hits: [], loading: false, scopes: ['settings'], requestToken: 1 },
      setQuery: vi.fn(),
      setScopes: vi.fn(),
    });
    render(<SearchView pushToast={vi.fn()} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    await waitFor(() => {
      expect(screen.getByText('OpenRouter override')).toBeTruthy();
      expect(screen.getByText(/results across settings/i)).toBeTruthy();
    });
  });
});

describe('pathMatchesGlob', () => {
  it('returns true when glob is empty (no filter applied)', () => {
    expect(pathMatchesGlob('src/main.rs', '')).toBe(true);
    expect(pathMatchesGlob('src/main.rs', '   ')).toBe(true);
  });

  it('returns false when path is null', () => {
    expect(pathMatchesGlob(null, '**/*.rs')).toBe(false);
  });

  it('** matches across path separators', () => {
    expect(pathMatchesGlob('crates/vox-search/src/lib.rs', '**/*.rs')).toBe(true);
    expect(pathMatchesGlob('a/b/c/d/e.rs', '**/*.rs')).toBe(true);
  });

  it('** does NOT match a .rss extension as .rs', () => {
    expect(pathMatchesGlob('feed.rss', '**/*.rs')).toBe(false);
  });

  it('single * does not cross path separators', () => {
    // *.rs at the root level only matches flat filenames (no /)
    expect(pathMatchesGlob('main.rs', '*.rs')).toBe(true);
    expect(pathMatchesGlob('src/main.rs', '*.rs')).toBe(false);
  });

  it('exact path match without wildcards', () => {
    expect(pathMatchesGlob('crates/vox-gui/src/main.rs', 'crates/vox-gui/src/main.rs')).toBe(true);
    expect(pathMatchesGlob('crates/vox-gui/src/lib.rs', 'crates/vox-gui/src/main.rs')).toBe(false);
  });

  it('? matches exactly one non-separator character', () => {
    expect(pathMatchesGlob('src/main.rs', 'src/ma?n.rs')).toBe(true);
    expect(pathMatchesGlob('src/mn.rs', 'src/ma?n.rs')).toBe(false);  // ? requires exactly 1 char
    expect(pathMatchesGlob('src/ma/n.rs', 'src/ma?n.rs')).toBe(false); // ? must not be /
  });

  it('crate-scoped glob works', () => {
    expect(pathMatchesGlob('crates/vox-search/src/execution.rs', 'crates/vox-search/**')).toBe(true);
    expect(pathMatchesGlob('crates/vox-gui/src/lib.rs', 'crates/vox-search/**')).toBe(false);
  });

  it('no match returns false', () => {
    expect(pathMatchesGlob('src/main.rs', '**/*.ts')).toBe(false);
  });

  it('handles regex special chars in path literally', () => {
    // A glob with a literal dot should not treat . as "any char"
    expect(pathMatchesGlob('src/main_rs', '**/*.rs')).toBe(false); // underscore not a dot
  });
});

describe('SearchView accessibility', () => {
  it('result count has aria-live="polite" for screen reader announcements', () => {
    render(<SearchView pushToast={vi.fn()} />);
    const liveRegion = document.querySelector('[aria-live="polite"]');
    expect(liveRegion).not.toBeNull();
  });

  it('result count has aria-atomic="true"', () => {
    render(<SearchView pushToast={vi.fn()} />);
    const liveRegion = document.querySelector('[aria-atomic="true"]');
    expect(liveRegion).not.toBeNull();
  });
});

import { scoreToPct } from './searchHelpers';

describe('scoreToPct', () => {
  it('clamps BM25 scores above 1.0 to 100', () => {
    expect(scoreToPct(1.5)).toBe('100.00%');
    expect(scoreToPct(99.9)).toBe('100.00%');
  });

  it('clamps negative scores to 0', () => {
    expect(scoreToPct(-0.1)).toBe('0.00%');
    expect(scoreToPct(-100)).toBe('0.00%');
  });

  it('maps 0.75 to 75', () => {
    expect(scoreToPct(0.75)).toBe('75.00%');
  });

  it('maps 1.0 to 100', () => {
    expect(scoreToPct(1.0)).toBe('100.00%');
  });

  it('maps 0.0 to 0', () => {
    expect(scoreToPct(0.0)).toBe('0.00%');
  });
});
