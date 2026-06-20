import React from 'react';

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Render engraved corner ticks (top-left gold, top-right verdigris). */
  ticks?: boolean;
}

/**
 * The Limes surface primitive (the app's `Glass`). A bordered panel over a
 * faint overlay with an inset highlight and a deep drop shadow. Optional
 * engraved corner ticks evoke a surveyor's plate.
 */
export function Card({ ticks = false, className, children, ...rest }: CardProps) {
  return (
    <div className={['ds-card', className].filter(Boolean).join(' ')} {...rest}>
      {ticks && (
        <>
          <span className="ds-tick ds-tick-tl" />
          <span className="ds-tick ds-tick-tr" />
        </>
      )}
      {children}
    </div>
  );
}
