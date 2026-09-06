// src/components/ui/Dialog.tsx
/**
 * Dialog primitive — thin wrapper over @radix-ui/react-dialog.
 *
 * Applies Glass-derived styling to DialogContent. Re-exports all sub-components
 * under semantic names so call sites only import from here.
 *
 * Built-in from Radix:
 *   - Focus trap (focus stays inside the dialog while open)
 *   - Escape-key close
 *   - scroll-lock on the body
 *   - aria-modal="true" on the dialog element
 *   - aria-labelledby wired to <DialogTitle>
 *   - aria-describedby wired to <DialogDescription>
 */
import React from 'react';
import * as RadixDialog from '@radix-ui/react-dialog';
import { cn } from '../../lib/cn';

/** Root dialog controller. Accepts `open`, `defaultOpen`, `onOpenChange`. */
export const Dialog = RadixDialog.Root;

/** Wraps a trigger element. Use `asChild` to avoid an extra DOM node. */
export const DialogTrigger = RadixDialog.Trigger;

/** Portal that mounts content outside the React tree root. */
export const DialogPortal = RadixDialog.Portal;

/** Programmatic close trigger. Use inside DialogContent to render a close button. */
export const DialogClose = RadixDialog.Close;

/** Dimmed backdrop behind the dialog panel. */
export const DialogOverlay = React.forwardRef<
  React.ElementRef<typeof RadixDialog.Overlay>,
  React.ComponentPropsWithoutRef<typeof RadixDialog.Overlay>
>(({ className, ...props }, ref) => (
  <RadixDialog.Overlay
    ref={ref}
    className={cn(
      'fixed inset-0 z-50 bg-black/60 backdrop-blur-xs',
      'data-[state=open]:animate-in data-[state=closed]:animate-out',
      'data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0',
      className
    )}
    {...props}
  />
));
DialogOverlay.displayName = 'DialogOverlay';

/**
 * The dialog panel itself.
 * Renders inside a portal. Applies Glass surface styling.
 */
export const DialogContent = React.forwardRef<
  React.ElementRef<typeof RadixDialog.Content>,
  React.ComponentPropsWithoutRef<typeof RadixDialog.Content>
>(({ className, children, ...props }, ref) => (
  <DialogPortal>
    <DialogOverlay />
    <RadixDialog.Content
      ref={ref}
      className={cn(
        'fixed left-1/2 top-1/2 z-50 -translate-x-1/2 -translate-y-1/2',
        'w-full max-w-lg',
        'rounded-xl border border-border-subtle bg-bg-base/80 backdrop-blur-xl',
        'shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]',
        'p-6',
        'data-[state=open]:animate-in data-[state=closed]:animate-out',
        'data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0',
        'data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95',
        'data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%]',
        'data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%]',
        className
      )}
      {...props}
    >
      {children}
    </RadixDialog.Content>
  </DialogPortal>
));
DialogContent.displayName = 'DialogContent';

/**
 * Dialog heading. Radix wires `aria-labelledby` on Content to this element's id.
 * Required whenever DialogContent is used.
 */
export const DialogTitle = React.forwardRef<
  React.ElementRef<typeof RadixDialog.Title>,
  React.ComponentPropsWithoutRef<typeof RadixDialog.Title>
>(({ className, ...props }, ref) => (
  <RadixDialog.Title
    ref={ref}
    className={cn(
      'font-display text-base font-semibold tracking-wide text-text-primary',
      className
    )}
    {...props}
  />
));
DialogTitle.displayName = 'DialogTitle';

/**
 * Dialog description. Radix wires `aria-describedby` on Content to this element's id.
 */
export const DialogDescription = React.forwardRef<
  React.ElementRef<typeof RadixDialog.Description>,
  React.ComponentPropsWithoutRef<typeof RadixDialog.Description>
>(({ className, ...props }, ref) => (
  <RadixDialog.Description
    ref={ref}
    className={cn('mt-1 text-[13px] text-text-muted', className)}
    {...props}
  />
));
DialogDescription.displayName = 'DialogDescription';
