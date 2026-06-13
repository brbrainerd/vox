/**
 * Per-session chat state: Loquela composer and Chat surface share one store keyed by session_id.
 */

import {
  type AgentEventFrame,
  type ChatMessage,
  type ChatState,
  chatReducer,
  initialChatState,
} from './chatCorrelation';

export interface SessionChatStore {
  sessions: Record<string, ChatState>;
  /** task_id → session_id for routing agent events */
  taskToSession: Record<string, string>;
}

export const initialSessionChatStore: SessionChatStore = {
  sessions: {},
  taskToSession: {},
};

export type SessionChatAction =
  | { type: 'submit'; sessionId: string; runId: string; prompt: string }
  | { type: 'submitResolved'; sessionId: string; runId: string; taskId: string }
  | { type: 'failRun'; sessionId: string; runId: string; error: string }
  | { type: 'agentEvent'; event: AgentEventFrame }
  | { type: 'hydrate'; sessionId: string; messages: ChatMessage[] };

function ensureSession(store: SessionChatStore, sessionId: string): ChatState {
  return store.sessions[sessionId] ?? initialChatState;
}

function withSession(
  store: SessionChatStore,
  sessionId: string,
  state: ChatState,
): SessionChatStore {
  return {
    ...store,
    sessions: { ...store.sessions, [sessionId]: state },
  };
}

/** Resolve which session should receive a live agent event. */
export function resolveSessionForEvent(store: SessionChatStore, event: AgentEventFrame): string | undefined {
  const kind = event.kind ?? { type: 'unknown' };
  const type = kind.type ?? 'unknown';

  if (
    type === 'task_started' ||
    type === 'task_completed' ||
    type === 'task_failed' ||
    type === 'task_phase_changed'
  ) {
    const taskId = kind.task_id != null ? String(kind.task_id) : '';
    if (taskId && store.taskToSession[taskId]) return store.taskToSession[taskId];
    if (kind.session_id != null && String(kind.session_id)) return String(kind.session_id);
    if (taskId) return store.taskToSession[taskId];
  }

  if (type === 'snapshot_captured' && kind.session_id != null && String(kind.session_id)) {
    return String(kind.session_id);
  }

  if (
    type === 'token_streamed' ||
    type === 'tool_timed_out' ||
    type === 'activity_changed' ||
    type === 'snapshot_captured'
  ) {
    const agentId = kind.agent_id != null ? String(kind.agent_id) : '';
    if (agentId) {
      for (const [sessionId, state] of Object.entries(store.sessions)) {
        if (state.agentToTask[agentId]) return sessionId;
      }
    }
  }

  return undefined;
}

export function getSessionMessages(store: SessionChatStore, sessionId: string): ChatMessage[] {
  return store.sessions[sessionId]?.messages ?? [];
}

export function sessionChatReducer(store: SessionChatStore, action: SessionChatAction): SessionChatStore {
  switch (action.type) {
    case 'submit': {
      const sid = action.sessionId;
      const prev = ensureSession(store, sid);
      const next = chatReducer(prev, {
        type: 'submit',
        runId: action.runId,
        prompt: action.prompt,
      });
      return withSession(store, sid, next);
    }

    case 'submitResolved': {
      const sid = action.sessionId;
      const taskId = String(action.taskId);
      const prev = ensureSession(store, sid);
      const next = chatReducer(prev, {
        type: 'submitResolved',
        runId: action.runId,
        taskId,
      });
      return {
        ...withSession(store, sid, next),
        taskToSession: { ...store.taskToSession, [taskId]: sid },
      };
    }

    case 'failRun': {
      const sid = action.sessionId;
      const prev = ensureSession(store, sid);
      const next = chatReducer(prev, {
        type: 'failRun',
        runId: action.runId,
        error: action.error,
      });
      return withSession(store, sid, next);
    }

    case 'agentEvent': {
      const kind = action.event.kind ?? { type: 'unknown' };
      let taskToSession = store.taskToSession;
      if (kind.type === 'task_started' && kind.task_id != null) {
        const taskId = String(kind.task_id);
        const sid =
          kind.session_id != null
            ? String(kind.session_id)
            : store.taskToSession[taskId];
        if (sid) {
          taskToSession = { ...taskToSession, [taskId]: sid };
        }
      }

      const base = { ...store, taskToSession };
      const sessionId = resolveSessionForEvent(base, action.event);
      if (!sessionId) return base;

      const prev = ensureSession(base, sessionId);
      const next = chatReducer(prev, { type: 'agentEvent', event: action.event });
      return withSession(base, sessionId, next);
    }

    case 'hydrate': {
      const sid = action.sessionId;
      const existing = store.sessions[sid];
      if (existing && existing.messages.length > 0) return store;
      return withSession(store, sid, {
        ...initialChatState,
        messages: action.messages,
      });
    }

    default:
      return store;
  }
}
