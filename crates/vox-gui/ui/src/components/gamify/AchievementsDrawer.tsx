import React, { useEffect } from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import type { LudusProfile } from '../../lib/ludus';
import { LudusHud } from '../surfaces/Gamify/LudusHud';

export interface AchievementsDrawerProps {
  open: boolean;
  onClose: () => void;
  profile: LudusProfile | null;
  onManageInSettings: () => void;
}

export function AchievementsDrawer({
  open,
  onClose,
  profile,
  onManageInSettings,
}: AchievementsDrawerProps) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <button
        type="button"
        aria-label="Close achievements overlay"
        className="flex-1 bg-black/50"
        onClick={onClose}
      />
      <Glass
        role="dialog"
        aria-label="Achievements"
        aria-modal="true"
        className="flex h-full w-full max-w-md flex-col rounded-none border-l border-white/10 shadow-2xl"
        inset={false}
      >
        <header className="flex items-center justify-between border-b border-white/5 px-5 py-4">
          <div>
            <h2 className="font-display text-sm uppercase tracking-[0.2em] text-zinc-100">
              Achievements
            </h2>
            {profile && (
              <p className="mt-1 font-mono text-[11px] text-zinc-500">
                Lv {profile.level} · {profile.xp} XP
              </p>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close achievements"
            className="rounded-md border border-white/10 p-1.5 text-zinc-400 hover:text-zinc-100"
          >
            <Icon.x className="size-4" aria-hidden="true" />
          </button>
        </header>

        <div className="flex-1 overflow-y-auto p-5 space-y-4">
          {profile ? (
            <LudusHud profile={profile} />
          ) : (
            <div className="rounded-xl border border-white/10 bg-white/[0.02] p-4 text-sm text-zinc-500">
              No profile yet — complete tasks to earn XP.
            </div>
          )}
        </div>

        <footer className="border-t border-white/5 p-4">
          <button
            type="button"
            onClick={onManageInSettings}
            className="w-full rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-xs text-zinc-300 hover:bg-white/[0.06] hover:text-brass transition"
          >
            Manage in Settings
          </button>
        </footer>
      </Glass>
    </div>
  );
}
