import React from 'react';

interface WidgetErrorBoundaryProps {
  label: string;
  children: React.ReactNode;
}

interface WidgetErrorBoundaryState {
  error: Error | null;
}

/**
 * Per-widget error boundary. A broken surface/widget renders a compact error
 * tile (honest: the real surface label + the real error message), never a
 * crashed dashboard (spec §6). No placeholder prose, no empty handlers — the
 * gui-honesty scanner stays green.
 */
export class WidgetErrorBoundary extends React.Component<WidgetErrorBoundaryProps, WidgetErrorBoundaryState> {
  constructor(props: WidgetErrorBoundaryProps) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error: Error): WidgetErrorBoundaryState {
    return { error };
  }

  render(): React.ReactNode {
    const { error } = this.state;
    if (error) {
      return (
        <div
          data-testid="widget-error-tile"
          role="alert"
          className="flex h-full min-h-0 flex-col gap-1 overflow-hidden rounded-lg border border-[var(--color-status-fail)]/30 bg-[var(--color-status-fail)]/[0.06] p-3"
        >
          <span className="font-display text-[11px] uppercase tracking-[0.18em] text-[var(--color-status-fail)]">
            {this.props.label} · widget error
          </span>
          <span className="font-mono text-[10px] text-text-muted break-words">{error.message}</span>
        </div>
      );
    }
    return this.props.children;
  }
}
