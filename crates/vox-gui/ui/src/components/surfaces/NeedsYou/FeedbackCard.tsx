import React from 'react';
import { Glass } from '../../ui/Glass';
import { Pill } from '../../ui/Pill';
import { Icon } from '../../ui/Icons';
import type { FeedbackRow } from '../../../transport';

interface Props {
  row: FeedbackRow;
  onResolve: (id: string, action: Record<string, any>) => void;
  onOpenContext: (id: string) => void;
}

export function FeedbackCard({ row, onResolve, onOpenContext }: Props) {
  if (row.kind === 'skill_proposal') {
    return (
      <Glass size="sm" className="border-b border-border-subtle">
        <div className="flex items-center gap-2 mb-1">
          <Pill phase="Clarification" label="Skill Proposal" />
          <span className="text-[10px] text-text-muted font-mono">{row.feedbackId}</span>
        </div>
        <p className="text-xs text-text-secondary mb-2">{row.prompt}</p>
        <div className="flex gap-1.5 flex-wrap">
          <button
            type="button"
            aria-label="Save as skill"
            className="text-[11px] font-semibold px-2.5 py-1 rounded-sm border border-emerald-400/30 text-emerald-300 bg-emerald-400/10 hover:bg-emerald-400/20"
            onClick={() => onResolve(row.feedbackId, { action: 'accept_skill' })}
          >
            Save as skill
          </button>
          <button
            type="button"
            aria-label="Dismiss this skill proposal"
            className="text-[11px] font-semibold px-2.5 py-1 rounded-sm border border-border-subtle text-text-muted hover:bg-overlay-subtle"
            onClick={() => onResolve(row.feedbackId, { action: 'skip' })}
          >
            Dismiss
          </button>
        </div>
      </Glass>
    );
  }
  const isDoubt = row.kind === 'doubt';
  return (
    <Glass size="sm" className="border-b border-border-subtle">
      <div className="flex items-center gap-2 mb-1">
        <Pill phase={isDoubt ? 'Doubted' : 'Verifying'} label={isDoubt ? 'Doubt' : 'Clarification'} />
        <span className="text-[10px] text-text-muted font-mono">{row.feedbackId}</span>
        {row.gates.length > 0 && (
          <span className="text-[11px] text-text-muted">parks {row.gates.length} task{row.gates.length > 1 ? 's' : ''}</span>
        )}
      </div>
      <button
        className="text-xs text-text-secondary mb-2 text-left block w-full hover:underline"
        aria-label="Open context"
        onClick={() => onOpenContext(row.feedbackId)}
      >
        {row.prompt}
      </button>
      <div className="flex gap-1.5 flex-wrap">
        {isDoubt ? (
          <>
            <button
              aria-label="Overrule the doubt"
              className="text-[11px] font-semibold px-2.5 py-1 rounded-sm border border-emerald-400/30 text-emerald-300 bg-emerald-400/10 inline-flex items-center gap-1 hover:bg-emerald-400/20"
              onClick={() => onResolve(row.feedbackId, { action: 'overrule' })}
            >
              <Icon.gavel className="size-3.5" />Overrule
            </button>
            <button
              aria-label="Let the agent verify"
              className="text-[11px] font-semibold px-2.5 py-1 rounded-sm border border-border-subtle text-text-muted hover:bg-overlay-subtle"
              onClick={() => onResolve(row.feedbackId, { action: 'let_verify' })}
            >
              Let it verify
            </button>
          </>
        ) : (
          <>
            {row.options.map((opt, i) => (
              <button
                key={i}
                aria-label={`Answer: ${opt}`}
                className="text-[11px] font-semibold px-2.5 py-1 rounded-sm border border-emerald-400/30 text-emerald-300 bg-emerald-400/10 hover:bg-emerald-400/20"
                onClick={() => onResolve(row.feedbackId, { action: 'answer', option: i, text: null })}
              >
                {opt}
              </button>
            ))}
            <button
              aria-label="Answer in free text"
              className="text-[11px] font-semibold px-2.5 py-1 rounded-sm border border-border-subtle text-text-muted hover:bg-overlay-subtle"
              onClick={() => onOpenContext(row.feedbackId)}
            >
              ✎ Answer…
            </button>
            <button
              aria-label="Skip this question"
              className="text-[11px] font-semibold px-2.5 py-1 rounded-sm border border-border-subtle text-text-muted hover:bg-overlay-subtle"
              onClick={() => onResolve(row.feedbackId, { action: 'skip' })}
            >
              Skip
            </button>
          </>
        )}
      </div>
    </Glass>
  );
}
