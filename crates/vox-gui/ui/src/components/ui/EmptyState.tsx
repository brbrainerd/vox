import React from 'react';

interface EmptyStateProps {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  action?: { label: string; onClick: () => void };
}

export function EmptyState({ icon, title, description, action }: EmptyStateProps) {
  return (
    <div
      className="flex flex-col items-center justify-center gap-3 py-16 text-center"
      role="status"
      aria-live="polite"
    >
      {icon && <div className="text-zinc-500">{icon}</div>}
      <p className="font-display text-sm tracking-wide text-zinc-300">{title}</p>
      {description && <p className="max-w-md text-sm text-zinc-500">{description}</p>}
      {action && (
        <button
          type="button"
          onClick={action.onClick}
          className="rounded-lg border border-brass/30 bg-brass/10 px-3 py-1.5 text-xs text-brass hover:bg-brass/20"
        >
          {action.label}
        </button>
      )}
    </div>
  );
}
