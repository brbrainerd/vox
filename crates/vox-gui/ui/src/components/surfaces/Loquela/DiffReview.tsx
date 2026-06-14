import React from 'react';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';

interface DiffReviewProps {
  diff: string;
  loading?: boolean;
  onClose: () => void;
}

export function DiffReview({ diff, loading, onClose }: DiffReviewProps) {
  if (loading) {
    return (
      <Glass className="mb-3 px-3 py-2 text-[12px] text-zinc-500">
        Loading worktree diff…
      </Glass>
    );
  }

  const trimmed = diff.trim();
  if (!trimmed) {
    return (
      <Glass className="mb-3 px-3 py-2 flex items-center justify-between gap-2">
        <span className="text-[12px] text-zinc-500">No unstaged changes in the worktree.</span>
        <button
          type="button"
          onClick={onClose}
          className="text-zinc-500 hover:text-zinc-300"
          aria-label="Close diff"
        >
          <Icon.x className="size-4" aria-hidden="true" />
        </button>
      </Glass>
    );
  }

  return (
    <Glass className="mb-3 max-h-[28vh] overflow-hidden flex flex-col">
      <div className="flex items-center justify-between gap-2 border-b border-white/5 px-3 py-2">
        <div className="flex items-center gap-2">
          <Icon.file className="size-3.5 text-brass" aria-hidden="true" />
          <span className="font-mono text-[10px] uppercase tracking-widest text-zinc-400">
            Pending diff
          </span>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="text-zinc-500 hover:text-zinc-300"
          aria-label="Close diff"
        >
          <Icon.x className="size-4" aria-hidden="true" />
        </button>
      </div>
      <pre className="custom-scrollbar overflow-auto px-3 py-2 font-mono text-[11px] leading-relaxed text-zinc-300 whitespace-pre-wrap break-all">
        {trimmed}
      </pre>
    </Glass>
  );
}
