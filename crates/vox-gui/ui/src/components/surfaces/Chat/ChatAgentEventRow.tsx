import React, { useState } from 'react';
import { Pill } from '../../ui/Pill';
import { Button } from '../../ui/Button';
import type { StreamItem } from '../../../types/dashboard';
import type { TranscriptAgentRow, TranscriptTokenGroupRow } from '../../../lib/chatTranscriptTimeline';
import { PhaseChip, type PavPhase } from './PhaseChip';
import { invoke } from '@tauri-apps/api/core';

type AgentTimelineRow = TranscriptAgentRow | TranscriptTokenGroupRow;

interface ChatAgentEventRowProps {
  row: AgentTimelineRow;
  onOpenAgent?: (agentId: string) => void;
}

function toneForItem(item: StreamItem) {
  if (item.kind === 'doubted') {
    return { phase: 'Doubted', bar: 'from-amber-400/40 to-amber-400/0' };
  }
  if (item.tag === 'TOKEN') {
    return { phase: 'Token', bar: 'from-cyan-400/40 to-cyan-400/0' };
  }
  return { phase: 'Agent', bar: 'from-brass/40 to-brass/0' };
}

function renderItemBody(item: StreamItem) {
  return (
    <>
      <div className="font-display text-[13px] font-medium tracking-tight text-text-primary">{item.title}</div>
      {item.body ? (
        <div className="text-[12px] leading-relaxed text-text-muted whitespace-pre-wrap wrap-break-word">{item.body}</div>
      ) : null}
    </>
  );
}

export function ChatAgentEventRow({ row, onOpenAgent }: ChatAgentEventRowProps) {
  const [expanded, setExpanded] = useState(!row.collapsed);
  const agentId = row.agentId;

  if (row.kind === 'token_group') {
    const combinedBody = row.items.map((i) => i.body).join('');
    const head = row.items[0];
    const tone = toneForItem(head);
    const summary = combinedBody.length > 80 ? `${combinedBody.slice(0, 80)}…` : combinedBody;

    return (
      <div
        className="relative overflow-hidden rounded-xl border border-border-subtle bg-overlay-subtle p-3"
        data-testid="chat-agent-token-group"
      >
        <div className={`pointer-events-none absolute inset-y-0 left-0 w-[3px] bg-linear-to-b ${tone.bar}`} />
        <div className="flex items-start justify-between gap-3 pl-1">
          <div className="min-w-0 flex-1">
            <div className="mb-1 flex flex-wrap items-center gap-2">
              <Pill phase={tone.phase} />
              <span className="font-display text-[10px] tracking-widest uppercase text-text-muted">TOKEN</span>
              <span className="font-mono text-[10px] text-text-muted">{row.items.length} chunks</span>
            </div>
            <button
              type="button"
              className="text-left text-[12px] text-text-muted hover:text-text-secondary"
              aria-expanded={expanded}
              onClick={() => setExpanded((v) => !v)}
            >
              {expanded ? 'Hide token stream' : summary || 'Show token stream'}
            </button>
            {expanded && (
              <div className="mt-2 rounded-lg border border-border-subtle bg-black/20 p-2 font-mono text-[11px] text-text-secondary whitespace-pre-wrap wrap-break-word">
                {combinedBody || '(empty)'}
              </div>
            )}
          </div>
          <div className="flex shrink-0 flex-col items-end gap-2">
            <span className="font-mono text-[10px] text-text-muted">{head.ts}</span>
            {agentId && onOpenAgent ? (
              <Button
                type="button"
                onClick={() => onOpenAgent(agentId)}
                className="rounded-md border border-border-subtle px-2 py-1 text-[10px] text-text-secondary hover:border-brass/30 hover:text-brass"
              >
                View in Flow
              </Button>
            ) : null}
          </div>
        </div>
      </div>
    );
  }

  const tone = toneForItem(row.item);

  // PAV phase from event metadata (populated when the task has a pav_loop).
  const pavPhase = (row.item.metadata?.pavPhase as PavPhase | undefined);
  const taskId = row.item.metadata?.taskId as number | undefined;

  const handleApprovePlan = () => {
    if (taskId != null) invoke('approve_orchestrator_task_plan', { taskId }).catch(console.error);
  };
  const handleSkipVerify = () => {
    if (taskId != null) invoke('skip_orchestrator_verify', { taskId }).catch(console.error);
  };
  const handleForceVerify = () => {
    if (taskId != null) invoke('force_orchestrator_verify', { taskId }).catch(console.error);
  };

  return (
    <div
      className="relative overflow-hidden rounded-xl border border-border-subtle bg-overlay-subtle p-3"
      data-testid="chat-agent-event-row"
    >
      <div className={`pointer-events-none absolute inset-y-0 left-0 w-[3px] bg-linear-to-b ${tone.bar}`} />
      <div className="flex items-start justify-between gap-3 pl-1">
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex flex-wrap items-center gap-2">
            <Pill phase={tone.phase} />
            <span className="font-display text-[10px] tracking-widest uppercase text-text-muted">{row.item.tag}</span>
            <span className="font-mono text-[10px] text-text-muted">{row.item.id}</span>
            {pavPhase != null && (
              <PhaseChip
                phase={pavPhase}
                onApprovePlan={handleApprovePlan}
                onSkipVerify={handleSkipVerify}
                onForceVerify={handleForceVerify}
              />
            )}
          </div>
          {row.collapsed ? (
            <button
              type="button"
              className="text-left"
              aria-expanded={expanded}
              onClick={() => setExpanded((v) => !v)}
            >
              {expanded ? renderItemBody(row.item) : (
                <div className="text-[12px] text-text-muted">{row.item.title}</div>
              )}
            </button>
          ) : (
            renderItemBody(row.item)
          )}
        </div>
        <div className="flex shrink-0 flex-col items-end gap-2">
          <span className="font-mono text-[10px] text-text-muted">{row.item.ts}</span>
          {agentId && onOpenAgent ? (
            <Button
              type="button"
              onClick={() => onOpenAgent(agentId)}
              className="rounded-md border border-border-subtle px-2 py-1 text-[10px] text-text-secondary hover:border-brass/30 hover:text-brass"
            >
              View in Flow
            </Button>
          ) : null}
        </div>
      </div>
    </div>
  );
}
