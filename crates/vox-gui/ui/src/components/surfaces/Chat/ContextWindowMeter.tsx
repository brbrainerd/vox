import React from 'react';

export interface ContextWindowMeterProps {
  /** Estimated tokens currently in the active context window. */
  usedTokens: number;
  /** Maximum tokens the model can hold (from CompactionConfig). */
  maxTokens: number;
  /** Token count at which compaction will trigger. */
  thresholdTokens: number;
  /** Human-readable strategy: "aggressive", "balanced", or "conservative". */
  strategy: string;
}

type Zone = 'safe' | 'warn' | 'danger';

function getZone(pct: number): Zone {
  if (pct >= 90) return 'danger';
  if (pct >= 70) return 'warn';
  return 'safe';
}

const ZONE_FILL: Record<Zone, string> = {
  safe:   'bg-[oklch(0.7_0.15_140)]',
  warn:   'bg-[oklch(0.75_0.15_60)]',
  danger: 'bg-[oklch(0.65_0.2_25)]',
};

const ZONE_TEXT: Record<Zone, string> = {
  safe:   'text-[oklch(0.7_0.15_140)]',
  warn:   'text-[oklch(0.75_0.15_60)]',
  danger: 'text-[oklch(0.65_0.2_25)]',
};

/** Color-coded token usage progress bar for the ChatExecutionRail. */
export function ContextWindowMeter({
  usedTokens,
  maxTokens,
  thresholdTokens,
  strategy,
}: ContextWindowMeterProps) {
  const pct = Math.min(100, maxTokens === 0 ? 0 : Math.round((usedTokens / maxTokens) * 100));
  const thresholdPct = maxTokens === 0 ? 80 : Math.round((thresholdTokens / maxTokens) * 100);
  const zone = getZone(pct);

  return (
    <div
      className="flex flex-col gap-0.5 px-2 py-1"
      role="meter"
      aria-valuenow={usedTokens}
      aria-valuemin={0}
      aria-valuemax={maxTokens}
      aria-label={`Context window: ${pct}% full`}
    >
      {/* Label row */}
      <div className="flex items-center justify-between">
        <span className="text-[9px] uppercase tracking-[0.14em] text-zinc-500">Context</span>
        <span className={`font-mono text-[10px] tabular-nums ${ZONE_TEXT[zone]}`}>
          {pct}%
        </span>
      </div>

      {/* Progress bar track */}
      <div className="relative h-1 w-full overflow-hidden rounded-full bg-white/[0.06]">
        {/* Fill */}
        <div
          data-zone={zone}
          className={`absolute inset-y-0 left-0 rounded-full transition-all duration-500 ${ZONE_FILL[zone]}`}
          style={{ width: `${pct}%` }}
        />
        {/* Threshold marker */}
        <div
          className="absolute inset-y-0 w-px bg-white/20"
          style={{ left: `${thresholdPct}%` }}
          title={`Compaction triggers at ${thresholdTokens.toLocaleString()} tokens`}
        />
      </div>

      {/* Strategy label */}
      <span className="text-[8px] text-zinc-600">{strategy}</span>
    </div>
  );
}
