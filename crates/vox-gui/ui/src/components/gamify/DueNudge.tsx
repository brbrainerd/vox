import React from 'react';
import { Glass } from '../ui/Glass';
import { Pill } from '../ui/Pill';

interface DueNudgeProps {
  count: number;
  onOpen: () => void;
}

export const DueNudge: React.FC<DueNudgeProps> = ({ count, onOpen }) => {
  if (count === 0) {
    return (
      <Glass size="sm" className="bg-zinc-950/65 border-zinc-800 text-zinc-400 select-none text-xs">
        ✨ All caught up! No due actions.
      </Glass>
    );
  }

  return (
    <Glass
      size="sm"
      interactive
      as="button"
      type="button"
      onClick={onOpen}
      className="bg-amber-950/20 border-amber-900/50 hover:bg-amber-950/30 hover:border-amber-700/50 text-amber-100 flex items-center justify-between w-full select-none"
    >
      <div className="flex items-center gap-2">
        <Pill phase="Executing" label="Due" />
        <span className="text-xs font-medium font-sans">
          {count} {count === 1 ? 'action' : 'actions'} due for review
        </span>
      </div>
      <span className="text-[10px] text-amber-400/80 uppercase font-semibold font-sans tracking-wide">
        Open &rarr;
      </span>
    </Glass>
  );
};
