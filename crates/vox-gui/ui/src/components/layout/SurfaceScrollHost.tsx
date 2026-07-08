import React from 'react';

export function SurfaceScrollHost({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden" data-testid="surface-scroll-host">
      <div
        className="h-full min-h-0 overflow-auto custom-scrollbar"
        data-testid="surface-scroll-viewport"
      >
        {children}
      </div>
    </div>
  );
}
