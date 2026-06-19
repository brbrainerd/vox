// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../../../transport', () => ({
  voxTransport: {
    getModelPool: vi.fn(),
    setModelPool: vi.fn(),
    listEnabledProviders: vi.fn(),
  },
}));

vi.mock('../../ui/Glass', () => ({
  Glass: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div data-testid="glass" className={className}>{children}</div>
  ),
}));

import { voxTransport } from '../../../transport';
import { ModelPoolView } from './ModelPoolView';

const mockTransport = voxTransport as { getModelPool: ReturnType<typeof vi.fn>; setModelPool: ReturnType<typeof vi.fn>; listEnabledProviders: ReturnType<typeof vi.fn> };

const basePool = {
  rules: [],
  includes: [],
  excludes: [],
  disabled_sources: [],
  member_ids: ['openrouter/gemini-flash', 'anthropic/claude-sonnet'],
  fell_open: false,
};

beforeEach(() => {
  mockTransport.getModelPool.mockResolvedValue(basePool);
  mockTransport.listEnabledProviders.mockResolvedValue(['openrouter', 'anthropic', 'ollama']);
  mockTransport.setModelPool.mockResolvedValue(undefined);
});

describe('ModelPoolView', () => {
  it('shows member count after load', async () => {
    render(<ModelPoolView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/2 models in pool/)).toBeInTheDocument());
    expect(screen.getByText(/3 providers enabled/)).toBeInTheDocument();
  });

  it('renders member ids as chips', async () => {
    render(<ModelPoolView pushToast={vi.fn()} />);
    await waitFor(() => screen.getByText('openrouter/gemini-flash'));
    expect(screen.getByText('anthropic/claude-sonnet')).toBeInTheDocument();
  });

  it('shows fallback note when fell_open is true', async () => {
    mockTransport.getModelPool.mockResolvedValue({ ...basePool, fell_open: true });
    render(<ModelPoolView pushToast={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/Pool resolved empty/)).toBeInTheDocument()
    );
  });

  it('shows exclude with remove button and calls setModelPool on click', async () => {
    const user = userEvent.setup();
    mockTransport.getModelPool.mockResolvedValue({
      ...basePool,
      excludes: ['bad/model'],
    });
    render(<ModelPoolView pushToast={vi.fn()} />);
    await waitFor(() => screen.getByText('bad/model'));
    await user.click(screen.getByLabelText('Remove exclusion bad/model'));
    expect(mockTransport.setModelPool).toHaveBeenCalledWith(
      expect.objectContaining({ excludes: [] })
    );
  });

  it('renders rule label for free rule', async () => {
    mockTransport.getModelPool.mockResolvedValue({
      ...basePool,
      rules: [{ kind: 'free' }],
    });
    render(<ModelPoolView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('Free models only')).toBeInTheDocument());
  });
});
