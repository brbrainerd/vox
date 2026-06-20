import React from 'react';
import { STATUS_TONE, StatusToneKind } from '../../styles/tokens';
import { cn } from '../../lib/cn';

export interface StatusPillProps extends React.HTMLAttributes<HTMLSpanElement> {
  tone: StatusToneKind;
  label?: string;
  size?: 'xs' | 'sm';
  pulse?: boolean;
  icon?: React.ReactNode;
}

const DEFAULT_GLYPHS: Record<StatusToneKind, string> = {
  pass: '✓',
  fail: '!',
  warn: '?',
  info: 'i',
  neutral: '·',
  accent: '◆',
  Executing: '◆',
  Verifying: '◆',
  Planning: '◆',
  Paused: '·',
  Validated: '✓',
  Doubted: '?',
  Speculative: '◆',
  Active: '◆',
  Root: '◆',
};

const SIZE_CLASS = {
  xs: 'px-1.5 py-px text-[9px]',
  sm: 'px-2 py-0.5 text-[10px]',
};

export function StatusPill({ 
  tone, 
  label, 
  size = 'sm', 
  pulse = false, 
  icon,
  className,
  ...rest
}: StatusPillProps) {
  const toneStyle = STATUS_TONE[tone] || STATUS_TONE.neutral;
  const glyph = DEFAULT_GLYPHS[tone] || '·';
  
  // Auto-pulse Executing/Active/Verifying states
  const shouldPulse = pulse || tone === 'Executing' || tone === 'Active' || tone === 'Verifying';

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full font-medium tracking-wide uppercase ring-1 bg-overlay-subtle shrink-0 select-none",
        toneStyle.ring,
        toneStyle.text,
        SIZE_CLASS[size],
        className
      )}
      {...rest}
    >
      <span className={cn("relative inline-block size-1.5 rounded-full", toneStyle.dot)}>
        {shouldPulse && (
          <span className={cn("absolute inset-0 rounded-full animate-vox-ping opacity-60", toneStyle.dot)} />
        )}
      </span>
      {icon || <span className="font-mono">{glyph}</span>}
      <span>{label || tone}</span>
    </span>
  );
}
