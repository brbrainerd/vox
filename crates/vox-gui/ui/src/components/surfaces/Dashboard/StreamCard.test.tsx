// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
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

const doubtedItem: StreamItem = {
  ...mockItem,
  id: 'item-2',
  kind: 'doubted',
};

describe('StreamCard', () => {
  it('renders the item title', () => {
    render(<StreamCard item={mockItem} onDoubt={vi.fn()} onOverrule={vi.fn()} />);
    expect(screen.getByText('Test action')).toBeDefined();
  });

  it('doubt button has aria-label for non-doubted items', () => {
    render(<StreamCard item={mockItem} onDoubt={vi.fn()} onOverrule={vi.fn()} />);
    const btn = screen.queryByLabelText(/doubt/i);
    expect(btn).not.toBeNull();
  });

  it('doubt button has type="button"', () => {
    render(<StreamCard item={mockItem} onDoubt={vi.fn()} onOverrule={vi.fn()} />);
    const btn = screen.queryByLabelText(/doubt/i);
    expect(btn?.getAttribute('type')).toBe('button');
  });

  it('overrule button has aria-label for doubted items', () => {
    render(<StreamCard item={doubtedItem} onDoubt={vi.fn()} onOverrule={vi.fn()} />);
    const btn = screen.queryByLabelText(/overrule/i);
    expect(btn).not.toBeNull();
  });

  it('overrule button has type="button"', () => {
    render(<StreamCard item={doubtedItem} onDoubt={vi.fn()} onOverrule={vi.fn()} />);
    const btn = screen.queryByLabelText(/overrule/i);
    expect(btn?.getAttribute('type')).toBe('button');
  });
});
