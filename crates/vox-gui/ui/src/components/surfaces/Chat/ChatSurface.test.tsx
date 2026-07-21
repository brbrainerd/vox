// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, act, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

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

import { ChatSurface, ApprovalsDockPanel, MercatusDockPanel, RepositoryDockPanel, NeedsYouDockPanel, VoxGraphDockPanel, ActivityDockPanel } from './ChatSurface';
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

  it('portals the Panels trigger into the workbench tab bar\'s trailing slot when present, instead of rendering its own row (F: the button must sit inline with the top tab bar, not float in a separate row)', () => {
    const slot = document.createElement('div');
    slot.id = 'workbench-tabbar-trailing-slot';
    document.body.appendChild(slot);
    try {
      render(
        <LanguageProvider>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </LanguageProvider>,
      );
      const trigger = screen.getByRole('button', { name: /panels/i });
      expect(slot.contains(trigger)).toBe(true);
      // No leftover own-row wrapper duplicating the button outside the slot.
      expect(screen.getAllByRole('button', { name: /panels/i })).toHaveLength(1);
    } finally {
      document.body.removeChild(slot);
    }
  });

  it('falls back to rendering its own Panels trigger row when no workbench tab bar slot exists (standalone rendering, e.g. these tests)', () => {
    expect(document.getElementById('workbench-tabbar-trailing-slot')).toBeNull();
    render(
      <LanguageProvider>
        <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
      </LanguageProvider>,
    );
    expect(screen.getByRole('button', { name: /panels/i })).toBeInTheDocument();
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
    fireEvent.click(screen.getByRole('checkbox', { name: /^flow$/i }));
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

  it('mounts a Needs You panel dockable from Chat via the Panels menu Add section', () => {
    const attention = {
      approvals: [],
      needsYou: [],
      withheld: [],
      blockedTasksCount: 0,
      hopperTasks: [],
      totalCount: 0,
      refresh: vi.fn(),
      resolveApproval: vi.fn(),
      resolveFeedback: vi.fn(),
    } as any;
    render(
      <LanguageProvider>
        <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} attention={attention} />
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^needs you$/i }));
    expect(screen.getByTestId('chat-dock-needs-you')).toBeInTheDocument();
  });

  it('the Needs You panel does not resurrect on the next render after being closed', async () => {
    const attention = {
      approvals: [],
      needsYou: [],
      withheld: [],
      blockedTasksCount: 0,
      hopperTasks: [],
      totalCount: 0,
      refresh: vi.fn(),
      resolveApproval: vi.fn(),
      resolveFeedback: vi.fn(),
    } as any;
    render(
      <LanguageProvider>
        <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} attention={attention} />
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^needs you$/i }));
    await screen.findByTestId('chat-dock-needs-you');

    const tab = screen
      .getAllByText('Needs You')
      .map(el => el.closest('.dv-default-tab'))
      .find((el): el is HTMLElement => el !== null) as HTMLElement;
    fireEvent.click(tab.querySelector('.dv-default-tab-action') as HTMLElement);
    await waitFor(() => expect(screen.queryByTestId('chat-dock-needs-you')).toBeNull());

    // Force an unrelated re-render — opt-in panels have NO auto-create
    // branch, so this must not bring it back (by construction, not by a
    // closedPanelIds guard, since opt-in panels don't use one).
    render(
      <LanguageProvider>
        <ChatSurface
          pushToast={vi.fn()}
          onNavigate={vi.fn()}
          messages={[{ id: 'm1', role: 'user', text: 'hi', status: 'done' } as any]}
          composer={<div>composer</div>}
          attention={attention}
        />
      </LanguageProvider>,
    );
    expect(screen.queryByTestId('chat-dock-needs-you')).toBeNull();
  });

  it('mounts a VoxGraph status panel dockable from Chat via the Panels menu Add section', () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^search index$/i }));
    expect(screen.getByTestId('chat-dock-voxgraph')).toBeInTheDocument();
  });

  it('the VoxGraph panel does not resurrect on the next render after being closed', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^search index$/i }));
    await screen.findByTestId('chat-dock-voxgraph');

    const tab = screen
      .getAllByText('Search Index')
      .map(el => el.closest('.dv-default-tab'))
      .find((el): el is HTMLElement => el !== null) as HTMLElement;
    fireEvent.click(tab.querySelector('.dv-default-tab-action') as HTMLElement);
    await waitFor(() => expect(screen.queryByTestId('chat-dock-voxgraph')).toBeNull());

    // Force an unrelated re-render — opt-in panels have NO auto-create
    // branch, so this must not bring it back (by construction, not by a
    // closedPanelIds guard, since opt-in panels don't use one).
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface
            pushToast={vi.fn()}
            onNavigate={vi.fn()}
            messages={[{ id: 'm1', role: 'user', text: 'hi', status: 'done' } as any]}
            composer={<div>composer</div>}
          />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    expect(screen.queryByTestId('chat-dock-voxgraph')).toBeNull();
  });

  it('mounts an Activity panel dockable from Chat via the Panels menu Add section', () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^discovery$/i }));
    expect(screen.getByTestId('chat-dock-activity')).toBeInTheDocument();
  });

  it('the Activity panel does not resurrect on the next render after being closed', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^discovery$/i }));
    await screen.findByTestId('chat-dock-activity');

    const tab = screen
      .getAllByText('Discovery')
      .map(el => el.closest('.dv-default-tab'))
      .find((el): el is HTMLElement => el !== null) as HTMLElement;
    fireEvent.click(tab.querySelector('.dv-default-tab-action') as HTMLElement);
    await waitFor(() => expect(screen.queryByTestId('chat-dock-activity')).toBeNull());

    // Force an unrelated re-render — opt-in panels have NO auto-create
    // branch, so this must not bring it back (by construction, not by a
    // closedPanelIds guard, since opt-in panels don't use one).
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface
            pushToast={vi.fn()}
            onNavigate={vi.fn()}
            messages={[{ id: 'm1', role: 'user', text: 'hi', status: 'done' } as any]}
            composer={<div>composer</div>}
          />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    expect(screen.queryByTestId('chat-dock-activity')).toBeNull();
  });

  it('mounts a Repository panel dockable from Chat via the Panels menu Add section', () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^repository$/i }));
    expect(screen.getByTestId('chat-dock-repository')).toBeInTheDocument();
  });

  it('the Repository panel does not resurrect on the next render after being closed', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^repository$/i }));
    await screen.findByTestId('chat-dock-repository');

    const tab = screen
      .getAllByText('Repository')
      .map(el => el.closest('.dv-default-tab'))
      .find((el): el is HTMLElement => el !== null) as HTMLElement;
    fireEvent.click(tab.querySelector('.dv-default-tab-action') as HTMLElement);
    await waitFor(() => expect(screen.queryByTestId('chat-dock-repository')).toBeNull());

    // Force an unrelated re-render — opt-in panels have NO auto-create
    // branch, so this must not bring it back (by construction, not by a
    // closedPanelIds guard, since opt-in panels don't use one).
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface
            pushToast={vi.fn()}
            onNavigate={vi.fn()}
            messages={[{ id: 'm1', role: 'user', text: 'hi', status: 'done' } as any]}
            composer={<div>composer</div>}
          />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    expect(screen.queryByTestId('chat-dock-repository')).toBeNull();
  });

  it('mounts a Mercatus panel dockable from Chat via the Panels menu Add section', () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^mercatus$/i }));
    expect(screen.getByTestId('chat-dock-mercatus')).toBeInTheDocument();
  });

  it('the Mercatus panel does not resurrect on the next render after being closed', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^mercatus$/i }));
    await screen.findByTestId('chat-dock-mercatus');

    const tab = screen
      .getAllByText('Mercatus')
      .map(el => el.closest('.dv-default-tab'))
      .find((el): el is HTMLElement => el !== null) as HTMLElement;
    fireEvent.click(tab.querySelector('.dv-default-tab-action') as HTMLElement);
    await waitFor(() => expect(screen.queryByTestId('chat-dock-mercatus')).toBeNull());

    // Force an unrelated re-render — opt-in panels have NO auto-create
    // branch, so this must not bring it back (by construction, not by a
    // closedPanelIds guard, since opt-in panels don't use one).
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface
            pushToast={vi.fn()}
            onNavigate={vi.fn()}
            messages={[{ id: 'm1', role: 'user', text: 'hi', status: 'done' } as any]}
            composer={<div>composer</div>}
          />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    expect(screen.queryByTestId('chat-dock-mercatus')).toBeNull();
  });

  it('mounts a Harness panel dockable from Chat via the Panels menu Add section', () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^harness$/i }));
    expect(screen.getByTestId('chat-dock-harness')).toBeInTheDocument();
  });

  it('positions a newly-activated opt-in panel after the most-recently-activated opt-in panel, not a fixed referenceChain slot', async () => {
    // Real assertion mechanism: spy on DockviewApi.prototype.addPanel (the
    // real dockview library runs in this jsdom suite — nothing about it is
    // mocked) and inspect the `position.referencePanel` dockview actually
    // received for Harness's addPanel call. The old fixed referenceChain for
    // every opt-in panel was `['todos', 'flow', 'executionRail',
    // 'transcript']`, and `todos` is always present in this test's default
    // render — so a fixed-chain implementation would call addPanel with
    // `referencePanel: 'todos'` for Harness regardless of what was opened
    // before it. Activation-order positioning must instead reference
    // 'mercatus', the panel activated immediately before Harness. Asserting
    // "both panels exist" alone (as the plan's own Step 1 sketch left
    // unresolved) would pass under either implementation and prove nothing
    // about relative position — this asserts the actual argument dockview's
    // API received, which only the new behavior can produce.
    const { DockviewApi } = await import('dockview');
    const addPanelSpy = vi.spyOn(DockviewApi.prototype, 'addPanel');
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^mercatus$/i }));
    await screen.findByTestId('chat-dock-mercatus');
    fireEvent.click(screen.getByRole('checkbox', { name: /^harness$/i }));
    await screen.findByTestId('chat-dock-harness');

    const harnessCall = addPanelSpy.mock.calls.find(([opts]) => opts.id === 'harness');
    expect(harnessCall).toBeDefined();
    expect(harnessCall![0].position).toEqual({ direction: 'right', referencePanel: 'mercatus' });
    addPanelSpy.mockRestore();
  });

  it('the Harness panel does not resurrect on the next render after being closed', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^harness$/i }));
    await screen.findByTestId('chat-dock-harness');

    const tab = screen
      .getAllByText('Harness')
      .map(el => el.closest('.dv-default-tab'))
      .find((el): el is HTMLElement => el !== null) as HTMLElement;
    fireEvent.click(tab.querySelector('.dv-default-tab-action') as HTMLElement);
    await waitFor(() => expect(screen.queryByTestId('chat-dock-harness')).toBeNull());

    // Force an unrelated re-render — opt-in panels have NO auto-create
    // branch, so this must not bring it back (by construction, not by a
    // closedPanelIds guard, since opt-in panels don't use one).
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface
            pushToast={vi.fn()}
            onNavigate={vi.fn()}
            messages={[{ id: 'm1', role: 'user', text: 'hi', status: 'done' } as any]}
            composer={<div>composer</div>}
          />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    expect(screen.queryByTestId('chat-dock-harness')).toBeNull();
  });

  it('Panels menu uses checkboxes, live-applying on check without closing the dropdown', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    const checkbox = screen.getByRole('checkbox', { name: /^mercatus$/i });
    expect(checkbox).not.toBeChecked();
    fireEvent.click(checkbox);
    await screen.findByTestId('chat-dock-mercatus');
    expect(checkbox).toBeChecked();
    // Dropdown stays open — the trigger's aria-expanded must still be true.
    expect(screen.getByRole('button', { name: /panels/i }).getAttribute('aria-expanded')).toBe('true');
  });

  it('clicking a Panels checkbox with a real pointer event sequence (mousedown+click, not just a synthetic click) toggles the panel and keeps the dropdown open (regression: outside-mousedown-closer treated the checkbox itself as "outside")', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    await user.click(screen.getByRole('button', { name: /panels/i }));
    expect(screen.getByRole('button', { name: /panels/i }).getAttribute('aria-expanded')).toBe('true');

    const checkbox = screen.getByRole('checkbox', { name: /^mercatus$/i });
    expect(checkbox).not.toBeChecked();
    // userEvent.click fires the full pointerdown/mousedown/pointerup/mouseup/click
    // sequence, exactly like a real mouse click — unlike fireEvent.click, which
    // only dispatches the bare 'click' event and never exercises the
    // document-level 'mousedown' outside-close listener.
    await user.click(checkbox);

    // The dropdown must still be open — this is the live bug report: "I can't
    // click on any of those checkboxes... it just closes the panel dropdown."
    expect(screen.getByRole('button', { name: /panels/i }).getAttribute('aria-expanded')).toBe('true');
    await screen.findByTestId('chat-dock-mercatus');
    expect(checkbox).toBeChecked();
  });

  it('unchecking a panel closes it', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^mercatus$/i }));
    await screen.findByTestId('chat-dock-mercatus');
    fireEvent.click(screen.getByRole('checkbox', { name: /^mercatus$/i }));
    await waitFor(() => expect(screen.queryByTestId('chat-dock-mercatus')).toBeNull());
  });

  it('checking two panels back-to-back with no await between clicks keeps both checked (regression: checked read from a live ref at render time desyncs on rapid multi-toggle)', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { rerender } = render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));

    fireEvent.click(screen.getByRole('checkbox', { name: /^mercatus$/i }));

    // Force a React re-render in between the two toggles — the same thing a
    // streamed token, a session poll, or any unrelated prop change does in
    // the real app between two rapid clicks. No await/waitFor: this must
    // happen before dockview's own event round-trip could settle. If
    // `checked` were still computed by reading dockApiRef at render time,
    // this reconciliation pass would stomp the DOM's checked state for
    // Mercatus back to its stale (unchecked) last-rendered value.
    rerender(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );

    fireEvent.click(screen.getByRole('checkbox', { name: /^harness$/i }));

    await waitFor(() => {
      expect(screen.getByTestId('chat-dock-mercatus')).toBeInTheDocument();
      expect(screen.getByTestId('chat-dock-harness')).toBeInTheDocument();
    });

    const mercatusCheckbox = screen.getByRole('checkbox', { name: /^mercatus$/i }) as HTMLInputElement;
    const harnessCheckbox = screen.getByRole('checkbox', { name: /^harness$/i }) as HTMLInputElement;
    expect(mercatusCheckbox.checked).toBe(true);
    expect(harnessCheckbox.checked).toBe(true);
  });

  it('checking two panels with zero awaits between the clicks (exact reviewer repro) keeps both checked', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));

    fireEvent.click(screen.getByRole('checkbox', { name: /^mercatus$/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^harness$/i }));

    expect(screen.getByTestId('chat-dock-mercatus')).toBeInTheDocument();
    expect(screen.getByTestId('chat-dock-harness')).toBeInTheDocument();

    const mercatusCheckbox = screen.getByRole('checkbox', { name: /^mercatus$/i }) as HTMLInputElement;
    const harnessCheckbox = screen.getByRole('checkbox', { name: /^harness$/i }) as HTMLInputElement;
    expect(mercatusCheckbox.checked).toBe(true);
    expect(harnessCheckbox.checked).toBe(true);
  });

  it('closing a panel externally (not via its checkbox) unchecks the corresponding Panels menu checkbox', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^mercatus$/i }));
    await screen.findByTestId('chat-dock-mercatus');
    expect(screen.getByRole('checkbox', { name: /^mercatus$/i })).toBeChecked();

    // Close via the real dockview tab-close action (simulates dragging a tab
    // out / clicking its native close X), not via the Panels checkbox.
    const tab = screen
      .getAllByText('Mercatus')
      .map(el => el.closest('.dv-default-tab'))
      .find((el): el is HTMLElement => el !== null) as HTMLElement;
    fireEvent.click(tab.querySelector('.dv-default-tab-action') as HTMLElement);
    await waitFor(() => expect(screen.queryByTestId('chat-dock-mercatus')).toBeNull());

    expect(screen.getByRole('checkbox', { name: /^mercatus$/i })).not.toBeChecked();
  });

  it('Approvals panel shows a condensed pending-count badge when docked narrow', () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <LanguageProvider>
        <QueryClientProvider client={client}>
          <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} pendingApprovals={3} />
        </QueryClientProvider>
      </LanguageProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /panels/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /^approvals$/i }));
    const panel = screen.getByTestId('chat-dock-approvals');
    expect(panel).toHaveTextContent('3 pending');
    // The full 4-column table must not render — condensed state renders
    // ApprovalsDockPanel's own summary markup only, never <ApprovalsView>.
    expect(screen.queryByRole('table')).toBeNull();
  });
});

describe('ApprovalsDockPanel width-driven toggle', () => {
  it('switches to a full-view link, not an inline table, when the toggle mechanism decides it is wide enough', () => {
    // dockview's own width isn't measurable in jsdom (no real layout engine),
    // so this test exercises ApprovalsDockPanel directly with a mocked
    // DockviewPanelApi rather than through the full ChatSurface dock — mirrors
    // how other width-dependent behavior in this codebase is unit-tested at
    // the component level when the full dockview integration can't produce
    // real pixel measurements under jsdom.
    const mockApi = { width: 900, onDidDimensionsChange: vi.fn(() => ({ dispose: vi.fn() })) } as any;
    render(
      <ApprovalsDockPanel
        api={mockApi}
        params={{ pendingApprovals: 3, permissionMode: 'Ask', onNavigate: vi.fn() }}
      />,
    );
    expect(screen.getByRole('link', { name: /open full view/i })).toBeInTheDocument();
  });
});

describe('MercatusDockPanel width-driven toggle', () => {
  function StubMercatus({ condensed }: { condensed?: boolean }) {
    return <div data-testid="mercatus-stub">{condensed ? 'condensed' : 'full'}</div>;
  }

  it('passes condensed=true to its node below the audited threshold, condensed=false at/above it', () => {
    let onChange: (() => void) | undefined;
    const api = {
      width: 300,
      onDidDimensionsChange: vi.fn((cb: () => void) => { onChange = cb; return { dispose: vi.fn() }; }),
    } as any;
    render(<MercatusDockPanel api={api} params={{ node: <StubMercatus /> }} />);
    expect(screen.getByTestId('mercatus-stub')).toHaveTextContent('condensed');

    api.width = 340;
    act(() => onChange?.());
    expect(screen.getByTestId('mercatus-stub')).toHaveTextContent('full');
  });
});

describe('RepositoryDockPanel width-driven toggle', () => {
  function StubRepository({ condensed }: { condensed?: boolean }) {
    return <div data-testid="repository-stub">{condensed ? 'condensed' : 'full'}</div>;
  }

  it('passes condensed=true to its node below the audited threshold, condensed=false at/above it', () => {
    let onChange: (() => void) | undefined;
    const api = {
      width: 200,
      onDidDimensionsChange: vi.fn((cb: () => void) => { onChange = cb; return { dispose: vi.fn() }; }),
    } as any;
    render(<RepositoryDockPanel api={api} params={{ node: <StubRepository /> }} />);
    expect(screen.getByTestId('repository-stub')).toHaveTextContent('condensed');

    api.width = 280;
    act(() => onChange?.());
    expect(screen.getByTestId('repository-stub')).toHaveTextContent('full');
  });
});

describe('NeedsYouDockPanel width-driven toggle', () => {
  function StubNeedsYou({ condensed }: { condensed?: boolean }) {
    return <div data-testid="needs-you-stub">{condensed ? 'condensed' : 'full'}</div>;
  }

  it('passes condensed=true to its node below the audited threshold, condensed=false at/above it', () => {
    let onChange: (() => void) | undefined;
    const api = {
      width: 200,
      onDidDimensionsChange: vi.fn((cb: () => void) => { onChange = cb; return { dispose: vi.fn() }; }),
    } as any;
    render(<NeedsYouDockPanel api={api} params={{ node: <StubNeedsYou /> }} />);
    expect(screen.getByTestId('needs-you-stub')).toHaveTextContent('condensed');

    api.width = 270;
    act(() => onChange?.());
    expect(screen.getByTestId('needs-you-stub')).toHaveTextContent('full');
  });
});

describe('VoxGraphDockPanel width-driven toggle', () => {
  function StubVoxGraph({ condensed }: { condensed?: boolean }) {
    return <div data-testid="voxgraph-stub">{condensed ? 'condensed' : 'full'}</div>;
  }

  it('passes condensed=true to its node below the audited threshold, condensed=false at/above it', () => {
    let onChange: (() => void) | undefined;
    const api = {
      width: 200,
      onDidDimensionsChange: vi.fn((cb: () => void) => { onChange = cb; return { dispose: vi.fn() }; }),
    } as any;
    render(<VoxGraphDockPanel api={api} params={{ node: <StubVoxGraph /> }} />);
    expect(screen.getByTestId('voxgraph-stub')).toHaveTextContent('condensed');

    api.width = 240;
    act(() => onChange?.());
    expect(screen.getByTestId('voxgraph-stub')).toHaveTextContent('full');
  });
});

describe('ActivityDockPanel width-driven toggle', () => {
  function StubDiscovery({ condensed }: { condensed?: boolean }) {
    return <div data-testid="activity-stub">{condensed ? 'condensed' : 'full'}</div>;
  }

  it('passes condensed=true to its node below the audited threshold, condensed=false at/above it', () => {
    let onChange: (() => void) | undefined;
    const api = {
      width: 300,
      onDidDimensionsChange: vi.fn((cb: () => void) => { onChange = cb; return { dispose: vi.fn() }; }),
    } as any;
    render(<ActivityDockPanel api={api} params={{ node: <StubDiscovery /> }} />);
    expect(screen.getByTestId('activity-stub')).toHaveTextContent('condensed');

    api.width = 360;
    act(() => onChange?.());
    expect(screen.getByTestId('activity-stub')).toHaveTextContent('full');
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
