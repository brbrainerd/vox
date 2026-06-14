// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { GamifyView } from './GamifyView';

const profile = {
  user_id: 'u1',
  level: 3,
  xp: 10,
  xp_to_next_level: 90,
  xp_progress: 0.1,
  total_xp_earned: 100,
  crystals: 1,
  lumens: 2,
  energy: 5,
  max_energy: 10,
  current_streak: 1,
  prestige_level: 0,
  title: 'Novice',
  full_title: 'Novice Smith',
  trust_tier: 'new',
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_ludus_profile') return Promise.resolve(profile);
    if (cmd === 'list_gamify_companions')
      return Promise.resolve([
        {
          id: 'c1',
          name: 'Sparky',
          description: null,
          language: 'rust',
          mood: 'happy',
          health: 8,
          max_health: 10,
          energy: 5,
          max_energy: 10,
          code_quality: 90,
          last_active: 0,
          svg: '<svg></svg>',
        },
      ]);
    return Promise.resolve([]);
  });
});

describe('GamifyView', () => {
  it('renders the heading and a typed refresh control', () => {
    render(<GamifyView pushToast={() => {}} />);
    expect(screen.getByText(/gamification/i)).toBeDefined();
    const refresh = screen.getByRole('button', { name: /refresh|loading/i });
    expect(refresh.getAttribute('type')).toBe('button');
  });

  it('exposes companion bars as accessible progressbars', async () => {
    render(<GamifyView pushToast={() => {}} />);
    await waitFor(() => expect(screen.getByText('Sparky')).toBeDefined());
    const bars = screen.getAllByRole('progressbar');
    expect(bars.length).toBeGreaterThan(0);
    const hp = screen.getByRole('progressbar', { name: 'HP' });
    expect(hp.getAttribute('aria-valuenow')).toBe('80');
  });

  it('every button carries an explicit type="button"', async () => {
    render(<GamifyView pushToast={() => {}} />);
    await waitFor(() => expect(screen.getByText('Sparky')).toBeDefined());
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });
});
