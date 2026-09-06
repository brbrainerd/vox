import React from 'react';
import { Glass } from '../../ui/Glass';
import type { ChatMessage } from '../../../lib/chatCorrelation';

/**
 * Presentational B4-chat transcript. The pure `chatReducer` in
 * `lib/chatCorrelation.ts` owns the state; this component only renders the
 * `ChatMessage[]` it is handed (user vs assistant bubbles, a streaming
 * indicator while pending/streaming, and an error line on failure).
 */
export function Transcript({ messages }: { messages: ChatMessage[] }) {
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
          <Bubble key={m.id} message={m} />
        ))}
      </div>
    </Glass>
  );
}

function Bubble({ message }: { message: ChatMessage }) {
  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';
  const streaming = message.status === 'streaming' || message.status === 'pending';
  const failed = message.status === 'failed';

  const tone = isSystem
    ? 'self-center border-amber-400/20 bg-amber-400/6 text-amber-100/90 text-center max-w-full'
    : isUser
      ? 'self-end border-brass/30 bg-brass/8 text-text-primary'
      : 'self-start border-border-subtle bg-overlay-subtle text-text-secondary';

  return (
    <div
      className={`max-w-[80%] rounded-xl border px-3 py-2 text-[12px] leading-relaxed whitespace-pre-wrap wrap-break-word ${tone}`}
    >
      {!isSystem && (
        <div className="mb-0.5 vox-display text-[9px] text-text-muted">
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
