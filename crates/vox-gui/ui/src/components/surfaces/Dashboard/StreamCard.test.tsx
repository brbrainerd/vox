// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { StreamCard } from './StreamCard';
import type { StreamItem } from '../../../types/dashboard';

const mockItem: StreamItem = {
  id: 'item-1',
  kind: 'speculative',
  tag: 'planner',
  title: 'Test action',
  body: 'Did something useful',
  ts: '12:00',
};

describe('StreamCard', () => {
  it('renders the item title', () => {
    render(<StreamCard item={mockItem} />);
    expect(screen.getByText('Test action')).toBeDefined();
  });

  it('does not render doubt/overrule when item has no taskId', () => {
    const item = { id: 'x', kind: 'in-progress', tag: 'TASK', title: 't', body: 'b', ts: '' } as StreamItem;
    render(<StreamCard item={item} onDoubt={vi.fn()} onOverrule={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /doubt/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /overrule/i })).not.toBeInTheDocument();
  });

  it('calls onDoubt with the item when Doubt is clicked', () => {
    const onDoubt = vi.fn();
    const item = { id: 'x', kind: 'in-progress', tag: 'TASK', title: 't', body: 'b', ts: '', taskId: 7 } as StreamItem;
    render(<StreamCard item={item} onDoubt={onDoubt} onOverrule={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /doubt/i }));
    expect(onDoubt).toHaveBeenCalledWith(item);
  });

  it('renders overrule button for doubted item with taskId', () => {
    const onOverrule = vi.fn();
    const item = { id: 'x', kind: 'doubted', tag: 'FAILED', title: 't', body: 'b', ts: '', taskId: 9 } as StreamItem;
    render(<StreamCard item={item} onDoubt={vi.fn()} onOverrule={onOverrule} />);
    const btn = screen.getByRole('button', { name: /overrule/i });
    expect(btn).toBeDefined();
    fireEvent.click(btn);
    expect(onOverrule).toHaveBeenCalledWith(item);
  });
});
