// crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';
import React from 'react';
import { DockviewApi } from 'dockview';
import { ChatDockShell, LAYOUT_STORAGE_KEY } from './ChatDockShell';

describe('ChatDockShell', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('mounts a dockview-theme-vox container and calls onReady with an api', () => {
    const onReady = vi.fn();
    const { container } = render(<ChatDockShell onReady={onReady} components={{}} />);
    expect(container.querySelector('.dockview-theme-vox')).not.toBeNull();
    expect(onReady).toHaveBeenCalledTimes(1);
    expect(onReady.mock.calls[0][0]).toHaveProperty('api');
  });

  it('restores a previously serialized layout via fromJSON on mount', () => {
    const savedLayout = { grid: {} };
    localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(savedLayout));

    const fromJSONSpy = vi.spyOn(DockviewApi.prototype, 'fromJSON').mockImplementation(() => {});
    const onReady = vi.fn();
    render(<ChatDockShell onReady={onReady} components={{}} />);

    expect(fromJSONSpy).toHaveBeenCalledTimes(1);
    expect(fromJSONSpy).toHaveBeenCalledWith(savedLayout);
    expect(onReady).toHaveBeenCalledTimes(1);

    fromJSONSpy.mockRestore();
  });

  it('never persists panel params (live React nodes) — only geometry survives the round trip', async () => {
    // Reproduces the React error #31 crash: a panel's params.node is a live
    // React element. JSON.stringify silently drops its `type` (a function)
    // and `$$typeof` (a Symbol), leaving a garbled {key, ref, props} object
    // that crashes on the next launch's first render if it's ever restored.
    // Real timers (not fake) — dockview's layout-change notification is
    // scheduled via requestAnimationFrame internally, which fake timers
    // don't intercept by default; a short real wait past the debounce is
    // simpler and non-flaky here.
    function Probe(props: { params: { node: React.ReactNode } }) {
      return <div>{props.params.node}</div>;
    }
    const onReady = vi.fn((event) => {
      event.api.addPanel({
        id: 'probe',
        component: 'probe',
        params: { node: <span>live content</span> },
      });
    });
    render(<ChatDockShell onReady={onReady} components={{ probe: Probe }} />);

    await new Promise((resolve) => setTimeout(resolve, 1200)); // past LAYOUT_PERSIST_DEBOUNCE_MS (1000ms)

    const persisted = localStorage.getItem(LAYOUT_STORAGE_KEY);
    expect(persisted).not.toBeNull();
    expect(persisted).not.toContain('"params"');
    // Sanity: geometry (the panel id) still made it through.
    expect(persisted).toContain('probe');
  }, 10000);
});
