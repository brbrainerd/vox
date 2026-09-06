import { useEffect, useMemo, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { useLocalStorage } from '../../../hooks/useLocalStorage';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';
import { buildGroupTree, needsAttention, overallWorst, statusForRow } from './policyTree';
import { policySetEnabled, policyEdit } from '../../../transport';
import type { PolicyRow, PolicyDetail, PolicyStatus, BranchInfo, RunStatus } from './types';

const STATUS_DOT: Record<RunStatus, string> = {
  fail: 'bg-red-500',
  warn: 'bg-amber-400',
  pass: 'bg-emerald-400',
  not_run: 'bg-text-muted',
};
const STATUS_GLYPH: Record<RunStatus, string> = { fail: '●', warn: '▲', pass: '✓', not_run: '—' };

function StatusCount({ counts }: { counts: Record<RunStatus, number> }) {
  return (
    <span className="flex items-center gap-1.5 font-mono text-[10px]">
      {(['fail', 'warn', 'pass', 'not_run'] as RunStatus[])
        .filter(s => counts[s] > 0)
        .map(s => (
          <span key={s} className={`flex items-center gap-0.5 ${s === 'fail' ? 'text-red-400' : s === 'warn' ? 'text-amber-300' : s === 'pass' ? 'text-emerald-300' : 'text-text-muted'}`}>
            {STATUS_GLYPH[s]}{counts[s]}
          </span>
        ))}
    </span>
  );
}

export function PoliciesView({
  pushToast,
  gamifyEnabled = false,
}: {
  pushToast: (t: any) => void;
  gamifyEnabled?: boolean;
}) {
  const [rows, setRows] = useState<PolicyRow[]>([]);
  const [status, setStatus] = useState<PolicyStatus[]>([]);
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [selectedBranches, setSelectedBranches] = useState<string[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<PolicyDetail | null>(null);
  const [railCollapsed, setRailCollapsed] = useLocalStorage<boolean>('vox_policy_rail_collapsed', false);
  const [collapsedGroups, setCollapsedGroups] = useLocalStorage<Record<string, boolean>>('vox_policy_groups', {});
  const [editOpen, setEditOpen] = useState(false);
  const [editTitle, setEditTitle] = useState('');
  const [editDesc, setEditDesc] = useState('');

  // Load catalog + branches once.
  useEffect(() => {
    invoke<PolicyRow[]>('policy_list', { domain: null, group: null })
      .then(r => {
        const list = Array.isArray(r) ? r : [];
        setRows(list);
        if (list.length) setSelectedId(prev => prev ?? list[0].id);
      })
      .catch(err => pushToast({ tone: 'warn', title: 'Policy catalog failed', body: sanitizeErrorForToast(err) }));
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
    recordGamifyGuiEvent(
      'policy_rule_viewed',
      { rule_id: selectedId },
      { enabled: gamifyEnabled },
    );
    invoke<PolicyDetail>('policy_show', { id: selectedId })
      .then(setDetail)
      .catch(err => pushToast({ tone: 'warn', title: 'Detail failed', body: sanitizeErrorForToast(err) }));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  const tree = useMemo(() => buildGroupTree(rows, status, selectedBranches), [rows, status, selectedBranches]);
  const attention = useMemo(() => needsAttention(rows, status, selectedBranches), [rows, status, selectedBranches]);
  const worst = useMemo(() => overallWorst(rows, status, selectedBranches), [rows, status, selectedBranches]);

  const refresh = () => {
    if (!selectedId) return;
    invoke<PolicyDetail>('policy_show', { id: selectedId })
      .then(setDetail)
      .catch(err => pushToast({ tone: 'warn', title: 'Detail failed', body: sanitizeErrorForToast(err) }));
  };

  const toggleBranch = (b: string) =>
    setSelectedBranches(prev => prev.includes(b) ? prev.filter(x => x !== b) : [...prev, b]);
  const toggleGroup = (g: string) =>
    setCollapsedGroups(prev => ({ ...prev, [g]: !prev[g] }));

  return (
    <div className="flex gap-4 h-full min-h-0">
      {/* ── SECONDARY: group rail (collapsible, mirrors Sidebar rail widths) ── */}
      <nav aria-label="Policy tree" className="shrink-0 transition-[width] duration-200" style={{ width: railCollapsed ? 56 : 300 }}>
        <Glass className="flex h-full flex-col p-3 gap-3 overflow-hidden">
          <div className="flex items-center justify-between">
            {!railCollapsed && (
              <span className="font-display text-[10px] uppercase tracking-[0.28em] text-text-muted">
                Policies <span className={`ml-1 inline-block size-1.5 rounded-full align-middle ${STATUS_DOT[worst]}`} />
              </span>
            )}
            <button type="button" onClick={() => setRailCollapsed(c => !c)}
              aria-label={railCollapsed ? 'Expand policy rail' : 'Collapse policy rail'}
              aria-expanded={!railCollapsed}
              title={railCollapsed ? 'Expand' : 'Collapse'}
              className="flex size-6 items-center justify-center rounded-md border border-border-subtle text-text-muted hover:bg-overlay-subtle hover:text-text-primary">
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
                    className={`rounded-full px-2 py-0.5 font-mono text-[9px] border ${selectedBranches.includes(b.branch) ? 'border-brass/40 bg-brass/10 text-brass' : 'border-border-subtle text-text-muted hover:text-text-secondary'}`}>
                    {b.branch}{b.isCurrent ? ' ◆' : ''}
                  </button>
                ))}
                {branches.length === 0 && <span className="font-mono text-[9px] text-text-muted">no git worktrees</span>}
              </div>

              {/* ⚠ Needs attention — gracefully shrinks to an all-clear strip */}
              <div className="rounded-lg border border-border-subtle bg-overlay-subtle p-2">
                {attention.length === 0 ? (
                  <div className="flex items-center gap-1.5 font-mono text-[10px] text-emerald-300/80">
                    <Icon.check aria-hidden="true" className="size-3" /> all clear
                  </div>
                ) : (
                  <div className="flex flex-col gap-0.5">
                    <span className="font-display text-[9px] uppercase tracking-[0.2em] text-red-300/90">⚠ Needs attention ({attention.length})</span>
                    {attention.map(r => (
                      <button key={r.id} type="button" onClick={() => setSelectedId(r.id)}
                        className="text-left font-mono text-[10px] text-text-secondary hover:text-red-200 truncate">{r.id}</button>
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
                        className="flex items-center justify-between gap-2 rounded-md px-1.5 py-1 hover:bg-overlay-subtle">
                        <span className="flex items-center gap-1.5 min-w-0">
                          <span aria-hidden="true" className={`size-1.5 rounded-full shrink-0 ${STATUS_DOT[node.worst]}`} />
                          <span className="font-display text-[10px] text-text-secondary truncate">{node.group}</span>
                        </span>
                        <span className="flex items-center gap-1.5 shrink-0">
                          <StatusCount counts={node.counts} />
                          <Icon.chevronDown aria-hidden="true" className={`size-3 text-text-muted transition-transform ${open ? '' : '-rotate-90'}`} />
                        </span>
                      </button>
                      {open && node.rows.map(r => {
                        const s = statusForRow(r.id, status, selectedBranches);
                        return (
                          <button key={r.id} type="button" onClick={() => setSelectedId(r.id)}
                            aria-pressed={selectedId === r.id}
                            className={`flex items-center gap-1.5 rounded-md pl-5 pr-1.5 py-1 text-left ${selectedId === r.id ? 'bg-overlay-subtle text-text-primary' : 'text-text-muted hover:bg-overlay-subtle hover:text-text-secondary'}`}>
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
      </nav>

      {/* ── PRIMARY: rule detail + contents (largest pane) ── */}
      <section aria-label="Policy detail" className="flex-1 min-w-0">
        <Glass className="flex h-full flex-col p-5 gap-4 overflow-y-auto custom-scrollbar">
          {!detail ? (
            <div className="m-auto font-mono text-xs text-text-muted">select a policy</div>
          ) : (
            <>
              <header className="flex items-start justify-between gap-4 border-b border-border-subtle pb-3">
                <div className="min-w-0">
                  <div className="font-mono text-sm text-text-primary truncate">{detail.id}</div>
                  <div className="font-display text-[11px] text-text-muted">{detail.title}</div>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <button type="button"
                    disabled={detail.protected}
                    title={detail.protected ? 'Protected policies cannot be edited' : 'Edit title and description'}
                    onClick={() => { setEditTitle(detail.title); setEditDesc(detail.description); setEditOpen(o => !o); }}
                    className={`flex items-center gap-1 rounded-md border border-border-subtle px-2 py-1 font-mono text-[10px] ${detail.protected ? 'text-text-muted opacity-50 cursor-not-allowed' : 'text-text-secondary hover:text-text-primary hover:bg-overlay-subtle'}`}>
                    ✎ Edit
                  </button>
                  <button type="button"
                    disabled={detail.protected}
                    title={detail.protected ? 'Protected policies cannot be toggled' : (detail.enabled ? 'Disable this policy' : 'Enable this policy')}
                    onClick={() => policySetEnabled(detail.id, !detail.enabled).then(refresh)}
                    className={`flex items-center gap-1 rounded-md border border-border-subtle px-2 py-1 font-mono text-[10px] ${detail.protected ? 'text-text-muted opacity-50 cursor-not-allowed' : 'text-text-secondary hover:text-text-primary hover:bg-overlay-subtle'}`}>
                    ⏻ {detail.enabled ? 'Disable' : 'Enable'}
                  </button>
                </div>
              </header>

              {editOpen && !detail.protected && (
                <section aria-label="Edit policy" className="flex flex-col gap-2 rounded-lg border border-border-subtle bg-overlay-subtle p-3">
                  <div className="font-display text-[9px] uppercase tracking-[0.28em] text-text-muted">Edit title / description</div>
                  <input
                    type="text"
                    aria-label="Policy title"
                    value={editTitle}
                    onChange={e => setEditTitle(e.target.value)}
                    className="rounded-sm border border-border-subtle bg-black/30 px-2 py-1 font-mono text-xs text-text-primary focus:outline-hidden focus:ring-1 focus:ring-brass/40"
                  />
                  <textarea
                    aria-label="Policy description"
                    value={editDesc}
                    onChange={e => setEditDesc(e.target.value)}
                    rows={3}
                    className="rounded-sm border border-border-subtle bg-black/30 px-2 py-1 font-mono text-xs text-text-secondary focus:outline-hidden focus:ring-1 focus:ring-brass/40 resize-y"
                  />
                  <div className="flex gap-2">
                    <button type="button"
                      onClick={() => policyEdit(detail.id, editTitle, editDesc).then(() => { setEditOpen(false); refresh(); })}
                      className="rounded-md border border-brass/40 bg-brass/10 px-2 py-1 font-mono text-[10px] text-brass hover:bg-brass/20">
                      Save
                    </button>
                    <button type="button"
                      onClick={() => setEditOpen(false)}
                      className="rounded-md border border-border-subtle px-2 py-1 font-mono text-[10px] text-text-muted hover:text-text-primary">
                      Cancel
                    </button>
                  </div>
                </section>
              )}

              <section className="flex flex-col gap-1.5">
                <div className="font-display text-[9px] uppercase tracking-[0.28em] text-text-muted">What it does</div>
                <p className="text-xs text-text-secondary leading-relaxed">{detail.description}</p>
                <div className="flex flex-wrap gap-3 pt-1 font-mono text-[10px] text-text-muted">
                  <span>domain: <span className="text-text-secondary">{detail.domain}</span></span>
                  <span>severity: <span className="text-text-secondary">{detail.severity ?? '—'}</span></span>
                  <span>{detail.blocking ? 'blocking' : 'non-blocking'}</span>
                  {detail.protected && <span className="text-amber-300/80">protected</span>}
                  <span>runs on: <span className="text-text-secondary">{detail.runsOn.join(', ') || '—'}</span></span>
                  <span>origin: <span className="text-text-secondary">{detail.origin}</span></span>
                </div>
              </section>

              <section className="flex flex-col gap-1.5">
                <div className="font-display text-[9px] uppercase tracking-[0.28em] text-text-muted">Contents (edit target)</div>
                <div className="rounded-lg border border-border-subtle bg-black/30 p-3 font-mono text-[11px] text-text-secondary">
                  <div className="text-text-muted">kind: <span className="text-text-secondary">{detail.sourceKind}</span></div>
                  <div className="text-text-muted">source: <span className="text-text-secondary break-all">{detail.sourceRef}</span></div>
                  {detail.sourceDetail && (
                    <pre className="mt-2 whitespace-pre-wrap break-all text-emerald-200/80">{detail.sourceDetail}</pre>
                  )}
                </div>
                {detail.docs && <a className="font-mono text-[10px] text-brass/80 hover:underline">{detail.docs}</a>}
              </section>

              <section className="flex flex-col gap-1.5">
                <div className="font-display text-[9px] uppercase tracking-[0.28em] text-text-muted">Last run (per branch)</div>
                <div className="flex flex-wrap gap-2">
                  {selectedBranches.length === 0 && <span className="font-mono text-[10px] text-text-muted">no branch selected</span>}
                  {selectedBranches.map(b => {
                    const s = statusForRow(detail.id, status, [b]);
                    return (
                      <span key={b} className="flex items-center gap-1.5 rounded-full border border-border-subtle px-2 py-0.5 font-mono text-[10px]">
                        <span className={`size-1.5 rounded-full ${STATUS_DOT[s]}`} />
                        <span className="text-text-secondary">{b}</span>
                        <span className="text-text-muted">{s}</span>
                      </span>
                    );
                  })}
                </div>
              </section>
            </>
          )}
        </Glass>
      </section>
    </div>
  );
}
