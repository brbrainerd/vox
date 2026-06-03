import React from 'react';
import type { StageStatus } from '../lib/pipeline';

const DOT: Record<StageStatus, string> = {
  done: 'bg-emerald-400 ring-emerald-400/30',
  active: 'bg-brass ring-brass/40 animate-vox-ping',
  pending: 'bg-white/10 ring-white/10',
  error: 'bg-rose-400 ring-rose-400/30',
};

export function PipelineTimeline({ stages, statuses }: {
  stages: readonly string[];
  statuses: Record<string, StageStatus>;
}) {
  return (
    <div className="flex flex-wrap items-center gap-1">
      {stages.map((stage, i) => (
        <React.Fragment key={stage}>
          <div className="flex items-center gap-1.5">
            <span className={`size-2.5 rounded-full ring-2 ${DOT[statuses[stage] ?? 'pending']}`} />
            <span className="font-mono text-[10px] text-zinc-500">{stage.replace(/_/g, ' ')}</span>
          </div>
          {i < stages.length - 1 && <span className="h-px w-3 bg-white/10" />}
        </React.Fragment>
      ))}
    </div>
  );
}
