// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { useState } from 'react';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { WorkbenchTabBar, type WorkbenchTabItem } from './WorkbenchTabBar';

/** Controlled harness so arrow-key navigation actually moves `activeTab`. */
function ControlledTabBar({
  tabs,
  initialActive,
  onClose,
}: {
  tabs: WorkbenchTabItem[];
  initialActive: string;
  onClose: (id: string) => void;
}) {
  const [activeTab, setActiveTab] = useState(initialActive);
  return <WorkbenchTabBar tabs={tabs} activeTab={activeTab} onSelect={setActiveTab} onClose={onClose} />;
}

describe('WorkbenchTabBar', () => {
  it('renders tabs with close affordances and marks active', () => {
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
    expect(screen.getByRole('tab', { name: /console/i })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByTestId('workbench-tab-close-console')).toBeDefined();
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
    await user.click(screen.getByRole('tab', { name: /console/i }));
    expect(onSelect).toHaveBeenCalledWith('console');
  });

  it('does not render close affordance for pinned tabs', () => {
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
    expect(screen.queryByTestId('workbench-tab-close-chat')).toBeNull();
    expect(screen.getByTestId('workbench-tab-close-console')).toBeDefined();
  });

  it('calls onClose when close affordance clicked', async () => {
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
    await user.click(screen.getByTestId('workbench-tab-close-console'));
    expect(onClose).toHaveBeenCalledWith('console');
  });

  it('tablist owns only tabs: every direct child is role=tab and no buttons exist in the a11y tree (F-07)', () => {
    render(
      <WorkbenchTabBar
        tabs={[{ id: 'chat', label: 'Chat', pinned: true }, { id: 'console', label: 'Console' }]}
        activeTab="chat"
        onSelect={() => {}}
        onClose={() => {}}
      />,
    );
    const tablist = screen.getByRole('tablist');
    for (const child of Array.from(tablist.children)) {
      expect(child.getAttribute('role')).toBe('tab');
    }
    // Buttons inside a tablist are what aria-required-children actually flags;
    // testing-library's role queries respect aria-hidden, approximating axe.
    expect(within(tablist).queryAllByRole('button')).toEqual([]);
  });

  it('Delete key on a focused tab closes it (keyboard replacement for the AT-hidden close affordance)', async () => {
    const onClose = vi.fn();
    render(
      <WorkbenchTabBar
        tabs={[{ id: 'chat', label: 'Chat', pinned: true }, { id: 'console', label: 'Console' }]}
        activeTab="console"
        onSelect={() => {}}
        onClose={onClose}
      />,
    );
    const tab = screen.getByRole('tab', { name: /console/i });
    tab.focus();
    await userEvent.keyboard('{Delete}');
    expect(onClose).toHaveBeenCalledWith('console');
  });

  it('ArrowRight moves focus (and roving tabIndex) to the next tab, which becomes selectable and closable (F-07 keyboard regression fix)', async () => {
    const onClose = vi.fn();
    render(
      <ControlledTabBar
        tabs={[
          { id: 'chat', label: 'Chat', pinned: true },
          { id: 'console', label: 'Console' },
        ]}
        initialActive="chat"
        onClose={onClose}
      />,
    );
    const chatTab = screen.getByRole('tab', { name: /chat/i });
    const consoleTab = screen.getByRole('tab', { name: /console/i });
    chatTab.focus();
    expect(chatTab).toHaveAttribute('tabIndex', '0');
    expect(consoleTab).toHaveAttribute('tabIndex', '-1');

    await userEvent.keyboard('{ArrowRight}');

    // Focus moved to the second tab, it is now selected, and roving tabIndex followed.
    expect(consoleTab).toHaveFocus();
    expect(consoleTab).toHaveAttribute('aria-selected', 'true');
    expect(consoleTab).toHaveAttribute('tabIndex', '0');
    expect(chatTab).toHaveAttribute('tabIndex', '-1');

    // The newly-focused tab is reachable and closable without ever using the mouse.
    await userEvent.keyboard('{Delete}');
    expect(onClose).toHaveBeenCalledWith('console');
  });

  it('ArrowLeft/Home/End wrap and jump between tabs', async () => {
    render(
      <ControlledTabBar
        tabs={[
          { id: 'chat', label: 'Chat', pinned: true },
          { id: 'console', label: 'Console' },
          { id: 'dashboard', label: 'Dashboard' },
        ]}
        initialActive="chat"
        onClose={vi.fn()}
      />,
    );
    const chatTab = screen.getByRole('tab', { name: /chat/i });
    const dashboardTab = screen.getByRole('tab', { name: /dashboard/i });
    chatTab.focus();

    // ArrowLeft from the first tab wraps to the last tab.
    await userEvent.keyboard('{ArrowLeft}');
    expect(dashboardTab).toHaveFocus();
    expect(dashboardTab).toHaveAttribute('aria-selected', 'true');

    await userEvent.keyboard('{Home}');
    expect(chatTab).toHaveFocus();
    expect(chatTab).toHaveAttribute('aria-selected', 'true');

    await userEvent.keyboard('{End}');
    expect(dashboardTab).toHaveFocus();
    expect(dashboardTab).toHaveAttribute('aria-selected', 'true');
  });

  it('exposes the Delete shortcut to AT via aria-keyshortcuts on the tab', () => {
    render(
      <WorkbenchTabBar
        tabs={[{ id: 'console', label: 'Console' }]}
        activeTab="console"
        onSelect={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.getByRole('tab', { name: /console/i })).toHaveAttribute('aria-keyshortcuts', 'Delete');
  });
});
