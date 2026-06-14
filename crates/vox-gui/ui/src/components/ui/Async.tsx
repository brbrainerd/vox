import React from 'react';

type AsyncStatus = 'idle' | 'pending' | 'error' | 'success';

interface AsyncProps<T> {
  status: AsyncStatus;
  data: T | undefined;
  error: Error | null | undefined;
  isEmpty?: (data: T) => boolean;
  renderLoading?: () => React.ReactNode;
  renderEmpty?: () => React.ReactNode;
  renderError?: (err: Error) => React.ReactNode;
  children: (data: T) => React.ReactNode;
}

function defaultIsEmpty<T>(data: T): boolean {
  return Array.isArray(data) && data.length === 0;
}

function DefaultLoading() {
  return (
    <div className="flex items-center justify-center p-4 text-sm text-zinc-400" role="status" aria-live="polite">
      Loading…
    </div>
  );
}

function DefaultEmpty() {
  return (
    <div className="flex items-center justify-center p-4 text-sm text-zinc-500">
      No results
    </div>
  );
}

function DefaultError({ message }: { message: string }) {
  return (
    <div className="flex items-center justify-center p-4 text-sm text-red-400" role="alert">
      {message}
    </div>
  );
}

/**
 * Render-prop component that handles the five query states produced by
 * useVoxQuery / TanStack Query:
 *
 *   idle     → renders nothing (query is disabled or not yet started)
 *   pending  → renders a loading indicator
 *   error    → renders an error message
 *   success + empty data → renders an empty state
 *   success + data  → calls `children(data)` with the non-null result
 *
 * All fallback states are customisable via optional render-prop slots.
 */
export function Async<T>({
  status,
  data,
  error,
  isEmpty = defaultIsEmpty,
  renderLoading,
  renderEmpty,
  renderError,
  children,
}: AsyncProps<T>): React.ReactElement | null {
  if (status === 'idle') return null;

  if (status === 'pending') {
    return <>{renderLoading ? renderLoading() : <DefaultLoading />}</>;
  }

  if (status === 'error') {
    const err = error ?? new Error('Unknown error');
    return <>{renderError ? renderError(err) : <DefaultError message={err.message} />}</>;
  }

  // status === 'success'
  if (data === undefined || isEmpty(data)) {
    return <>{renderEmpty ? renderEmpty() : <DefaultEmpty />}</>;
  }

  return <>{children(data)}</>;
}
