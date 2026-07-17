import React, { useCallback, useEffect, useState } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { codeRabbitPlan, codeRabbitReport, codeRabbitRunAsync, codeRabbitTokenPresent } from '../../../transport';
import type { Toast } from '../../../types/tauri';

interface CodeRabbitViewProps {
  pushToast: (t: Toast) => void;
  gamifyEnabled?: boolean;
}

interface Chunk {
  order: number;
  name: string;
  files: string[];
}
interface Manifest {
  baseline_branch?: string;
  total_files?: number;
  chunks?: Chunk[];
}
interface RunChunk {
  name: string;
  pr_number?: number | null;
  status?: string;
}
interface Report {
  run_state?: { chunks?: RunChunk[] } | null;
  db_status?: any;
}

const PILL: Record<string, { bg: string; fg: string }> = {
  completed: { bg: 'var(--bg-success, #E1F5EE)', fg: 'var(--text-success, #0F6E56)' },
  failed: { bg: 'var(--bg-danger, #FCEBEB)', fg: 'var(--text-danger, #A32D2D)' },
  pending: { bg: 'var(--surface-1, #f1efe8)', fg: 'var(--text-muted, #888780)' },
};

/** Merge the planned manifest with run-state statuses into display rows. */
export function toSliceRows(manifest: Manifest | null, report: Report | null) {
  const statusByName = new Map<string, RunChunk>();
  for (const c of report?.run_state?.chunks ?? []) statusByName.set(c.name, c);
  return (manifest?.chunks ?? []).map((c) => {
    const rs = statusByName.get(c.name);
    return {
      name: c.name,
      files: c.files?.length ?? 0,
      status: rs?.status ?? 'planned',
      pr: rs?.pr_number ?? null,
    };
  });
}

export function CodeRabbitView({ pushToast }: CodeRabbitViewProps): React.ReactElement {
  const [since, setSince] = useState('2026-04-01');
  const [fullRepo, setFullRepo] = useState(false);
  const [cap, setCap] = useState(150);
  const [weights, setWeights] = useState('1,1,1');
  // Empty = review everything selected (no truncation). A number keeps only the
  // top-N most important files — never default this on for a full-repo sweep.
  const [top, setTop] = useState('');
  const [manifest, setManifest] = useState<Manifest | null>(null);
  const [report, setReport] = useState<Report | null>(null);
  const [tokenOk, setTokenOk] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState(false);

  useEffect(() => {
    codeRabbitTokenPresent().then(setTokenOk).catch(() => setTokenOk(false));
    codeRabbitReport<Report>().then(setReport).catch(() => {});
  }, []);

  useEffect(() => {
    let un: UnlistenFn | undefined;
    let cancelled = false;
    listen<{ status: string; error?: string }>('coderabbit://progress', (e) => {
      setRunning(false);
      pushToast(
        e.payload.status === 'error'
          ? { tone: 'warn', title: 'CodeRabbit run failed', body: e.payload.error, cause: 'backend-error' }
          : { tone: 'ok', title: 'CodeRabbit run finished', cause: 'backend-ok' },
      );
      codeRabbitReport<Report>().then(setReport).catch(() => {});
    }).then((u) => {
      // If we unmounted before listen() resolved, unlisten immediately (no leak).
      if (cancelled) u();
      else un = u;
    }).catch(() => { /* event bridge unavailable — no progress toasts */ });
    return () => {
      cancelled = true;
      un?.();
    };
  }, [pushToast]);

  // Same top-N for plan and run so the preview matches what actually executes.
  const topN = top.trim() ? Math.max(1, Math.trunc(Number(top))) : null;

  const plan = useCallback(async () => {
    setBusy(true);
    try {
      const m = await codeRabbitPlan<Manifest>({ since, cap, rankWeights: weights, top: topN, fullRepo });
      setManifest(m);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Plan failed', body: String(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  }, [since, cap, weights, topN, fullRepo, pushToast]);

  const run = useCallback(async () => {
    setRunning(true);
    try {
      await codeRabbitRunAsync({ since, cap, rankWeights: weights, top: topN, fullRepo });
      pushToast({ tone: 'info', title: 'CodeRabbit sweep started', cause: 'backend-ok' });
    } catch (err) {
      setRunning(false);
      pushToast({ tone: 'warn', title: 'Run failed', body: String(err), cause: 'backend-error' });
    }
  }, [since, cap, weights, topN, fullRepo, pushToast]);

  const rows = toSliceRows(manifest, report);
  const totalFiles = rows.reduce((a, r) => a + r.files, 0);

  return (
    <div style={{ padding: '1rem 1.25rem', maxWidth: 920 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: '1rem' }}>
        <h2 style={{ fontSize: 18, fontWeight: 500, margin: 0 }}>CodeRabbit review</h2>
        <span style={{ background: 'var(--bg-pro, #EEEDFE)', color: 'var(--text-pro, #534AB7)', fontSize: 12, padding: '3px 10px', borderRadius: 20 }}>
          Pro · 150 files · 5/hr
        </span>
        <span style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--text-muted, #888780)' }}>
          token: {tokenOk == null ? '…' : tokenOk ? 'present ✓' : 'absent'}
        </span>
      </div>

      <div style={{ display: 'flex', gap: 14, flexWrap: 'wrap', alignItems: 'end', marginBottom: '1rem' }}>
        <label style={{ fontSize: 13 }}>
          <div style={{ color: 'var(--text-secondary, #5F5E5A)', marginBottom: 4 }}>Scope</div>
          <label style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
            <input type="checkbox" checked={fullRepo} onChange={(e) => setFullRepo(e.target.checked)} />
            Full repo
          </label>
        </label>
        <label style={{ fontSize: 13 }}>
          <div style={{ color: 'var(--text-secondary, #5F5E5A)', marginBottom: 4 }}>Modified since</div>
          <input type="date" value={since} disabled={fullRepo} onChange={(e) => setSince(e.target.value)} />
        </label>
        <label style={{ fontSize: 13 }}>
          <div style={{ color: 'var(--text-secondary, #5F5E5A)', marginBottom: 4 }}>Max files / PR</div>
          <input type="number" min={1} max={150} value={cap} onChange={(e) => { const n = Number(e.target.value); setCap(Number.isFinite(n) ? Math.max(1, Math.min(150, Math.trunc(n))) : 1); }} style={{ width: 90 }} />
        </label>
        <label style={{ fontSize: 13 }}>
          <div style={{ color: 'var(--text-secondary, #5F5E5A)', marginBottom: 4 }}>Top N (blank = all)</div>
          <input type="number" min={1} value={top} placeholder="all" onChange={(e) => setTop(e.target.value)} style={{ width: 90 }} />
        </label>
        <label style={{ fontSize: 13 }}>
          <div style={{ color: 'var(--text-secondary, #5F5E5A)', marginBottom: 4 }}>Rank weights (r,c,g)</div>
          <input type="text" value={weights} onChange={(e) => setWeights(e.target.value)} style={{ width: 110 }} />
        </label>
        <button onClick={plan} disabled={busy || (!fullRepo && !since)}>{busy ? 'Planning…' : 'Plan sweep'}</button>
        <button onClick={run} disabled={running || !rows.length}>{running ? 'Running…' : 'Run'}</button>
      </div>

      <div style={{ fontSize: 13, color: 'var(--text-secondary, #5F5E5A)', marginBottom: 8 }}>
        {rows.length
          ? `Planned ${rows.length} PRs · ${totalFiles} files · est ~${Math.ceil(rows.length / 5)}h at 5/hr`
          : 'No plan yet — pick a date (or Full repo) and click “Plan sweep”.'}
      </div>

      {rows.length > 0 && (
        <div style={{ border: '0.5px solid var(--border, #d3d1c7)', borderRadius: 8, overflow: 'hidden' }}>
          {rows.map((r, i) => {
            const pill = PILL[r.status] ?? PILL.pending;
            return (
              <div key={r.name} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 14px', borderTop: i ? '0.5px solid var(--border, #d3d1c7)' : 'none' }}>
                <span style={{ fontFamily: 'var(--font-mono, monospace)', fontSize: 13, flex: 1 }}>{r.name}</span>
                <span style={{ fontSize: 12, color: 'var(--text-muted, #888780)', width: 70, textAlign: 'right' }}>{r.files} files</span>
                {r.pr ? <span style={{ fontSize: 12, color: 'var(--text-muted, #888780)' }}>#{r.pr}</span> : <span style={{ width: 28 }} />}
                <span style={{ background: pill.bg, color: pill.fg, fontSize: 11, padding: '3px 9px', borderRadius: 20, width: 84, textAlign: 'center' }}>{r.status}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
