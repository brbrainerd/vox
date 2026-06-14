import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { useLocalStorage } from '../../../hooks/useLocalStorage';
import { buildGroupTree, needsAttention, overallWorst, statusForRow } from './policyTree';
import type { PolicyRow, PolicyDetail, PolicyStatus, BranchInfo, RunStatus } from './types';

const STATUS_DOT: Record<RunStatus, string> = {
  fail: 'bg-red-500',
  warn: 'bg-amber-400',
  pass: 'bg-emerald-400',
  not_run: 'bg-zinc-600',
};
const STATUS_GLYPH: Record<RunStatus, string> = { fail: '●', warn: '▲', pass: '✓', not_run: '—' };

function StatusCount({ counts }: { counts: Record<RunStatus, number> }) {
  return (
    <span className="flex items-center gap-1.5 font-mono text-[10px]">
      {(['fail', 'warn', 'pass', 'not_run'] as RunStatus[])
        .filter(s => counts[s] > 0)
        .map(s => (
          <span key={s} className={`flex items-center gap-0.5 ${s === 'fail' ? 'text-red-400' : s === 'warn' ? 'text-amber-300' : s === 'pass' ? 'text-emerald-300' : 'text-zinc-500'}`}>
            {STATUS_GLYPH[s]}{counts[s]}
          </span>
        ))}
    </span>
  );
}

export function PoliciesView({ pushToast }: { pushToast: (t: any) => void }) {
  const [rows, setRows] = useState<PolicyRow[]>([]);
  const [status, setStatus] = useState<PolicyStatus[]>([]);
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [selectedBranches, setSelectedBranches] = useState<string[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<PolicyDetail | null>(null);
  const [railCollapsed, setRailCollapsed] = useLocalStorage<boolean>('vox_policy_rail_collapsed', false);
  const [collapsedGroups, setCollapsedGroups] = useLocalStorage<Record<string, boolean>>('vox_policy_groups', {});

  // Load catalog + branches once.
  useEffect(() => {
    invoke<PolicyRow[]>('policy_list', { domain: null, group: null })
      .then(r => {
        const list = Array.isArray(r) ? r : [];
        setRows(list);
        if (list.length) setSelectedId(prev => prev ?? list[0].id);
      })
      .catch(err => pushToast({ tone: 'warn', title: 'Policy catalog failed', body: String(err) }));
    invoke<BranchInfo[]>('list_branches')
      .then(b => { setBranches(b); setSelectedBranches(b.filter(x => x.isCurrent).map(x => x.branch)); })
      .catch(() => setBranches([]));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Reload status when the branch selection changes.
  useEffect(() => {
    if (selectedBranches.length === 0) { setStatus([]); return; }
    invoke<PolicyStatus[]>('policy_status', { branches: selectedBranches })
      .then(setStatus)
      .catch(() => setStatus([]));
  }, [selectedBranches]);

  // Load detail when the selected rule changes.
  useEffect(() => {
    if (!selectedId) { setDetail(null); return; }
    invoke<PolicyDetail>('policy_show', { id: selectedId })
      .then(setDetail)
      .catch(err => pushToast({ tone: 'warn', title: 'Detail failed', body: String(err) }));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  const tree = useMemo(() => buildGroupTree(rows, status, selectedBranches), [rows, status, selectedBranches]);
  const attention = useMemo(() => needsAttention(rows, status, selectedBranches), [rows, status, selectedBranches]);
  const worst = useMemo(() => overallWorst(rows, status, selectedBranches), [rows, status, selectedBranches]);

  const toggleBranch = (b: string) =>
    setSelectedBranches(prev => prev.includes(b) ? prev.filter(x => x !== b) : [...prev, b]);
  const toggleGroup = (g: string) =>
    setCollapsedGroups(prev => ({ ...prev, [g]: !prev[g] }));

  return (
    <div className="flex gap-4 h-full min-h-0">
      {/* ── SECONDARY: group rail (collapsible, mirrors Sidebar rail widths) ── */}
      <div className="shrink-0 transition-[width] duration-200" style={{ width: railCollapsed ? 56 : 300 }}>
        <Glass className="flex h-full flex-col p-3 gap-3 overflow-hidden">
          <div className="flex items-center justify-between">
            {!railCollapsed && (
              <span className="font-display text-[10px] uppercase tracking-[0.28em] text-zinc-400">
                Policies <span className={`ml-1 inline-block size-1.5 rounded-full align-middle ${STATUS_DOT[worst]}`} />
              </span>
            )}
            <button type="button" onClick={() => setRailCollapsed(c => !c)}
              aria-label={railCollapsed ? 'Expand policy rail' : 'Collapse policy rail'}
              aria-expanded={!railCollapsed}
              title={railCollapsed ? 'Expand' : 'Collapse'}
              className="flex size-6 items-center justify-center rounded-md border border-white/5 text-zinc-400 hover:bg-white/5 hover:text-zinc-100">
              <Icon.chevL aria-hidden="true" className={`size-3 transition-transform ${railCollapsed ? 'rotate-180' : ''}`} />
            </button>
          </div>

          {!railCollapsed && (
            <>
              {/* Multi-branch selector (worktrees) */}
              <div className="flex flex-wrap gap-1">
                {branches.map(b => (
                  <button key={b.branch} type="button" onClick={() => toggleBranch(b.branch)}
                    aria-pressed={selectedBranches.includes(b.branch)}
                    aria-label={`Branch: ${b.branch}`}
                    className={`rounded-full px-2 py-0.5 font-mono text-[9px] border ${selectedBranches.includes(b.branch) ? 'border-brass/40 bg-brass/10 text-brass' : 'border-white/5 text-zinc-500 hover:text-zinc-300'}`}>
                    {b.branch}{b.isCurrent ? ' ◆' : ''}
                  </button>
                ))}
                {branches.length === 0 && <span className="font-mono text-[9px] text-zinc-600">no git worktrees</span>}
              </div>

              {/* ⚠ Needs attention — gracefully shrinks to an all-clear strip */}
              <div className="rounded-lg border border-white/5 bg-white/[0.02] p-2">
                {attention.length === 0 ? (
                  <div className="flex items-center gap-1.5 font-mono text-[10px] text-emerald-300/80">
                    <Icon.check aria-hidden="true" className="size-3" /> all clear
                  </div>
                ) : (
                  <div className="flex flex-col gap-0.5">
                    <span className="font-display text-[9px] uppercase tracking-[0.2em] text-red-300/90">⚠ Needs attention ({attention.length})</span>
                    {attention.map(r => (
                      <button key={r.id} type="button" onClick={() => setSelectedId(r.id)}
                        className="text-left font-mono text-[10px] text-zinc-300 hover:text-red-200 truncate">{r.id}</button>
                    ))}
                  </div>
                )}
              </div>

              {/* Group tree with status-colored counts */}
              <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar flex flex-col gap-1">
                {tree.map(node => {
                  const open = !collapsedGroups[node.group];
                  return (
                    <div key={node.group} className="flex flex-col">
                      <button type="button" onClick={() => toggleGroup(node.group)}
                        aria-expanded={open}
                        aria-label={`${node.group} group`}
                        className="flex items-center justify-between gap-2 rounded-md px-1.5 py-1 hover:bg-white/[0.03]">
                        <span className="flex items-center gap-1.5 min-w-0">
                          <span aria-hidden="true" className={`size-1.5 rounded-full shrink-0 ${STATUS_DOT[node.worst]}`} />
                          <span className="font-display text-[10px] text-zinc-300 truncate">{node.group}</span>
                        </span>
                        <span className="flex items-center gap-1.5 shrink-0">
                          <StatusCount counts={node.counts} />
                          <Icon.chevronDown aria-hidden="true" className={`size-3 text-zinc-600 transition-transform ${open ? '' : '-rotate-90'}`} />
                        </span>
                      </button>
                      {open && node.rows.map(r => {
                        const s = statusForRow(r.id, status, selectedBranches);
                        return (
                          <button key={r.id} type="button" onClick={() => setSelectedId(r.id)}
                            aria-pressed={selectedId === r.id}
                            className={`flex items-center gap-1.5 rounded-md pl-5 pr-1.5 py-1 text-left ${selectedId === r.id ? 'bg-white/[0.05] text-zinc-100' : 'text-zinc-500 hover:bg-white/[0.02] hover:text-zinc-300'}`}>
                            <span aria-hidden="true" className={`size-1 rounded-full shrink-0 ${STATUS_DOT[s]}`} />
                            <span className="font-mono text-[10px] truncate">{r.title}</span>
                          </button>
                        );
                      })}
                    </div>
                  );
                })}
              </div>
            </>
          )}
        </Glass>
      </div>

      {/* ── PRIMARY: rule detail + contents (largest pane) ── */}
      <div className="flex-1 min-w-0">
        <Glass className="flex h-full flex-col p-5 gap-4 overflow-y-auto custom-scrollbar">
          {!detail ? (
            <div className="m-auto font-mono text-xs text-zinc-600">select a policy</div>
          ) : (
            <>
              <header className="flex items-start justify-between gap-4 border-b border-white/5 pb-3">
                <div className="min-w-0">
                  <div className="font-mono text-sm text-zinc-100 truncate">{detail.id}</div>
                  <div className="font-display text-[11px] text-zinc-400">{detail.title}</div>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <button type="button" disabled title="Editing arrives in Phase 3 (read-only now)"
                    className="flex items-center gap-1 rounded-md border border-white/5 px-2 py-1 font-mono text-[10px] text-zinc-600 opacity-50 cursor-not-allowed">
                    ✎ Edit
                  </button>
                  <button type="button" disabled title="Enable/disable arrives in Phase 2 (read-only now)"
                    className="flex items-center gap-1 rounded-md border border-white/5 px-2 py-1 font-mono text-[10px] text-zinc-600 opacity-50 cursor-not-allowed">
                    ⏻ Disable
                  </button>
                </div>
              </header>

              <section className="flex flex-col gap-1.5">
                <div className="font-display text-[9px] uppercase tracking-[0.28em] text-zinc-500">What it does</div>
                <p className="text-xs text-zinc-300 leading-relaxed">{detail.description}</p>
                <div className="flex flex-wrap gap-3 pt-1 font-mono text-[10px] text-zinc-500">
                  <span>domain: <span className="text-zinc-300">{detail.domain}</span></span>
                  <span>severity: <span className="text-zinc-300">{detail.severity ?? '—'}</span></span>
                  <span>{detail.blocking ? 'blocking' : 'non-blocking'}</span>
                  {detail.protected && <span className="text-amber-300/80">protected</span>}
                  <span>runs on: <span className="text-zinc-300">{detail.runsOn.join(', ') || '—'}</span></span>
                  <span>origin: <span className="text-zinc-300">{detail.origin}</span></span>
                </div>
              </section>

              <section className="flex flex-col gap-1.5">
                <div className="font-display text-[9px] uppercase tracking-[0.28em] text-zinc-500">Contents (edit target)</div>
                <div className="rounded-lg border border-white/5 bg-black/30 p-3 font-mono text-[11px] text-zinc-300">
                  <div className="text-zinc-500">kind: <span className="text-zinc-300">{detail.sourceKind}</span></div>
                  <div className="text-zinc-500">source: <span className="text-zinc-300 break-all">{detail.sourceRef}</span></div>
                  {detail.sourceDetail && (
                    <pre className="mt-2 whitespace-pre-wrap break-all text-emerald-200/80">{detail.sourceDetail}</pre>
                  )}
                </div>
                {detail.docs && <a className="font-mono text-[10px] text-brass/80 hover:underline">{detail.docs}</a>}
              </section>

              <section className="flex flex-col gap-1.5">
                <div className="font-display text-[9px] uppercase tracking-[0.28em] text-zinc-500">Last run (per branch)</div>
                <div className="flex flex-wrap gap-2">
                  {selectedBranches.length === 0 && <span className="font-mono text-[10px] text-zinc-600">no branch selected</span>}
                  {selectedBranches.map(b => {
                    const s = statusForRow(detail.id, status, [b]);
                    return (
                      <span key={b} className="flex items-center gap-1.5 rounded-full border border-white/5 px-2 py-0.5 font-mono text-[10px]">
                        <span className={`size-1.5 rounded-full ${STATUS_DOT[s]}`} />
                        <span className="text-zinc-300">{b}</span>
                        <span className="text-zinc-500">{s}</span>
                      </span>
                    );
                  })}
                </div>
              </section>
            </>
          )}
        </Glass>
      </div>
    </div>
  );
}
