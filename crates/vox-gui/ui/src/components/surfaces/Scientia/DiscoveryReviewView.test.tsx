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
  if (cmd === 'execute_command' && args?.path?.[1] === 'publication-claim-review') {
    return Promise.resolve({ exit_code: 0, stdout: '{}', stderr: '' });
  }
  if (cmd === 'execute_command' && args?.path?.[1] === 'publication-nanopub-build') {
    return Promise.resolve({
      exit_code: 0,
      stdout: 'http://purl.org/np/RAaBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789',
      stderr: '',
    });
  }
  return Promise.resolve({ exit_code: 0, stdout: '{}', stderr: '' });
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

const recordGamifyMock = vi.fn().mockResolvedValue(null);
vi.mock('../../../lib/gamifyGuiEvents', () => ({
  recordGamifyGuiEvent: (...args: unknown[]) => recordGamifyMock(...args),
}));

import { DiscoveryReviewView } from './DiscoveryReviewView';

describe('DiscoveryReviewView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
    recordGamifyMock.mockClear();
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

  it('fires claim_approved when approve review succeeds', async () => {
    render(<DiscoveryReviewView pushToast={vi.fn()} gamifyEnabled />);
    fireEvent.change(screen.getByPlaceholderText('publication id'), { target: { value: 'pub-1' } });
    fireEvent.click(screen.getByText('Load claims'));
    await screen.findByText('Latency dropped by 30ms.');
    fireEvent.click(screen.getByText('Approve'));
    await vi.waitFor(() => {
      expect(recordGamifyMock).toHaveBeenCalledWith(
        'claim_approved',
        { publication_id: 'pub-1', claim_id: 5 },
        { enabled: true },
      );
    });
  });

  it('fires nanopub_built when build succeeds after approval', async () => {
    render(<DiscoveryReviewView pushToast={vi.fn()} gamifyEnabled />);
    fireEvent.change(screen.getByPlaceholderText('publication id'), { target: { value: 'pub-1' } });
    fireEvent.click(screen.getByText('Load claims'));
    await screen.findByText('Latency dropped by 30ms.');
    fireEvent.click(screen.getByText('Approve'));
    await screen.findByText('approve');
    fireEvent.click(screen.getByText('Build nanopub'));
    await vi.waitFor(() => {
      expect(recordGamifyMock).toHaveBeenCalledWith(
        'nanopub_built',
        expect.objectContaining({ publication_id: 'pub-1', claim_id: 5 }),
        { enabled: true },
      );
    });
  });
});
