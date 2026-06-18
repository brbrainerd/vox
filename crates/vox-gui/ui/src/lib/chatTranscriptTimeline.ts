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
