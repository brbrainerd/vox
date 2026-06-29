// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';

// One research-originated strong candidate the inbox should render.
const STRONG_ROW = {
  id: 7,
  publication_id: 'commit-abc123',
  surfaced_at_ms: Date.now() - 5_000,
  intake_tier: 'strong_candidate',
  signal_codes: ['research_pipeline.supported_claims'],
  origin: 'research',
};

// Mock the Tauri invoke boundary: list returns the strong row, acknowledge
// resolves void. Captured so the test can assert it was called.
const invokeMock = vi.fn((cmd: string, _args?: unknown) => {
  if (cmd === 'list_discovery_inbox') return Promise.resolve([STRONG_ROW]);
  if (cmd === 'acknowledge_discovery') return Promise.resolve();
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

// Mock the transport bridge: listenScientiaQueue rejects (no Tauri).
const listenDiscoverySurfacedMock = vi.fn().mockRejectedValue(new Error('not in tauri'));
vi.mock('../../../transport', () => ({
  listenScientiaQueue: vi.fn().mockRejectedValue(new Error('not in tauri')),
  listenDiscoverySurfaced: (...args: unknown[]) => listenDiscoverySurfacedMock(...args),
}));

import { LanguageProvider } from '../../../hooks/useLanguage';
import { DiscoveryInbox } from './DiscoveryInbox';

describe('DiscoveryInbox', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders an unacknowledged strong_candidate row (tier badge + research origin)', async () => {
    render(<LanguageProvider><DiscoveryInbox pushToast={vi.fn()} /></LanguageProvider>);
    expect(await screen.findByText('commit-abc123')).toBeTruthy();
    expect(screen.getByText('strong candidate')).toBeTruthy();
    expect(screen.getByText('research')).toBeTruthy();
    expect(screen.getByText('research_pipeline.supported_claims')).toBeTruthy();
    expect(listenDiscoverySurfacedMock).toHaveBeenCalled();
  });

  it('all controls are explicit type="button" and rows form an aria-live list', async () => {
    render(<LanguageProvider><DiscoveryInbox pushToast={vi.fn()} /></LanguageProvider>);
    await screen.findByText('commit-abc123');
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
    expect(screen.getByRole('list')).toBeTruthy();
    expect(screen.getAllByRole('listitem').length).toBeGreaterThan(0);
  });

  it('acknowledging calls the acknowledge command and removes the row', async () => {
    render(<LanguageProvider><DiscoveryInbox pushToast={vi.fn()} /></LanguageProvider>);
    const pub = await screen.findByText('commit-abc123');
    expect(pub).toBeTruthy();

    fireEvent.click(screen.getByText('Acknowledge'));

    await waitFor(() => {
      expect(
        invokeMock.mock.calls.some(([cmd, args]) =>
          cmd === 'acknowledge_discovery' && (args as { id: number }).id === 7,
        ),
      ).toBe(true);
    });
    await waitFor(() => {
      expect(screen.queryByText('commit-abc123')).toBeNull();
    });
  });
});
