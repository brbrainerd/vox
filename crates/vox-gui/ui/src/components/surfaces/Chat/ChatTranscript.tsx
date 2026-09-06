import React, { useEffect, useState } from 'react';
import { Glass } from '../../ui/Glass';
import type { ChatMessage } from '../../../lib/chatCorrelation';
import type { StreamItem } from '../../../types/dashboard';
import { buildChatOnlyTimeline } from '../../../lib/chatTranscriptTimeline';
import { StatusLine } from './StatusLine';
import { ModelBadge } from './ModelBadge';
import { ChatTurnEventRow } from './ChatTurnEventRow';
import { useChatVerbosity } from '../../../hooks/useChatVerbosity';
import { listHarnessIssuesForSession, type HarnessIssueRow } from '../Scientia/harnessIssuesApi';

interface ChatTranscriptProps {
  messages: ChatMessage[];
  agentStreamItems?: StreamItem[];
  sessionId?: string;
  /** "not this one" on a skill-activation chip — see `ChatTurnEventRow`. */
  onExcludeSkill?: (skillId: string) => void;
}

export function MessageBubble({
  message,
  onExcludeSkill,
}: {
  message: ChatMessage;
  onExcludeSkill?: (skillId: string) => void;
}) {
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
      id={`msg-${message.id}`}
      className={`max-w-[80%] rounded-xl border px-3 py-2 text-[12px] leading-relaxed whitespace-pre-wrap wrap-break-word ${tone}`}
    >
      {!isSystem && (
        <div className="mb-0.5 font-mono text-[9px] uppercase tracking-wide text-text-muted">
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
      {message.role === 'assistant' && message.status === 'done' && message.modelId && (
        <div className="mt-1 flex justify-end">
          <ModelBadge
            model={message.modelId}
            latencyMs={message.latencyMs}
            selectionReason={message.selectionReason}
          />
        </div>
      )}
      {message.role === 'assistant' && message.groundingFlagged && (
        <div className="mt-1 flex justify-end">
          <span className="rounded-sm border border-amber-400/30 bg-amber-400/8 px-1.5 py-0.5 font-mono text-[9px] text-amber-300">
            low confidence — unverified
          </span>
        </div>
      )}
      {message.role === 'assistant' && message.events && message.events.length > 0 && (
        <div className="mt-1 flex flex-wrap justify-end gap-1">
          {message.events.map((ev, i) => (
            <ChatTurnEventRow key={i} event={ev} onExcludeSkill={onExcludeSkill} />
          ))}
        </div>
      )}
    </div>
  );
}

function HarnessIssueSummary({ issue }: { issue: HarnessIssueRow }) {
  const statusTone =
    issue.status === 'dismissed' ? 'text-text-muted line-through' : 'text-amber-300';
  return (
    <div
      data-testid={`transcript-harness-issue-${issue.id}`}
      className={`self-center rounded-sm border border-amber-400/30 bg-amber-400/8 px-2 py-1 text-center text-[10px] ${statusTone}`}
    >
      Issue detected ({issue.status}): {issue.summary}
    </div>
  );
}

/** Merged chat bubbles + inline agent execution rows for the active session. */
export function ChatTranscript({ messages, agentStreamItems, sessionId, onExcludeSkill }: ChatTranscriptProps) {
  const [verbosity] = useChatVerbosity();
  const timeline = buildChatOnlyTimeline(messages, agentStreamItems ?? [], { verbosity });
  const [harnessIssues, setHarnessIssues] = useState<HarnessIssueRow[]>([]);

  useEffect(() => {
    if (!sessionId) {
      setHarnessIssues([]);
      return;
    }
    let cancelled = false;
    const fetchIssues = () => {
      listHarnessIssuesForSession(sessionId)
        .then((rows) => {
          // Defend against a mocked/misbehaving backend resolving null/undefined
          // instead of rejecting — harnessIssues.length below assumes an array.
          if (!cancelled) setHarnessIssues(rows ?? []);
        })
        .catch(() => {
          if (!cancelled) setHarnessIssues([]);
        });
    };
    fetchIssues();
    // Poll (not fetch-once) so an issue detected mid-session appears without
    // requiring a session switch — matches the cadence used elsewhere for
    // this same data (App.tsx's 8s poll, HarnessIssuesPanel's 10s poll).
    const id = window.setInterval(fetchIssues, 8_000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [sessionId]);

  if (timeline.length === 0 && harnessIssues.length === 0) return null;

  return (
    <Glass
      role="log"
      aria-live="polite"
      aria-relevant="additions text"
      aria-label="Chat transcript"
      className="mb-3 min-h-0 flex-1 overflow-y-auto custom-scrollbar p-3 pb-6"
    >
      <div className="mx-auto flex w-full max-w-[900px] flex-col gap-2">
        {harnessIssues.length > 0 && (
          <div className="mb-1 flex flex-col gap-1 border-b border-border-subtle pb-2">
            {harnessIssues.map((issue) => (
              <HarnessIssueSummary key={issue.id} issue={issue} />
            ))}
          </div>
        )}
        {timeline.map((row) => {
          if (row.kind === 'message') {
            return <MessageBubble key={row.id} message={row.message} onExcludeSkill={onExcludeSkill} />;
          }
          if (row.kind === 'status') {
            return <StatusLine key={row.id} phase={row.phase} elapsedMs={row.elapsedMs} />;
          }
          return (
            <div key={row.id} className="self-start px-1 font-mono text-[10px] text-text-muted">
              Done · ${row.costUsd.toFixed(4)}
            </div>
          );
        })}
      </div>
    </Glass>
  );
}
