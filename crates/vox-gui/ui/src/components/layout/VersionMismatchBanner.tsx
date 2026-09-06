import React, { useState } from 'react';

export interface VersionMismatchBannerProps {
  mismatch: { daemon: string; gui: string } | null;
}

export function VersionMismatchBanner({ mismatch }: VersionMismatchBannerProps) {
  const [dismissedFor, setDismissedFor] = useState<string | null>(null);
  const key = mismatch ? `${mismatch.daemon}|${mismatch.gui}` : null;
  if (!mismatch || dismissedFor === key) return null;
  return (
    <div
      data-testid="version-mismatch-banner"
      className="flex items-center justify-between gap-3 border-b border-amber-400/30 bg-amber-400/6 px-4 py-1.5 text-[11px] text-amber-200"
    >
      <span role="alert">
        GUI v{mismatch.gui} / daemon v{mismatch.daemon} — restart the daemon to update.
      </span>
      <button
        type="button"
        aria-label="Dismiss version mismatch warning"
        onClick={() => setDismissedFor(key)}
        className="shrink-0 rounded-sm px-1.5 py-0.5 text-amber-200/70 hover:bg-amber-400/10 hover:text-amber-200"
      >
        ✕
      </button>
    </div>
  );
}
