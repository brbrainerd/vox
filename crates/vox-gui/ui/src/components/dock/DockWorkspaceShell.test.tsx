// crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.test.tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';
import React from 'react';
import { DockviewApi } from 'dockview';
import { DockWorkspaceShell, layoutStorageKeyFor } from './DockWorkspaceShell';

describe('DockWorkspaceShell', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('mounts a dockview-theme-vox container and calls onReady with an api', () => {
    const onReady = vi.fn();
    const { container } = render(
      <DockWorkspaceShell storageKeyPrefix="test.host" onReady={onReady} components={{}} />,
    );
    expect(container.querySelector('.dockview-theme-vox')).not.toBeNull();
    expect(onReady).toHaveBeenCalledTimes(1);
    expect(onReady.mock.calls[0][0]).toHaveProperty('api');
  });

  it('restores a previously serialized layout via fromJSON on mount, keyed per storageKeyPrefix', () => {
    const savedLayout = { grid: {} };
    localStorage.setItem(layoutStorageKeyFor('test.host'), JSON.stringify(savedLayout));

    const fromJSONSpy = vi.spyOn(DockviewApi.prototype, 'fromJSON').mockImplementation(() => {});
    const onReady = vi.fn();
    render(<DockWorkspaceShell storageKeyPrefix="test.host" onReady={onReady} components={{}} />);

    expect(fromJSONSpy).toHaveBeenCalledTimes(1);
    expect(fromJSONSpy).toHaveBeenCalledWith(savedLayout);
    fromJSONSpy.mockRestore();
  });

  it('two different storageKeyPrefix values persist to two different localStorage keys', () => {
    expect(layoutStorageKeyFor('gui.chat')).not.toBe(layoutStorageKeyFor('gui.other-host'));
  });

  it('never persists panel params (live React nodes) — only geometry survives the round trip', async () => {
    function Probe(props: { params: { node: React.ReactNode } }) {
      return <div>{props.params.node}</div>;
    }
    const onReady = vi.fn((event) => {
      event.api.addPanel({ id: 'probe', component: 'probe', params: { node: <span>live content</span> } });
    });
    render(<DockWorkspaceShell storageKeyPrefix="test.host2" onReady={onReady} components={{ probe: Probe }} />);

    await new Promise((resolve) => setTimeout(resolve, 1200)); // past LAYOUT_PERSIST_DEBOUNCE_MS (1000ms)

    const persisted = localStorage.getItem(layoutStorageKeyFor('test.host2'));
    expect(persisted).not.toBeNull();
    expect(persisted).not.toContain('"params"');
    expect(persisted).toContain('probe');
  }, 10000);
});
