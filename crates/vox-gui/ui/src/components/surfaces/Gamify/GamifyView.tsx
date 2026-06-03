import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { LudusProfile } from '../../../lib/ludus';
import { LudusHud } from './LudusHud';

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

export function GamifyView({ pushToast }: GamifyViewProps) {
  const [profile, setProfile] = useState<LudusProfile | null>(null);
  const [notes, setNotes] = useState<LudusNotification[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [p, n] = await Promise.all([
        invoke<LudusProfile>('get_ludus_profile'),
        invoke<LudusNotification[]>('list_ludus_notifications', { limit: 20 }),
      ]);
      setProfile(p);
      setNotes(n);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Ludus load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 15000);
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
        <button onClick={refresh} disabled={loading}
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
                <button onClick={() => ack(n.id)} className="shrink-0 rounded-md border border-white/5 bg-white/[0.03] px-2 py-1 text-[10px] text-zinc-400 hover:text-zinc-100">Ack</button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
