// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import {
  chatRailVisibility,
  CHAT_SESSION_RAIL_MIN_WIDTH,
  CHAT_EXECUTION_RAIL_MIN_WIDTH,
} from './chatRailVisibility';

describe('chatRailVisibility (pure helper)', () => {
  it('hides both rails at mobile width (~375)', () => {
    expect(chatRailVisibility(375)).toEqual({ sessionRail: false, executionRail: false });
  });

  it('hides both rails at tablet width (~768)', () => {
    expect(chatRailVisibility(768)).toEqual({ sessionRail: false, executionRail: false });
  });

  it('shows session rail but hides execution rail at mid width (~1000)', () => {
    expect(chatRailVisibility(1000)).toEqual({ sessionRail: true, executionRail: false });
  });

  it('shows both rails at desktop width (~1400)', () => {
    expect(chatRailVisibility(1400)).toEqual({ sessionRail: true, executionRail: true });
  });

  it('is inclusive exactly at each breakpoint', () => {
    expect(chatRailVisibility(CHAT_SESSION_RAIL_MIN_WIDTH).sessionRail).toBe(true);
    expect(chatRailVisibility(CHAT_SESSION_RAIL_MIN_WIDTH - 1).sessionRail).toBe(false);
    expect(chatRailVisibility(CHAT_EXECUTION_RAIL_MIN_WIDTH).executionRail).toBe(true);
    expect(chatRailVisibility(CHAT_EXECUTION_RAIL_MIN_WIDTH - 1).executionRail).toBe(false);
  });

  it('treats unmeasured width (0) as the wide layout to avoid first-paint flicker', () => {
    expect(chatRailVisibility(0)).toEqual({ sessionRail: true, executionRail: true });
    expect(chatRailVisibility(NaN)).toEqual({ sessionRail: true, executionRail: true });
  });
});

// Component-level "ChatSurface responsive rails" coverage (narrow-container
// collapse-to-toggle-button behavior) was intentionally removed: ChatSurface
// no longer uses this helper for layout — real dockview panels (Task B2,
// see docs/superpowers/plans/2026-07-20-chat-flow-docking-redesign.md)
// replaced the hand-rolled ResizeObserver + show/hide toggle mechanism.
// dockview's own panel visibility/collapse/tab UI supersedes it. The pure
// `chatRailVisibility` helper above is retained/tested standalone in case a
// future consumer needs width-based breakpoint logic.
