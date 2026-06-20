import React from 'react';

interface PopoverProps {
  open: boolean;
  children: React.ReactNode;
  align?: 'left' | 'right';
}

export function Popover({ open, children, align = 'left' }: PopoverProps) {
  if (!open) return null;
  return (
    <div
      className={`absolute ${align === 'right' ? 'right-0' : 'left-0'} bottom-9 z-50 min-w-[240px] rounded-lg border border-white/10 bg-zinc-950/95 p-1 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]`}
    >
      {children}
    </div>
  );
}
