import React from 'react';
import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';
import { labelForNavKey, resolveNavigation } from './navigation';
import { childRenderer, type SurfaceProps } from '../components/layout/surfaceComponents';

/**
 * Dockable surfaces = every registry entry that has a viewKey AND a navLabel
 * (i.e. something a user can actually open). This is derived from the nav SSOT,
 * so adding a surface to the registry makes it dockable automatically.
 */
export const DOCKABLE_VIEW_KEYS: string[] = Array.from(
  new Set(
    SURFACE_REGISTRY
      .filter(e => e.viewKey && e.navLabel)
      .map(e => e.viewKey as string),
  ),
);

export function isDockable(viewKey: string): boolean {
  return DOCKABLE_VIEW_KEYS.includes(viewKey);
}

/**
 * Resolve a viewKey to the CHILD view that `childRenderer` actually renders.
 * Top-level parent keys (knowledge/agents/workspace/commands/compute) have no
 * childRenderer case — they must be mapped to their default child first.
 * `resolveNavigation` is idempotent for keys that are already children, so this
 * is safe to call on any viewKey.
 */
export function resolvePanelView(viewKey: string): string {
  return resolveNavigation(viewKey).child;
}

export function panelTitle(viewKey: string): string {
  return labelForNavKey(resolvePanelView(viewKey));
}

/**
 * Render a surface as a dock panel body. Reuses the EXISTING childRenderer so a
 * panel is pixel-identical to the surface rendered inline — no duplicate views.
 */
export function renderPanel(viewKey: string, props: SurfaceProps): React.ReactNode {
  return childRenderer(props, resolvePanelView(viewKey));
}
