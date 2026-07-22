// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

vi.mock('../../../transport', () => ({
  voxTransport: {
    mercatusLoadConfig: vi.fn(() => Promise.resolve({ _meta: { sources: {} }, watchlist: [] })),
  },
}));

import { Mercatus } from './Mercatus';
import { LanguageProvider } from '../../../hooks/useLanguage';
import { labelFor } from '../../../lib/lexicon';

// Reproduces a live bug: the sidebar nav item for this surface is labeled
// via the lexicon ('Market' in English), but the surface's own header
// hardcoded a different string ("Mercatus"), so the page you land on didn't
// match what you clicked.
describe('Mercatus', () => {
  it("headers with the same label the sidebar uses ('mercatus' lexicon key)", () => {
    render(
      <LanguageProvider>
        <Mercatus />
      </LanguageProvider>,
    );
    const expected = labelFor('mercatus', 'en');
    expect(screen.getByRole('heading', { name: new RegExp(expected) })).toBeInTheDocument();
  });

  it('condensed prop renders only the parts/sources summary line, not the coverage table', async () => {
    render(
      <LanguageProvider>
        <Mercatus condensed />
      </LanguageProvider>,
    );
    // Same "N parts · N enabled sources" line the full view already shows,
    // rendered on its own without the coverage matrix or source registry.
    expect(await screen.findByText(/0 parts · 0 enabled sources/i)).toBeInTheDocument();
    expect(screen.queryByRole('table')).toBeNull();
  });
});
