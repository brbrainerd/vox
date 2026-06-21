import React from 'react';
import { Glass } from '../ui/Glass';

interface FunGaugeProps {
  grindRatio: number;
  avgMultiplier: number;
  questsCompleted: number;
}

export const FunGauge: React.FC<FunGaugeProps> = ({
  grindRatio,
  avgMultiplier,
  questsCompleted,
}) => {
  return (
    <Glass size="md" className="bg-zinc-950/80 border-zinc-800 text-zinc-100 flex flex-col gap-4 select-none w-full">
      <div className="text-zinc-400 text-xs font-semibold uppercase tracking-wider">
        Engagement Metrics
      </div>
      <div className="flex flex-col gap-3">
        {/* Grind Ratio */}
        <div className="flex flex-col gap-1">
          <div className="flex justify-between items-center text-xs">
            <span className="text-zinc-400">Grind Ratio</span>
            <span data-testid="grind-ratio" className="font-mono font-bold text-zinc-300">
              {Math.round(grindRatio * 100)}%
            </span>
          </div>
          <div className="h-2 bg-zinc-900 border border-zinc-800 rounded-full overflow-hidden">
            <div
              className="h-full bg-amber-500 transition-all duration-300"
              style={{ width: `${Math.min(100, Math.max(0, grindRatio * 100))}%` }}
            />
          </div>
        </div>

        {/* Avg Multiplier */}
        <div className="flex flex-col gap-1">
          <div className="flex justify-between items-center text-xs">
            <span className="text-zinc-400">Avg Multiplier</span>
            <span data-testid="avg-multiplier" className="font-mono font-bold text-brass">
              {avgMultiplier.toFixed(1)}x
            </span>
          </div>
          <div className="h-2 bg-zinc-900 border border-zinc-800 rounded-full overflow-hidden">
            <div
              className="h-full bg-yellow-500 transition-all duration-300"
              // normalize 1.0 - 3.0 multiplier to 0 - 100%
              style={{
                width: `${Math.min(100, Math.max(0, ((avgMultiplier - 1.0) / 2.0) * 100))}%`,
              }}
            />
          </div>
        </div>

        {/* Quests Completed */}
        <div className="flex flex-col gap-1">
          <div className="flex justify-between items-center text-xs">
            <span className="text-zinc-400">Quests Completed</span>
            <span data-testid="quests-completed" className="font-mono font-bold text-emerald-400">
              {questsCompleted}
            </span>
          </div>
          <div className="h-2 bg-zinc-900 border border-zinc-800 rounded-full overflow-hidden">
            <div
              className="h-full bg-emerald-500 transition-all duration-300"
              // arbitrary cap at 10 quests for 100% representation
              style={{ width: `${Math.min(100, (questsCompleted / 10) * 100)}%` }}
            />
          </div>
        </div>
      </div>
    </Glass>
  );
};
