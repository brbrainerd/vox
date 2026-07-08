// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { WorkbenchTabBar } from './WorkbenchTabBar';

describe('WorkbenchTabBar', () => {
  it('renders tabs with close buttons and marks active', () => {
    render(
      <WorkbenchTabBar
        tabs={[
          { id: 'console', label: 'Console' },
          { id: 'chat', label: 'Chat' },
        ]}
        activeTab="console"
        onSelect={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole('tab', { name: 'Console' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('button', { name: 'Close Console' })).toBeDefined();
  });

  it('calls onSelect when tab clicked', async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <WorkbenchTabBar
        tabs={[{ id: 'console', label: 'Console' }]}
        activeTab="console"
        onSelect={onSelect}
        onClose={vi.fn()}
      />,
    );
    await user.click(screen.getByRole('tab', { name: 'Console' }));
    expect(onSelect).toHaveBeenCalledWith('console');
  });

  it('does not render close button for pinned tabs', () => {
    render(
      <WorkbenchTabBar
        tabs={[
          { id: 'chat', label: 'Chat', pinned: true },
          { id: 'console', label: 'Console' },
        ]}
        activeTab="chat"
        onSelect={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(screen.queryByRole('button', { name: 'Close Chat' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Close Console' })).toBeDefined();
  });

  it('calls onClose when close button clicked', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <WorkbenchTabBar
        tabs={[{ id: 'console', label: 'Console' }]}
        activeTab="console"
        onSelect={vi.fn()}
        onClose={onClose}
      />,
    );
    await user.click(screen.getByRole('button', { name: 'Close Console' }));
    expect(onClose).toHaveBeenCalledWith('console');
  });
});
