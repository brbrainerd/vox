import React from 'react';

export function AxisMark({ size = 24, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 100 100" role="img" aria-label="Vox Axis"
      className={className} style={{ color: 'rgb(var(--brass))' }}>
      <circle cx="50" cy="50" r="40" fill="none" stroke="currentColor" strokeWidth="7" />
      <path d="M50 12 V88 M12 50 H88" stroke="currentColor" strokeWidth="9" strokeLinecap="round" />
      <circle cx="50" cy="50" r="9" fill="none" stroke="currentColor" strokeWidth="7" />
      <path d="M50 6 l6 10 h-12 z M50 94 l6 -10 h-12 z M6 50 l10 6 v-12 z M94 50 l-10 6 v-12 z" fill="currentColor" />
    </svg>
  );
}
