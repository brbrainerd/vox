// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent, screen } from '@testing-library/react';
import { DriveConsole } from './DriveConsole';
import { defaultControl } from '../../../lib/driveConsole';

describe('DriveConsole', () => {
  const base = {
    control: defaultControl(),
    onControlChange: vi.fn(),
    spentUsd: 0.42,
    budgetUsd: 1.0,
    burnPerMin: 0.08,
    model: 'flash',
    auto: true,
  };

  it('renders all four clutch detents, cost, risk, model', () => {
    render(<DriveConsole {...base} />);
    ['Free', 'Effic.', 'Bal.', 'Genius'].forEach(l =>
      expect(screen.getByRole('radio', { name: new RegExp(l, 'i') })).toBeTruthy()
    );
    expect(screen.getByText(/0\.42/)).toBeTruthy();
    expect(screen.getByText(/Moderate/i)).toBeTruthy();
    expect(screen.getByText(/flash/i)).toBeTruthy();
  });

  it('clutch detents are radios with aria-checked reflecting selection', () => {
    render(<DriveConsole {...base} control={{ clutch: 'genius', risk: 'moderate' }} />);
    const radios = screen.getAllByRole('radio');
    expect(radios).toHaveLength(4);
    const genius = screen.getByRole('radio', { name: /Genius/i });
    expect(genius.getAttribute('aria-checked')).toBe('true');
    const free = screen.getByRole('radio', { name: /Free/i });
    expect(free.getAttribute('aria-checked')).toBe('false');
  });

  it('emits clutch change', () => {
    const onControlChange = vi.fn();
    render(<DriveConsole {...base} onControlChange={onControlChange} />);
    fireEvent.click(screen.getByRole('radio', { name: /Genius/i }));
    expect(onControlChange).toHaveBeenCalledWith(expect.objectContaining({ clutch: 'genius' }));
  });

  it('shows risk label from control state', () => {
    render(<DriveConsole {...base} control={{ clutch: 'free', risk: 'high' }} />);
    expect(screen.getByText(/High/i)).toBeTruthy();
  });

  it('shows budget bar when budgetUsd > 0', () => {
    const { container } = render(<DriveConsole {...base} />);
    // The bar span exists
    const bar = container.querySelector('.bg-gradient-to-r');
    expect(bar).toBeTruthy();
  });
});
