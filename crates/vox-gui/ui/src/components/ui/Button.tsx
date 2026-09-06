// src/components/ui/Button.tsx
import React from 'react';
import { Slot } from '@radix-ui/react-slot';
import { cn } from '../../lib/cn';

const VARIANT_CLASS = {
  primary: 'bg-brass text-bg-base hover:bg-brass-light active:bg-brass-dark disabled:opacity-50',
  secondary: 'bg-overlay-subtle text-text-primary hover:bg-overlay-subtle active:bg-overlay-subtle',
  ghost: 'bg-transparent text-text-muted hover:text-text-primary hover:bg-overlay-subtle',
  outline: 'bg-transparent border border-border-subtle text-text-secondary hover:bg-overlay-subtle',
  danger: 'bg-red-500 text-white hover:bg-red-600 active:bg-red-700',
};

const SIZE_CLASS = {
  xs: 'px-2 py-0.5 text-[10px] h-6 rounded-sm',
  sm: 'px-2.5 py-1 text-[11px] h-7 rounded-md',
  md: 'px-3.5 py-1.5 text-[13px] h-9 rounded-lg',
  lg: 'px-4.5 py-2 text-[15px] h-11 rounded-xl',
  icon: 'size-8 p-0 flex items-center justify-center rounded-lg',
};

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: keyof typeof VARIANT_CLASS;
  size?: keyof typeof SIZE_CLASS;
  loading?: boolean;
  icon?: React.ReactNode;
  trailingIcon?: React.ReactNode;
  asChild?: boolean;
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ 
    variant = 'secondary', 
    size = 'md', 
    loading = false, 
    icon, 
    trailingIcon, 
    asChild = false, 
    className, 
    type = 'button', 
    children, 
    disabled,
    ...props 
  }, ref) => {
    const Comp = asChild ? Slot : 'button';
    
    if (asChild) {
      return (
        <Slot
          ref={ref}
          className={cn(
            "inline-flex items-center justify-center font-medium tracking-wide transition-all focus-visible:outline-solid focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brass",
            VARIANT_CLASS[variant],
            SIZE_CLASS[size],
            className
          )}
          {...props}
        >
          {children}
        </Slot>
      );
    }

    return (
      <button
        ref={ref}
        type={type}
        disabled={loading || disabled}
        className={cn(
          "inline-flex items-center justify-center font-medium tracking-wide transition-all focus-visible:outline-solid focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brass",
          VARIANT_CLASS[variant],
          SIZE_CLASS[size],
          className
        )}
        {...props}
      >
        {loading ? (
          <svg className="animate-spin -ml-1 mr-2 h-3.5 w-3.5 text-current" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
        ) : icon ? (
          <span className="mr-1.5 flex items-center">{icon}</span>
        ) : null}
        {children}
        {!loading && trailingIcon && <span className="ml-1.5 flex items-center">{trailingIcon}</span>}
      </button>
    );
  }
);

Button.displayName = 'Button';
