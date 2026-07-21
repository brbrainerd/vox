// crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx
import React, { useCallback, useRef } from 'react';
import {
  DockviewReact,
  type DockviewReadyEvent,
  type IDockviewPanelProps,
  type IDockviewPanelHeaderProps,
} from 'dockview';
import { LAYOUT_PERSIST_DEBOUNCE_MS } from '../../config/constants';

/**
 * Per-host localStorage key for a persisted dockview layout. Each host view
 * (Chat today; any future host) gets its own independent persisted layout —
 * `storageKeyPrefix` scopes it.
 *
 * IMPORTANT for any future consumer of DockWorkspaceShell: this shell does
 * NOT itself track which panels a user has explicitly closed (see
 * ChatSurface.tsx's `closedPanelIds` ref + `onDidRemovePanel` listener — that
 * mechanism stays host-local, it was NOT folded into this shell). If your
 * host view has any auto-recreate-if-missing logic for its own panels (the
 * way ChatSurface's refresh effect does for its 5 core panels), you MUST
 * build the same closedPanelIds-style guard yourself, or you will
 * reintroduce the exact bug this whole effort started by fixing: a refresh
 * effect silently re-adding a panel the user just closed. See
 * ChatSurface.test.tsx's "does not resurrect the Flow panel on the next
 * render after the user closes it" test as the required template.
 */
export function layoutStorageKeyFor(storageKeyPrefix: string): string {
  return `${storageKeyPrefix}.dockview_layout.v3`;
}

interface DockWorkspaceShellProps {
  storageKeyPrefix: string;
  components: Record<string, React.FunctionComponent<IDockviewPanelProps>>;
  tabComponents?: Record<string, React.FunctionComponent<IDockviewPanelHeaderProps>>;
  onReady: (event: DockviewReadyEvent) => void;
}

export function DockWorkspaceShell({
  storageKeyPrefix,
  components,
  tabComponents,
  onReady,
}: DockWorkspaceShellProps) {
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const storageKey = layoutStorageKeyFor(storageKeyPrefix);

  const handleReady = useCallback(
    (event: DockviewReadyEvent) => {
      const saved = window.localStorage.getItem(storageKey);
      if (saved) {
        try {
          event.api.fromJSON(JSON.parse(saved));
        } catch (err) {
          console.warn('failed to restore dockview layout, using default', err);
        }
      }

      event.api.onDidLayoutChange(() => {
        if (debounceRef.current) clearTimeout(debounceRef.current);
        debounceRef.current = setTimeout(() => {
          try {
            const serialized = JSON.stringify(event.api.toJSON(), (key, value) =>
              key === 'params' ? undefined : value,
            );
            window.localStorage.setItem(storageKey, serialized);
          } catch (err) {
            console.warn('failed to persist dockview layout', err);
          }
        }, LAYOUT_PERSIST_DEBOUNCE_MS);
      });

      onReady(event);
    },
    [onReady, storageKey],
  );

  return (
    <div className="dockview-theme-vox h-full min-h-[60vh] w-full">
      <DockviewReact components={components} tabComponents={tabComponents} onReady={handleReady} />
    </div>
  );
}
