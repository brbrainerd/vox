import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { LudusProfile } from '../../../lib/ludus';
import { LudusHud } from './LudusHud';
import {
  GAMIFY_POLL_MS,
  GAMIFY_NOTIFICATIONS_LIMIT,
  GAMIFY_LEADERBOARD_LIMIT,
} from '../../../config/constants';

interface GamifyViewProps {
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

interface LudusNotification {
  id: string;
  level: 'ok' | 'warn' | 'info';
  title: string;
  message: string;
  created_at: number;
  kind: string;
}

interface LeaderboardEntry {
  rank: number;
  user_id: string;
  level: number;
  score: number;
}

interface Companion {
  id: string;
  name: string;
  description: string | null;
  language: string;
  mood: string;
  health: number;
  max_health: number;
  energy: number;
  max_energy: number;
  code_quality: number;
  last_active: number;
  svg: string;
}

interface Quest {
  id: string;
  quest_type: string;
  description: string;
  hint: string;
  target: number;
  progress: number;
  xp_reward: number;
  crystal_reward: number;
  completed: boolean;
  status: string;
  expires_at: number;
}

export function GamifyView({ pushToast }: GamifyViewProps) {
  const [profile, setProfile] = useState<LudusProfile | null>(null);
  const [notes, setNotes] = useState<LudusNotification[]>([]);
  const [leaderboard, setLeaderboard] = useState<LeaderboardEntry[]>([]);
  const [companions, setCompanions] = useState<Companion[]>([]);
  const [quests, setQuests] = useState<Quest[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [p, n, lb, comp, q] = await Promise.all([
        invoke<LudusProfile>('get_ludus_profile'),
        invoke<LudusNotification[]>('list_ludus_notifications', { limit: GAMIFY_NOTIFICATIONS_LIMIT }),
        invoke<LeaderboardEntry[]>('list_gamify_leaderboard', { limit: GAMIFY_LEADERBOARD_LIMIT }),
        invoke<Companion[]>('list_gamify_companions'),
        invoke<Quest[]>('list_gamify_quests'),
      ]);
      // IPC can resolve to null (command unimplemented, backend error path); coalesce so a
      // null list never reaches `.length` and crashes the whole surface.
      setProfile(p);
      setNotes(n ?? []);
      setLeaderboard(lb ?? []);
      setCompanions(comp ?? []);
      setQuests(q ?? []);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Ludus load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, GAMIFY_POLL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  const ack = async (id: string) => {
    try {
      await invoke('ack_ludus_notification', { notificationId: id });
      setNotes(curr => curr.filter(x => x.id !== id));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Ack failed', body: String(err) });
    }
  };

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Gamification</h2>
        <button type="button" onClick={refresh} disabled={loading}
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs hover:bg-white/[0.06]">
          {loading ? 'Loading…' : 'Refresh'}
        </button>
      </div>

      {profile ? <LudusHud profile={profile} /> : (
        <div className="rounded-xl border border-white/10 bg-white/[0.02] p-4 text-sm text-zinc-500">No profile yet.</div>
      )}

      <div>
        <div className="mb-2 font-display text-[12px] uppercase tracking-wide text-zinc-400">Notifications</div>
        {notes.length === 0 ? (
          <div className="rounded-lg border border-white/5 bg-white/[0.02] p-3 text-[12px] text-zinc-500">No unread notifications.</div>
        ) : (
          <ul className="space-y-2">
            {notes.map(n => (
              <li key={n.id} className="flex items-start justify-between gap-3 rounded-lg border border-white/10 bg-white/[0.02] p-3">
                <div className="min-w-0">
                  <div className="text-[12px] text-zinc-200">{n.title}</div>
                  <div className="text-[11px] text-zinc-500">{n.message}</div>
                </div>
                <button type="button" onClick={() => ack(n.id)} aria-label={`Acknowledge ${n.title}`} className="shrink-0 rounded-md border border-white/5 bg-white/[0.03] px-2 py-1 text-[10px] text-zinc-400 hover:text-zinc-100">Ack</button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Leaderboard (F3) */}
      <div>
        <div className="mb-2 font-display text-[12px] uppercase tracking-wide text-zinc-400">Leaderboard</div>
        {leaderboard.length === 0 ? (
          <div className="rounded-lg border border-white/5 bg-white/[0.02] p-3 text-[12px] text-zinc-500">No ranked players yet.</div>
        ) : (
          <ul className="divide-y divide-white/5 overflow-hidden rounded-lg border border-white/10 bg-white/[0.02]">
            {leaderboard.map(e => (
              <li key={e.user_id} className="flex items-center gap-3 px-3 py-2 font-mono text-[12px]">
                <span className="w-6 text-right text-zinc-500">#{e.rank}</span>
                <span className="min-w-0 flex-1 truncate text-zinc-200">{e.user_id}</span>
                <span className="text-zinc-500">L{e.level}</span>
                <span className="w-20 text-right text-cyan">{e.score.toLocaleString()}</span>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Companions (F3) — rendered from server-generated mood SVG */}
      <div>
        <div className="mb-2 font-display text-[12px] uppercase tracking-wide text-zinc-400">Companions</div>
        {companions.length === 0 ? (
          <div className="rounded-lg border border-white/5 bg-white/[0.02] p-3 text-[12px] text-zinc-500">No companions yet.</div>
        ) : (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
            {companions.map(c => (
              <div key={c.id} className="rounded-xl border border-white/10 bg-white/[0.02] p-3">
                <div
                  className="mx-auto mb-2 h-16 w-16 [&>svg]:h-full [&>svg]:w-full"
                  // SVG is generated server-side by vox-gamify::sprite_svg (no external runtime, no user input).
                  dangerouslySetInnerHTML={{ __html: c.svg }}
                />
                <div className="truncate text-center text-[12px] text-zinc-200">{c.name}</div>
                <div className="text-center text-[10px] uppercase tracking-wider text-zinc-500">{c.mood} · {c.language}</div>
                <div className="mt-2 space-y-1">
                  <Bar label="HP" value={c.health} max={c.max_health} tone="bg-emerald-400/70" />
                  <Bar label="EN" value={c.energy} max={c.max_energy} tone="bg-amber-400/70" />
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Quests (F3) */}
      <div>
        <div className="mb-2 font-display text-[12px] uppercase tracking-wide text-zinc-400">Quests</div>
        {quests.length === 0 ? (
          <div className="rounded-lg border border-white/5 bg-white/[0.02] p-3 text-[12px] text-zinc-500">No active quests.</div>
        ) : (
          <ul className="space-y-2">
            {quests.map(q => (
              <li key={q.id} className="rounded-lg border border-white/10 bg-white/[0.02] p-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-[12px] text-zinc-200">{q.description}</div>
                    <div className="truncate text-[11px] text-zinc-500">{q.hint}</div>
                  </div>
                  <span className={`shrink-0 rounded px-2 py-0.5 font-mono text-[10px] uppercase ${q.completed ? 'bg-emerald-500/15 text-emerald-300' : 'bg-white/5 text-zinc-400'}`}>
                    {q.completed ? 'done' : q.status}
                  </span>
                </div>
                <div className="mt-2">
                  <Bar label={`${q.progress}/${q.target}`} value={q.progress} max={q.target} tone="bg-cyan/70" />
                </div>
                <div className="mt-1 font-mono text-[10px] text-zinc-500">+{q.xp_reward} XP · +{q.crystal_reward} ◆ · {q.quest_type}</div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

/** A compact labeled progress bar. `max <= 0` renders an empty track. */
function Bar({ label, value, max, tone }: { label: string; value: number; max: number; tone: string }) {
  const pct = max > 0 ? Math.max(0, Math.min(100, (value / max) * 100)) : 0;
  return (
    <div className="flex items-center gap-2">
      <span className="w-12 shrink-0 font-mono text-[9px] uppercase tracking-wider text-zinc-500">{label}</span>
      <div
        className="h-1.5 flex-1 overflow-hidden rounded-full bg-white/5"
        role="progressbar"
        aria-label={label}
        aria-valuenow={Math.round(pct)}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div className={`h-full rounded-full ${tone}`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}
