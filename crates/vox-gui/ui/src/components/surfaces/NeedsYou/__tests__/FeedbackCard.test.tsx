// @vitest-environment jsdom
import React from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { FeedbackCard } from '../FeedbackCard';

const clar = {
  feedbackId: 'F-1',
  kind: 'clarification' as const,
  prompt: 'schema?',
  options: ['In hopper', 'Separate'],
  gates: [7],
  doubtedTaskId: null,
  surface: 'needs_you' as const,
  infoGainBits: 0.8,
};

const doubt = {
  feedbackId: 'F-2',
  kind: 'doubt' as const,
  prompt: 'suspect',
  options: [],
  gates: [],
  doubtedTaskId: 9,
  surface: 'needs_you' as const,
  infoGainBits: 0,
};

const proposal = {
  feedbackId: 'F-9',
  kind: 'skill_proposal' as const,
  prompt: "Recurring procedure 'read-edit-run': read → edit → run. Consider saving it as a reusable skill.",
  options: ['Dismiss'],
  gates: [],
  doubtedTaskId: null,
  surface: 'needs_you' as const,
  infoGainBits: 0,
};

describe('FeedbackCard', () => {
  it('skill_proposal: Dismiss resolves with skip action', () => {
    const onResolve = vi.fn();
    render(<FeedbackCard row={proposal} onResolve={onResolve} onOpenContext={() => {}} />);
    expect(screen.getByText(/Recurring procedure/)).toBeTruthy();
    fireEvent.click(screen.getByText('Dismiss'));
    expect(onResolve).toHaveBeenCalledWith('F-9', { action: 'skip' });
  });

  it('skill_proposal: Save as skill resolves with accept_skill action', () => {
    const onResolve = vi.fn();
    render(<FeedbackCard row={proposal} onResolve={onResolve} onOpenContext={() => {}} />);
    fireEvent.click(screen.getByText('Save as skill'));
    expect(onResolve).toHaveBeenCalledWith('F-9', { action: 'accept_skill' });
  });

  it('clarification: option click resolves with answer action', () => {
    const onResolve = vi.fn();
    render(<FeedbackCard row={clar} onResolve={onResolve} onOpenContext={() => {}} />);
    fireEvent.click(screen.getByText('Separate'));
    expect(onResolve).toHaveBeenCalledWith('F-1', { action: 'answer', option: 1, text: null });
  });

  it('doubt: overrule resolves with overrule action', () => {
    const onResolve = vi.fn();
    render(<FeedbackCard row={doubt} onResolve={onResolve} onOpenContext={() => {}} />);
    fireEvent.click(screen.getByLabelText(/overrule/i));
    expect(onResolve).toHaveBeenCalledWith('F-2', { action: 'overrule' });
  });
});
