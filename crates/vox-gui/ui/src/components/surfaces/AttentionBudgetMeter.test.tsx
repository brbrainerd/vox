// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
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

  it('renders collapsed by default when defaultCollapsed is true: compact summary, no caption paragraphs', () => {
    render(<AttentionBudgetMeter budget={snap} defaultCollapsed />);
    // Meter semantics must survive collapse (same aria contract as expanded).
    expect(screen.getByRole('meter')).toHaveAttribute('aria-valuenow', '50');
    // Compact one-line summary (pct + minutes) replaces the caption paragraphs.
    expect(screen.getByText(/50%/)).toBeInTheDocument();
    // Full-card-only captions are gone while collapsed.
    expect(screen.queryByText(/Suppressed prompts/)).not.toBeInTheDocument();
    const toggle = screen.getByRole('button', { name: /expand/i });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
  });

  it('expands to the full card on toggle click, keeping meter aria attributes intact', async () => {
    const user = userEvent.setup();
    render(<AttentionBudgetMeter budget={snap} defaultCollapsed />);
    const toggle = screen.getByRole('button', { name: /expand/i });
    await user.click(toggle);
    expect(screen.getByRole('meter')).toHaveAttribute('aria-valuenow', '50');
    expect(screen.getByText(/Suppressed prompts \(Deep focus\): 2/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /collapse/i })).toHaveAttribute('aria-expanded', 'true');
  });

  it('defaults to expanded (current behavior) when defaultCollapsed is omitted', () => {
    render(<AttentionBudgetMeter budget={snap} />);
    expect(screen.getByText(/Suppressed prompts/)).toBeInTheDocument();
  });

  it('tracks a defaultCollapsed prop transition after mount (e.g. messages hydrate async after the meter first renders)', () => {
    const { rerender } = render(<AttentionBudgetMeter budget={snap} defaultCollapsed={false} />);
    expect(screen.getByText(/Suppressed prompts/)).toBeInTheDocument();
    rerender(<AttentionBudgetMeter budget={snap} defaultCollapsed />);
    expect(screen.queryByText(/Suppressed prompts/)).not.toBeInTheDocument();
  });

  it('stops following defaultCollapsed once the user has manually toggled', async () => {
    const user = userEvent.setup();
    const { rerender } = render(<AttentionBudgetMeter budget={snap} defaultCollapsed />);
    await user.click(screen.getByRole('button', { name: /expand/i }));
    expect(screen.getByText(/Suppressed prompts/)).toBeInTheDocument();
    // Caller re-computes defaultCollapsed and it actually changes value
    // (e.g. false -> true) — the user already expanded it manually, so it
    // must stay expanded regardless.
    rerender(<AttentionBudgetMeter budget={snap} defaultCollapsed={false} />);
    expect(screen.getByText(/Suppressed prompts/)).toBeInTheDocument();
    rerender(<AttentionBudgetMeter budget={snap} defaultCollapsed />);
    expect(screen.getByText(/Suppressed prompts/)).toBeInTheDocument();
  });
});
