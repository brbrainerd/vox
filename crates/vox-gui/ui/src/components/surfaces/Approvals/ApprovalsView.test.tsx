// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';

const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const noopToast = () => {};

import { ApprovalsView } from './ApprovalsView';
import { LanguageProvider } from '../../../hooks/useLanguage';

function envelope(approvals: unknown[]) {
  return {
    is_error: false,
    result: { approvals },
  };
}

describe('ApprovalsView', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('shows the empty state when there are no pending approvals', async () => {
    invokeMock.mockResolvedValue(envelope([]));
    render(<LanguageProvider><ApprovalsView pushToast={noopToast} /></LanguageProvider>);
    await waitFor(() => {
      expect(screen.getByText(/No pending approvals/i)).toBeDefined();
    });
  });

  it('renders the approval queue as a polite live region', async () => {
    invokeMock.mockResolvedValue(
      envelope([
        { approval_id: 'ap-1', tool: 'shell', summary: 'rm -rf', requested_at_ms: Date.now() },
      ]),
    );
    render(<LanguageProvider><ApprovalsView pushToast={noopToast} /></LanguageProvider>);
    await waitFor(() => {
      expect(screen.getByText('rm -rf')).toBeDefined();
    });
    const region = screen.getByText('rm -rf').closest('[aria-live="polite"]');
    expect(region).not.toBeNull();
  });

  it('gives the approve/reject buttons accessible labels and explicit type', async () => {
    invokeMock.mockResolvedValue(
      envelope([
        { approval_id: 'ap-1', tool: 'shell', summary: 'do thing', requested_at_ms: Date.now() },
      ]),
    );
    render(<LanguageProvider><ApprovalsView pushToast={noopToast} /></LanguageProvider>);
    const approve = await screen.findByRole('button', { name: /approve do thing|approve ap-1/i });
    const reject = await screen.findByRole('button', { name: /reject do thing|reject ap-1/i });
    expect(approve.getAttribute('type')).toBe('button');
    expect(reject.getAttribute('type')).toBe('button');
  });

  it('does not leak a raw TypeError toast when vox_pending_approvals resolves null', async () => {
    invokeMock.mockResolvedValue(null);
    const toasts: any[] = [];
    render(<LanguageProvider><ApprovalsView pushToast={(t: any) => toasts.push(t)} /></LanguageProvider>);
    await waitFor(() => {
      expect(screen.getByText(/No pending approvals/i)).toBeDefined();
    });
    // No leaked "res is null" / "is_error" TypeError text anywhere in the toasts or the DOM.
    expect(toasts.some((t) => /is_error|typeerror|res is null/i.test(String(t.body)))).toBe(false);
    expect(screen.queryByText(/is_error|TypeError|res is null/i)).toBeNull();
  });

  it('does not leak a raw TypeError toast when vox_resolve_approval resolves null', async () => {
    invokeMock.mockImplementation((_cmd: string, args: any) => {
      if (args?.tool === 'vox_pending_approvals') {
        return Promise.resolve(
          envelope([{ approval_id: 'ap-1', tool: 'shell', summary: 'do thing', requested_at_ms: Date.now() }]),
        );
      }
      if (args?.tool === 'vox_resolve_approval') {
        return Promise.resolve(null);
      }
      return Promise.resolve(envelope([]));
    });
    const toasts: any[] = [];
    render(<LanguageProvider><ApprovalsView pushToast={(t: any) => toasts.push(t)} /></LanguageProvider>);
    const approve = await screen.findByRole('button', { name: /approve do thing|approve ap-1/i });
    approve.click();
    await waitFor(() => {
      expect(toasts.some((t) => t.title === 'Resolve failed')).toBe(true);
    });
    expect(toasts.some((t) => /is_error|typeerror|res is null/i.test(String(t.body)))).toBe(false);
  });

  it('renders ApprovalsView columns with appropriate headers', async () => {
    invokeMock.mockResolvedValue(
      envelope([
        { approval_id: 'ap-1', tool: 'shell', summary: 'do thing', requested_at_ms: Date.now() },
      ]),
    );
    render(<LanguageProvider><ApprovalsView pushToast={vi.fn()} /></LanguageProvider>);
    await waitFor(() => {
      expect(screen.getByText('Request ID')).toBeDefined();
      expect(screen.getByText('Action Description')).toBeDefined();
    });
  });
});
