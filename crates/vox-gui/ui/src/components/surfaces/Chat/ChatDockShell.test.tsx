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
});
