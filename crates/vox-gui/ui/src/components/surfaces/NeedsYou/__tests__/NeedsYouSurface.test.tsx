// @vitest-environment jsdom
import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { NeedsYouSurface } from '../NeedsYouSurface';
import * as transport from '../../../../transport';

beforeEach(() => {
  vi.spyOn(transport, 'feedbackList').mockResolvedValue({
    needsYou: [
      {
        feedbackId: 'F-1',
        kind: 'clarification',
        prompt: 'schema?',
        options: ['a'],
        gates: [7],
        doubtedTaskId: null,
        surface: 'needs_you',
        infoGainBits: 0.8,
      },
    ],
    withheld: [
      {
        feedbackId: 'F-9',
        kind: 'clarification',
        prompt: 'low',
        options: [],
        gates: [],
        doubtedTaskId: null,
        surface: 'withheld',
        infoGainBits: 0.05,
      },
    ],
  });
  vi.spyOn(transport, 'listenFeedbackChanged').mockResolvedValue(() => {});
});

describe('NeedsYouSurface', () => {
  it('lists open items + withheld section', async () => {
    render(<NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} />);
    await waitFor(() => expect(screen.getByText('schema?')).toBeTruthy());
    expect(screen.getByText(/Withheld by policy/i)).toBeTruthy();
  });

  it('empty state when nothing needs you', async () => {
    vi.spyOn(transport, 'feedbackList').mockResolvedValue({ needsYou: [], withheld: [] });
    render(<NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} />);
    await waitFor(() => expect(screen.getByText(/Nothing needs you/i)).toBeTruthy());
  });
});
