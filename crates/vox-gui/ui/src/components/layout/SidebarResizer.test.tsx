// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SidebarResizer } from './SidebarResizer';

describe('SidebarResizer', () => {
  it('commits a snapped width on pointer up after drag', () => {
    const onCommit = vi.fn();
    render(<SidebarResizer onResize={() => {}} onCommit={onCommit} onReset={() => {}} />);
    const handle = screen.getByRole('separator', { name: /resize sidebar/i });
    fireEvent.pointerDown(handle);
    fireEvent.pointerMove(window, { clientX: 210 });
    fireEvent.pointerUp(window);
    expect(onCommit).toHaveBeenCalledWith(212); // 210 snaps to default preset
  });

  it('double-click resets', () => {
    const onReset = vi.fn();
    render(<SidebarResizer onResize={() => {}} onCommit={() => {}} onReset={onReset} />);
    fireEvent.doubleClick(screen.getByRole('separator', { name: /resize sidebar/i }));
    expect(onReset).toHaveBeenCalled();
  });
});
