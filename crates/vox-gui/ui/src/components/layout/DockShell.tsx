import React, { useCallback, useEffect, useRef } from 'react';
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

interface DockShellProps {
  panelId: string;
  panelTitle: string;
  children: React.ReactNode;
  layoutKey?: string;
}

export type DockShellKeyAction = 'split-horizontal' | 'close-panel';

type DockShellApi = DockviewReadyEvent['api'];

export interface DockShellKeyboardContext {
  api: DockShellApi | null;
  container: HTMLElement | null;
  panelId: string;
  panelTitle: string;
  content: React.ReactNode;
}

function hasModKey(event: Pick<KeyboardEvent, 'metaKey' | 'ctrlKey'>) {
  return event.metaKey || event.ctrlKey;
}

export function dockShellKeybindingForEvent(
  event: Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey'>,
): DockShellKeyAction | null {
  if (!hasModKey(event)) return null;
  if (event.key === '\\') return 'split-horizontal';
  if (event.key === 'w' || event.key === 'W') return 'close-panel';
  return null;
}

export function isDockShellFocused(container: HTMLElement | null): boolean {
  if (!container) return false;
  const active = document.activeElement;
  if (!active) return false;
  return container.contains(active);
}

function nextSplitPanelId(api: DockShellApi, baseId: string): string {
  let index = 1;
  while (api.getPanel(`${baseId}-split-${index}`)) {
    index += 1;
  }
  return `${baseId}-split-${index}`;
}

function splitActivePanelHorizontal(ctx: DockShellKeyboardContext): void {
  const { api, panelId, panelTitle, content } = ctx;
  if (!api) return;

  const reference = api.activePanel ?? api.getPanel(panelId);
  if (!reference) return;

  const newId = nextSplitPanelId(api, reference.id);
  api.addPanel({
    id: newId,
    component: 'panel',
    title: `${panelTitle} (split)`,
    params: { content },
    position: {
      referencePanel: reference,
      direction: 'right',
    },
  });
}

function closeActivePanelIfAllowed(api: DockShellApi): void {
  if (api.panels.length <= 1) return;
  api.activePanel?.api.close();
}

export function handleDockShellKeydown(
  event: KeyboardEvent,
  ctx: DockShellKeyboardContext,
): boolean {
  if (!isDockShellFocused(ctx.container) || !ctx.api) return false;

  const action = dockShellKeybindingForEvent(event);
  if (!action) return false;

  if (action === 'split-horizontal') {
    splitActivePanelHorizontal(ctx);
    return true;
  }

  closeActivePanelIfAllowed(ctx.api);
  return true;
}

function PanelHost({ params }: IDockviewPanelProps<{ content: React.ReactNode }>) {
  return <div className="h-full min-h-0 overflow-auto custom-scrollbar p-1">{params.content}</div>;
}

const components = { panel: PanelHost };

export function DockShell({
  panelId,
  panelTitle,
  children,
  layoutKey = 'gui.layout.v1',
}: DockShellProps) {
  const apiRef = useRef<DockShellApi | null>(null);
  const shellRef = useRef<HTMLDivElement | null>(null);
  const persistTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const persistLayout = useCallback(
    (api: DockShellApi) => {
      if (persistTimer.current) clearTimeout(persistTimer.current);
      persistTimer.current = setTimeout(() => {
        try {
          const json = JSON.stringify(api.toJSON());
          voxTransport.setGuiPreference(layoutKey, json).catch(() => {});
        } catch {
          // ignore serialization errors
        }
      }, LAYOUT_PERSIST_DEBOUNCE_MS);
    },
    [layoutKey],
  );

  const onReady = useCallback(
    (event: DockviewReadyEvent) => {
      apiRef.current = event.api;
      voxTransport.getGuiPreference(layoutKey)
        .then(raw => {
          if (raw) {
            try {
              event.api.fromJSON(JSON.parse(raw));
              return;
            } catch {
              // fall through to default
            }
          }
          event.api.addPanel({
            id: panelId,
            component: 'panel',
            title: panelTitle,
            params: { content: children },
          });
        })
        .catch(() => {
          event.api.addPanel({
            id: panelId,
            component: 'panel',
            title: panelTitle,
            params: { content: children },
          });
        });

      event.api.onDidLayoutChange(() => persistLayout(event.api));
    },
    [children, panelId, panelTitle, layoutKey, persistLayout],
  );

  useEffect(() => {
    const api = apiRef.current;
    if (!api) return;
    const panel = api.getPanel(panelId);
    if (panel) {
      panel.api.updateParameters({ content: children });
    }
  }, [children, panelId]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const handled = handleDockShellKeydown(event, {
        api: apiRef.current,
        container: shellRef.current,
        panelId,
        panelTitle,
        content: children,
      });
      if (handled) {
        event.preventDefault();
        event.stopPropagation();
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [children, panelId, panelTitle]);

  return (
    <div ref={shellRef} className="h-full min-h-0" tabIndex={-1}>
      <DockviewReact
        className="dockview-theme-vox h-full min-h-0"
        onReady={onReady}
        components={components}
        theme={themeDark}
      />
    </div>
  );
}
