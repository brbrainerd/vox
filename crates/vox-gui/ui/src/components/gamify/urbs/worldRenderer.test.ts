// crates/vox-gui/ui/src/components/gamify/urbs/worldRenderer.test.ts
import { describe, it, expect } from 'vitest';
import { redrawKey, type WorldState } from './worldRenderer';
import { layoutTown } from './layout';
import type { TownScan } from './types';

const scan: TownScan = {
  crates: [{ name: 'a', root: 'crates/a', files: [{ path: 'crates/a/x.rs', lines: 10 }] }],
  root: '/ws', scanned_at_ms: 0, truncated: false,
};

function state(over: Partial<WorldState> = {}): WorldState {
  return {
    layout: layoutTown(scan, new Set()),
    buildings: {}, agentTasks: {},
    harness: { ci: null, vcs: null, queueLen: null, mcp: null },
    ...over,
  };
}

describe('redrawKey', () => {
  it('is stable when nothing changed', () => {
    const s = state();
    expect(redrawKey(s, 1)).toEqual(redrawKey(s, 1));
  });
  it('changes when diagnostics change (buffer must repaint)', () => {
    const a = redrawKey(state(), 1);
    const b = redrawKey(state({ buildings: { 'crates/a/x.rs': { x: 0, y: 0, warnings: 1, errors: 0 } } }), 1);
    expect(a).not.toEqual(b);
  });
  it('changes across LOD bands but has NO camera or animation-frame input', () => {
    const s = state();
    expect(redrawKey(s, 0)).not.toEqual(redrawKey(s, 1));
    // No camera and no fire-frame parameter exist at all — a compile-level
    // guarantee that pan/zoom and fire animation never repaint the buffer.
    expect(redrawKey(s, 1)).toEqual(redrawKey(s, 1));
  });
});
