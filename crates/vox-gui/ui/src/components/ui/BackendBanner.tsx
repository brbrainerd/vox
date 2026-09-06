import React, { useState } from 'react';
import { backendAvailable } from '../../lib/backendGuard';

/** Normal-flow honesty banner for bare-browser mode: pushes the shell down
 * instead of overlaying it (no occlusion). Dismissible. */
export function BackendBanner() {
  const [dismissed, setDismissed] = useState(false);
  if (backendAvailable() || dismissed) return null;
  return (
    <div
      role="status"
      aria-label="Browser preview mode"
      className="flex shrink-0 items-center justify-center gap-3 border-b border-amber-500/40 bg-amber-950/90 px-4 py-1.5 text-[12px] text-amber-200"
    >
      <span>Browser preview — no desktop backend connected; surfaces show empty states.</span>
      <button
        type="button"
        aria-label="Dismiss browser preview notice"
        onClick={() => setDismissed(true)}
        className="rounded-sm px-1.5 text-amber-300 hover:bg-amber-900/60"
      >
        ×
      </button>
    </div>
  );
}
