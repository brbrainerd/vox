// crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx
import React from 'react';
import { DockviewReact, type DockviewReadyEvent, type IDockviewPanelProps } from 'dockview';

interface ChatDockShellProps {
  components: Record<string, React.FunctionComponent<IDockviewPanelProps>>;
  onReady: (event: DockviewReadyEvent) => void;
}

/**
 * The dockview shell for the chat workspace: sessions list, execution rail,
 * Flow, and plan panels all dock/resize/hide within this container around
 * the central chat pane. Theming via the `dockview-theme-vox` class
 * (crates/vox-gui/ui/src/styles/dockview-vox.css), not the `theme` prop.
 */
export function ChatDockShell({ components, onReady }: ChatDockShellProps) {
  return (
    <div className="dockview-theme-vox h-full min-h-[60vh] w-full">
      <DockviewReact components={components} onReady={onReady} />
    </div>
  );
}
