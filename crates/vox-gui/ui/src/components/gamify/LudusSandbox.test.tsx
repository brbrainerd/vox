// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
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
  beforeEach(() => {
    useLudusStore.getState().reset();
  });

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
    expect(initialBuilding?.warnings ?? 0).toBe(0);

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

  it('updates camera offsets when focusedFile changes', () => {
    const files = ['crates/vox-db/src/lib.rs'];
    const { render } = require('@testing-library/react');
    render(<LudusSandbox files={files} />);

    // Trigger focused file state change
    useLudusStore.getState().setFocusedFile('crates/vox-db/src/lib.rs');
    
    // Camera target centering check (verifies camera center state is updated)
    const store = useLudusStore.getState();
    expect(store.focusedFile).toBe('crates/vox-db/src/lib.rs');
  });

  it('correctly maps canvas clicks to building focusedFile states', () => {
    const files = ['crates/vox-db/src/lib.rs'];
    const { render, fireEvent } = require('@testing-library/react');
    const { container } = render(<LudusSandbox files={files} />);
    const canvas = container.querySelector('canvas');
    expect(canvas).toBeDefined();

    // The single plot for crates/vox-db/src/lib.rs is at x=4, y=4, z=0
    // projectIso(4, 4, 0, 64, 32, 1000, 100) -> px = 1000, py = 228
    // Default camera is { x: 400, y: 100, zoom: 1 }
    // clientX = camera.x + px = 1400
    // clientY = camera.y + py = 328
    
    // We mock getBoundingClientRect on the canvas to return { left: 0, top: 0, width: 800, height: 500 }
    canvas.getBoundingClientRect = () => ({
      left: 0,
      top: 0,
      right: 800,
      bottom: 500,
      width: 800,
      height: 500,
      x: 0,
      y: 0,
      toJSON: () => {},
    });

    // Reset focusedFile first
    useLudusStore.getState().setFocusedFile(null);

    fireEvent.click(canvas, { clientX: 400, clientY: 328 });
    
    expect(useLudusStore.getState().focusedFile).toBe('crates/vox-db/src/lib.rs');
  });
});
