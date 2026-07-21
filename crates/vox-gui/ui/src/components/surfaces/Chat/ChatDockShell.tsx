// crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx
import React, { useCallback, useRef } from 'react';
import { DockviewReact, type DockviewReadyEvent, type IDockviewPanelProps } from 'dockview';
import { LAYOUT_PERSIST_DEBOUNCE_MS } from '../../../config/constants';

/** localStorage key for the persisted chat dockview layout. Exported so
 * other modules (e.g. a future "reset layout" action) can reuse it without
 * duplicating the string literal. */
// v3: bumped again — v2 (and v1 before it) persisted each panel's `params`
// verbatim as part of dockview's toJSON() grid tree, which for every panel
// in this app is `{ node: <live React element> }`. JSON.stringify silently
// drops a React element's `type` (a function) and `$$typeof` (a Symbol),
// leaving a garbled `{key, ref, props}` plain object. Restoring that via
// fromJSON on the next launch fed the garbage straight into a panel's
// first render (`{props.params.node}`), before the refresh effect ever got
// a chance to overwrite it with a real node — crashing with React error #31
// on every subsequent launch, since the corrupted snapshot just re-persisted
// itself. Panel params (any live React content) must never be persisted;
// only geometry should be. See the params-stripping replacer below.
export const LAYOUT_STORAGE_KEY = 'gui.chat.dockview_layout.v3';

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
            // Strip every `params` field (each panel's live React node, per
            // GroupviewPanelState) at any depth before persisting — only
            // geometry should survive the round trip. See the
            // LAYOUT_STORAGE_KEY comment above for why this is required,
            // not optional.
            const serialized = JSON.stringify(event.api.toJSON(), (key, value) =>
              key === 'params' ? undefined : value,
            );
            window.localStorage.setItem(LAYOUT_STORAGE_KEY, serialized);
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
