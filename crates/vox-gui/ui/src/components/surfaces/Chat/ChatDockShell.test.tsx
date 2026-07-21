// crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import React from 'react';
import { ChatDockShell } from './ChatDockShell';

describe('ChatDockShell', () => {
  it('mounts a dockview-theme-vox container and calls onReady with an api', () => {
    const onReady = vi.fn();
    const { container } = render(<ChatDockShell onReady={onReady} components={{}} />);
    expect(container.querySelector('.dockview-theme-vox')).not.toBeNull();
    expect(onReady).toHaveBeenCalledTimes(1);
    expect(onReady.mock.calls[0][0]).toHaveProperty('api');
  });
});
