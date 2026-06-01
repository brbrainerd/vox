import type React from 'react';
import { ScientiaDashboard } from './Scientia/ScientiaDashboard';

/**
 * Props every surface decorator receives. Decorators are hand-built views that
 * replace the default (generated/built-in) view for a surface key. They MUST
 * route command execution through the shared `execute_command` Tauri path so
 * every surface stays on one run seam.
 */
export interface SurfaceDecoratorProps {
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

/**
 * Surface key → decorator. `App.tsx::renderView` consults this before its
 * built-in switch, so promoting a surface to a decorated view is a one-line
 * registration here and removing the entry reverts to the default with no other
 * change.
 */
export const surfaceDecorators: Record<string, React.ComponentType<SurfaceDecoratorProps>> = {
  scientia: ScientiaDashboard,
};
