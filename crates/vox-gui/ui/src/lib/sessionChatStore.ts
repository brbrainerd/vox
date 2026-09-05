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
  /** Unroutable token_streamed/task_started frames buffered until the
   *  submit that owns them resolves; replayed in `submitResolved`. Fixes
   *  the token-loss race where task_started precedes submitResolved. */
  pending: AgentEventFrame[];
}

export const initialSessionChatStore: SessionChatStore = {
  sessions: {},
  taskToSession: {},
  pending: [],
};

/** Replay window for unroutable frames: anything older than this (relative
 *  to the newest buffered frame) is a lost cause, not a race, and is evicted. */
const PENDING_REPLAY_WINDOW_MS = 30_000;
/** Hard cap so a runaway stream cannot grow the buffer without bound. */
const PENDING_MAX_FRAMES = 200;
/** Frame types that participate in the submit race and are worth holding. */
const BUFFERABLE_TYPES = new Set(['token_streamed', 'task_started']);

function bufferPending(pending: AgentEventFrame[], event: AgentEventFrame): AgentEventFrame[] {
  const cutoff = event.timestamp_ms - PENDING_REPLAY_WINDOW_MS;
  return [...pending.filter((f) => f.timestamp_ms >= cutoff), event].slice(-PENDING_MAX_FRAMES);
}

export type SessionChatAction =
  | { type: 'submit'; sessionId: string; runId: string; prompt: string }
  | { type: 'submitResolved'; sessionId: string; runId: string; taskId: string }
  | { type: 'failRun'; sessionId: string; runId: string; error: string }
  | { type: 'agentEvent'; event: AgentEventFrame }
  | { type: 'hydrate'; sessionId: string; messages: ChatMessage[] }
  | { type: 'chatPending'; sessionId: string; tempId: string; userText: string }
  | {
      type: 'chatReplySettled';
      sessionId: string;
      tempId: string;
      result:
        | { ok: true; message: ChatMessage }
        | { ok: false; error: string };
    };

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

  // Task G1: the sync chat path (`chat_turn`'s `run_sync` -> `vox_chat_message`
  // -> `run_agent_turn`) streams tokens with no background task/agent
  // correlation at all -- there is no `task_started`/`submitResolved` for a
  // sync turn to populate `agentToTask`/`taskToSession` with. When the
  // orchestrator sets `TokenStreamed.session_id` (real for the sync path,
  // `None`/absent for the pre-existing background `AiTaskProcessor` stream),
  // route directly to that session instead of falling through to the
  // agent-scan below, which would never find anything for a sync turn.
  if (type === 'token_streamed' && kind.session_id != null && String(kind.session_id)) {
    return String(kind.session_id);
  }

  if (
    type === 'token_streamed' ||
    type === 'tool_timed_out' ||
    type === 'activity_changed' ||
    type === 'snapshot_captured' ||
    type === 'cost_incurred'
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
      const resolved = chatReducer(prev, {
        type: 'submitResolved',
        runId: action.runId,
        taskId,
      });
      let next: SessionChatStore = {
        ...withSession(store, sid, resolved),
        taskToSession: { ...store.taskToSession, [taskId]: sid },
      };
      // Replay frames that raced ahead of this resolution, in arrival
      // order. Frames that are STILL unroutable re-buffer themselves via
      // the agentEvent case, so nothing is lost or reordered.
      const queued = next.pending;
      if (queued.length > 0) {
        next = { ...next, pending: [] };
        for (const event of queued) {
          next = sessionChatReducer(next, { type: 'agentEvent', event });
        }
      }
      return next;
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
      if (!sessionId) {
        // Race: token_streamed/task_started can precede submitResolved (no
        // task→session mapping yet, no session_id on the frame). Buffer
        // instead of dropping; `submitResolved` replays the queue.
        if (BUFFERABLE_TYPES.has(kind.type)) {
          return { ...base, pending: bufferPending(base.pending, action.event) };
        }
        return base;
      }

      const prev = ensureSession(base, sessionId);
      const next = chatReducer(prev, { type: 'agentEvent', event: action.event });
      return withSession(base, sessionId, next);
    }

    case 'chatPending': {
      const sid = action.sessionId;
      const prev = ensureSession(store, sid);
      const next = chatReducer(prev, {
        type: 'chatPending',
        sessionId: sid,
        tempId: action.tempId,
        userText: action.userText,
      });
      return withSession(store, sid, next);
    }

    case 'chatReplySettled': {
      const sid = action.sessionId;
      const prev = ensureSession(store, sid);
      const next = chatReducer(prev, {
        type: 'chatReplySettled',
        sessionId: sid,
        tempId: action.tempId,
        result: action.result,
      });
      return withSession(store, sid, next);
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
