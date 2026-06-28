// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';

vi.mock('../../transport', () => ({
  voxTransport: {
    voxDocsIndex: vi.fn().mockResolvedValue([]),
    openLocator: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../../hooks/useSearchController', () => ({
  useSearchController: vi.fn(() => ({
    state: { query: 'brain', hits: [], loading: false, scopes: ['code'], requestToken: 1 },
    setQuery: vi.fn(),
    setScopes: vi.fn(),
  })),
}));

const mockFederatedSearch = vi.fn(() => ({
  entries: [] as unknown[],
  search: vi.fn(() => [] as unknown[]),
}));

vi.mock('../../hooks/useFederatedSearchIndex', () => ({
  useFederatedSearchIndex: (...args: unknown[]) => mockFederatedSearch(...args),
}));

import { CommandPalette } from './CommandPalette';
import { useSearchController } from '../../hooks/useSearchController';
import type { FederatedIndexEntry } from '../../lib/federatedSearchIndex';

const policyHit: FederatedIndexEntry = {
  kind: 'policy',
  id: 'policy:fmt.rust',
  label: 'fmt.rust',
  detail: 'pass',
  payload: { type: 'policy', policyId: 'fmt.rust' },
};

const commandHit: FederatedIndexEntry = {
  kind: 'command',
  id: 'command:fmt',
  label: 'fmt',
  detail: 'Format Rust sources',
  payload: { type: 'command', command: 'fmt' },
};

describe('CommandPalette', () => {
  beforeEach(() => {
    vi.mocked(useSearchController).mockClear();
    mockFederatedSearch.mockClear();
    mockFederatedSearch.mockReturnValue({
      entries: [policyHit],
      search: vi.fn((query: string) =>
        query.toLowerCase().includes('fmt') ? [policyHit] : [],
      ),
    });
  });

  it('delegates backend search to useSearchController', () => {
    render(
      <CommandPalette
        open
        agents={[]}
        skills={[]}
        onClose={vi.fn()}
        onAction={vi.fn()}
      />,
    );
    expect(useSearchController).toHaveBeenCalled();
  });

  it('/ prefix surfaces installed skills in skills mode', () => {
    render(
      <CommandPalette
        open
        agents={[]}
        skills={[
          {
            path: ['skill', 'brainstorming'],
            command: 'brainstorming',
            about: 'Design before implementation',
            aliases: [],
            has_subcommands: false,
            compiled_in: true,
            source_group: 'skill',
            feature_gate: null,
            tier: 'recommended',
            capability_id: 'brainstorming',
          },
        ]}
        onClose={vi.fn()}
        onAction={vi.fn()}
      />,
    );
    const input = screen.getByPlaceholderText(/Search commands/i);
    fireEvent.change(input, { target: { value: '/brain' } });
    expect(screen.getByText('brainstorming')).toBeTruthy();
    expect(screen.getByText('Design before implementation')).toBeTruthy();
  });

  it('shows prefix legend when query is empty', () => {
    render(
      <CommandPalette
        open
        agents={[]}
        skills={[]}
        onClose={vi.fn()}
        onAction={vi.fn()}
      />,
    );
    const legend = screen.getByTestId('palette-prefix-legend');
    expect(legend.textContent).toContain('> commands');
    expect(legend.textContent).toContain('@ agents');
    expect(legend.textContent).toContain('/ docs+skills');
  });

  it('shows federated command row when catalog has fmt command', () => {
    mockFederatedSearch.mockReturnValue({
      entries: [commandHit],
      search: vi.fn(() => [commandHit]),
    });
    render(
      <CommandPalette
        open
        agents={[]}
        skills={[]}
        onClose={vi.fn()}
        onAction={vi.fn()}
      />,
    );
    const input = screen.getByPlaceholderText(/Search commands/i);
    fireEvent.change(input, { target: { value: 'fmt' } });
    expect(screen.getByText('fmt')).toBeTruthy();
    expect(screen.getByText('Commands')).toBeTruthy();
    expect(screen.getByText('Format Rust sources')).toBeTruthy();
  });

  it('shows federated policy row when query matches fmt', () => {
    render(
      <CommandPalette
        open
        agents={[]}
        skills={[]}
        onClose={vi.fn()}
        onAction={vi.fn()}
      />,
    );
    const input = screen.getByPlaceholderText(/Search commands/i);
    fireEvent.change(input, { target: { value: 'fmt' } });
    expect(screen.getByText('fmt.rust')).toBeTruthy();
    expect(screen.getByText('Policies')).toBeTruthy();
  });

  it('Enter on federated policy row navigates to policies view', async () => {
    const onAction = vi.fn();
    render(
      <CommandPalette
        open
        agents={[]}
        skills={[]}
        onClose={vi.fn()}
        onAction={onAction}
      />,
    );
    const input = screen.getByPlaceholderText(/Search commands/i);
    fireEvent.change(input, { target: { value: 'fmt' } });
    await waitFor(() => expect(screen.getByText('fmt.rust')).toBeTruthy());
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    expect(onAction).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'navigate', viewKey: 'policies' }),
    );
  });

  it('clicking federated policy row navigates to policies view', async () => {
    const onAction = vi.fn();
    render(
      <CommandPalette
        open
        agents={[]}
        skills={[]}
        onClose={vi.fn()}
        onAction={onAction}
      />,
    );
    const input = screen.getByPlaceholderText(/Search commands/i);
    fireEvent.change(input, { target: { value: 'fmt' } });
    await waitFor(() => expect(screen.getByText('fmt.rust')).toBeTruthy());
    fireEvent.click(screen.getByText('fmt.rust'));
    expect(onAction).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'navigate', viewKey: 'policies' }),
    );
  });
});
