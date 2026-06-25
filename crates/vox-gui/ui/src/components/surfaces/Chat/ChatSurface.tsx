import React, { useCallback, useEffect, useState } from 'react';
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
import type { ChatMessage } from '../../../lib/chatCorrelation';
import { SecretaryToast } from './SecretaryToast';
import { listenSecretaryProposed, type SecretaryProposedPayload, feedbackList } from '../../../transport';



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
  onHydrateSession?: (sessionId: string) => void;
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
}

export function ChatSurface({
  pushToast,
  onNavigate,
  messages = [],
  activeSessionId,
  onSessionChange,
  onHydrateSession,
  tasks = [],
  intents,
  executionKpis,
  activeModel,
  openrouterSpendUsd,
  agentStreamItems,
  onOpenAgentInFlow,
  composer,
  focusedFeedbackId,
}: ChatSurfaceProps) {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [secretaryToast, setSecretaryToast] = useState<SecretaryProposedPayload | null>(null);
  const activeId = activeSessionId ?? '';

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
    if (activeId && onHydrateSession) onHydrateSession(activeId);
  }, [activeId, onHydrateSession]);

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

  return (
    <div className="relative flex min-h-[60vh] gap-4" data-testid="chat-surface-layout">
      <ChatSessionRail
        sessions={sessions}
        activeSessionId={activeId}
        onSessionChange={id => onSessionChange?.(id)}
        onCreateSession={() => void createSession()}
      />

      <div className="flex min-w-0 flex-1 flex-col gap-4">
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
          <div className="mt-auto shrink-0 border-t border-border-subtle pt-3">{composer}</div>
        ) : null}
      </div>

      {onNavigate && (
        <ChatExecutionRail
          tasks={tasks}
          kpis={railKpis}
          intents={intents}
          activeModel={activeModel}
          openrouterSpendUsd={openrouterSpendUsd}
          onNavigate={onNavigate}
        />
      )}

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
    </div>
  );
}
