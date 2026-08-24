// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

const mockLoadConfig = vi.fn();
const mockConfigPath = vi.fn();
vi.mock('../../../transport', () => ({
  voxTransport: {
    mercatusLoadConfig: () => mockLoadConfig(),
    mercatusConfigPath: () => mockConfigPath(),
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
  beforeEach(() => {
    mockLoadConfig.mockReset();
    mockConfigPath.mockReset();
    mockLoadConfig.mockResolvedValue({ _meta: { sources: {} }, watchlist: [] });
    mockConfigPath.mockResolvedValue('/cfg/price-watch.config.json');
  });

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

describe('Mercatus empty and null states', () => {
  beforeEach(() => {
    mockLoadConfig.mockReset();
    mockConfigPath.mockReset();
    mockConfigPath.mockResolvedValue('/cfg/price-watch.config.json');
  });

  it('renders an empty state instead of a blank void when the config is null', async () => {
    // Regression: the render body was gated on `state === 'ok' && cfg`, so a
    // null payload (any unmocked/absent backend response) left the whole
    // content area empty — no table, no message, no error. The review harness
    // captured this as a critical "blank" defect at every viewport.
    mockLoadConfig.mockResolvedValue(null);
    render(
      <LanguageProvider>
        <Mercatus />
      </LanguageProvider>,
    );
    expect(await screen.findByTestId('mercatus-empty')).toBeInTheDocument();
  });

  it('renders the empty state for a first-run config with no watchlist entries', async () => {
    mockLoadConfig.mockResolvedValue({ _meta: { sources: {} }, watchlist: [] });
    render(
      <LanguageProvider>
        <Mercatus />
      </LanguageProvider>,
    );
    expect(await screen.findByTestId('mercatus-empty')).toBeInTheDocument();
    // A headers-only table is not an empty state.
    expect(screen.queryByRole('table')).toBeNull();
  });

  it('renders the coverage table once parts exist', async () => {
    mockLoadConfig.mockResolvedValue({
      _meta: { sources: { newegg: { enabled: true, costUsd: 0.002, cadenceHours: 6, tier: 'paid' } } },
      watchlist: [{ id: 'gpu-1', role: 'gpu', model: 'RTX 5090', sources: ['newegg'], ids: { newegg: 'N82E' }, target_usd: 1999 }],
    });
    render(
      <LanguageProvider>
        <Mercatus />
      </LanguageProvider>,
    );
    // Two tables render once parts exist: the coverage matrix and the
    // source-registry table inside the collapsed <details>.
    expect(await screen.findAllByRole('table')).toHaveLength(2);
    expect(screen.queryByTestId('mercatus-empty')).toBeNull();
  });
});

describe('Mercatus empty state names the config file', () => {
  beforeEach(() => {
    mockLoadConfig.mockReset();
    mockConfigPath.mockReset();
    mockLoadConfig.mockResolvedValue({ _meta: { sources: {} }, watchlist: [] });
  });

  it('shows where the price-watch config lives so first-run is actionable', async () => {
    mockConfigPath.mockResolvedValue('/home/u/.config/storage-tier/price-watch/price-watch.config.json');
    render(
      <LanguageProvider>
        <Mercatus />
      </LanguageProvider>,
    );
    expect(await screen.findByTestId('mercatus-config-path')).toHaveTextContent(
      '/home/u/.config/storage-tier/price-watch/price-watch.config.json',
    );
  });

  it('still renders the empty state when the path lookup fails', async () => {
    mockConfigPath.mockRejectedValue(new Error('no backend'));
    render(
      <LanguageProvider>
        <Mercatus />
      </LanguageProvider>,
    );
    expect(await screen.findByTestId('mercatus-empty')).toBeInTheDocument();
    expect(screen.queryByTestId('mercatus-config-path')).toBeNull();
  });
});
