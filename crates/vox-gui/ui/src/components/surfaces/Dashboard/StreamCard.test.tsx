// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
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

describe('StreamCard after doubt migration', () => {
  it('renders the item title', () => {
    render(<StreamCard item={mockItem} />);
    expect(screen.getByText('Test action')).toBeDefined();
  });

  it('no longer renders doubt or overrule controls', () => {
    render(<StreamCard item={mockItem} />);
    expect(screen.queryByTitle(/doubt/i)).toBeNull();
    expect(screen.queryByTitle(/overrule/i)).toBeNull();
    expect(screen.queryByLabelText(/doubt/i)).toBeNull();
    expect(screen.queryByLabelText(/overrule/i)).toBeNull();
  });
});
