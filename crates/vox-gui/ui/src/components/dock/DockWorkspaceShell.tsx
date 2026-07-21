// crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx
import React, { useCallback, useEffect, useRef, useState } from 'react';
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
  // Two-div split, deliberate: `outerRef` keeps the existing percentage-based
  // sizing (`h-full`, flex-stretched by its own parent) untouched — measuring
  // it is safe and side-effect-free. `pixelHeight` is applied to the INNER
  // div that actually wraps DockviewReact, converting that one box from a
  // percentage height to a concrete pixel value.
  //
  // Why this is required, not cosmetic: dockview-react's own internal root
  // node renders with `style="height: 100%"`. CSS percentage heights only
  // resolve against an ancestor with a *definite* height — and empirically
  // (confirmed live via CDP DOM inspection, not jsdom, which cannot detect
  // this class of bug at all) a flex-item whose own height comes from
  // `h-full`/flex-stretch does NOT count as definite far enough down this
  // specific chain: dockview's internal `.dv-shell` computed 0x0 even though
  // our own wrapper measured a real, correct, non-zero size one level up.
  // `DockviewApi.layout(w, h)` does NOT fix this — it only feeds dockview's
  // internal pane-arithmetic, not the DOM height cascade, so `.dv-shell`
  // stayed 0x0 even after calling it. Giving the DockviewReact wrapper an
  // explicit `px` height breaks the percentage chain at a definite value,
  // which is the only reliable fix for this specific CSS interaction.
  const outerRef = useRef<HTMLDivElement | null>(null);
  const [pixelHeight, setPixelHeight] = useState<number | null>(null);

  useEffect(() => {
    const el = outerRef.current;
    if (!el || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(entries => {
      const h = entries[0]?.contentRect.height;
      if (h && h > 0) setPixelHeight(h);
    });
    ro.observe(el);
    const rect = el.getBoundingClientRect();
    if (rect.height > 0) setPixelHeight(rect.height);
    return () => ro.disconnect();
  }, []);

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
    <div ref={outerRef} className="h-full min-h-0 w-full">
      <div
        className="dockview-theme-vox w-full"
        style={{ height: pixelHeight != null ? `${pixelHeight}px` : '60vh' }}
      >
        {/*
          Tab drag-and-drop-to-reorder was reported broken in the live app
          (Tauri/WebView2 on Windows) even though it works under jsdom-free
          manual testing assumptions and has no app-level DnD-blocking code.
          dockview-core's default `dndStrategy` ('auto') drives mouse drags
          through native HTML5 drag-and-drop, which is exactly the class of
          browser feature WebView2 has historically been unreliable with
          (dockview-core's own `dndStrategy` docs call out "embedded
          webviews" as an environment where HTML5 DnD is unreliable).
          Forcing the `'pointer'` strategy makes every drag — including
          mouse — go through dockview's pointer-event backend instead of
          native HTML5 DnD, sidestepping the WebView2-specific limitation
          entirely. Trade-off: cross-window HTML5 drag and the native drag
          ghost image are unavailable in this mode, but Axis never relies on
          either (single dockview root, no popout-drag flows).
        */}
        <DockviewReact
          components={components}
          tabComponents={tabComponents}
          onReady={handleReady}
          dndStrategy="pointer"
        />
      </div>
    </div>
  );
}
