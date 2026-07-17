import React, { useCallback, useEffect, useRef, useState } from 'react';
import { chatRailVisibility } from './chatRailVisibility';
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
import type { ChatMessage } from '../../../lib/chatCorrelation';
import type { AttentionBudgetSnapshot } from '../../../types/tauri';
import { AttentionBudgetMeter } from '../AttentionBudgetMeter';
import { SecretaryToast } from './SecretaryToast';
import { listenSecretaryProposed, type SecretaryProposedPayload, feedbackList } from '../../../transport';
import { Matrix } from '../Matrix/Matrix';



interface ChatSession {
  session_id: string;
  title: string;
  message_count: number;
}

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
  /** Primary Loquela composer — embedded when global shell dock is hidden on Chat. */
  composer?: React.ReactNode;
  focusedFeedbackId?: string | null;
  gamifyEnabled?: boolean;
  attention_budget?: AttentionBudgetSnapshot | null;
  waitingQuestions?: number;
  blockedTasks?: number;
  modelOverride?: string | null;
  onModelOverrideChange?: (id: string | null) => void;
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
  composer,
  focusedFeedbackId,
  gamifyEnabled,
  attention_budget,
  waitingQuestions = 0,
  blockedTasks = 0,
  modelOverride,
  onModelOverrideChange,
}: ChatSurfaceProps) {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [secretaryToast, setSecretaryToast] = useState<SecretaryProposedPayload | null>(null);
  const activeId = activeSessionId ?? '';

  // Responsive rails: measure the surface container (NOT the window) so the app
  // shell sidebar width is accounted for, then auto-hide rails when narrow.
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [containerWidth, setContainerWidth] = useState(0);
  const [sessionOverlayOpen, setSessionOverlayOpen] = useState(false);
  const [executionOverlayOpen, setExecutionOverlayOpen] = useState(false);
  const [routingOpen, setRoutingOpen] = useState(false);

  useEffect(() => {
    if (!routingOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setRoutingOpen(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [routingOpen]);

  useEffect(() => {
    const el = containerRef.current;
    if (!el || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(entries => {
      const w = entries[0]?.contentRect.width ?? el.clientWidth;
      setContainerWidth(w);
    });
    ro.observe(el);
    setContainerWidth(el.clientWidth);
    return () => ro.disconnect();
  }, []);

  const railVis = chatRailVisibility(containerWidth);
  // Close overlays automatically once the container is wide enough to show inline.
  useEffect(() => {
    if (railVis.sessionRail) setSessionOverlayOpen(false);
    if (railVis.executionRail) setExecutionOverlayOpen(false);
  }, [railVis.sessionRail, railVis.executionRail]);

  const loadSessions = useCallback(async () => {
    try {
      const list = await invoke<ChatSession[]>('chat_list_sessions', { limit: 40 });
      setSessions(list);
      if (!activeId && list.length > 0) {
        onSessionChange?.(list[0].session_id);
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Chat sessions', body: String(err), cause: 'backend-error' });
    }
  }, [activeId, onSessionChange, pushToast]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

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
      pushToast({ tone: 'warn', title: 'New session failed', body: String(err), cause: 'backend-error' });
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
        setSessionOverlayOpen(false);
      }}
      onCreateSession={() => void createSession()}
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

  return (
    <div
      ref={containerRef}
      className="relative flex min-h-[60vh] gap-4"
      data-testid="chat-surface-layout"
    >
      {railVis.sessionRail ? (
        sessionRailNode
      ) : (
        <button
          type="button"
          data-testid="chat-session-rail-toggle"
          aria-label="Show sessions rail"
          aria-expanded={sessionOverlayOpen}
          onClick={() => setSessionOverlayOpen(o => !o)}
          className="absolute left-0 top-0 z-30 rounded-lg border border-border-subtle bg-overlay-subtle p-2 text-text-muted transition hover:border-brass/40 hover:text-brass"
        >
          <span className="font-mono text-sm" aria-hidden="true">»</span>
        </button>
      )}
      {!railVis.sessionRail && sessionOverlayOpen ? (
        <div
          data-testid="chat-session-rail-overlay"
          className="absolute left-0 top-0 z-40 max-h-full"
        >
          {sessionRailNode}
        </div>
      ) : null}

      <div className="flex min-w-0 flex-1 flex-col gap-4">
        <div className="mb-2 flex justify-end"><ChatModelPicker activeModel={modelOverride ?? activeModel} onApplied={id => onModelOverrideChange?.(id)} /></div>
        {messages.length === 0 && !(agentStreamItems?.length ?? 0) ? (
          <EmptyState
            icon={<Icon.spark className="size-8 text-brass" aria-hidden="true" />}
            title="No messages yet"
            description="Describe a task in the composer below to start this session."
          />
        ) : (
          <ChatTranscript
            messages={messages}
            agentStreamItems={agentStreamItems}
            onOpenAgentInFlow={onOpenAgentInFlow}
          />
        )}
        {composer != null ? (
          <div className="mt-auto shrink-0 border-t border-border-subtle pt-3">
            {attention_budget ? (
              <div className="mb-2 px-1" data-testid="chat-attention-meter">
                <AttentionBudgetMeter
                  budget={attention_budget}
                  waitingQuestions={waitingQuestions}
                  blockedTasks={blockedTasks}
                />
              </div>
            ) : null}
            {composer}
          </div>
        ) : null}
      </div>

      {executionRailNode != null && railVis.executionRail ? executionRailNode : null}
      {executionRailNode != null && !railVis.executionRail ? (
        <button
          type="button"
          data-testid="chat-execution-rail-toggle"
          aria-label="Show execution rail"
          aria-expanded={executionOverlayOpen}
          onClick={() => setExecutionOverlayOpen(o => !o)}
          className="absolute right-0 top-0 z-30 rounded-lg border border-border-subtle bg-overlay-subtle p-2 text-text-muted transition hover:border-brass/40 hover:text-brass"
        >
          <span className="font-mono text-sm" aria-hidden="true">«</span>
        </button>
      ) : null}
      {executionRailNode != null && !railVis.executionRail && executionOverlayOpen ? (
        <div
          data-testid="chat-execution-rail-overlay"
          className="absolute right-0 top-0 z-40 max-h-full"
        >
          {executionRailNode}
        </div>
      ) : null}

      {secretaryToast && (
        <div className="absolute bottom-4 left-1/2 z-50 w-[480px] -translate-x-1/2">
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
        <div className="fixed inset-0 z-50" role="dialog" aria-modal="true" aria-label="Routing">
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
