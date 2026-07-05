// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { HudPanels } from './HudPanels';

describe('HudPanels', () => {
  it('renders treasury and energy values', () => {
    render(<HudPanels treasuryUsd={120} energy={90} maxEnergy={100} speed={1} onSetSpeed={vi.fn()} />);
    expect(screen.getByTestId('hud-value')).toHaveTextContent('120');
    expect(screen.getByTestId('hud-energy')).toHaveTextContent('90');
  });

  it('renders real USD spend and energy fraction', () => {
    render(<HudPanels treasuryUsd={12.4} energy={82} maxEnergy={100} speed={1} onSetSpeed={() => {}} />);
    expect(screen.getByTestId('hud-value').textContent).toContain('$12.40');
    expect(screen.getByTestId('hud-energy').textContent).toBe('82/100');
  });

  it('renders an em-dash when spend is unknown (tap failed) — never a fake 0', () => {
    render(<HudPanels treasuryUsd={null} energy={82} maxEnergy={100} speed={1} onSetSpeed={() => {}} />);
    expect(screen.getByTestId('hud-value').textContent).toBe('—');
  });

  it('offers pause (0x), 1x and 3x speeds', () => {
    const spy = vi.fn();
    render(<HudPanels treasuryUsd={1} energy={1} maxEnergy={1} speed={1} onSetSpeed={spy} />);
    screen.getByRole('button', { name: '0x' }).click();
    expect(spy).toHaveBeenCalledWith(0);
  });
});
