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
});
