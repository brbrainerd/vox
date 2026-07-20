import type { ChatMessage } from './chatCorrelation';
import type { StreamItem } from '../types/dashboard';

export type TranscriptMessageRow = {
  kind: 'message';
  id: string;
  atMs: number;
  message: ChatMessage;
};

export type TranscriptAgentRow = {
  kind: 'agent';
  id: string;
  atMs: number;
  item: StreamItem;
  agentId?: string;
  collapsed: boolean;
};

export type TranscriptTokenGroupRow = {
  kind: 'token_group';
  id: string;
  atMs: number;
  items: StreamItem[];
  agentId?: string;
  collapsed: boolean;
};

export type TranscriptTimelineRow =
  | TranscriptMessageRow
  | TranscriptAgentRow
  | TranscriptTokenGroupRow;

export function isTokenStreamEvent(item: StreamItem): boolean {
  const eventType = item.metadata?.eventType;
  if (typeof eventType === 'string') return eventType === 'token_streamed';
  return item.tag === 'TOKEN';
}

function itemTimestampMs(item: StreamItem): number {
  const ts = item.metadata?.timestampMs;
  return typeof ts === 'number' ? ts : 0;
}

function itemAgentId(item: StreamItem): string | undefined {
  const id = item.metadata?.agentId;
  return typeof id === 'string' && id.length > 0 ? id : undefined;
}

type RawTimelineEntry =
  | { kind: 'message'; id: string; atMs: number; message: ChatMessage }
  | { kind: 'agent'; id: string; atMs: number; item: StreamItem; agentId?: string };

/** Merge chat bubbles and agent stream items into a single time-ordered transcript. */
export function buildTranscriptTimeline(
  messages: ChatMessage[],
  agentItems: StreamItem[],
  options?: { messageStepMs?: number },
): TranscriptTimelineRow[] {
  const messageStepMs = options?.messageStepMs ?? 1000;

  const raw: RawTimelineEntry[] = [
    ...messages.map((message, index) => ({
      kind: 'message' as const,
      id: message.id,
      atMs: index * messageStepMs,
      message,
    })),
    ...agentItems.map((item) => ({
      kind: 'agent' as const,
      id: item.id,
      atMs: itemTimestampMs(item),
      item,
      agentId: itemAgentId(item),
    })),
  ];

  raw.sort((a, b) => (a.atMs !== b.atMs ? a.atMs - b.atMs : a.id.localeCompare(b.id)));

  const rows: TranscriptTimelineRow[] = [];
  let tokenBuffer: StreamItem[] = [];
  let tokenAgentId: string | undefined;
  let tokenAtMs = 0;

  const flushTokens = () => {
    if (tokenBuffer.length === 0) return;
    rows.push({
      kind: 'token_group',
      id: `token-group-${tokenBuffer[0].id}`,
      atMs: tokenAtMs,
      items: tokenBuffer,
      agentId: tokenAgentId,
      collapsed: true,
    });
    tokenBuffer = [];
    tokenAgentId = undefined;
    tokenAtMs = 0;
  };

  for (const entry of raw) {
    if (entry.kind === 'message') {
      flushTokens();
      rows.push(entry);
      continue;
    }

    if (isTokenStreamEvent(entry.item)) {
      const entryAgent = entry.agentId;
      if (
        tokenBuffer.length > 0 &&
        tokenAgentId !== undefined &&
        entryAgent !== undefined &&
        tokenAgentId !== entryAgent
      ) {
        flushTokens();
      }
      if (tokenBuffer.length === 0) {
        tokenAtMs = entry.atMs;
        tokenAgentId = entryAgent;
      }
      tokenBuffer.push(entry.item);
      continue;
    }

    flushTokens();
    rows.push({
      kind: 'agent',
      id: entry.id,
      atMs: entry.atMs,
      item: entry.item,
      agentId: entry.agentId,
      collapsed: false,
    });
  }

  flushTokens();
  return rows;
}

export type TranscriptStatusRow = {
  kind: 'status';
  id: string;
  atMs: number;
  taskId: number;
  phase: string;
  elapsedMs: number;
};

export type TranscriptSummaryRow = {
  kind: 'summary';
  id: string;
  atMs: number;
  taskId: number;
  costUsd: number;
};

export type ChatOnlyTimelineRow = TranscriptMessageRow | TranscriptStatusRow | TranscriptSummaryRow;

const IN_FLIGHT_EVENT_TYPES = new Set(['task_started', 'task_phase_changed']);
const TASK_END_EVENT_TYPES = new Set(['task_completed', 'task_failed']);

/**
 * Chat-feed-only view of the timeline: real messages plus at most one live
 * status row per in-flight task (phase + elapsed time), plus an optional
 * done-summary row once a task completes (verbosity-gated). Every raw agent
 * event (CHECKPOINT/TASK/PHASE/COST/TOKEN) that used to render as its own
 * row via `ChatAgentEventRow` is excluded here — full detail stays available
 * via `buildTranscriptTimeline` for the Flow panel.
 */
export function buildChatOnlyTimeline(
  messages: ChatMessage[],
  agentItems: StreamItem[],
  options?: { messageStepMs?: number; nowMs?: number; verbosity?: 'quiet' | 'normal' | 'verbose' },
): ChatOnlyTimelineRow[] {
  const messageStepMs = options?.messageStepMs ?? 1000;
  const nowMs = options?.nowMs ?? Date.now();
  const verbosity = options?.verbosity ?? 'normal';

  const rows: ChatOnlyTimelineRow[] = messages.map((message, index) => ({
    kind: 'message' as const,
    id: message.id,
    atMs: index * messageStepMs,
    message,
  }));

  // Track the latest in-flight task per taskId, in arrival order, and drop
  // any task that has since completed/failed. Also track each task's last
  // known cost and whether it completed, for the optional summary row.
  const inFlight = new Map<number, { phase: string; startedAtMs: number }>();
  const lastCostByTask = new Map<number, number>();
  const completedTasks = new Set<number>();

  for (const item of agentItems) {
    const eventType = item.metadata?.eventType;
    const taskId = item.taskId ?? (item.metadata?.taskId as number | undefined);
    if (taskId == null) continue;

    if (typeof eventType === 'string' && eventType === 'cost_incurred') {
      const costUsd = item.metadata?.costUsd;
      if (typeof costUsd === 'number') lastCostByTask.set(taskId, costUsd);
      continue;
    }
    if (typeof eventType === 'string' && TASK_END_EVENT_TYPES.has(eventType)) {
      inFlight.delete(taskId);
      if (eventType === 'task_completed') completedTasks.add(taskId);
      continue;
    }
    if (typeof eventType === 'string' && IN_FLIGHT_EVENT_TYPES.has(eventType)) {
      const ts = typeof item.metadata?.timestampMs === 'number' ? item.metadata.timestampMs : 0;
      const existing = inFlight.get(taskId);
      const startedAtMs = eventType === 'task_started' ? ts : (existing?.startedAtMs ?? ts);
      const phase =
        eventType === 'task_phase_changed' && typeof item.metadata?.phase === 'string'
          ? item.metadata.phase
          : (existing?.phase ?? 'Working');
      inFlight.set(taskId, { phase, startedAtMs });
    }
  }

  for (const [taskId, { phase, startedAtMs }] of inFlight) {
    rows.push({
      kind: 'status',
      id: `status-${taskId}`,
      atMs: nowMs,
      taskId,
      phase,
      elapsedMs: Math.max(0, nowMs - startedAtMs),
    });
  }

  if (verbosity !== 'quiet') {
    for (const taskId of completedTasks) {
      const costUsd = lastCostByTask.get(taskId);
      if (costUsd == null) continue;
      rows.push({ kind: 'summary', id: `summary-${taskId}`, atMs: nowMs, taskId, costUsd });
    }
  }

  return rows;
}
