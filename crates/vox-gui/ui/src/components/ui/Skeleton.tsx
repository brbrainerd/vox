// src/components/ui/Skeleton.tsx
import React from 'react';
import { cn } from '../../lib/cn';

export interface SkeletonProps {
  /** Extra Tailwind classes. Use to set w-*, h-*, rounded-*, etc. */
  className?: string;
  /**
   * Explicit height as a number (px) or CSS string (e.g. "2rem").
   * Alternative to setting height via `className`.
   */
  height?: number | string;
  /**
   * Explicit width as a number (px) or CSS string (e.g. "100%").
   * Alternative to setting width via `className`.
   */
  width?: number | string;
}

/**
 * Shimmer placeholder for content that is still loading.
 *
 * - `aria-hidden="true"` — carries no information; invisible to assistive tech.
 * - `data-slot="skeleton"` — stable selector for tests and visual-audit sweeps.
 * - Animation is a CSS `background-position` shift (gradient shimmer).
 *   The global `prefers-reduced-motion` block in `index.css` zeroes
 *   `animation-duration` for users with motion sensitivity — no per-component work needed.
 */
export function Skeleton({ className, height, width }: SkeletonProps) {
  const style: React.CSSProperties = {};
  if (height !== undefined) {
    style.height = typeof height === 'number' ? `${height}px` : height;
  }
  if (width !== undefined) {
    style.width = typeof width === 'number' ? `${width}px` : width;
  }

  return (
    <div
      aria-hidden="true"
      data-slot="skeleton"
      style={style}
      className={cn(
        'animate-shimmer',
        'bg-[linear-gradient(90deg,var(--color-bg-elevated)_25%,var(--color-border-strong)_50%,var(--color-bg-elevated)_75%)]',
        'bg-size-[200%_100%]',
        'rounded-md',
        className
      )}
    />
  );
}
