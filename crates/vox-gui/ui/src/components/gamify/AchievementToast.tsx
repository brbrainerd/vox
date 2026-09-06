import React, { useEffect } from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';

export interface AchievementToastProps {
  title: string;
  body?: string;
  autoDismissMs?: number;
  onDismiss?: () => void;
}

export function AchievementToast({ title, body, autoDismissMs, onDismiss }: AchievementToastProps) {
  useEffect(() => {
    if (autoDismissMs == null || onDismiss == null) return;
    const id = window.setTimeout(onDismiss, autoDismissMs);
    return () => window.clearTimeout(id);
  }, [autoDismissMs, onDismiss]);

  return (
    <Glass className="pointer-events-auto flex items-start gap-2 p-3 shadow-lg">
      <div className="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-sm bg-brass/15 text-brass">
        <Icon.spark className="size-3.5" aria-hidden="true" />
      </div>
      <div className="min-w-0 flex-1 leading-tight">
        <div className="font-display text-[12px] tracking-wide text-text-primary">{title}</div>
        {body && <div className="mt-0.5 text-[11px] text-text-muted">{body}</div>}
      </div>
    </Glass>
  );
}
