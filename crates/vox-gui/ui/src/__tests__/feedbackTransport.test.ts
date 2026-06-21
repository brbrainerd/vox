// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { normalizeFeedback } from '../transport';

describe('normalizeFeedback', () => {
  it('splits needs_you from withheld and pins doubts first', () => {
    const raw = {
      needs_you: [
        {
          id: 'F-1',
          kind: 'clarification',
          prompt: 'q',
          options: ['a'],
          gates: [7],
          doubted_task_id: null,
          surface: 'needs_you',
          info_gain_bits: 0.8,
        },
        {
          id: 'F-2',
          kind: 'doubt',
          prompt: 'd',
          options: [],
          gates: [],
          doubted_task_id: 9,
          surface: 'needs_you',
          info_gain_bits: 0,
        },
      ],
      withheld: [
        {
          id: 'F-3',
          kind: 'clarification',
          prompt: 'low',
          options: [],
          gates: [],
          doubted_task_id: null,
          surface: 'withheld',
          info_gain_bits: 0.05,
        },
      ],
    };

    const { needsYou, withheld } = normalizeFeedback(raw);
    expect(needsYou[0].feedbackId).toBe('F-2'); // doubt pinned first
    expect(needsYou[1].feedbackId).toBe('F-1'); // clarification by gain
    expect(withheld.map((r) => r.feedbackId)).toEqual(['F-3']);
  });
});
