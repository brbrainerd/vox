// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, act } from '@testing-library/react';
import React from 'react';

const { mockSecretaryPayload, getSecretaryEventHandler, setSecretaryEventHandler } = vi.hoisted(() => {
  let secretaryEventHandler: ((event: { payload: any }) => void) | null = null;
  return {
    mockSecretaryPayload: {
      item_id: 'item-abc',
      intent: 'Fix the broken authentication flow in the login page today',
      confidence_pct: 85,
    },
    getSecretaryEventHandler: () => secretaryEventHandler,
    setSecretaryEventHandler: (handler: any) => {
      secretaryEventHandler = handler;
    },
  };
});

vi.mock('../../../transport', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../../transport')>();
  return {
    ...actual,
    listenSecretaryProposed: vi.fn((handler: (payload: any) => void) => {
      setSecretaryEventHandler((event: any) => handler(event.payload));
      return Promise.resolve(() => {});
    }),
  };
});



const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const noopToast = () => {};

import { ChatSurface } from './ChatSurface';
import type { ChatMessage } from '../../../lib/chatCorrelation';


describe('ChatSurface', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') {
        return Promise.resolve([
          { session_id: 's1', title: 'First', message_count: 2 },
          { session_id: 's2', title: 'Second', message_count: 0 },
        ]);
      }
      return Promise.resolve(null);
    });
  });

  it('marks the active session tab with aria-pressed', async () => {
    render(<ChatSurface pushToast={noopToast} activeSessionId="s1" />);
    const active = await screen.findByRole('tab', { name: /First/ });
    expect(active.getAttribute('aria-pressed')).toBe('true');
    const inactive = screen.getByRole('tab', { name: /Second/ });
    expect(inactive.getAttribute('aria-pressed')).toBe('false');
  });

  it('gives the New session button an explicit type', async () => {
    render(<ChatSurface pushToast={noopToast} activeSessionId="s1" />);
    const newBtn = await screen.findByRole('button', { name: /new chat session/i });
    expect(newBtn.getAttribute('type')).toBe('button');
  });

  it('renders an empty state when the session has no messages', async () => {
    render(<ChatSurface pushToast={noopToast} activeSessionId="s1" messages={[]} />);
    await waitFor(() => {
      expect(screen.getByText(/no messages yet/i)).toBeDefined();
    });
  });

  it('exposes the transcript as a polite log region when messages exist', async () => {
    const messages: ChatMessage[] = [
      { id: 'm1', role: 'user', text: 'hi', status: 'done' } as ChatMessage,
    ];
    render(<ChatSurface pushToast={noopToast} activeSessionId="s1" messages={messages} />);
    const log = await screen.findByRole('log', { name: /chat transcript/i });
    expect(log.getAttribute('aria-live')).toBe('polite');
  });

  it('renders an agent event row when agentStreamItems is provided', async () => {
    render(
      <ChatSurface
        pushToast={noopToast}
        activeSessionId="s1"
        messages={[]}
        agentStreamItems={[
          {
            id: 'evt-1',
            kind: 'agent',
            tag: 'TASK',
            title: 'TASK · task 7',
            body: 'agent agent-1',
            ts: '12:00',
            metadata: { eventType: 'task_started', agentId: 'agent-1', timestampMs: 1000 },
          },
        ]}
      />,
    );
    await waitFor(() => {
      expect(screen.getByTestId('chat-agent-event-row')).toBeDefined();
    });
    expect(screen.getByText(/TASK · task 7/i)).toBeDefined();
  });

  it('renders embedded composer when composer slot is provided', async () => {
    render(
      <ChatSurface
        pushToast={noopToast}
        activeSessionId="s1"
        composer={<div data-testid="loquela-composer">composer</div>}
      />,
    );
    await waitFor(() => {
      expect(screen.getByTestId('loquela-composer')).toBeDefined();
    });
  });

  it('shows SecretaryToast when secretary-proposed-task event fires', async () => {
    const onNavigate = vi.fn();
    render(<ChatSurface pushToast={noopToast} onNavigate={onNavigate} activeSessionId="s1" />);
    // Wait for the listen subscriptions to be set up
    await waitFor(() => expect(getSecretaryEventHandler()).not.toBeNull());
    // Simulate the event
    act(() => {
      getSecretaryEventHandler()!({ payload: mockSecretaryPayload });
    });
    // Toast should appear
    await waitFor(() => {
      expect(screen.getByTestId('secretary-toast-intent')).toBeInTheDocument();
      expect(screen.getByText(/Fix the broken authentication/)).toBeInTheDocument();
    });
  });
});


