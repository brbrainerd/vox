import React from 'react';
import { cn } from '../../lib/cn';

type Tone = 'success' | 'running' | 'warning' | 'neutral';

const toneClass: Record<Tone, string> = {
  success: 'bg-emerald-400/10 text-emerald-400 border-emerald-400/30',
  running: 'bg-sky-400/10 text-sky-400 border-sky-400/30',
  warning: 'bg-amber-400/10 text-amber-400 border-amber-400/30',
  neutral: 'bg-overlay-subtle text-text-muted border-border-subtle',
};

export function StateChip({ label, tone }: { label: string; tone: Tone }) {
  return (
    <span
      className={cn(
        'px-3 py-1 rounded-lg text-[9px] font-extrabold uppercase tracking-widest border',
        toneClass[tone],
      )}
    >
      {label}
    </span>
  );
}
