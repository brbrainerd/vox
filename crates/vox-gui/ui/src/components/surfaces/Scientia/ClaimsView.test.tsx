// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';

const CLAIM = {
  claim_id: 1,
  text: 'The widget improves throughput by 12%.',
  is_numeric: true,
  verifiability_score: 0.8,
  verdict: 'Supported',
  confidence: 0.9,
  verifier_model: 'm1',
  created_at_ms: 0,
};

const invokeMock = vi.fn((cmd: string, args?: { path?: string[] }) => {
  if (cmd === 'execute_command' && args?.path?.[1] === 'claims') {
    return Promise.resolve({ exit_code: 0, stdout: JSON.stringify({ claims: [CLAIM] }), stderr: '' });
  }
  return Promise.resolve({ exit_code: 0, stdout: '{}', stderr: '' });
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { ClaimsView } from './ClaimsView';

describe('ClaimsView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('action buttons are explicit type="button"', () => {
    render(<ClaimsView pushToast={vi.fn()} />);
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('renders loaded claims inside an aria-live list', async () => {
    render(<ClaimsView pushToast={vi.fn()} />);
    fireEvent.change(screen.getByPlaceholderText('publication id'), { target: { value: 'pub-1' } });
    fireEvent.click(screen.getByText('Load'));
    const claim = await screen.findByText('The widget improves throughput by 12%.');
    expect(claim).toBeTruthy();
    await waitFor(() => {
      expect(screen.getByRole('list')).toBeTruthy();
    });
  });
});
