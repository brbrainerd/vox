import React, { useState } from 'react';
import { CLUTCH_DETENTS, RISK_POSTURES, type ControlState } from '../../../lib/driveConsole';
import { RiskPopover } from './RiskPopover';

const TONE_BG: Record<string, string> = {
  rose: 'bg-rose-400',
  amber: 'bg-amber-400',
  emerald: 'bg-emerald-400',
};

interface DriveConsoleProps {
  control: ControlState;
  onControlChange: (next: Partial<ControlState>) => void;
  spentUsd: number;
  budgetUsd: number;
  burnPerMin?: number;
  model: string;
  auto: boolean;
}

export function DriveConsole({
  control,
  onControlChange,
  spentUsd,
  budgetUsd,
  burnPerMin,
  model,
  auto,
}: DriveConsoleProps) {
  const [riskOpen, setRiskOpen] = useState(false);
  const risk = RISK_POSTURES.find(r => r.id === control.risk)!;
  const pct = budgetUsd > 0 ? Math.min(100, (spentUsd / budgetUsd) * 100) : 0;

  return (
    <div className="relative flex items-stretch overflow-hidden rounded-lg border border-white/10 text-[11px]">
      {/* ① Clutch */}
      <div className="flex items-center gap-1 border-r border-white/[0.07] px-2.5 py-1.5">
        <span className="text-zinc-500" aria-hidden>⚙</span>
        <div role="radiogroup" aria-label="Clutch — how much to spend" className="flex gap-0.5">
          {CLUTCH_DETENTS.map(d => (
            <button
              key={d.id}
              type="button"
              role="radio"
              title={d.hint}
              aria-checked={control.clutch === d.id}
              onClick={() => onControlChange({ clutch: d.id })}
              className={`min-h-[24px] rounded px-1.5 font-medium ${
                control.clutch === d.id
                  ? 'bg-brass/[0.16] text-brass'
                  : 'text-zinc-400 hover:text-zinc-200'
              }`}
            >
              {d.label}
            </button>
          ))}
        </div>
      </div>

      {/* ② Cost */}
      <div
        className="flex items-center gap-2 border-r border-white/[0.07] px-2.5 py-1.5"
        title="Live spend"
      >
        <span className="font-mono text-brass">${spentUsd.toFixed(2)}</span>
        <span className="font-mono text-zinc-500">/{budgetUsd.toFixed(2)}</span>
        <span className="relative h-[3px] w-12 rounded bg-white/[0.08]">
          <span
            className="absolute inset-y-0 left-0 rounded bg-gradient-to-r from-emerald-400 to-brass"
            style={{ width: `${pct}%` }}
          />
        </span>
        {burnPerMin != null && (
          <span className="text-zinc-500">↑${burnPerMin.toFixed(2)}/m</span>
        )}
      </div>

      {/* ③ Risk */}
      <button
        type="button"
        aria-label={`Risk: ${risk.label} — click to configure`}
        aria-expanded={riskOpen}
        onClick={() => setRiskOpen(o => !o)}
        className="flex items-center gap-1.5 border-r border-white/[0.07] px-2.5 py-1.5 hover:bg-white/[0.03]"
      >
        <span className={`h-3.5 w-[3px] rounded ${TONE_BG[risk.tone]}`} aria-hidden />
        <span>{risk.label}</span>
        <span className="text-zinc-600">▾</span>
      </button>

      {/* ④ Model read-out */}
      <div
        className="flex items-center gap-1 px-2.5 py-1.5"
        title="Active model (Auto shows live pick)"
      >
        {auto && <span className="text-zinc-500">Auto·</span>}
        <span className="text-brass">{model}</span>
        <span className="text-zinc-600" aria-hidden>ⓘ</span>
      </div>

      <RiskPopover
        open={riskOpen}
        risk={control.risk}
        onChange={(n) => { onControlChange(n); setRiskOpen(false); }}
        onClose={() => setRiskOpen(false)}
      />
    </div>
  );
}
