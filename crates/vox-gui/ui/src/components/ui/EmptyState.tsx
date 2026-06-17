import React from 'react';
import { Button } from './Button';
import { Icon } from './Icons';

export interface EmptyStateProps {
  variant?: 'no-data' | 'no-permission' | 'no-connection' | 'error' | 'welcome';
  icon?: React.ReactNode;
  title: string;
  description?: string;
  primaryAction?: { label: string; onClick: () => void };
  secondaryAction?: { label: string; onClick: () => void };
  action?: { label: string; onClick: () => void };
  children?: React.ReactNode;
}

const DEFAULT_ICONS = {
  'no-data': <Icon.alert className="size-8 text-zinc-500" />,
  'no-permission': <Icon.x className="size-8 text-red-400" />,
  'no-connection': <Icon.bolt className="size-8 text-amber-400" />,
  'error': <Icon.alert className="size-8 text-red-500" />,
  'welcome': <Icon.check className="size-8 text-brass animate-pulse" />,
};

export function EmptyState({ 
  variant = 'no-data', 
  icon, 
  title, 
  description, 
  primaryAction, 
  secondaryAction,
  action,
  children
}: EmptyStateProps) {
  const actualPrimary = primaryAction || action;

  return (
    <div
      className="flex flex-col items-center justify-center gap-3 py-16 px-4 text-center max-w-lg mx-auto"
      role="status"
      aria-live="polite"
    >
      <div className="flex justify-center mb-1">
        {icon || DEFAULT_ICONS[variant]}
      </div>
      <h3 className="font-display text-sm tracking-widest uppercase text-zinc-200">{title}</h3>
      {description && <p className="text-xs text-zinc-500 leading-relaxed max-w-sm">{description}</p>}
      
      {children}

      {(actualPrimary || secondaryAction) && (
        <div className="flex items-center justify-center gap-3 mt-3">
          {secondaryAction && (
            <Button variant="ghost" size="sm" onClick={secondaryAction.onClick}>
              {secondaryAction.label}
            </Button>
          )}
          {actualPrimary && (
            <Button variant="primary" size="sm" onClick={actualPrimary.onClick}>
              {actualPrimary.label}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}

