// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const CARDS = [
  { id: 'openai/gpt-x', provider: 'openai', tier: 'frontier', cost_per_1k: 0.01, max_tokens: 128000, is_free: false, latency_p50_ms: 800 },
  { id: 'ollama/llama', provider: 'ollama', tier: 'local', cost_per_1k: 0, max_tokens: 8000, is_free: true, latency_p50_ms: 50 },
];
const SUMMARY = {
  active_model: 'openai/gpt-x',
  exploration_spent_usd: 1.2,
  exploration_budget_usd: 10,
  arm_count: 4,
  model_count: 2,
  decision_preview: null,
};

const invokeMock = vi.fn((cmd: string) => {
  if (cmd === 'list_model_cards') return Promise.resolve(CARDS);
  if (cmd === 'get_routing_summary_live') return Promise.resolve(SUMMARY);
  if (cmd === 'get_active_model') return Promise.resolve('openai/gpt-x');
  if (cmd === 'set_active_model') return Promise.resolve(null);
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { ModelsView } from './ModelsView';

describe('ModelsView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders the Model Registry heading', () => {
    render(<ModelsView pushToast={vi.fn()} />);
    expect(screen.getByText('Model Registry')).toBeTruthy();
  });

  it('every button carries an explicit type="button"', async () => {
    render(<ModelsView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByText('Set active').length).toBeGreaterThan(0));
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('marks the active model card with aria-pressed and aria-current', async () => {
    render(<ModelsView pushToast={vi.fn()} />);
    const active = await screen.findByLabelText('Set openai/gpt-x as active model (currently active)');
    expect(active.getAttribute('aria-pressed')).toBe('true');
  });

  it('exposes the model list with role=list/listitem', async () => {
    render(<ModelsView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByRole('list').length).toBeGreaterThan(0));
    expect(screen.getAllByRole('listitem').length).toBe(CARDS.length);
  });
});
