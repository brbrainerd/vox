//! B4-chat: pure client-side correlation of streamed agent events to chat
//! bubbles. No orchestrator change — `submit_orchestrator_task` returns a
//! `task_id`, `task_started` ties an `agent_id` to that task, `token_streamed`
//! (which carries only `agent_id`) is routed through that map, and
//! `task_completed`/`task_failed` finalize. See
//! `docs/src/architecture/vox-gui-harness-buildout-plan-2026.md` (B4-chat).

import type { TurnEventDto } from '../types/dashboard';

export type ChatRole = 'user' | 'assistant' | 'system';
export type ChatStatus = 'pending' | 'streaming' | 'done' | 'failed';

export interface ChatMessage {
  id: string;
  role: ChatRole;
  text: string;
  status: ChatStatus;
  runId: string;
  taskId?: string;
  error?: string;
  /** Chat session (tab) this message belongs to. */
  sessionId?: string;
  /** Model that produced this assistant message (from cost_incurred). */
  modelId?: string;
  /** Turn latency in milliseconds, when reported by the reply (synchronous
   *  chat path only — see `lib/chatSend.ts`'s `ParsedChatReply`). */
  latencyMs?: number;
  /** Human-readable reason the model was chosen, when reported by the reply
   *  (synchronous chat path only — see `lib/chatSend.ts`'s `ParsedChatReply`). */
  selectionReason?: string;
  /** True when the opt-in post-reply grounding check flagged this reply as
   *  low-confidence (from grounding_check_completed). */
  groundingFlagged?: boolean;
  /** Wall-clock ms when the bubble was created (drives the pending watchdog). */
  createdAtMs?: number;
  /** Turn events derived from tool RESULTS (synchronous chat path only —
   *  see `lib/chatSend.ts`'s `ParsedChatReply` and Rust `turn_event_for_result`). */
  events?: TurnEventDto[];
}

/** How long an assistant bubble may sit in `pending` before the client-side
 *  watchdog flips it to `failed`. Nothing server-side ever expires a pending
 *  bubble, so without this a dropped/never-drained task "thinks" forever. */
export const PENDING_TIMEOUT_MS = 90_000;

/** Honest failure text for a watchdog-expired pending bubble. */
export const PENDING_TIMEOUT_MESSAGE =
  'No agent picked this up — the orchestrator may be overloaded or down.';

/** A frame delivered over the `vox://agent-events` Tauri event. */
export interface AgentEventFrame {
  id: number;
  timestamp_ms: number;
  kind: { type: string; [k: string]: unknown };
}

export interface ChatState {
  messages: ChatMessage[];
  /** agent_id -> task_id (seeded by `task_started`; tokens carry only agent_id). */
  agentToTask: Record<string, string>;
  /** task_id -> runId (seeded when submit resolves). */
  taskToRun: Record<string, string>;
}

export const initialChatState: ChatState = {
  messages: [],
  agentToTask: {},
  taskToRun: {},
};

/** Assistant bubbles that finished streaming and are not yet in `alreadyPersisted`. */
export function assistantMessagesReadyToPersist(
  messages: ChatMessage[],
  alreadyPersisted: ReadonlySet<string>,
): ChatMessage[] {
  return messages.filter(
    (m) =>
      m.role === 'assistant' &&
      (m.status === 'done' || m.status === 'failed') &&
      !alreadyPersisted.has(m.id),
  );
}

/** Text stored for a persisted assistant row (errors surface in content). */
export function assistantPersistContent(message: ChatMessage): string {
  if (message.status === 'failed') {
    const err = message.error?.trim();
    if (err) return err;
  }
  return message.text;
}

export type ChatAction =
  | { type: 'submit'; runId: string; prompt: string; sessionId?: string; nowMs?: number }
  | { type: 'submitResolved'; runId: string; taskId: string }
  | { type: 'failRun'; runId: string; error: string }
  | { type: 'agentEvent'; event: AgentEventFrame }
  /** Watchdog sweep: flip bubbles stuck in `pending` since before
   *  `nowMs - PENDING_TIMEOUT_MS` to `failed` (honest client-side expiry). */
  | { type: 'pendingTimeout'; nowMs: number }
  /** Plain chat send via `chat_send_message` (synchronous agent-loop reply,
   *  not the `submit`/`submitResolved`/`agentEvent` background-task
   *  correlation path). Adds the user bubble + a pending assistant bubble
   *  keyed by `tempId` rather than a `runId`/`taskId` pair, since there is
   *  no background task to correlate against. */
  | { type: 'chatPending'; sessionId: string; tempId: string; userText: string; nowMs?: number }
  /** Replaces the `chatPending` bubble identified by `tempId` with the real
   *  persisted reply (on success) or marks it failed (on error). */
  | {
      type: 'chatReplySettled';
      sessionId: string;
      tempId: string;
      result:
        | { ok: true; message: ChatMessage }
        | { ok: false; error: string };
    };

/**
 * Messages belonging to `sessionId`. Pre-session messages (sessionId == null)
 * stay visible in whatever tab is active rather than being orphaned.
 */
export function messagesForSession(state: ChatState, sessionId: string): ChatMessage[] {
  return state.messages.filter((m) => m.sessionId === sessionId || m.sessionId == null);
}

const assistantId = (runId: string) => `${runId}:asst`;

/** Apply `f` to the assistant message for `runId`, if present (immutable). */
function mapAssistant(
  state: ChatState,
  runId: string | undefined,
  f: (m: ChatMessage) => ChatMessage,
): ChatState {
  if (!runId) return state;
  let changed = false;
  const messages = state.messages.map((m) => {
    if (m.role === 'assistant' && m.runId === runId) {
      changed = true;
      return f(m);
    }
    return m;
  });
  return changed ? { ...state, messages } : state;
}

/** Drop a completed/failed task's agentToTask entry so a later CostIncurred
 *  frame for a reused agent_id can't be misrouted to this (now-stale) task
 *  by cross-session scans (see resolveSessionForEvent). CostIncurred carries
 *  no task_id/session_id of its own, so this eviction is what keeps the
 *  agent_id -> task_id mapping from outliving the task it described. */
function evictAgentToTask(state: ChatState, agentId: unknown): ChatState {
  const key = String(agentId);
  if (!(key in state.agentToTask)) return state;
  const agentToTask = { ...state.agentToTask };
  delete agentToTask[key];
  return { ...state, agentToTask };
}

/**
 * Pure reducer correlating Loquela submissions with the live agent-event
 * stream. `task_id` is normalized to a string everywhere (submit returns a
 * string; event frames carry a JSON number).
 */
export function chatReducer(state: ChatState, action: ChatAction): ChatState {
  switch (action.type) {
    case 'submit': {
      const createdAtMs = action.nowMs ?? Date.now();
      const user: ChatMessage = {
        id: `${action.runId}:user`,
        role: 'user',
        text: action.prompt,
        status: 'done',
        runId: action.runId,
        sessionId: action.sessionId,
        createdAtMs,
      };
      const assistant: ChatMessage = {
        id: assistantId(action.runId),
        role: 'assistant',
        text: '',
        status: 'pending',
        runId: action.runId,
        sessionId: action.sessionId,
        createdAtMs,
      };
      return { ...state, messages: [...state.messages, user, assistant] };
    }

    case 'pendingTimeout': {
      const cutoff = action.nowMs - PENDING_TIMEOUT_MS;
      let changed = false;
      const messages = state.messages.map((m) => {
        if (
          m.role === 'assistant' &&
          m.status === 'pending' &&
          m.createdAtMs != null &&
          m.createdAtMs <= cutoff
        ) {
          changed = true;
          return { ...m, status: 'failed' as const, error: PENDING_TIMEOUT_MESSAGE };
        }
        return m;
      });
      return changed ? { ...state, messages } : state;
    }

    case 'chatPending': {
      const createdAtMs = action.nowMs ?? Date.now();
      const user: ChatMessage = {
        id: `${action.tempId}:user`,
        role: 'user',
        text: action.userText,
        status: 'done',
        runId: action.tempId,
        sessionId: action.sessionId,
        createdAtMs,
      };
      const pending: ChatMessage = {
        id: action.tempId,
        role: 'assistant',
        text: '',
        status: 'pending',
        runId: action.tempId,
        sessionId: action.sessionId,
        createdAtMs,
      };
      return { ...state, messages: [...state.messages, user, pending] };
    }

    case 'chatReplySettled': {
      let changed = false;
      const result = action.result;
      const errorText = 'error' in result ? result.error : undefined;
      const okMessage = 'message' in result ? result.message : undefined;
      const messages = state.messages.map((m) => {
        if (m.id !== action.tempId) return m;
        changed = true;
        if (okMessage) {
          return { ...okMessage, sessionId: action.sessionId };
        }
        return { ...m, status: 'failed' as const, error: errorText };
      });
      return changed ? { ...state, messages } : state;
    }

    case 'submitResolved': {
      const taskId = String(action.taskId);
      const next = { ...state, taskToRun: { ...state.taskToRun, [taskId]: action.runId } };
      return mapAssistant(next, action.runId, (m) => ({ ...m, taskId }));
    }

    case 'failRun':
      return mapAssistant(state, action.runId, (m) => ({
        ...m,
        status: 'failed',
        error: action.error,
      }));

    case 'agentEvent': {
      const { kind } = action.event;
      switch (kind.type) {
        case 'task_started': {
          const agentId = String(kind.agent_id);
          const taskId = String(kind.task_id);
          return { ...state, agentToTask: { ...state.agentToTask, [agentId]: taskId } };
        }
        case 'cost_incurred': {
          const agentId = String(kind.agent_id);
          const taskId = state.agentToTask[agentId];
          const runId = taskId ? state.taskToRun[taskId] : undefined;
          const model = typeof kind.model === 'string' ? kind.model : '';
          if (!model) return state;
          return mapAssistant(state, runId, (m) => (m.modelId ? m : { ...m, modelId: model }));
        }
        case 'token_streamed': {
          const text = typeof kind.text === 'string' ? kind.text : '';
          const agentId = String(kind.agent_id);
          const taskId = state.agentToTask[agentId];
          const runId = taskId ? state.taskToRun[taskId] : undefined;
          if (runId) {
            return mapAssistant(state, runId, (m) => ({
              ...m,
              text: m.text + text,
              status: 'streaming',
            }));
          }
          // Task G1: no background-task correlation for this frame (the sync
          // `chat_turn` path never populates agentToTask/taskToRun -- there is
          // no task_started for it). `sessionChatStore.resolveSessionForEvent`
          // already routed us to the right session via the frame's
          // `session_id`, so append to that session's single in-flight
          // pending/streaming assistant bubble (the `chatPending` row) instead
          // of dropping the token on the floor.
          const target = [...state.messages]
            .reverse()
            .find((m) => m.role === 'assistant' && (m.status === 'pending' || m.status === 'streaming'));
          if (!target) return state;
          return mapAssistant(state, target.runId, (m) => ({
            ...m,
            text: m.text + text,
            status: 'streaming',
          }));
        }
        case 'task_completed': {
          const runId = state.taskToRun[String(kind.task_id)];
          const next = mapAssistant(state, runId, (m) => ({ ...m, status: 'done' }));
          return evictAgentToTask(next, kind.agent_id);
        }
        case 'task_failed': {
          const runId = state.taskToRun[String(kind.task_id)];
          const error = typeof kind.error === 'string' ? kind.error : undefined;
          const next = mapAssistant(state, runId, (m) => ({ ...m, status: 'failed', error }));
          return evictAgentToTask(next, kind.agent_id);
        }
        case 'grounding_check_completed': {
          const runId = state.taskToRun[String(kind.task_id)];
          if (!kind.flagged) return state;
          return mapAssistant(state, runId, (m) => ({ ...m, groundingFlagged: true }));
        }
        case 'tool_timed_out': {
          const tool = typeof kind.tool_key === 'string' ? kind.tool_key : 'tool';
          const agentId = String(kind.agent_id ?? '?');
          return {
            ...state,
            messages: [
              ...state.messages,
              {
                id: `sys-tool-${action.event.id}`,
                role: 'system',
                text: `Tool timed out (${tool}) · agent ${agentId}`,
                status: 'done',
                runId: '',
              },
            ],
          };
        }
        case 'task_phase_changed': {
          const phase = typeof kind.phase === 'string' ? kind.phase : '';
          if (phase !== 'act') return state;
          const taskId = String(kind.task_id);
          const runId = state.taskToRun[taskId];
          const line = `[executing tools · task ${taskId}]`;
          return mapAssistant(state, runId, (m) => ({
            ...m,
            text: m.text.includes(line) ? m.text : `${m.text}${m.text ? '\n' : ''}${line}`,
          }));
        }
        case 'activity_changed': {
          const activity = typeof kind.activity === 'string' ? kind.activity : '';
          if (activity !== 'executing') return state;
          const agentId = String(kind.agent_id ?? '?');
          const skill =
            typeof kind.active_skill === 'string' && kind.active_skill.trim()
              ? kind.active_skill.trim()
              : null;
          const line = skill
            ? `[tool start · ${skill} · agent ${agentId}]`
            : `[tool start · agent ${agentId}]`;
          return {
            ...state,
            messages: [
              ...state.messages,
              {
                id: `sys-tool-start-${action.event.id}`,
                role: 'system',
                text: line,
                status: 'done',
                runId: '',
              },
            ],
          };
        }
        case 'snapshot_captured': {
          const snapshotId =
            typeof kind.snapshot_id === 'string' ? kind.snapshot_id : 'snapshot';
          const fileCount = typeof kind.file_count === 'number' ? kind.file_count : 0;
          const desc =
            typeof kind.description === 'string' && kind.description.trim()
              ? kind.description.trim()
              : 'workspace checkpoint';
          return {
            ...state,
            messages: [
              ...state.messages,
              {
                id: `sys-checkpoint-${action.event.id}`,
                role: 'system',
                text: `Checkpoint saved · ${desc} (${fileCount} files) · ${snapshotId}`,
                status: 'done',
                runId: '',
              },
            ],
          };
        }
        default:
          return state;
      }
    }

    default:
      return state;
  }
}
