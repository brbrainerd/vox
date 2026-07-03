// @vitest-environment jsdom
import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { NeedsYouSurface } from '../NeedsYouSurface';
import * as transport from '../../../../transport';
import { LanguageProvider } from '../../../../hooks/useLanguage';

beforeEach(() => {
  vi.spyOn(transport, 'feedbackList').mockResolvedValue({
    needsYou: [
      {
        feedbackId: 'F-1',
        kind: 'clarification',
        prompt: 'schema?',
        options: ['a'],
        gates: [7],
        doubtedTaskId: null,
        surface: 'needs_you',
        infoGainBits: 0.8,
      },
    ],
    withheld: [
      {
        feedbackId: 'F-9',
        kind: 'clarification',
        prompt: 'low',
        options: [],
        gates: [],
        doubtedTaskId: null,
        surface: 'withheld',
        infoGainBits: 0.05,
      },
    ],
  });
  vi.spyOn(transport, 'listenFeedbackChanged').mockResolvedValue(() => {});
});

describe('NeedsYouSurface', () => {
  it('lists open items + withheld section', async () => {
    render(<LanguageProvider><NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} /></LanguageProvider>);
    await waitFor(() => expect(screen.getByText('schema?')).toBeTruthy());
    expect(screen.getByText(/Withheld by policy/i)).toBeTruthy();
  });

  it('empty state when nothing needs you', async () => {
    vi.spyOn(transport, 'feedbackList').mockResolvedValue({ needsYou: [], withheld: [] });
    render(<LanguageProvider><NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} /></LanguageProvider>);
    await waitFor(() => expect(screen.getByText(/Nothing needs you/i)).toBeTruthy());
  });

  const attention = {
    approvals: [{ approval_id: 'A-1', tool: 'bash', summary: 'rm -rf build', requested_at_ms: 0 }],
    needsYou: [], withheld: [], blockedTasksCount: 0, totalCount: 1,
    refresh: vi.fn(), resolveApproval: vi.fn().mockResolvedValue(undefined), resolveFeedback: vi.fn().mockResolvedValue(undefined),
  };

  it('renders an Approvals section from the shared inbox and resolves inline', async () => {
    render(<LanguageProvider><NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} attention={attention} /></LanguageProvider>);
    expect(await screen.findByText('rm -rf build')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /approve rm -rf build|^approve$/i }));
    await waitFor(() => expect(attention.resolveApproval).toHaveBeenCalledWith('A-1', 'approved'));
  });

  it('does not start its own poll when the shared inbox is provided', async () => {
    const spy = vi.spyOn(transport, 'feedbackList');
    render(<LanguageProvider><NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} attention={{ ...attention, approvals: [] }} /></LanguageProvider>);
    await waitFor(() => expect(screen.getByText(/Nothing needs you/i)).toBeTruthy());
    expect(spy).not.toHaveBeenCalled();
  });
});
