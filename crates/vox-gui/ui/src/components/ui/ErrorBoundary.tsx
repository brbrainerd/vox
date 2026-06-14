import React from 'react';

interface Props {
  /** Human-readable name of the surface being rendered, shown in the fallback. */
  surface?: string;
  children: React.ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Isolates a single surface render.
 *
 * Without this, an uncaught error in any surface unmounts the entire React tree and blanks
 * the operator console (sidebar and all). The boundary keeps the app shell alive, shows an
 * actionable fallback, and exposes a `data-surface-error` hook that the visual-audit sweep
 * asserts against — so a crashed surface fails the audit instead of silently going black.
 *
 * Wrap with `key={activeView}` so navigating to a different surface resets the boundary.
 */
export class SurfaceErrorBoundary extends React.Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    // Surface to the console so the visual-audit sweep records it as a real failure.
    console.error(`[surface:${this.props.surface ?? 'unknown'}]`, error, info.componentStack);
  }

  render() {
    const { error } = this.state;
    if (error) {
      return (
        <div
          data-surface-error
          role="alert"
          className="mx-auto mt-16 max-w-md rounded-xl border border-rose-400/20 bg-rose-950/20 p-5 text-center"
        >
          <div className="font-display text-sm uppercase tracking-wider text-rose-300">
            {this.props.surface ?? 'Surface'} failed to render
          </div>
          <div className="mt-2 font-mono text-[11px] text-zinc-400 break-words">{error.message}</div>
          <button
            type="button"
            aria-label="Retry loading surface"
            onClick={() => this.setState({ error: null })}
            className="mt-4 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs text-zinc-300 hover:bg-white/[0.06]"
          >
            Retry
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
