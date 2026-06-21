// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { AttentionStrip } from './AttentionStrip';

const snap = { max_attention_ms: 3_600_000, spent_ms: 1_800_000, interrupt_freq_per_hour: 5.2,
  total_requests: 0, auto_approved: 0, rejected: 0, last_interrupt_ms: 0, inbox_suppressed_count: 0 };

describe('AttentionStrip', () => {
  it('renders nothing when budget is null', () => {
    const { container } = render(<AttentionStrip budget={null} waitingQuestions={0} blockedTasks={0} />);
    expect(container.firstChild).toBeNull();
  });
  it('renders the meter when budget present', () => {
    const { container } = render(<AttentionStrip budget={snap} waitingQuestions={1} blockedTasks={0} />);
    expect(container.textContent).toMatch(/waiting/i);
  });
});
