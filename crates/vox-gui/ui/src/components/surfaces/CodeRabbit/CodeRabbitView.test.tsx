// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import React from 'react';

type ProgressEvent = { payload: { status: string; error?: string } };
let progressCb: ((e: ProgressEvent) => void) | null = null;
const listenMock = vi.fn();
vi.mock('@tauri-apps/api/event', () => ({
  listen: (...a: unknown[]) => (listenMock as (...a: unknown[]) => unknown)(...a),
}));

const planMock = vi.fn();
vi.mock('../../../transport', () => ({
  codeRabbitPlan: (...a: unknown[]) => planMock(...a),
  codeRabbitReport: vi.fn().mockResolvedValue({}),
  codeRabbitRunAsync: vi.fn().mockResolvedValue(undefined),
  codeRabbitTokenPresent: vi.fn().mockResolvedValue(true),
}));

import { CodeRabbitView } from './CodeRabbitView';

describe('CodeRabbitView toast shape (B6)', () => {
  beforeEach(() => {
    progressCb = null;
    planMock.mockReset();
    listenMock.mockReset();
    listenMock.mockImplementation((_evt: string, cb: (e: ProgressEvent) => void) => {
      progressCb = cb;
      return Promise.resolve(() => {});
    });
  });

  it('pushes a Toast-shaped success toast when a run finishes', async () => {
    const pushToast = vi.fn();
    render(<CodeRabbitView pushToast={pushToast} />);
    await waitFor(() => expect(progressCb).toBeTruthy());
    await act(async () => {
      progressCb!({ payload: { status: 'ok' } });
    });
    expect(pushToast).toHaveBeenCalledWith({
      tone: 'ok',
      title: 'CodeRabbit run finished',
      cause: 'backend-ok',
    });
  });

  it('pushes a Toast-shaped warn toast when a run fails', async () => {
    const pushToast = vi.fn();
    render(<CodeRabbitView pushToast={pushToast} />);
    await waitFor(() => expect(progressCb).toBeTruthy());
    await act(async () => {
      progressCb!({ payload: { status: 'error', error: 'rate limited' } });
    });
    expect(pushToast).toHaveBeenCalledWith({
      tone: 'warn',
      title: 'CodeRabbit run failed',
      body: 'rate limited',
      cause: 'backend-error',
    });
  });

  it('pushes a Toast-shaped warn toast when planning fails', async () => {
    planMock.mockRejectedValue(new Error('boom'));
    const pushToast = vi.fn();
    render(<CodeRabbitView pushToast={pushToast} />);
    fireEvent.click(screen.getByRole('button', { name: /plan sweep/i }));
    await waitFor(() => expect(pushToast).toHaveBeenCalled());
    const toast = pushToast.mock.calls[0][0];
    expect(toast.tone).toBe('warn');
    expect(toast.title).toBe('Plan failed');
    expect(String(toast.body)).toContain('boom');
    expect(toast.cause).toBe('backend-error');
    expect(toast).not.toHaveProperty('kind');
    expect(toast).not.toHaveProperty('message');
  });

  it('mounts and unmounts without unhandled rejections when the progress listener fails', async () => {
    listenMock.mockReset();
    listenMock.mockRejectedValue(new Error('event bridge unavailable'));
    const { unmount } = render(<CodeRabbitView pushToast={vi.fn()} />);
    await new Promise((r) => setTimeout(r, 0));
    unmount();
    await new Promise((r) => setTimeout(r, 0));
  });
});
