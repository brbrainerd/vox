//! B4-chat: pure client-side correlation of streamed agent events to chat
//! bubbles. No orchestrator change — `submit_orchestrator_task` returns a
//! `task_id`, `task_started` ties an `agent_id` to that task, `token_streamed`
//! (which carries only `agent_id`) is routed through that map, and
//! `task_completed`/`task_failed` finalize. See
//! `docs/src/architecture/vox-gui-harness-buildout-plan-2026.md` (B4-chat).

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
  | { type: 'submit'; runId: string; prompt: string; sessionId?: string }
  | { type: 'submitResolved'; runId: string; taskId: string }
  | { type: 'failRun'; runId: string; error: string }
  | { type: 'agentEvent'; event: AgentEventFrame };

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
        sessionId: action.sessionId,
      };
      const assistant: ChatMessage = {
        id: assistantId(action.runId),
        role: 'assistant',
        text: '',
        status: 'pending',
        runId: action.runId,
        sessionId: action.sessionId,
      };
      return { ...state, messages: [...state.messages, user, assistant] };
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
