// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, act, fireEvent } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
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
import { LanguageProvider } from '../../../hooks/useLanguage';


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

  it('has exactly one accessible h1 for the surface root (axe page-has-heading-one)', async () => {
    render(
      <LanguageProvider>
        <ChatSurface pushToast={() => {}} activeSessionId="s1" />
      </LanguageProvider>,
    );
    expect(await screen.findAllByRole('heading', { level: 1 })).toHaveLength(1);
  });

  it('marks the active session tab with aria-pressed', async () => {
    render(<LanguageProvider><ChatSurface pushToast={noopToast} activeSessionId="s1" /></LanguageProvider>);
    const active = await screen.findByRole('tab', { name: /First/ });
    expect(active.getAttribute('aria-pressed')).toBe('true');
    const inactive = screen.getByRole('tab', { name: /Second/ });
    expect(inactive.getAttribute('aria-pressed')).toBe('false');
  });

  it('gives the New session button an explicit type', async () => {
    render(<LanguageProvider><ChatSurface pushToast={noopToast} activeSessionId="s1" /></LanguageProvider>);
    const newBtn = await screen.findByRole('button', { name: /new chat session/i });
    expect(newBtn.getAttribute('type')).toBe('button');
  });

  it('renders an empty state when the session has no messages', async () => {
    render(<LanguageProvider><ChatSurface pushToast={noopToast} activeSessionId="s1" messages={[]} /></LanguageProvider>);
    await waitFor(() => {
      expect(screen.getByText(/no messages yet/i)).toBeDefined();
    });
  });

  it('updates the transcript panel content when messages change (does not go stale after first render)', async () => {
    const { rerender } = render(
      <LanguageProvider>
        <ChatSurface pushToast={noopToast} activeSessionId="s1" messages={[]} />
      </LanguageProvider>,
    );
    rerender(
      <LanguageProvider>
        <ChatSurface
          pushToast={noopToast}
          activeSessionId="s1"
          messages={[{ id: 'm1', role: 'user', text: 'hello', status: 'done' } as ChatMessage]}
        />
      </LanguageProvider>,
    );
    expect(await screen.findByText('hello')).toBeInTheDocument();
  });

  it('exposes the transcript as a polite log region when messages exist', async () => {
    const messages: ChatMessage[] = [
      { id: 'm1', role: 'user', text: 'hi', status: 'done' } as ChatMessage,
    ];
    render(<LanguageProvider><ChatSurface pushToast={noopToast} activeSessionId="s1" messages={messages} /></LanguageProvider>);
    const log = await screen.findByRole('log', { name: /chat transcript/i });
    expect(log.getAttribute('aria-live')).toBe('polite');
  });

  it('renders a status line (not raw event rows) when a task is in-flight', async () => {
    render(
      <LanguageProvider>
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
              taskId: 7,
              metadata: { eventType: 'task_started', agentId: 'agent-1', timestampMs: 1000 },
            },
          ]}
        />
      </LanguageProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId('chat-status-line')).toBeDefined();
    });
    expect(screen.queryByTestId('chat-agent-event-row')).toBeNull();
  });

  it('renders embedded composer when composer slot is provided', async () => {
    render(
      <LanguageProvider>
        <ChatSurface
          pushToast={noopToast}
          activeSessionId="s1"
          composer={<div data-testid="loquela-composer">composer</div>}
        />
      </LanguageProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId('loquela-composer')).toBeDefined();
    });
  });

  it('does not render its own model pill when a composer slot is provided', async () => {
    // The model-route picker used to be a second row ChatSurface rendered
    // below the composer. It now lives inside the composer's own toolbar
    // row (Loquela's `trailingSlot`, wired by App.tsx) — with a real
    // Loquela this test's plain-div composer stub can't exercise that, but
    // ChatSurface itself must no longer render a duplicate/stray picker.
    render(
      <LanguageProvider>
        <ChatSurface
          pushToast={noopToast}
          activeSessionId="s1"
          composer={<div data-testid="loquela-composer">composer</div>}
        />
      </LanguageProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId('loquela-composer')).toBeDefined();
    });
    expect(screen.queryByRole('button', { name: /^model:/i })).toBeNull();
  });

  it('transcript fills the column (flex-1) instead of a max-h vh cap', async () => {
    const messages: ChatMessage[] = [
      { id: 'm1', role: 'user', text: 'hi', status: 'done' } as ChatMessage,
    ];
    render(<LanguageProvider><ChatSurface pushToast={noopToast} activeSessionId="s1" messages={messages} /></LanguageProvider>);
    const log = await screen.findByRole('log', { name: /chat transcript/i });
    expect(log.className).toContain('flex-1');
    expect(log.className).toContain('min-h-0');
    expect(log.className).not.toContain('max-h-[40vh]');
  });

  it('shows SecretaryToast when secretary-proposed-task event fires', async () => {
    const onNavigate = vi.fn();
    render(<LanguageProvider><ChatSurface pushToast={noopToast} onNavigate={onNavigate} activeSessionId="s1" /></LanguageProvider>);
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

  it('shows AttentionBudgetMeter above composer when budget present', async () => {
    render(
      <LanguageProvider>
        <ChatSurface
          pushToast={noopToast}
          activeSessionId="s1"
          composer={<div data-testid="loquela-composer">composer</div>}
          attention_budget={{
            max_attention_ms: 600_000,
            spent_ms: 300_000,
            total_requests: 10,
            auto_approved: 8,
            rejected: 1,
            interrupt_freq_per_hour: 5,
            last_interrupt_ms: 0,
            inbox_suppressed_count: 2,
          }}
        />
      </LanguageProvider>,
    );
    await waitFor(() => {
      expect(screen.getByRole('meter', { name: /attention spent/i })).toBeDefined();
    });
    expect(screen.getByTestId('chat-attention-meter')).toBeDefined();
  });

  it('mounts sessions, chat, and execution rail as dockview panels', async () => {
    render(
      <LanguageProvider>
        <ChatSurface
          pushToast={noopToast}
          onNavigate={vi.fn()}
          activeSessionId="s1"
          messages={[{ id: 'm1', role: 'user', text: 'hi', status: 'done' } as ChatMessage]}
          composer={<div>composer</div>}
        />
      </LanguageProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId('chat-dock-sessions')).toBeInTheDocument();
      expect(screen.getByTestId('chat-dock-transcript')).toBeInTheDocument();
      expect(screen.getByTestId('chat-dock-execution-rail')).toBeInTheDocument();
    });
  });

  it('the transcript panel has no visible tab strip (pinned, not a normal closable/draggable panel)', async () => {
    render(
      <LanguageProvider>
        <ChatSurface pushToast={noopToast} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
      </LanguageProvider>,
    );
    await screen.findByTestId('chat-dock-transcript');
    expect(screen.queryByText('Chat', { selector: '.dv-default-tab-content' })).toBeNull();
  });

  it('prevents a native dragstart originating from the transcript panel tab (EmptyTab hides chrome but dockview-core still sets draggable=true unconditionally)', async () => {
    render(
      <LanguageProvider>
        <ChatSurface
          pushToast={noopToast}
          onNavigate={vi.fn()}
          activeSessionId="s1"
          messages={[]}
          agentStreamItems={[]}
          composer={<div>composer</div>}
        />
      </LanguageProvider>,
    );
    await screen.findByTestId('chat-dock-transcript');
    const transcriptTabMarker = await screen.findByTestId('chat-dock-transcript-tab-marker');
    const transcriptTab = transcriptTabMarker.closest('.dv-tab') as HTMLElement;
    expect(transcriptTab).not.toBeNull();

    const event = new Event('dragstart', { bubbles: true, cancelable: true }) as DragEvent;
    Object.defineProperty(event, 'dataTransfer', {
      value: { setDragImage: vi.fn(), setData: vi.fn(), getData: vi.fn(), types: [], items: [], effectAllowed: '' },
    });
    act(() => {
      transcriptTab.dispatchEvent(event);
    });
    expect(event.defaultPrevented).toBe(true);
  });

  it('does not prevent dragstart from an unrelated panel tab (Flow) — the fix is scoped to the transcript panel only', async () => {
    render(
      <LanguageProvider>
        <ChatSurface
          pushToast={noopToast}
          onNavigate={vi.fn()}
          activeSessionId="s1"
          messages={[]}
          agentStreamItems={[]}
          composer={<div>composer</div>}
        />
      </LanguageProvider>,
    );
    await screen.findByTestId('chat-dock-flow');
    const flowTab = screen.getByText('Flow').closest('.dv-tab') as HTMLElement;
    expect(flowTab).not.toBeNull();

    const event = new Event('dragstart', { bubbles: true, cancelable: true }) as DragEvent;
    Object.defineProperty(event, 'dataTransfer', {
      value: { setDragImage: vi.fn(), setData: vi.fn(), getData: vi.fn(), types: [], items: [], effectAllowed: '' },
    });
    act(() => {
      flowTab.dispatchEvent(event);
    });
    expect(event.defaultPrevented).toBe(false);
  });

  it('mounts a Flow panel dockable alongside chat, using the same agent data as the top-level Flow tab', async () => {
    render(
      <LanguageProvider>
        <ChatSurface
          pushToast={noopToast}
          onNavigate={vi.fn()}
          activeSessionId="s1"
          messages={[]}
          agentStreamItems={[]}
          composer={<div>composer</div>}
        />
      </LanguageProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId('chat-dock-flow')).toBeInTheDocument();
    });
  });

  it('does not resurrect the Flow panel on the next render after the user closes it', async () => {
    const { rerender } = render(
      <LanguageProvider>
        <ChatSurface
          pushToast={vi.fn()}
          onNavigate={vi.fn()}
          messages={[]}
          composer={<div>composer</div>}
        />
      </LanguageProvider>,
    );
    await screen.findByTestId('chat-dock-flow');

    // Close the way a user would: find the real dockview tab close action.
    // NOTE: this app's ChatDockShell does not register a custom React tab
    // component, so dockview falls back to dockview-core's vanilla
    // `DefaultTab` (built via plain `document.createElement`, not the
    // `dockview-react` `DockviewDefaultTab` component) — confirmed via
    // screen.debug() against the actual rendered tree. That vanilla tab has
    // no `data-testid` (only `dockview-react`'s version sets one); the class
    // structure the plan documented is otherwise accurate: `.dv-default-tab`
    // contains a `.dv-default-tab-content` and a sibling `.dv-default-tab-action`
    // close target.
    const flowTab = screen.getByText('Flow').closest('.dv-default-tab') as HTMLElement;
    const closeBtn = flowTab.querySelector('.dv-default-tab-action') as HTMLElement;
    fireEvent.click(closeBtn);
    await waitFor(() => expect(screen.queryByTestId('chat-dock-flow')).toBeNull());

    // Force an unrelated re-render — the exact trigger a real close-fighting
    // bug would react to (a streamed token, a session poll, anything that
    // isn't the user reopening the panel).
    rerender(
      <LanguageProvider>
        <ChatSurface
          pushToast={vi.fn()}
          onNavigate={vi.fn()}
          messages={[{ id: 'm1', role: 'user', text: 'hello', status: 'done' } as any]}
          composer={<div>composer</div>}
        />
      </LanguageProvider>,
    );

    // The bug: without a fix, the refresh effect sees getPanel('flow') is
    // undefined and immediately re-adds it, fighting the user's own close.
    expect(screen.queryByTestId('chat-dock-flow')).toBeNull();
  });

  it('mounts the To-dos panel as a dockview panel, not a hand-rolled collapsible aside', () => {
    render(
      <LanguageProvider>
        <ChatSurface
          pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>}
          planSessionId="sess-1" planVersion={1}
        />
      </LanguageProvider>,
    );
    expect(screen.getByTestId('chat-dock-todos')).toBeInTheDocument();
    expect(screen.queryByLabelText('Collapse plan panel')).toBeNull();
    expect(screen.queryByLabelText('Expand plan panel')).toBeNull();
  });

  it('Panels button toggles a popover open and closed, with Escape and focus-return', () => {
    render(
      <LanguageProvider>
        <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
      </LanguageProvider>,
    );
    const trigger = screen.getByRole('button', { name: /panels/i });
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(trigger);
    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(trigger);
  });

  it('Panels popover lists a closed core panel and reopens it on click; clicking outside closes it', async () => {
    render(
      <LanguageProvider>
        <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
      </LanguageProvider>,
    );
    await screen.findByTestId('chat-dock-flow');

    const flowTab = screen.getByText('Flow').closest('.dv-default-tab') as HTMLElement;
    fireEvent.click(flowTab.querySelector('.dv-default-tab-action') as HTMLElement);
    await waitFor(() => expect(screen.queryByTestId('chat-dock-flow')).toBeNull());

    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('button', { name: /^flow$/i }));
    await waitFor(() => expect(screen.getByTestId('chat-dock-flow')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.mouseDown(document.body);
    expect(screen.queryByText('All panels open')).toBeNull();
  });

  it('Reset layout clears the persisted layout and closedPanelIds, and recreates only the 5 core panels', async () => {
    const { layoutStorageKeyFor } = await import('../../dock/DockWorkspaceShell');
    window.localStorage.setItem(layoutStorageKeyFor('gui.chat'), JSON.stringify({ grid: {} }));
    render(
      <LanguageProvider>
        <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
      </LanguageProvider>,
    );
    await screen.findByTestId('chat-dock-flow');

    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('button', { name: /reset layout/i }));

    expect(window.localStorage.getItem(layoutStorageKeyFor('gui.chat'))).toBeNull();
    await waitFor(() => {
      expect(screen.getByTestId('chat-dock-sessions')).toBeInTheDocument();
      expect(screen.getByTestId('chat-dock-transcript')).toBeInTheDocument();
      expect(screen.getByTestId('chat-dock-flow')).toBeInTheDocument();
      expect(screen.getByTestId('chat-dock-todos')).toBeInTheDocument();
    });
  });

  it('Reset layout does not throw when no layout was ever persisted', () => {
    render(
      <LanguageProvider>
        <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    expect(() => fireEvent.click(screen.getByRole('button', { name: /reset layout/i }))).not.toThrow();
  });
});

describe('session hydration ownership (F18: redundant double hydrate per session switch)', () => {
  it('ChatSurface has no hydrate trigger — App.tsx owns hydration', () => {
    const surface = readFileSync(resolve(__dirname, './ChatSurface.tsx'), 'utf8');
    // The redundant effect (`if (activeId && onHydrateSession) onHydrateSession(activeId)`)
    // and its prop are gone — App's activeSessionId effect is the only trigger.
    expect(surface).not.toContain('onHydrateSession');
    const surfaces = readFileSync(resolve(__dirname, '../../layout/surfaceComponents.tsx'), 'utf8');
    expect(surfaces).not.toContain('onHydrateChatSession');
    const app = readFileSync(resolve(__dirname, '../../../App.tsx'), 'utf8');
    expect(app).toContain('hydrateChatSession(activeSessionId)');
  });
});
