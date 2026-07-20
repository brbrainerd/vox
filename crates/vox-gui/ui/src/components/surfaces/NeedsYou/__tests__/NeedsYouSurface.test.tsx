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

  it('an approval-resolve failure produces a sanitized toast, not a raw error leak (F-02/F-03)', async () => {
    const pushToast = vi.fn();
    const failingAttention = {
      ...attention,
      resolveApproval: vi.fn().mockRejectedValue(new TypeError(`can't access property "invoke", window.__TAURI_INTERNALS__ is undefined`)),
    };
    render(<LanguageProvider><NeedsYouSurface onOpenContext={() => {}} pushToast={pushToast} attention={failingAttention} /></LanguageProvider>);
    fireEvent.click(await screen.findByRole('button', { name: /approve rm -rf build|^approve$/i }));
    await waitFor(() => expect(pushToast).toHaveBeenCalled());
    const bodies = pushToast.mock.calls.map(c => String(c[0]?.body ?? ''));
    for (const body of bodies) {
      expect(body).not.toMatch(/__TAURI_INTERNALS__|\binvoke\b/i);
    }
  });

  it('a feedbackList failure sets a sanitized error message, not a raw leak (F-02/F-03)', async () => {
    vi.spyOn(transport, 'feedbackList').mockRejectedValue(
      new TypeError(`can't access property "invoke", window.__TAURI_INTERNALS__ is undefined`),
    );
    render(<LanguageProvider><NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} /></LanguageProvider>);
    await waitFor(() => expect(screen.getByText(/error loading feedback/i)).toBeTruthy());
    expect(screen.queryByText(/__TAURI_INTERNALS__|\binvoke\b/i)).toBeNull();
  });

  it('unlistens immediately when unmounted before listenFeedbackChanged resolves (leak guard)', async () => {
    const unlisten = vi.fn();
    let resolveListen!: (u: () => void) => void;
    vi.spyOn(transport, 'listenFeedbackChanged').mockImplementation(
      () => new Promise((res) => { resolveListen = res; }),
    );
    const { unmount } = render(
      <LanguageProvider>
        <NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} />
      </LanguageProvider>,
    );
    await waitFor(() => expect(resolveListen).toBeTruthy());
    unmount();
    resolveListen(unlisten);
    await waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });
});
