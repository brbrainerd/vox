// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AchievementsDrawer } from './AchievementsDrawer';
import type { LudusProfile } from '../../lib/ludus';

const mockProfile: LudusProfile = {
  user_id: 'u1',
  level: 5,
  xp: 250,
  xp_to_next_level: 750,
  xp_progress: 0.25,
  total_xp_earned: 1250,
  crystals: 3,
  lumens: 10,
  energy: 8,
  max_energy: 10,
  current_streak: 2,
  prestige_level: 1,
  title: 'Adept',
  full_title: 'Adept Operator',
  trust_tier: 'trusted',
};

describe('AchievementsDrawer', () => {
  it('has role="dialog" and aria-label "Achievements" when open', () => {
    render(
      <AchievementsDrawer
        open
        onClose={vi.fn()}
        profile={mockProfile}
        onManageInSettings={vi.fn()}
      />,
    );
    expect(screen.getByRole('dialog', { name: 'Achievements' })).toBeInTheDocument();
  });

  it('shows XP and level from profile when open', () => {
    render(
      <AchievementsDrawer
        open
        onClose={vi.fn()}
        profile={mockProfile}
        onManageInSettings={vi.fn()}
      />,
    );
    const dialog = screen.getByRole('dialog', { name: 'Achievements' });
    expect(dialog).toHaveTextContent(/lv\s*5/i);
    expect(dialog).toHaveTextContent(/250\s*xp/i);
  });

  it('shows Manage in Settings button without duplicating settings form', async () => {
    const onManage = vi.fn();
    const user = userEvent.setup();
    render(
      <AchievementsDrawer
        open
        onClose={vi.fn()}
        profile={mockProfile}
        onManageInSettings={onManage}
      />,
    );
    const btn = screen.getByRole('button', { name: /manage in settings/i });
    expect(btn).toBeInTheDocument();
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
    await user.click(btn);
    expect(onManage).toHaveBeenCalledTimes(1);
  });

  it('does not render when closed', () => {
    render(
      <AchievementsDrawer
        open={false}
        onClose={vi.fn()}
        profile={mockProfile}
        onManageInSettings={vi.fn()}
      />,
    );
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});
