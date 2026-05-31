//! B4-chat: pure client-side correlation of streamed agent events to chat
//! bubbles. No orchestrator change — `submit_orchestrator_task` returns a
//! `task_id`, `task_started` ties an `agent_id` to that task, `token_streamed`
//! (which carries only `agent_id`) is routed through that map, and
//! `task_completed`/`task_failed` finalize. See
//! `docs/src/architecture/vox-gui-harness-buildout-plan-2026.md` (B4-chat).

export type ChatRole = 'user' | 'assistant';
export type ChatStatus = 'pending' | 'streaming' | 'done' | 'failed';

export interface ChatMessage {
  id: string;
  role: ChatRole;
  text: string;
  status: ChatStatus;
  runId: string;
  taskId?: string;
  error?: string;
}

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

export type ChatAction =
  | { type: 'submit'; runId: string; prompt: string }
  | { type: 'submitResolved'; runId: string; taskId: string }
  | { type: 'agentEvent'; event: AgentEventFrame };

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

/**
 * Pure reducer correlating Loquela submissions with the live agent-event
 * stream. `task_id` is normalized to a string everywhere (submit returns a
 * string; event frames carry a JSON number).
 */
export function chatReducer(state: ChatState, action: ChatAction): ChatState {
  switch (action.type) {
    case 'submit': {
      const user: ChatMessage = {
        id: `${action.runId}:user`,
        role: 'user',
        text: action.prompt,
        status: 'done',
        runId: action.runId,
      };
      const assistant: ChatMessage = {
        id: assistantId(action.runId),
        role: 'assistant',
        text: '',
        status: 'pending',
        runId: action.runId,
      };
      return { ...state, messages: [...state.messages, user, assistant] };
    }

    case 'submitResolved': {
      const taskId = String(action.taskId);
      const next = { ...state, taskToRun: { ...state.taskToRun, [taskId]: action.runId } };
      return mapAssistant(next, action.runId, (m) => ({ ...m, taskId }));
    }

    case 'agentEvent': {
      const { kind } = action.event;
      switch (kind.type) {
        case 'task_started': {
          const agentId = String(kind.agent_id);
          const taskId = String(kind.task_id);
          return { ...state, agentToTask: { ...state.agentToTask, [agentId]: taskId } };
        }
        case 'token_streamed': {
          const agentId = String(kind.agent_id);
          const taskId = state.agentToTask[agentId];
          const runId = taskId ? state.taskToRun[taskId] : undefined;
          const text = typeof kind.text === 'string' ? kind.text : '';
          return mapAssistant(state, runId, (m) => ({
            ...m,
            text: m.text + text,
            status: 'streaming',
          }));
        }
        case 'task_completed': {
          const runId = state.taskToRun[String(kind.task_id)];
          return mapAssistant(state, runId, (m) => ({ ...m, status: 'done' }));
        }
        case 'task_failed': {
          const runId = state.taskToRun[String(kind.task_id)];
          const error = typeof kind.error === 'string' ? kind.error : undefined;
          return mapAssistant(state, runId, (m) => ({ ...m, status: 'failed', error }));
        }
        default:
          return state;
      }
    }

    default:
      return state;
  }
}
