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

describe('FeedbackCard', () => {
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
