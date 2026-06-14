// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { LudusHud } from './LudusHud';
import type { LudusProfile } from '../../../lib/ludus';

const profile: LudusProfile = {
  user_id: 'u1',
  level: 7,
  xp: 1200,
  xp_to_next_level: 300,
  xp_progress: 0.6,
  total_xp_earned: 5000,
  crystals: 42,
  lumens: 13,
  energy: 80,
  max_energy: 100,
  current_streak: 5,
  prestige_level: 2,
  title: 'Adept',
  full_title: 'Adept of the Forge',
  trust_tier: 'trusted',
};

describe('LudusHud', () => {
  it('renders the XP bar as an accessible progressbar', () => {
    render(<LudusHud profile={profile} />);
    const bar = screen.getByRole('progressbar', { name: /xp/i });
    expect(bar.getAttribute('aria-valuenow')).toBe('60');
    expect(bar.getAttribute('aria-valuemin')).toBe('0');
    expect(bar.getAttribute('aria-valuemax')).toBe('100');
  });
});
