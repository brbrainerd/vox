import React from 'react';
import { Glass } from '../ui/Glass';

interface HudPanelsProps {
  treasuryValue: number;
  energy: number;
  speed: number;
  onSetSpeed: (speed: number) => void;
}

export const HudPanels: React.FC<HudPanelsProps> = ({
  treasuryValue,
  energy,
  speed,
  onSetSpeed,
}) => {
  return (
    <Glass size="sm" className="flex items-center gap-4 bg-zinc-950/80 pointer-events-auto border border-zinc-800 text-zinc-100 select-none">
      <div className="flex items-center gap-2 border-r border-zinc-800 pr-3">
        <span className="text-zinc-500 text-xs font-semibold uppercase tracking-wider animate-pulse">Treasury</span>
        <span data-testid="hud-value" className="text-amber-400 font-bold font-mono">
          {treasuryValue}
        </span>
      </div>
      <div className="flex items-center gap-2 border-r border-zinc-800 pr-3">
        <span className="text-zinc-500 text-xs font-semibold uppercase tracking-wider">Energy</span>
        <span data-testid="hud-energy" className="text-emerald-400 font-bold font-mono">
          {energy}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <span className="text-zinc-500 text-xs font-semibold uppercase tracking-wider">Speed</span>
        <div className="flex items-center gap-1">
          {[1, 2, 4].map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => onSetSpeed(s)}
              className={`px-1.5 py-0.5 rounded text-[10px] font-mono transition-colors ${
                speed === s
                  ? 'bg-amber-500/20 border border-amber-500/50 text-amber-300'
                  : 'bg-zinc-900 border border-zinc-800 text-zinc-400 hover:text-zinc-200'
              }`}
            >
              {s}x
            </button>
          ))}
        </div>
      </div>
    </Glass>
  );
};
