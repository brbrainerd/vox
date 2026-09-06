import React from 'react';
import { cn } from '../../lib/cn';

const SIZE_PADDING = {
  sm: 'p-3 rounded-xl',
  md: 'p-5 rounded-2xl',
  lg: 'p-6 rounded-3xl',
};

export interface GlassProps extends React.HTMLAttributes<HTMLDivElement> {
  size?: keyof typeof SIZE_PADDING;
  inset?: boolean;
  interactive?: boolean;
  as?: React.ElementType;
}

export function Glass({ 
  className, 
  size = 'md',
  inset = true, 
  interactive = false,
  as: Comp = 'div',
  children, 
  ...rest 
}: GlassProps) {
  const isButton = Comp === 'button';
  const buttonProps = isButton ? { type: 'button' as const } : {};

  return (
    <Comp
      {...buttonProps}
      {...rest}
      className={cn(
        "relative border border-border-subtle bg-overlay-solid backdrop-blur-2xl shadow-[0_1px_0_rgba(255,255,255,0.04)_inset,0_24px_60px_-30px_rgba(0,0,0,0.9)]",
        isButton && "text-left bg-transparent border-0 p-0 outline-hidden w-full",
        SIZE_PADDING[size],
        interactive && "hover:border-border-subtle hover:bg-bg-elevated cursor-pointer transition-all duration-150 active:scale-[0.99] focus-visible:outline-hidden focus-visible:ring-1 focus-visible:ring-brass/40",
        className
      )}
    >
      {inset && (
        <div className="pointer-events-none absolute inset-0 rounded-[inherit] ring-1 ring-inset ring-overlay-subtle" />
      )}
      {children}
    </Comp>
  );
}

