import React from 'react';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** `primary` uses the imperial-gold accent; `default` is the quiet surface button. */
  variant?: 'default' | 'primary';
}

/**
 * Limes button. Cinzel caps with engraved letter-spacing. The primary variant
 * carries the gold accent at low opacity for a struck-metal feel.
 */
export function Button({ variant = 'default', className, children, ...rest }: ButtonProps) {
  const cls = ['ds-btn', variant === 'primary' ? 'ds-btn-primary' : '', className]
    .filter(Boolean)
    .join(' ');
  return (
    <button type="button" className={cls} {...rest}>
      {children}
    </button>
  );
}
