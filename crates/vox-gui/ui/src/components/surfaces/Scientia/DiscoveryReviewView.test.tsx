// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import React from 'react';

const CLAIM = {
  claim_id: 5,
  text: 'Latency dropped by 30ms.',
  is_numeric: true,
  verifiability_score: 0.7,
  verdict: null,
  confidence: 0.8,
  verifier_model: null,
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

import { DiscoveryReviewView } from './DiscoveryReviewView';

describe('DiscoveryReviewView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('all buttons are explicit type="button"', async () => {
    render(<DiscoveryReviewView pushToast={vi.fn()} />);
    fireEvent.change(screen.getByPlaceholderText('publication id'), { target: { value: 'pub-1' } });
    fireEvent.click(screen.getByText('Load claims'));
    await screen.findByText('Latency dropped by 30ms.');
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('renders loaded claims inside an aria-live list', async () => {
    render(<DiscoveryReviewView pushToast={vi.fn()} />);
    fireEvent.change(screen.getByPlaceholderText('publication id'), { target: { value: 'pub-1' } });
    fireEvent.click(screen.getByText('Load claims'));
    await screen.findByText('Latency dropped by 30ms.');
    expect(screen.getByRole('list')).toBeTruthy();
  });
});
