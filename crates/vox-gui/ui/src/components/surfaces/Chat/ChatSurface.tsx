import React, { useCallback, useEffect, useRef, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { invoke } from '@tauri-apps/api/core';
import { type UnlistenFn } from '@tauri-apps/api/event';
import { ChatTranscript } from './ChatTranscript';
import type { StreamItem } from '../../../types/dashboard';
import { EmptyState } from '../../ui/EmptyState';
import { Icon } from '../../ui/Icons';
import {
  ChatExecutionRail,
  type ChatExecutionRailKpis,
  type ChatExecutionTask,
} from './ChatExecutionRail';
import { ChatSessionRail } from './ChatSessionRail';
import { ChatModelPicker } from './ChatModelPicker';
import { PlanPanel, type PlanNodeView } from './PlanPanel';
import { listPlanNodes } from '../../../transport';
import type { ChatMessage } from '../../../lib/chatCorrelation';
import type { AttentionBudgetSnapshot } from '../../../types/tauri';
import { AttentionBudgetMeter } from '../AttentionBudgetMeter';
import { SecretaryToast } from './SecretaryToast';
import { listenSecretaryProposed, type SecretaryProposedPayload, feedbackList } from '../../../transport';
import { Matrix } from '../Matrix/Matrix';
import { DockWorkspaceShell, layoutStorageKeyFor } from '../../dock/DockWorkspaceShell';
import type { DockviewApi, IDockviewPanelProps } from 'dockview';
import { AgentFlow } from '../Flow/AgentFlow';
import type { Agent } from '../../../types/dashboard';



const CORE_PANEL_IDS = ['sessions', 'transcript', 'executionRail', 'flow', 'todos'] as const;
type CorePanelId = (typeof CORE_PANEL_IDS)[number];

interface ChatSession {
  session_id: string;
  title: string;
  message_count: number;
}

function SessionsPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-sessions" className="h-full overflow-y-auto">{props.params.node}</div>;
}

function TranscriptPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-transcript" className="flex h-full min-w-0 flex-col gap-4 p-2">{props.params.node}</div>;
}

function ExecutionRailPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-execution-rail" className="h-full overflow-y-auto">{props.params.node}</div>;
}

function FlowPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-flow" className="h-full overflow-y-auto p-2">{props.params.node}</div>;
}

function TodosDockPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-todos" className="h-full overflow-y-auto p-2">{props.params.node}</div>;
}

const CHAT_DOCK_COMPONENTS = {
  sessions: SessionsPanel,
  transcript: TranscriptPanel,
  executionRail: ExecutionRailPanel,
  flow: FlowPanel,
  todos: TodosDockPanel,
};

// Marker rendered inside the transcript panel's dockview tab element. Task
// 1.8 pinned the transcript panel by giving it this empty tab component,
// which removes the visible tab/close chrome — but dockview-core 6.6.1 sets
// `draggable = true` unconditionally on every tab's DOM element
// (dockview-core/dist/esm/dockview/components/tab/tab.js), with no per-panel
// API to disable it. The marker below gives the capture-phase `dragstart`
// listener (see the `dockRootRef` effect in ChatSurface) a way to identify
// "this drag started from the transcript panel's tab" purely from the DOM,
// without a blanket `disableDnd` that would break dragging every other panel.
function EmptyTab() {
  return <span data-testid="chat-dock-transcript-tab-marker" style={{ display: 'none' }} />;
}
const CHAT_DOCK_TAB_COMPONENTS = { transcript: EmptyTab };

interface ChatSurfaceProps {
  pushToast: (t: any) => void;
  onNavigate?: (viewKey: string) => void;
  messages?: ChatMessage[];
  activeSessionId?: string;
  onSessionChange?: (sessionId: string) => void;
  tasks?: ChatExecutionTask[];
  intents?: string[];
  executionKpis?: ChatExecutionRailKpis;
  activeModel?: string | null;
  openrouterSpendUsd?: number | null;
  agentStreamItems?: StreamItem[];
  onOpenAgentInFlow?: (agentId: string) => void;
  /**
   * Same agent-graph data source that feeds the top-level Flow tab
   * (`props.data.agents` in surfaceComponents.tsx `case 'flow'`) — reused
   * here, not refetched, so the dockable Flow panel stays in sync with the
   * real dashboard state.
   */
  flowAgents?: Agent[];
  flowSelectedAgentId?: string;
  onFlowSelectAgent?: (id: string) => void;
  /** Primary Loquela composer — embedded when global shell dock is hidden on Chat. */
  composer?: React.ReactNode;
  focusedFeedbackId?: string | null;
  gamifyEnabled?: boolean;
  attention_budget?: AttentionBudgetSnapshot | null;
  waitingQuestions?: number;
  blockedTasks?: number;
  modelOverride?: string | null;
  onModelOverrideChange?: (id: string | null) => void;
  /** Live plan-DAG identity for this session, if the current task has synthesized a plan. */
  planSessionId?: string | null;
  planVersion?: number | null;
}

export function ChatSurface({
  pushToast,
  onNavigate,
  messages = [],
  activeSessionId,
  onSessionChange,
  tasks = [],
  intents,
  executionKpis,
  activeModel,
  openrouterSpendUsd,
  agentStreamItems,
  onOpenAgentInFlow,
  flowAgents = [],
  flowSelectedAgentId,
  onFlowSelectAgent,
  composer,
  focusedFeedbackId,
  gamifyEnabled,
  attention_budget,
  waitingQuestions = 0,
  blockedTasks = 0,
  modelOverride,
  onModelOverrideChange,
  planSessionId,
  planVersion,
}: ChatSurfaceProps) {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [secretaryToast, setSecretaryToast] = useState<SecretaryProposedPayload | null>(null);
  const activeId = activeSessionId ?? '';
  const [planNodes, setPlanNodes] = useState<PlanNodeView[]>([]);
  // dockview's `addPanel` captures `params` at call time — it does not track
  // React re-renders. Keep a handle to the ready API so each panel's `node`
  // param can be refreshed whenever the underlying content changes (new
  // sessions loaded, transcript updated, execution rail data changed, etc).
  const dockApiRef = useRef<DockviewApi | null>(null);
  // Tracks panels removed (by the user's tab-close, or by the reset action
  // below) so the refresh effect below can tell "not yet ready to create"
  // apart from "the user closed this — leave it closed until something
  // explicitly asks for it back." Cleared per-id by the reopen/reset actions
  // (Tasks 4/5), not by this effect.
  const closedPanelIds = useRef<Set<string>>(new Set());

  const [routingOpen, setRoutingOpen] = useState(false);
  const [panelsMenuOpen, setPanelsMenuOpen] = useState(false);
  const panelsTriggerRef = useRef<HTMLButtonElement | null>(null);
  const dockRootRef = useRef<HTMLDivElement | null>(null);

  // Task 1.8 follow-up: dockview-core sets `draggable = true` on every tab
  // element unconditionally, regardless of the (empty) tab component the
  // transcript panel uses to hide its chrome. There is no dockview API to
  // disable dragging for a single panel. A capture-phase `dragstart`
  // listener on the dock's own root container preventDefault()s only when
  // the drag originated from within a `.dv-tab` that contains the
  // transcript panel's marker element (rendered by `EmptyTab` above) — every
  // other panel's tab is untouched.
  useEffect(() => {
    const root = dockRootRef.current;
    if (!root) return;
    const handleDragStart = (event: DragEvent) => {
      const target = event.target;
      if (!(target instanceof Element)) return;
      const tabEl = target.closest('.dv-tab');
      if (!tabEl) return;
      if (tabEl.querySelector('[data-testid="chat-dock-transcript-tab-marker"]')) {
        event.preventDefault();
      }
    };
    root.addEventListener('dragstart', handleDragStart, true);
    return () => root.removeEventListener('dragstart', handleDragStart, true);
  }, []);

  useEffect(() => {
    if (!routingOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setRoutingOpen(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [routingOpen]);

  useEffect(() => {
    if (!panelsMenuOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setPanelsMenuOpen(false);
        panelsTriggerRef.current?.focus();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [panelsMenuOpen]);

  useEffect(() => {
    if (!panelsMenuOpen) return;
    const onPointerDown = (e: MouseEvent) => {
      if (panelsTriggerRef.current?.contains(e.target as Node)) return;
      setPanelsMenuOpen(false);
    };
    document.addEventListener('mousedown', onPointerDown);
    return () => document.removeEventListener('mousedown', onPointerDown);
  }, [panelsMenuOpen]);

  const loadSessions = useCallback(async () => {
    try {
      const list = await invoke<ChatSession[]>('chat_list_sessions', { limit: 40 });
      setSessions(list);
      if (!activeId && list.length > 0) {
        onSessionChange?.(list[0].session_id);
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Chat sessions', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  }, [activeId, onSessionChange, pushToast]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  useEffect(() => {
    if (!planSessionId || planVersion == null) {
      setPlanNodes([]);
      return;
    }
    let cancelled = false;
    listPlanNodes(planSessionId, planVersion)
      .then((rows: PlanNodeView[]) => {
        if (!cancelled) setPlanNodes(rows);
      })
      .catch(() => {
        if (!cancelled) setPlanNodes([]);
      });
    return () => {
      cancelled = true;
    };
  }, [planSessionId, planVersion]);

  useEffect(() => {
    const sub = listenSecretaryProposed((payload) => {
      setSecretaryToast(payload);
    });
    return () => {
      sub.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (!focusedFeedbackId) return;
    
    feedbackList().then((data) => {
      const allFeedback = [...data.needsYou, ...data.withheld];
      const item = allFeedback.find((f) => f.feedbackId === focusedFeedbackId);
      if (!item) return;

      const match = messages.find((msg) => {
        if (msg.taskId) {
          const mTaskId = Number(msg.taskId);
          if (item.gates.includes(mTaskId) || mTaskId === item.doubtedTaskId) {
            return true;
          }
        }
        return msg.text.includes(item.prompt);
      });

      if (match) {
        const el = document.getElementById(`msg-${match.id}`);
        if (el) {
          el.scrollIntoView({ behavior: 'smooth', block: 'center' });
          el.classList.add('ring-2', 'ring-amber-400', 'ring-offset-2', 'ring-offset-zinc-950');
          setTimeout(() => {
            el.classList.remove('ring-2', 'ring-amber-400', 'ring-offset-2', 'ring-offset-zinc-950');
          }, 3000);
        }
      }
    }).catch(() => {});
  }, [focusedFeedbackId, messages]);



  const createSession = async () => {
    try {
      const s = await invoke<ChatSession & { conversation_id: number }>('chat_create_session', {
        title: 'New chat',
      });
      setSessions(prev => [s, ...prev]);
      onSessionChange?.(s.session_id);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'New session failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  };

  const renameSession = async (sessionId: string, title: string) => {
    try {
      await invoke('chat_rename_session', { sessionId, title });
      await loadSessions();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Rename failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  };

  const archiveSession = async (sessionId: string) => {
    try {
      await invoke('chat_archive_session', { sessionId });
      const remaining = sessions.filter(s => s.session_id !== sessionId);
      setSessions(remaining);
      if (activeId === sessionId && remaining.length > 0) onSessionChange?.(remaining[0].session_id);
      await loadSessions();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Archive failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  };

  const railKpis = executionKpis ?? {
    activeAgents: { value: 0 },
    queueDepth: { value: 0 },
    mesh: { peers: 0 },
  };

  const sessionRailNode = (
    <ChatSessionRail
      sessions={sessions}
      activeSessionId={activeId}
      onSessionChange={id => {
        onSessionChange?.(id);
      }}
      onCreateSession={() => void createSession()}
      onRenameSession={(id, t) => void renameSession(id, t)}
      onArchiveSession={id => void archiveSession(id)}
    />
  );

  const executionRailNode = onNavigate ? (
    <ChatExecutionRail
      tasks={tasks}
      kpis={railKpis}
      intents={intents}
      activeModel={activeModel}
      openrouterSpendUsd={openrouterSpendUsd}
      onNavigate={onNavigate}
      sessionId={activeSessionId}
      onOpenRouting={() => setRoutingOpen(true)}
    />
  ) : null;

  const flowNode = (
    <AgentFlow
      agents={flowAgents}
      selectedId={flowSelectedAgentId}
      onSelect={onFlowSelectAgent}
    />
  );

  const todosNode = <PlanPanel planSessionId={planSessionId} planVersion={planVersion} nodes={planNodes} />;

  const centerContent = (
    <>
      {messages.length === 0 && !(agentStreamItems?.length ?? 0) ? (
        <EmptyState
          icon={<Icon.spark className="size-8 text-brass" aria-hidden="true" />}
          title="No messages yet"
          description="Describe a task in the composer below to start this session."
        />
      ) : (
        <ChatTranscript messages={messages} agentStreamItems={agentStreamItems} />
      )}
      {composer != null ? (
        <div
          className="mt-auto shrink-0 border-t border-border-subtle pt-3"
          data-testid="chat-composer-dock"
        >
          {attention_budget ? (
            <div className="mb-2 px-1" data-testid="chat-attention-meter">
              <AttentionBudgetMeter
                budget={attention_budget}
                waitingQuestions={waitingQuestions}
                blockedTasks={blockedTasks}
              />
            </div>
          ) : null}
          {/* The model-route picker now renders inside the composer's own
              toolbar row (Loquela's `trailingSlot`, wired in App.tsx),
              right-aligned alongside the cost display, rather than as a
              separate row below it. */}
          {composer}
        </div>
      ) : (
        <div className="mt-auto flex shrink-0 justify-end px-1">
          <ChatModelPicker activeModel={modelOverride ?? activeModel} onApplied={id => onModelOverrideChange?.(id)} />
        </div>
      )}
    </>
  );

  // ChatDockPanelId is aliased to CorePanelId for now; Task 2.1 widens it to
  // `CorePanelId | OptInPanelId` once opt-in panels exist.
  type ChatDockPanelId = CorePanelId;

  const panelDefs: Record<ChatDockPanelId, { title: string; node: React.ReactNode; referenceChain: ChatDockPanelId[] }> = {
    sessions: { title: 'Sessions', node: sessionRailNode, referenceChain: [] },
    transcript: { title: 'Chat', node: centerContent, referenceChain: ['sessions'] },
    executionRail: { title: 'Execution', node: executionRailNode, referenceChain: ['transcript'] },
    flow: { title: 'Flow', node: flowNode, referenceChain: ['executionRail', 'transcript'] },
    todos: { title: 'To-dos', node: todosNode, referenceChain: ['flow', 'executionRail', 'transcript'] },
  };

  // Plain function, not useCallback: panelDefs is a fresh object every render.
  const addDefaultPanel = (api: DockviewApi, id: ChatDockPanelId) => {
    const def = panelDefs[id];
    const referencePanel = def.referenceChain.find(candidateId => api.getPanel(candidateId));
    api.addPanel({
      id,
      component: id,
      ...(id === 'transcript' ? { tabComponent: 'transcript' } : {}),
      title: def.title,
      params: { node: def.node },
      position: referencePanel ? { direction: 'right', referencePanel } : undefined,
    });
    closedPanelIds.current.delete(id);
  };

  // Refresh each panel's `node` param on every render so dockview reflects
  // the latest sessions/transcript/execution-rail content (addPanel only
  // captures params once, at panel-creation time).
  useEffect(() => {
    const api = dockApiRef.current;
    if (!api) return;
    api.getPanel('sessions')?.update({ params: { node: sessionRailNode } });
    api.getPanel('transcript')?.update({ params: { node: centerContent } });
    const executionPanel = api.getPanel('executionRail');
    if (executionRailNode) {
      if (executionPanel) {
        executionPanel.update({ params: { node: executionRailNode } });
      } else if (!closedPanelIds.current.has('executionRail')) {
        api.addPanel({
          id: 'executionRail',
          component: 'executionRail',
          title: 'Execution',
          params: { node: executionRailNode },
          position: { direction: 'right', referencePanel: 'transcript' },
        });
      }
    } else if (executionPanel) {
      api.removePanel(executionPanel);
    }
    const flowPanel = api.getPanel('flow');
    if (flowPanel) {
      flowPanel.update({ params: { node: flowNode } });
    } else if (!closedPanelIds.current.has('flow')) {
      api.addPanel({
        id: 'flow',
        component: 'flow',
        title: 'Flow',
        params: { node: flowNode },
        // Note: the plan's original sketch tabbed Flow `within` the
        // execution rail group. dockview-react only mounts the active tab's
        // panel body, so that hid `chat-dock-execution-rail` from the DOM
        // whenever Flow's tab was active — a regression against the B2 test
        // asserting the execution rail is present without any tab
        // interaction. Placing Flow as its own group (to the right of
        // whichever panel is currently last) keeps all panels simultaneously
        // present, matching how sessions/transcript/executionRail already
        // coexist.
        position: {
          direction: 'right',
          referencePanel: api.getPanel('executionRail') ? 'executionRail' : 'transcript',
        },
      });
    }
    const todosPanel = api.getPanel('todos');
    if (todosPanel) {
      todosPanel.update({ params: { node: todosNode } });
    } else if (!closedPanelIds.current.has('todos')) {
      api.addPanel({
        id: 'todos',
        component: 'todos',
        title: 'To-dos',
        params: { node: todosNode },
        position: {
          direction: 'right',
          referencePanel: api.getPanel('flow') ? 'flow' : api.getPanel('executionRail') ? 'executionRail' : 'transcript',
        },
      });
    }
  });

  return (
    <div
      className="relative flex h-full min-h-[60vh] gap-4"
      data-testid="chat-surface-layout"
    >
      {/* Axe page-has-heading-one: surfaces render inside a heading-less shell.
          NOTE: if chatDocked (App.tsx, currently hardcoded false) is ever
          enabled, a docked ChatSurface adds a second h1 to the page. */}
      <h1 className="sr-only">Chat</h1>

      <div className="min-w-0 flex-1">
        <div className="relative mb-2 flex justify-end">
          <div className="relative">
            <button
              ref={panelsTriggerRef}
              type="button"
              aria-label="Panels"
              aria-expanded={panelsMenuOpen}
              onClick={() => setPanelsMenuOpen(o => !o)}
              className="rounded-lg border border-border-subtle bg-overlay-subtle p-2 text-text-muted transition hover:border-brass/40 hover:text-brass"
            >
              <span className="font-mono text-xs">Panels ▾</span>
            </button>
            {panelsMenuOpen ? (
              <div className="absolute right-0 top-full z-50 mt-1 w-56 rounded-lg border border-border-subtle bg-bg-base p-1 shadow-2xl">
                {CORE_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null)
                  .filter(id => closedPanelIds.current.has(id))
                  .map(id => (
                    <button
                      key={id}
                      type="button"
                      onClick={() => {
                        const api = dockApiRef.current;
                        if (api) addDefaultPanel(api, id);
                        setPanelsMenuOpen(false);
                        panelsTriggerRef.current?.focus();
                      }}
                      className="block w-full rounded px-2 py-1.5 text-left text-xs text-text-muted hover:bg-overlay-hover hover:text-text-primary"
                    >
                      {panelDefs[id].title}
                    </button>
                  ))}
                {CORE_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null).every(
                  id => !closedPanelIds.current.has(id),
                ) ? (
                  <div className="px-2 py-1.5 text-xs text-text-muted/60">All panels open</div>
                ) : null}
                <div className="my-1 border-t border-border-subtle" />
                <button
                  type="button"
                  onClick={() => {
                    window.localStorage.removeItem(layoutStorageKeyFor('gui.chat'));
                    const api = dockApiRef.current;
                    if (api) {
                      api.panels.forEach(p => api.removePanel(p)); // .panels is a fresh snapshot per access — safe to iterate while removePanel mutates the underlying model
                      closedPanelIds.current.clear();
                      // Only CORE_PANEL_IDS — opt-in panels (Phase 2/3) are intentionally
                      // NOT recreated by Reset, even if they were open before the reset.
                      CORE_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null).forEach(id =>
                        addDefaultPanel(api, id),
                      );
                    }
                    setPanelsMenuOpen(false);
                    panelsTriggerRef.current?.focus();
                  }}
                  className="block w-full rounded px-2 py-1.5 text-left text-xs text-text-muted hover:bg-overlay-hover hover:text-text-primary"
                >
                  Reset layout
                </button>
              </div>
            ) : null}
          </div>
        </div>
        <div ref={dockRootRef}>
        <DockWorkspaceShell
          storageKeyPrefix="gui.chat"
          components={CHAT_DOCK_COMPONENTS}
          tabComponents={CHAT_DOCK_TAB_COMPONENTS}
          onReady={(event) => {
            dockApiRef.current = event.api;
            event.api.onDidRemovePanel(panel => {
              closedPanelIds.current.add(panel.id);
            });
            // Guarded against duplicate-add: a restored dockview layout
            // (ChatDockShell's localStorage persistence) already recreates
            // these panels, so onReady must not re-add them.
            if (!event.api.getPanel('sessions')) {
              event.api.addPanel({ id: 'sessions', component: 'sessions', title: 'Sessions', params: { node: sessionRailNode } });
            }
            if (!event.api.getPanel('transcript')) {
              event.api.addPanel({
                id: 'transcript',
                component: 'transcript',
                tabComponent: 'transcript',
                title: 'Chat',
                params: { node: centerContent },
                position: { direction: 'right', referencePanel: 'sessions' },
              });
            }
            if (executionRailNode && !event.api.getPanel('executionRail')) {
              event.api.addPanel({
                id: 'executionRail',
                component: 'executionRail',
                title: 'Execution',
                params: { node: executionRailNode },
                position: { direction: 'right', referencePanel: 'transcript' },
              });
            }
          }}
        />
        </div>
      </div>

      {secretaryToast && (
        <div className="absolute bottom-4 left-1/2 z-[60] w-[min(480px,90%)] -translate-x-1/2">
          <SecretaryToast
            intent={secretaryToast.intent}
            itemId={secretaryToast.item_id}
            onDismiss={() => setSecretaryToast(null)}
            onViewTask={() => {
              setSecretaryToast(null);
              onNavigate?.('tasks');
            }}
          />
        </div>
      )}

      {routingOpen && (
        <div className="fixed inset-0 z-[60]" role="dialog" aria-modal="true" aria-label="Routing">
          <div className="absolute inset-0 bg-black/60" onClick={() => setRoutingOpen(false)} />
          <div className="absolute right-0 top-0 h-full w-[760px] max-w-full overflow-y-auto border-l border-border-subtle bg-bg-base shadow-2xl">
            <div className="flex items-center justify-between px-5 pt-4">
              <h2 className="font-display text-[13px] uppercase tracking-[0.2em] text-text-secondary">Routing</h2>
              <button
                type="button"
                aria-label="Close routing panel"
                onClick={() => setRoutingOpen(false)}
                className="rounded-md border border-border-subtle px-2 py-1 font-mono text-xs text-text-muted hover:bg-overlay-hover hover:text-text-primary"
              >
                ✕
              </button>
            </div>
            <Matrix pushToast={pushToast} gamifyEnabled={gamifyEnabled} />
          </div>
        </div>
      )}
    </div>
  );
}
