import React from 'react';

export type StatusTone = 'neutral' | 'pass' | 'warn' | 'fail' | 'accent';

export interface StatusPillProps {
  tone?: StatusTone;
  label: string;
  /** When true, no leading dot is rendered. */
  hideDot?: boolean;
}

const TONE_CLASS: Record<StatusTone, string> = {
  neutral: '',
  pass: 'ds-pill-pass',
  warn: 'ds-pill-warn',
  fail: 'ds-pill-fail',
  accent: 'ds-pill-accent',
};

/**
 * Compact status chip. Tones map to the semantic status tokens
 * (verdigris pass, terracotta warn, oxblood/rose fail, verdigris accent).
 */
export function StatusPill({ tone = 'neutral', label, hideDot = false }: StatusPillProps) {
  return (
    <span className={['ds-pill', TONE_CLASS[tone]].filter(Boolean).join(' ')}>
      {!hideDot && <span className="ds-pill-dot" />}
      {label}
    </span>
  );
}
