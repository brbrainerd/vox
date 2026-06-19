// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AttentionBudgetMeter } from '../AttentionBudgetMeter';

const snap = { max_attention_ms: 3_600_000, spent_ms: 1_800_000, interrupt_freq_per_hour: 5.2,
  total_requests: 0, auto_approved: 0, rejected: 0, last_interrupt_ms: 0, inbox_suppressed_count: 0 };

describe('AttentionBudgetMeter counts', () => {
  it('renders waiting + blocked counts when provided', () => {
    render(<AttentionBudgetMeter budget={snap} waitingQuestions={2} blockedTasks={1} />);
    expect(screen.getByText(/2 waiting/i)).toBeTruthy();
    expect(screen.getByText(/1 blocked/i)).toBeTruthy();
  });
  it('omits count chips when zero / undefined', () => {
    const { container } = render(<AttentionBudgetMeter budget={snap} />);
    expect(container.textContent).not.toMatch(/waiting/i);
  });
});
