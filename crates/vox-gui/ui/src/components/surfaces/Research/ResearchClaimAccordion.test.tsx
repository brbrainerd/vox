// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ResearchClaimAccordion, type ResearchClaimRow } from './ResearchClaimAccordion';

const claims: ResearchClaimRow[] = [
  {
    claimId: 'c1',
    text: 'The sky is blue.',
    verdict: 'Supported',
    confidence: 0.92,
    resampleStability: 0.8,
    citations: [{ url: 'https://example.com/a', trust: { kind: 'formal', venueType: 'peer-reviewed', retracted: false } }],
  },
  {
    claimId: 'c2',
    text: 'The moon is made of cheese.',
    verdict: 'Contested',
    confidence: 0.4,
    resampleStability: 0.5,
    citations: [{ url: 'https://example.com/b', trust: { kind: 'uncorroborated' } }],
  },
];

describe('ResearchClaimAccordion', () => {
  it('is collapsed by default and shows only the summary line with correct counts', () => {
    render(<ResearchClaimAccordion claims={claims} sourceCount={5} />);
    expect(screen.getByText(/2 claims verified · 1 contested · 5 sources/i)).toBeTruthy();
    expect(screen.queryByText('The sky is blue.')).toBeNull();
    const toggle = screen.getByRole('button');
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
  });

  it('expands to show claim rows when the toggle is clicked', async () => {
    const user = userEvent.setup();
    render(<ResearchClaimAccordion claims={claims} sourceCount={5} />);
    await user.click(screen.getByRole('button'));
    expect(screen.getByText('The sky is blue.')).toBeTruthy();
    expect(screen.getByText('The moon is made of cheese.')).toBeTruthy();
    const toggle = screen.getByRole('button');
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
  });

  it('shows a TrustChip for each claim row citation once expanded', async () => {
    const user = userEvent.setup();
    render(<ResearchClaimAccordion claims={claims} sourceCount={5} />);
    await user.click(screen.getByRole('button'));
    expect(screen.getByText(/peer-reviewed/i)).toBeTruthy();
    expect(screen.getByText(/not independently corroborated/i)).toBeTruthy();
  });

  it('treats a genuine 2-of-3-resample majority (0.667) as stable, not flipped', async () => {
    // RESAMPLE_COUNT is 3 on the backend, so the only real agreement rates
    // are 1/3≈0.333, 2/3≈0.667, and 1.0 — a threshold of 0.67 would
    // wrongly classify a stable 2-of-3 majority as "flipped".
    const twoOfThreeClaim: ResearchClaimRow[] = [
      {
        claimId: 'c3',
        text: 'Two of three resamples agreed.',
        verdict: 'Supported',
        confidence: 0.7,
        resampleStability: 2 / 3,
        citations: [],
      },
    ];
    const user = userEvent.setup();
    render(<ResearchClaimAccordion claims={twoOfThreeClaim} sourceCount={1} />);
    await user.click(screen.getByRole('button'));
    expect(screen.getByText(/Stable across resamples/i)).toBeTruthy();
    expect(screen.queryByText(/Verdict flipped in resampling/i)).toBeNull();
  });
});
