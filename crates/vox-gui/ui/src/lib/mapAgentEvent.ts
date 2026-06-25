import type { AgentEventFrame } from './chatCorrelation';
import type { StreamItem } from '../types/dashboard';

const AGENT_EVENT_LABELS: Record<string, string> = {
  token_streamed: 'TOKEN',
  task_started: 'TASK',
  task_phase_changed: 'PHASE',
  task_completed: 'DONE',
  task_failed: 'FAILED',
  agent_spawned: 'SPAWN',
  agent_retired: 'RETIRE',
  cost_incurred: 'COST',
  snapshot_captured: 'CHECKPOINT',
  activity_changed: 'ACTIVITY',
  tool_timed_out: 'TOOL',
};

/** Map a live `vox://agent-events` frame to dashboard/chat stream item shape. */
export function mapAgentEvent(e: AgentEventFrame): StreamItem {
  const kind = e.kind ?? ({ type: 'unknown' } as AgentEventFrame['kind']);
  const type = kind.type ?? 'unknown';
  const tag = AGENT_EVENT_LABELS[type] ?? type.replace(/_/g, ' ').toUpperCase();
  const agentId = kind.agent_id != null ? String(kind.agent_id) : undefined;

  let title = type.replace(/_/g, ' ');
  let body = '';
  switch (type) {
    case 'token_streamed':
      title = `Token · ${agentId ?? '?'}`;
      body = String(kind.text ?? '');
      break;
    case 'task_started':
    case 'task_phase_changed':
    case 'task_completed':
    case 'task_failed':
      title = `${tag} · task ${kind.task_id ?? '?'}`;
      body = kind.phase
        ? `phase: ${kind.phase}`
        : kind.error
          ? `error: ${kind.error}`
          : agentId
            ? `agent ${agentId}`
            : '';
      break;
    case 'agent_spawned':
    case 'agent_retired':
      title = `${tag} · agent ${agentId ?? '?'}`;
      break;
    case 'snapshot_captured':
      title = `${tag} · ${kind.snapshot_id ?? 'snapshot'}`;
      body = typeof kind.description === 'string' ? kind.description : '';
      break;
    case 'activity_changed':
      title = `${tag} · agent ${agentId ?? '?'}`;
      body = typeof kind.activity === 'string' ? kind.activity : '';
      break;
    default:
      body = '';
  }

  const isFailed = type === 'task_failed';
  const rawTaskId = kind.task_id;
  const taskId = rawTaskId != null ? Number(rawTaskId) : undefined;

  return {
    id: String(e.id),
    kind: isFailed ? 'doubted' : 'agent',
    tag,
    title,
    body,
    ts: e.timestamp_ms
      ? new Date(e.timestamp_ms).toLocaleTimeString()
      : 'now',
    ...(taskId != null ? { taskId } : {}),
    metadata: {
      eventType: type,
      agentId,
      timestampMs: e.timestamp_ms ?? 0,
    },
  };
}
