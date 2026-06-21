// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ModelBadge } from './ModelBadge';

describe('ModelBadge', () => {
  const attr = {
    model: 'claude-opus',
    reqTokens: 4200,
    respTokens: 1100,
    costUsd: 0.06,
    selectionReason: 'scored',
    latencyMs: 820,
  };

  it('shows model + tokens collapsed', () => {
    render(<ModelBadge {...attr} />);
    expect(screen.getByText(/claude-opus/)).toBeTruthy();
    expect(screen.queryByText(/scored/)).toBeNull(); // detail hidden by default
  });

  it('reveals detail on activate (keyboard reachable)', () => {
    render(<ModelBadge {...attr} />);
    fireEvent.click(screen.getByRole('button', { name: /claude-opus/i }));
    expect(screen.getByText(/scored/)).toBeTruthy();
    expect(screen.getByText(/820/)).toBeTruthy();
  });

  it('renders unknown when no model', () => {
    render(<ModelBadge model={undefined} />);
    expect(screen.getByText(/model unknown/i)).toBeTruthy();
  });
});
