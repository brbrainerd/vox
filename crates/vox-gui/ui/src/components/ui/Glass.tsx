import React from 'react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface GlassProps extends React.HTMLAttributes<HTMLDivElement> {
  inset?: boolean;
}

export function Glass({ className, inset = true, children, ...rest }: GlassProps) {
  return (
    <div
      {...rest}
      className={cn(
        "relative rounded-2xl border border-white/[0.06] bg-white/[0.025] ",
        "backdrop-blur-2xl shadow-[0_1px_0_rgba(255,255,255,0.04)_inset,0_24px_60px_-30px_rgba(0,0,0,0.9)] ",
        className
      )}
    >
      {inset && (
        <div className="pointer-events-none absolute inset-0 rounded-2xl ring-1 ring-inset ring-white/[0.04]" />
      )}
      {children}
    </div>
  );
}
