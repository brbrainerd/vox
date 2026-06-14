// src/components/ui/Button.tsx
import React from 'react';
import { Slot } from '@radix-ui/react-slot';
import { cn } from '../../lib/cn';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /**
   * When true, the Button renders its single child element directly, merging
   * all props into it. Use for icon-only buttons wrapping a router <Link> or
   * any case where an extra <button> DOM node is undesirable.
   */
  asChild?: boolean;
}

/**
 * Accessible button primitive.
 *
 * - Defaults `type` to "button" to prevent accidental form submission.
 * - Supports `asChild` (via Radix Slot) for polymorphic rendering.
 * - Accepts `aria-label` for icon-only buttons — pass it when visible text is absent.
 * - Applies `focus-visible:outline` via the global CSS rule in index.css; per-element
 *   overrides via the `className` prop take precedence.
 */
export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ asChild = false, className, type = 'button', children, ...props }, ref) => {
    const Comp = asChild ? Slot : 'button';
    return (
      <Comp
        ref={ref as React.Ref<HTMLButtonElement>}
        type={asChild ? undefined : type}
        className={cn(className)}
        {...props}
      >
        {children}
      </Comp>
    );
  }
);

Button.displayName = 'Button';
