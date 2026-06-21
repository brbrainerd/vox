// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { HudPanels } from './HudPanels';

describe('HudPanels', () => {
  it('renders treasury and energy values', () => {
    render(<HudPanels treasuryValue={120} energy={90} speed={1} onSetSpeed={vi.fn()} />);
    expect(screen.getByTestId('hud-value')).toHaveTextContent('120');
    expect(screen.getByTestId('hud-energy')).toHaveTextContent('90');
  });
});
