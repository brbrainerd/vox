// crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx
import React, { useCallback, useRef } from 'react';
import { DockviewReact, type DockviewReadyEvent, type IDockviewPanelProps } from 'dockview';
import { LAYOUT_PERSIST_DEBOUNCE_MS } from '../../../config/constants';

/** localStorage key for the persisted chat dockview layout. Exported so
 * other modules (e.g. a future "reset layout" action) can reuse it without
 * duplicating the string literal. */
// v2: bumped to invalidate any layout snapshot persisted by v1, which had
// no guard against saving degenerate geometry captured before the webview's
// first real paint (e.g. a near-zero-width container) — such a snapshot
// would otherwise replay forever via fromJSON on every future launch.
export const LAYOUT_STORAGE_KEY = 'gui.chat.dockview_layout.v2';

interface ChatDockShellProps {
  components: Record<string, React.FunctionComponent<IDockviewPanelProps>>;
  onReady: (event: DockviewReadyEvent) => void;
}

/**
 * The dockview shell for the chat workspace: sessions list, execution rail,
 * Flow, and plan panels all dock/resize/hide within this container around
 * the central chat pane. Theming via the `dockview-theme-vox` class
 * (crates/vox-gui/ui/src/styles/dockview-vox.css), not the `theme` prop.
 *
 * Layout persistence: the dockview grid layout is serialized to
 * localStorage (debounced) on every change, and restored on mount before
 * the caller's `onReady` runs. Callers must guard their `addPanel` calls
 * with `if (!event.api.getPanel(id))` so a restored layout doesn't get
 * duplicate panels re-added.
 */
export function ChatDockShell({ components, onReady }: ChatDockShellProps) {
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleReady = useCallback(
    (event: DockviewReadyEvent) => {
      const saved = window.localStorage.getItem(LAYOUT_STORAGE_KEY);
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
            window.localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(event.api.toJSON()));
          } catch (err) {
            console.warn('failed to persist dockview layout', err);
          }
        }, LAYOUT_PERSIST_DEBOUNCE_MS);
      });

      // The caller's onReady is always invoked, even when a saved layout
      // was restored — its addPanel calls must guard with
      // `if (!event.api.getPanel(id))` so restored panels aren't duplicated.
      onReady(event);
    },
    [onReady],
  );

  return (
    <div className="dockview-theme-vox h-full min-h-[60vh] w-full">
      <DockviewReact components={components} onReady={handleReady} />
    </div>
  );
}
