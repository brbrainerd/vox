import React, { createContext, useContext } from 'react';

/**
 * Signals that a surface is being rendered as an EMBEDDED mini-render (a
 * dashboard thumbnail), not as the live full-screen surface.
 *
 * When true, surfaces MUST suppress their repeating mount-time work — the
 * `setInterval` poll loops and pushed streaming subscriptions. The first fetch
 * (to populate the thumbnail with real data) is fine; the repeating poll is the
 * waste a thumbnail must not incur. Read it via {@link useIsEmbeddedSurface}.
 *
 * Default is `false`: a surface rendered anywhere outside SurfaceMiniRender
 * behaves exactly as before.
 */
export const EmbeddedSurfaceContext = createContext<boolean>(false);

/** True when the calling surface is inside a SurfaceMiniRender thumbnail. */
export function useIsEmbeddedSurface(): boolean {
  return useContext(EmbeddedSurfaceContext);
}
