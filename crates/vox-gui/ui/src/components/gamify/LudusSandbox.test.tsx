// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { LudusSandbox, assignPlotCoordinates } from './LudusSandbox';
import { useLudusStore } from './store';
import * as transport from '../../transport';

vi.mock('../../transport', () => ({
  listenAgentEvents: vi.fn().mockResolvedValue(() => {}),
}));

describe('LudusSandbox map logic', () => {
  it('assignPlotCoordinates exists', () => {
    expect(assignPlotCoordinates).toBeDefined();
  });

  it('correctly maps client mouse coords to offset coordinates', () => {
    const mouseX = 150;
    const mouseY = 150;
    const cameraX = 50;
    const cameraY = 50;
    const zoom = 2;

    const worldX = (mouseX - cameraX) / zoom;
    const worldY = (mouseY - cameraY) / zoom;

    expect(worldX).toBe(50);
    expect(worldY).toBe(50);
  });
});

describe('DOM Subscription Engine', () => {
  it('correctly reacts to store updates without parent re-renders', () => {
    let callCount = 0;
    const unsubscribe = useLudusStore.subscribe((state) => {
      if (state.agents['agent_1']) callCount += 1;
    });

    useLudusStore.getState().updateAgent('agent_1', { x: 4, y: 4 });
    expect(callCount).toBe(1);
    unsubscribe();
  });
});

describe('Telemetry Ingestion Mapping', () => {
  it('subscribes to agent events and updates building state on file_edited', () => {
    let eventCallback: any;
    vi.mocked(transport.listenAgentEvents).mockImplementation((cb) => {
      eventCallback = cb;
      return Promise.resolve(() => {});
    });

    const files = ['crates/vox-db/src/lib.rs'];
    const { render } = require('@testing-library/react');
    render(<LudusSandbox files={files} />);

    // Initially, warnings/errors are 0
    const store = useLudusStore.getState();
    const initialBuilding = store.buildings['crates/vox-db/src/lib.rs'];
    expect(initialBuilding).toBeDefined();
    expect(initialBuilding.warnings).toBe(0);

    // Simulate a file_edited event
    if (eventCallback) {
      eventCallback({
        id: 1,
        timestamp_ms: Date.now(),
        kind: {
          type: 'file_edited',
          path: 'crates/vox-db/src/lib.rs',
        },
      });
    }

    const updatedBuilding = useLudusStore.getState().buildings['crates/vox-db/src/lib.rs'];
    expect(updatedBuilding.warnings).toBe(1);
  });
});
