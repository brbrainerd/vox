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
  };

  it('renders all four clutch detents, cost, risk — no model read-out segment', () => {
    render(<DriveConsole {...base} />);
    ['Free', 'Effic.', 'Bal.', 'Genius'].forEach(l =>
      expect(screen.getByRole('radio', { name: new RegExp(l, 'i') })).toBeTruthy()
    );
    expect(screen.getByText(/0\.42/)).toBeTruthy();
    expect(screen.getByText(/Moderate/i)).toBeTruthy();
    // Segment ④ (redundant model read-out) was dropped.
    expect(screen.queryByTitle(/active model/i)).toBeNull();
  });

  it('strip root does not clip the risk popover (no overflow-hidden)', () => {
    const { container } = render(<DriveConsole {...base} />);
    expect((container.firstChild as HTMLElement).className).not.toContain('overflow-hidden');
  });

  it('opens the risk popover anchored above the Risk trigger', () => {
    render(<DriveConsole {...base} />);
    fireEvent.click(screen.getByRole('button', { name: /risk: moderate/i }));
    const dialog = screen.getByRole('dialog', { name: /acceptable risk/i });
    expect(dialog.className).toContain('bottom-full');
    // Anchored to a relative wrapper around the trigger, not the strip root.
    expect((dialog.parentElement as HTMLElement).className).toContain('relative');
  });

  it('closes the risk popover on outside pointerdown', () => {
    render(<DriveConsole {...base} />);
    fireEvent.click(screen.getByRole('button', { name: /risk: moderate/i }));
    expect(screen.getByRole('dialog', { name: /acceptable risk/i })).toBeTruthy();
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole('dialog', { name: /acceptable risk/i })).toBeNull();
  });

  it('keeps the risk popover open on pointerdown inside it', () => {
    render(<DriveConsole {...base} />);
    fireEvent.click(screen.getByRole('button', { name: /risk: moderate/i }));
    fireEvent.pointerDown(screen.getByRole('dialog', { name: /acceptable risk/i }));
    expect(screen.getByRole('dialog', { name: /acceptable risk/i })).toBeTruthy();
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
    const bar = container.querySelector('.bg-linear-to-r');
    expect(bar).toBeTruthy();
  });
});
