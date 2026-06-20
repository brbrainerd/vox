import React, { useCallback, useEffect, useImperativeHandle, useRef, forwardRef } from 'react';
import {
  DockviewReact,
  DockviewReadyEvent,
  IDockviewPanelProps,
  themeDark,
} from 'dockview';
import 'dockview/dist/styles/dockview.css';
import '../../styles/dockview-vox.css';
import { voxTransport } from '../../transport';
import { LAYOUT_PERSIST_DEBOUNCE_MS } from '../../config/constants';
import { SHELL_PREFERENCE_KEYS } from '../../lib/shellPersistence';
import { renderPanel, panelTitle, isDockable, resolvePanelView } from '../../lib/panelRegistry';
import type { SurfaceProps } from './surfaceComponents';

/**
 * Panel id is keyed by the RESOLVED child view, so opening a parent (`agents`)
 * and its default child (`dashboard`) target the same panel instead of two.
 */
export function panelIdForView(viewKey: string): string {
  return `surface:${resolvePanelView(viewKey)}`;
}

export type OpenPlan =
  | { action: 'focus'; id: string }
  | { action: 'add'; id: string; viewKey: string };

/** Pure decision: focus an open panel or add a new one. */
export function planOpen(viewKey: string, openIds: Set<string>): OpenPlan {
  const id = panelIdForView(viewKey);
  return openIds.has(id) ? { action: 'focus', id } : { action: 'add', id, viewKey };
}

export interface DockWorkspaceHandle {
  openPanel: (viewKey: string) => void;
  resetLayout: () => void;
}

interface DockWorkspaceProps {
  /** The currently-selected nav view; seeded as the first panel. */
  activeView: string;
  /** Shared surface props passed to every panel body (same object AppShell builds). */
  surfaceProps: SurfaceProps;
  layoutKey?: string;
}

type Api = DockviewReadyEvent['api'];

function PanelHost({ params }: IDockviewPanelProps<{ viewKey: string; surfaceProps: SurfaceProps }>) {
  const { viewKey, surfaceProps } = params;
  if (!isDockable(viewKey)) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-sm text-zinc-500">
        Unknown panel “{viewKey}” — close it from the tab.
      </div>
    );
  }
  return <div className="h-full min-h-0 overflow-auto custom-scrollbar p-1">{renderPanel(viewKey, surfaceProps)}</div>;
}

const components = { panel: PanelHost };

export const DockWorkspace = forwardRef<DockWorkspaceHandle, DockWorkspaceProps>(function DockWorkspace(
  { activeView, surfaceProps, layoutKey = SHELL_PREFERENCE_KEYS.dockLayout },
  ref,
) {
  const apiRef = useRef<Api | null>(null);
  const persistTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Keep the freshest surfaceProps so live panels re-render with new data.
  const propsRef = useRef(surfaceProps);
  propsRef.current = surfaceProps;

  const persist = useCallback((api: Api) => {
    if (persistTimer.current) clearTimeout(persistTimer.current);
    persistTimer.current = setTimeout(() => {
      try {
        voxTransport.setGuiPreference(layoutKey, JSON.stringify(api.toJSON())).catch(() => {});
      } catch { /* ignore serialization errors */ }
    }, LAYOUT_PERSIST_DEBOUNCE_MS);
  }, [layoutKey]);

  const addSurfacePanel = useCallback((api: Api, viewKey: string, setActive: boolean) => {
    api.addPanel({
      id: panelIdForView(viewKey),
      component: 'panel',
      title: panelTitle(viewKey),
      params: { viewKey, surfaceProps: propsRef.current },
      inactive: !setActive,
    });
  }, []);

  const openPanel = useCallback((viewKey: string) => {
    const api = apiRef.current;
    if (!api) return;
    const openIds = new Set(api.panels.map(p => p.id));
    const plan = planOpen(viewKey, openIds);
    if (plan.action === 'focus') {
      api.getPanel(plan.id)?.api.setActive();
    } else {
      addSurfacePanel(api, plan.viewKey, true);
    }
  }, [addSurfacePanel]);

  const resetLayout = useCallback(() => {
    const api = apiRef.current;
    if (!api) return;
    api.clear();
    addSurfacePanel(api, activeView, true);
  }, [activeView, addSurfacePanel]);

  useImperativeHandle(ref, () => ({ openPanel, resetLayout }), [openPanel, resetLayout]);

  const onReady = useCallback((event: DockviewReadyEvent) => {
    apiRef.current = event.api;
    voxTransport.getGuiPreference(layoutKey)
      .then(raw => {
        let restored = false;
        if (raw) {
          try { event.api.fromJSON(JSON.parse(raw)); restored = true; } catch { /* fall through */ }
        }
        if (!restored || event.api.panels.length === 0) {
          addSurfacePanel(event.api, activeView, true);
        }
      })
      .catch(() => addSurfacePanel(event.api, activeView, true));
    event.api.onDidLayoutChange(() => persist(event.api));
  // activeView intentionally read once at mount; live switches go through the effect below.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [layoutKey, addSurfacePanel, persist]);

  // When the nav selection changes, open/focus that surface as a panel.
  useEffect(() => {
    if (apiRef.current) openPanel(activeView);
  }, [activeView, openPanel]);

  // Push fresh surfaceProps into every live panel so data stays current.
  useEffect(() => {
    const api = apiRef.current;
    if (!api) return;
    for (const p of api.panels) {
      const vk = (p.params as { viewKey?: string } | undefined)?.viewKey;
      if (vk) p.api.updateParameters({ viewKey: vk, surfaceProps });
    }
  }, [surfaceProps]);

  return (
    <DockviewReact
      className="dockview-theme-vox h-full min-h-0"
      onReady={onReady}
      components={components}
      theme={themeDark}
    />
  );
});
