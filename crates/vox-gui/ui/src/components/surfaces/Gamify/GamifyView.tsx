import React, { useCallback, useEffect, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { useLabel } from '../../../hooks/useLanguage';
import { LudusSandbox } from '../../gamify/LudusSandbox';
import { invoke } from '@tauri-apps/api/core';
import { LudusProfile } from '../../../lib/ludus';
import { LudusHud } from './LudusHud';
import { GAMIFY_POLL_MS } from '../../../config/constants';
import type { Toast } from '../../../types/tauri';
import { useIsEmbeddedSurface } from '../../dashboard/EmbeddedSurfaceContext';

interface GamifyViewProps {
  pushToast: (item: Toast) => void;
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
  const embedded = useIsEmbeddedSurface();
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
        invoke<LudusNotification[]>('list_ludus_notifications', { limit: 20 }),
        invoke<LeaderboardEntry[]>('list_gamify_leaderboard', { limit: 10 }),
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
      pushToast({ tone: 'warn', title: 'Ludus load failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    // Embedded mini-render: one initial fetch only, no repeating poll.
    if (embedded) return;
    const id = setInterval(refresh, GAMIFY_POLL_MS);
    return () => clearInterval(id);
  }, [refresh, embedded]);

  const ack = async (id: string) => {
    try {
      await invoke('ack_ludus_notification', { notificationId: id });
      setNotes(curr => curr.filter(x => x.id !== id));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Ack failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  };

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="font-display text-lg text-text-primary tracking-wider uppercase">{useLabel('gamification')}</h2>
        <button type="button" onClick={refresh} disabled={loading}
          className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-xs hover:bg-overlay-subtle">
          {loading ? 'Loading…' : 'Refresh'}
        </button>
      </div>

      {profile ? <LudusHud profile={profile} /> : (
        <div className="rounded-xl border border-border-subtle bg-overlay-subtle p-4 text-sm text-text-muted">No profile yet.</div>
      )}

      <div className="mb-6 border border-border-subtle rounded-xl overflow-hidden bg-bg-base/60 p-4">
        <h3 className="mb-2 font-display text-[12px] uppercase tracking-wide text-text-muted">Simulation Map</h3>
        <div className="h-[560px]">
          <LudusSandbox energy={profile?.energy ?? 0} maxEnergy={profile?.max_energy ?? 0} />
        </div>
      </div>

      <div>
        <div className="mb-2 font-display text-[12px] uppercase tracking-wide text-text-muted">Notifications</div>
        {notes.length === 0 ? (
          <div className="rounded-lg border border-border-subtle bg-overlay-subtle p-3 text-[12px] text-text-muted">No unread notifications.</div>
        ) : (
          <ul className="space-y-2">
            {notes.map(n => (
              <li key={n.id} className="flex items-start justify-between gap-3 rounded-lg border border-border-subtle bg-overlay-subtle p-3">
                <div className="min-w-0">
                  <div className="text-[12px] text-text-secondary">{n.title}</div>
                  <div className="text-[11px] text-text-muted">{n.message}</div>
                </div>
                <button type="button" onClick={() => ack(n.id)} aria-label={`Acknowledge ${n.title}`} className="shrink-0 rounded-md border border-border-subtle bg-overlay-subtle px-2 py-1 text-[10px] text-text-muted hover:text-text-primary">Ack</button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Leaderboard (F3) */}
      <div>
        <div className="mb-2 font-display text-[12px] uppercase tracking-wide text-text-muted">Leaderboard</div>
        {leaderboard.length === 0 ? (
          <div className="rounded-lg border border-border-subtle bg-overlay-subtle p-3 text-[12px] text-text-muted">No ranked players yet.</div>
        ) : (
          <ul className="divide-y divide-white/5 overflow-hidden rounded-lg border border-border-subtle bg-overlay-subtle">
            {leaderboard.map(e => (
              <li key={e.user_id} className="flex items-center gap-3 px-3 py-2 font-mono text-[12px]">
                <span className="w-6 text-right text-text-muted">#{e.rank}</span>
                <span className="min-w-0 flex-1 truncate text-text-secondary">{e.user_id}</span>
                <span className="text-text-muted">L{e.level}</span>
                <span className="w-20 text-right text-cyan">{e.score.toLocaleString()}</span>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Companions (F3) — rendered from server-generated mood SVG */}
      <div>
        <div className="mb-2 font-display text-[12px] uppercase tracking-wide text-text-muted">Companions</div>
        {companions.length === 0 ? (
          <div className="rounded-lg border border-border-subtle bg-overlay-subtle p-3 text-[12px] text-text-muted">No companions yet.</div>
        ) : (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
            {companions.map(c => (
              <div key={c.id} className="rounded-xl border border-border-subtle bg-overlay-subtle p-3">
                <div
                  className="mx-auto mb-2 h-16 w-16 [&>svg]:h-full [&>svg]:w-full"
                  // SVG is generated server-side by vox-gamify::sprite_svg (no external runtime, no user input).
                  dangerouslySetInnerHTML={{ __html: c.svg }}
                />
                <div className="truncate text-center text-[12px] text-text-secondary">{c.name}</div>
                <div className="text-center text-[10px] uppercase tracking-wider text-text-muted">{c.mood} · {c.language}</div>
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
        <div className="mb-2 font-display text-[12px] uppercase tracking-wide text-text-muted">Quests</div>
        {quests.length === 0 ? (
          <div className="rounded-lg border border-border-subtle bg-overlay-subtle p-3 text-[12px] text-text-muted">No active quests.</div>
        ) : (
          <ul className="space-y-2">
            {quests.map(q => (
              <li key={q.id} className="rounded-lg border border-border-subtle bg-overlay-subtle p-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-[12px] text-text-secondary">{q.description}</div>
                    <div className="truncate text-[11px] text-text-muted">{q.hint}</div>
                  </div>
                  <span className={`shrink-0 rounded-sm px-2 py-0.5 font-mono text-[10px] uppercase ${q.completed ? 'bg-emerald-500/15 text-emerald-300' : 'bg-overlay-subtle text-text-muted'}`}>
                    {q.completed ? 'done' : q.status}
                  </span>
                </div>
                <div className="mt-2">
                  <Bar label={`${q.progress}/${q.target}`} value={q.progress} max={q.target} tone="bg-cyan/70" />
                </div>
                <div className="mt-1 font-mono text-[10px] text-text-muted">+{q.xp_reward} XP · +{q.crystal_reward} ◆ · {q.quest_type}</div>
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
      <span className="w-12 shrink-0 font-mono text-[9px] uppercase tracking-wider text-text-muted">{label}</span>
      <div
        className="h-1.5 flex-1 overflow-hidden rounded-full bg-overlay-subtle"
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
