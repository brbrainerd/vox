import React, { useCallback, useEffect, useRef } from 'react';
import {
  DockviewReact,
  DockviewReadyEvent,
  IDockviewPanelProps,
  themeDark,
} from 'dockview';
import 'dockview/dist/styles/dockview.css';
import '../../styles/dockview-vox.css';
import { invoke } from '@tauri-apps/api/core';
import { LAYOUT_PERSIST_DEBOUNCE_MS } from '../../config/constants';

interface DockShellProps {
  panelId: string;
  panelTitle: string;
  children: React.ReactNode;
  layoutKey?: string;
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
  const apiRef = useRef<DockviewReadyEvent['api'] | null>(null);
  const persistTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const persistLayout = useCallback(
    (api: DockviewReadyEvent['api']) => {
      if (persistTimer.current) clearTimeout(persistTimer.current);
      persistTimer.current = setTimeout(() => {
        try {
          const json = JSON.stringify(api.toJSON());
          invoke('set_gui_preference', { key: layoutKey, value: json }).catch(() => {});
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
      invoke<string | null>('get_gui_preference', { key: layoutKey })
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

  return (
    <DockviewReact
      className="dockview-theme-vox h-full min-h-0"
      onReady={onReady}
      components={components}
      theme={themeDark}
    />
  );
}
