import React from 'react';
import { useSubAgentStore } from './subAgentStore';
import type { ActivityEvent } from './subAgentStore';

// Stable empty array — avoids creating a new [] on every getSnapshot call,
// which would trigger React 19's useSyncExternalStore tearing detection loop.
const EMPTY_EVENTS: ActivityEvent[] = [];

export function SubAgentActivityStream({ windowId }: { windowId: string }) {
  const events = useSubAgentStore((s) => s.eventsByWindow[windowId] ?? EMPTY_EVENTS);
  return (
    <ul aria-label={`activity for ${windowId}`} aria-live="polite">
      {events.map((e) => (
        <li key={e.id}>
          {e.kind.type === 'context_pull'
            ? <span>pulled {String(e.kind.hash)} from {String(e.kind.from_window)}</span>
            : <span>{e.kind.type}</span>}
        </li>
      ))}
    </ul>
  );
}
