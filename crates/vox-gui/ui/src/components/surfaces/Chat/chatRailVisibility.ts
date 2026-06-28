/**
 * Pure responsive-layout helper for the Chat surface side rails.
 *
 * The Chat surface composes a left session rail (~176px, `w-44`) + center
 * transcript + right execution rail (~224px, `w-56`). On narrow containers the
 * rails crowd out the transcript, so they auto-hide below width breakpoints and
 * become reveal-on-demand overlays.
 *
 * This decision is kept pure (no DOM / layout) so it is unit-testable at
 * representative container widths. Measure the ChatSurface container width with
 * a ResizeObserver and feed it here — NOT window media queries, since the app
 * shell sidebar changes the space available to the surface.
 */

/** Below this container width the execution (right) rail auto-hides. */
export const CHAT_EXECUTION_RAIL_MIN_WIDTH = 1100;
/** Below this container width the session (left) rail also auto-hides. */
export const CHAT_SESSION_RAIL_MIN_WIDTH = 820;

export interface ChatRailVisibility {
  /** Whether the left session rail is shown inline (not as an overlay). */
  sessionRail: boolean;
  /** Whether the right execution rail is shown inline (not as an overlay). */
  executionRail: boolean;
}

/**
 * Decide which side rails are shown inline for a given container width.
 *
 * A width of 0 (or unmeasured) is treated as "wide" so the first paint before
 * the ResizeObserver fires keeps both rails visible exactly as today.
 */
export function chatRailVisibility(containerWidth: number): ChatRailVisibility {
  // Unmeasured / pre-paint: default to the wide layout (both visible).
  if (!Number.isFinite(containerWidth) || containerWidth <= 0) {
    return { sessionRail: true, executionRail: true };
  }
  return {
    sessionRail: containerWidth >= CHAT_SESSION_RAIL_MIN_WIDTH,
    executionRail: containerWidth >= CHAT_EXECUTION_RAIL_MIN_WIDTH,
  };
}
