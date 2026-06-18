import React from 'react';
import { Glass } from '../../ui/Glass';
import type { ChatMessage } from '../../../lib/chatCorrelation';
import type { StreamItem } from '../../../types/dashboard';
import { buildTranscriptTimeline } from '../../../lib/chatTranscriptTimeline';
import { ChatAgentEventRow } from './ChatAgentEventRow';

interface ChatTranscriptProps {
  messages: ChatMessage[];
  agentStreamItems?: StreamItem[];
  onOpenAgentInFlow?: (agentId: string) => void;
}

function MessageBubble({ message }: { message: ChatMessage }) {
  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';
  const streaming = message.status === 'streaming' || message.status === 'pending';
  const failed = message.status === 'failed';

  const tone = isSystem
    ? 'self-center border-amber-400/20 bg-amber-400/[0.06] text-amber-100/90 text-center max-w-full'
    : isUser
      ? 'self-end border-brass/30 bg-brass/[0.08] text-zinc-100'
      : 'self-start border-white/10 bg-white/[0.03] text-zinc-300';

  return (
    <div
      className={`max-w-[80%] rounded-xl border px-3 py-2 text-[12px] leading-relaxed whitespace-pre-wrap break-words ${tone}`}
    >
      {!isSystem && (
        <div className="mb-0.5 font-mono text-[9px] uppercase tracking-wide text-zinc-500">
          {isUser ? 'You' : 'Assistant'}
        </div>
      )}
      {message.text}
      {streaming && (
        <span className="ml-1 inline-flex items-center gap-1 text-[10px] text-cyan-300">
          <span className="size-1.5 animate-pulse rounded-full bg-cyan-300" />
          {message.text ? 'streaming…' : 'thinking…'}
        </span>
      )}
      {failed && (
        <div className="mt-1 font-mono text-[10px] text-rose-400">
          error: {message.error ?? 'task failed'}
        </div>
      )}
    </div>
  );
}

/** Merged chat bubbles + inline agent execution rows for the active session. */
export function ChatTranscript({
  messages,
  agentStreamItems,
  onOpenAgentInFlow,
}: ChatTranscriptProps) {
  const hasAgentItems = (agentStreamItems?.length ?? 0) > 0;

  if (!hasAgentItems) {
    if (messages.length === 0) return null;
    return (
      <Glass
        role="log"
        aria-live="polite"
        aria-relevant="additions text"
        aria-label="Chat transcript"
        className="mb-3 max-h-[40vh] overflow-y-auto custom-scrollbar p-3"
      >
        <div className="flex flex-col gap-2">
          {messages.map((m) => (
            <MessageBubble key={m.id} message={m} />
          ))}
        </div>
      </Glass>
    );
  }

  const timeline = buildTranscriptTimeline(messages, agentStreamItems ?? []);

  if (timeline.length === 0) return null;

  return (
    <Glass
      role="log"
      aria-live="polite"
      aria-relevant="additions text"
      aria-label="Chat transcript"
      className="mb-3 max-h-[40vh] overflow-y-auto custom-scrollbar p-3"
    >
      <div className="flex flex-col gap-2">
        {timeline.map((row) => {
          if (row.kind === 'message') {
            return <MessageBubble key={row.id} message={row.message} />;
          }
          return (
            <ChatAgentEventRow
              key={row.id}
              row={row}
              onOpenAgent={onOpenAgentInFlow}
            />
          );
        })}
      </div>
    </Glass>
  );
}
