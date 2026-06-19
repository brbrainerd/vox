// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { AttentionBudgetMeter } from './AttentionBudgetMeter';
import type { AttentionBudgetSnapshot } from '../../types/tauri';

const snap: AttentionBudgetSnapshot = {
  max_attention_ms: 3_600_000,
  spent_ms: 1_800_000,
  total_requests: 4,
  auto_approved: 1,
  rejected: 1,
  interrupt_freq_per_hour: 9.0,
  last_interrupt_ms: 0,
  inbox_suppressed_count: 2,
};

describe('AttentionBudgetMeter', () => {
  it('renders spent ratio, derived focus depth, and suppressed count', () => {
    render(<AttentionBudgetMeter budget={snap} />);
    expect(screen.getByRole('meter')).toHaveAttribute('aria-valuenow', '50');
    expect(screen.getByText("Deep focus")).toBeInTheDocument();
    expect(screen.getByText(/2/)).toBeInTheDocument(); // inbox_suppressed_count
  });

  it('derives focused vs ambient from interrupt frequency', () => {
    render(<AttentionBudgetMeter budget={{ ...snap, interrupt_freq_per_hour: 4 }} />);
    expect(screen.getByText("Focused")).toBeInTheDocument();
  });

  it('renders nothing when budget is null', () => {
    const { container } = render(<AttentionBudgetMeter budget={null} />);
    expect(container).toBeEmptyDOMElement();
  });
});
