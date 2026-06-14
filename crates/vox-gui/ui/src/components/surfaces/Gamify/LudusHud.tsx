import React from 'react';
import { LudusProfile, xpBarPct } from '../../../lib/ludus';

export function LudusHud({ profile }: { profile: LudusProfile }) {
  return (
    <div className="rounded-xl border border-white/10 bg-white/[0.02] p-4">
      <div className="flex items-baseline justify-between">
        <div className="font-display text-sm tracking-wider text-brass uppercase">{profile.full_title}</div>
        <div className="font-mono text-[11px] text-zinc-500">Lv {profile.level} · prestige {profile.prestige_level}</div>
      </div>
      <div
        className="mt-3 h-2 overflow-hidden rounded-full bg-white/[0.05]"
        role="progressbar"
        aria-label="XP progress"
        aria-valuenow={Math.round(Math.max(0, Math.min(1, profile.xp_progress)) * 100)}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div className="h-full rounded-full bg-brass/70 transition-all" style={{ width: xpBarPct(profile.xp_progress) }} />
      </div>
      <div className="mt-1 flex justify-between font-mono text-[10px] text-zinc-500">
        <span>{profile.xp} XP</span>
        <span>{profile.xp_to_next_level} to next</span>
      </div>
      <div className="mt-3 grid grid-cols-2 gap-2 text-[11px] sm:grid-cols-4">
        <Stat label="Crystals" value={`${profile.crystals} 💎`} />
        <Stat label="Lumens" value={`${profile.lumens}`} />
        <Stat label="Energy" value={`${profile.energy}/${profile.max_energy}`} />
        <Stat label="Streak" value={`${profile.current_streak} 🔥`} />
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg bg-white/[0.02] px-2 py-1.5 ring-1 ring-white/5">
      <div className="text-[9px] uppercase tracking-wide text-zinc-500">{label}</div>
      <div className="font-mono text-zinc-200">{value}</div>
    </div>
  );
}
