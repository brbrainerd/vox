import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { voxTransport } from '../../transport';
import { Icon } from '../ui/Icons';
import { CommandCatalogEntry } from '../../types/catalog';
import { Agent } from '../../types/dashboard';
import type { CommandPaletteAction } from '../../types/tauri';
import { UnifiedHit } from '../surfaces/Search/searchHelpers';
import { parsePaletteQuery, type PalettePrefixMode } from './paletteSources';
import {
  type FederatedIndexEntry,
  type FederatedIndexKind,
} from '../../lib/federatedSearchIndex';
import { useSearchController } from '../../hooks/useSearchController';
import { useFederatedSearchIndex } from '../../hooks/useFederatedSearchIndex';
import { viewKeyForLocator } from '../../lib/locatorNavigation';
import { recordGamifyGuiEvent } from '../../lib/gamifyGuiEvents';

const SEARCH_SEED_KEY = 'vox_search_seed';
const SETTINGS_SEED_KEY = 'vox_settings_seed';

const FED_KIND_SECTION_ORDER: FederatedIndexKind[] = [
  'surface',
  'setting',
  'doc',
  'policy',
  'command',
  'action',
  'skill',
];

const FED_KIND_LABELS: Record<FederatedIndexKind, string> = {
  surface: 'Windows',
  setting: 'Settings',
  doc: 'Documentation',
  policy: 'Policies',
  command: 'Commands',
  action: 'Actions',
  skill: 'Skills',
};

function federatedKindsForMode(mode: PalettePrefixMode): FederatedIndexKind[] {
  switch (mode) {
    case 'skills':
      return ['doc', 'skill'];
    case 'commands':
      return ['command', 'action'];
    case 'agents':
      return [];
    default:
      return ['surface', 'setting', 'policy', 'command', 'action', 'doc'];
  }
}

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  onAction: (item: CommandPaletteAction) => void;
  agents: Agent[];
  skills: CommandCatalogEntry[];
  gamifyEnabled?: boolean;
}

function SourceBadge({ source }: { source: string }) {
  return (
    <span className="shrink-0 rounded border border-border-subtle bg-overlay-subtle px-1.5 py-px font-mono text-[9px] uppercase tracking-widest text-text-muted">
      {source}
    </span>
  );
}

export function CommandPalette({ open, onClose, onAction, agents, skills, gamifyEnabled = false }: CommandPaletteProps) {
  const [q, setQ] = useState('');
  const [selectedRowIdx, setSelectedRowIdx] = useState(-1);

  const { mode: prefixMode, query: effectiveQ } = parsePaletteQuery(q);
  const fedKinds = useMemo(() => federatedKindsForMode(prefixMode), [prefixMode]);

  const backendSearchEnabled = open && prefixMode === 'default';
  const { state: searchState, setQuery: setSearchQuery } = useSearchController({
    enabled: backendSearchEnabled,
  });
  const backendHits = useMemo(
    () => (backendSearchEnabled ? (searchState.hits as UnifiedHit[]) : []),
    [searchState.hits, backendSearchEnabled],
  );
  const backendLoading = backendSearchEnabled && searchState.loading;

  const skillSources = useMemo(
    () =>
      skills.map(s => ({
        id: s.capability_id ?? s.command,
        name: s.command,
        description: s.about,
      })),
    [skills],
  );

  const { search: searchFederated } = useFederatedSearchIndex(skillSources);

  const fedHits = useMemo(
    () =>
      effectiveQ.trim() && fedKinds.length > 0
        ? searchFederated(effectiveQ, { kinds: fedKinds })
        : [],
    [searchFederated, effectiveQ, fedKinds],
  );

  const orderedFedHits = useMemo(() => {
    const byKind = new Map<FederatedIndexKind, FederatedIndexEntry[]>();
    for (const hit of fedHits) {
      const list = byKind.get(hit.kind) ?? [];
      list.push(hit);
      byKind.set(hit.kind, list);
    }
    const ordered: FederatedIndexEntry[] = [];
    for (const kind of FED_KIND_SECTION_ORDER) {
      if (!fedKinds.includes(kind)) continue;
      ordered.push(...(byKind.get(kind) ?? []));
    }
    return ordered;
  }, [fedHits, fedKinds]);

  const activateFedEntry = useCallback(
    (entry: FederatedIndexEntry) => {
      recordGamifyGuiEvent(
        'palette_navigation',
        { kind: entry.kind, label: entry.label },
        { enabled: gamifyEnabled },
      );
      switch (entry.payload.type) {
        case 'surface':
          onAction({ id: 'navigate', type: 'navigate', viewKey: entry.payload.viewKey });
          break;
        case 'setting':
          try {
            localStorage.setItem(
              SETTINGS_SEED_KEY,
              JSON.stringify({
                section: entry.payload.section,
                settingId: entry.payload.settingId,
              }),
            );
          } catch {
            /* ignore */
          }
          onAction({ id: 'navigate', type: 'navigate', viewKey: 'settings' });
          try {
            window.dispatchEvent(new Event('vox-settings-seed'));
          } catch {
            /* ignore */
          }
          break;
        case 'policy':
          onAction({ id: 'navigate', type: 'navigate', viewKey: 'policies' });
          break;
        case 'doc':
          voxTransport.openLocator({ kind: 'file', value: entry.payload.path }).catch(() => {});
          break;
        case 'skill':
          onAction({ id: `skill:${entry.payload.skillId}` });
          break;
        case 'command':
          onAction({
            id: 'hit',
            type: 'hit',
            locator: { kind: 'command', value: entry.payload.command },
            viewKey: 'catalog',
          });
          break;
        case 'action':
          onAction({
            id: 'hit',
            type: 'hit',
            locator: { kind: 'command', value: entry.payload.actionId },
            viewKey: 'console',
            label: entry.label,
          });
          break;
      }
      onClose();
    },
    [onAction, onClose, gamifyEnabled],
  );

  const filteredAgents = useMemo(
    () =>
      prefixMode === 'default' || prefixMode === 'agents'
        ? agents.filter(
            a =>
              a.codename.toLowerCase().includes(effectiveQ.toLowerCase()) ||
              a.id.toLowerCase().includes(effectiveQ.toLowerCase()),
          )
        : [],
    [agents, effectiveQ, prefixMode],
  );

  const filteredSkills = useMemo(
    () =>
      prefixMode === 'default' || prefixMode === 'skills' || prefixMode === 'commands'
        ? skills.filter(
            s =>
              s.command.toLowerCase().includes(effectiveQ.toLowerCase()) ||
              s.about.toLowerCase().includes(effectiveQ.toLowerCase()),
          )
        : [],
    [skills, effectiveQ, prefixMode],
  );

  useEffect(() => {
    if (!open) {
      setQ('');
      setSearchQuery('');
      setSelectedRowIdx(-1);
    }
  }, [open, setSearchQuery]);

  useEffect(() => {
    setSelectedRowIdx(-1);
  }, [searchState.hits, q]);

  const openHit = useCallback(async (hit: UnifiedHit) => {
    recordGamifyGuiEvent(
      'palette_navigation',
      { kind: 'search_hit', source: hit.source },
      { enabled: gamifyEnabled },
    );
    const viewKey = viewKeyForLocator(hit.locator);
    if (hit.locator.kind === 'file' || hit.locator.kind === 'web') {
      try {
        await voxTransport.openLocator(hit.locator);
      } catch {
        // swallow — palette is closing
      }
    }
    onAction({ id: 'hit', type: 'hit', locator: hit.locator, viewKey });
    onClose();
  }, [onAction, onClose, gamifyEnabled]);

  const selectableRows = useMemo(() => {
    const rows: Array<() => void> = [];
    filteredAgents.forEach(a => rows.push(() => { onAction(a); onClose(); }));
    filteredSkills.forEach(s => rows.push(() => { onAction(s); onClose(); }));
    orderedFedHits.forEach(entry => rows.push(() => activateFedEntry(entry)));
    backendHits.forEach(hit => rows.push(() => openHit(hit)));
    return rows;
  }, [
    filteredAgents,
    filteredSkills,
    orderedFedHits,
    backendHits,
    onAction,
    onClose,
    openHit,
    activateFedEntry,
  ]);

  const selectableRowsRef = useRef(selectableRows);
  const selectedRowIdxRef = useRef(selectedRowIdx);
  selectableRowsRef.current = selectableRows;
  selectedRowIdxRef.current = selectedRowIdx;

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      const rows = selectableRowsRef.current;
      let idx = selectedRowIdxRef.current;
      if (e.key === 'Escape') {
        onClose();
      } else if (e.key === 'ArrowDown' && rows.length > 0) {
        e.preventDefault();
        idx = idx < 0 ? 0 : Math.min(idx + 1, rows.length - 1);
        selectedRowIdxRef.current = idx;
        setSelectedRowIdx(idx);
      } else if (e.key === 'ArrowUp' && rows.length > 0) {
        e.preventDefault();
        idx = idx < 0 ? rows.length - 1 : Math.max(idx - 1, 0);
        selectedRowIdxRef.current = idx;
        setSelectedRowIdx(idx);
      } else if (e.key === 'Enter' && idx >= 0 && idx < rows.length) {
        e.preventDefault();
        rows[idx]();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  const fedHitsByKind = useMemo(() => {
    const grouped = new Map<FederatedIndexKind, FederatedIndexEntry[]>();
    for (const hit of orderedFedHits) {
      const list = grouped.get(hit.kind) ?? [];
      list.push(hit);
      grouped.set(hit.kind, list);
    }
    return grouped;
  }, [orderedFedHits]);

  if (!open) return null;

  const hasClientResults =
    filteredAgents.length > 0 ||
    filteredSkills.length > 0 ||
    orderedFedHits.length > 0;
  const hasBackendResults = backendHits.length > 0;
  const noResults = q.length > 0 && !hasClientResults && !hasBackendResults && !backendLoading;

  let rowOffset = 0;
  const rowSelected = (idx: number) => idx === selectedRowIdx;
  const rowClass = (idx: number, base = 'hover:bg-overlay-subtle') =>
    `flex w-full items-center justify-between rounded-lg px-3 py-2 text-left transition ${
      rowSelected(idx) ? 'bg-brass/[0.08] border border-brass/20' : base
    }`;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 backdrop-blur-sm pt-[14vh]"
      onClick={onClose}
    >
      <div
        className="w-[640px] max-w-[92vw] rounded-2xl border border-border-subtle bg-bg-base/90 shadow-[0_40px_120px_-30px_rgba(0,0,0,0.9)] backdrop-blur-2xl"
        onClick={e => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-border-subtle px-4 py-3">
          <Icon.command className="size-4 text-brass" />
          <input
            autoFocus
            value={q}
            onChange={e => {
              const v = e.target.value;
              setQ(v);
              setSearchQuery(v);
            }}
            placeholder="Search commands, settings, docs, windows, agents…"
            className="flex-1 bg-transparent text-[14px] text-text-primary placeholder:text-text-muted outline-none"
          />
          {backendLoading && (
            <div className="size-3 rounded-full border-2 border-brass/20 border-t-brass/80 animate-spin shrink-0" />
          )}
          <kbd className="rounded border border-border-subtle bg-overlay-subtle px-1.5 py-0.5 font-mono text-[10px] text-text-muted">
            esc
          </kbd>
        </div>

        <div className="max-h-[460px] overflow-auto p-2 custom-scrollbar">
          {q.length === 0 && (
            <div
              data-testid="palette-prefix-legend"
              className="px-3 py-2 text-[11px] text-text-muted border-b border-border-subtle mb-1"
            >
              {'> commands · @ agents · / docs+skills'}
            </div>
          )}

          {q.length === 0 && (
            <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-text-muted border-b border-border-subtle mb-1">
              Quick Actions
            </div>
          )}

          {q.length === 0 && (
            <button
              onClick={() => { onAction({ id: 'submit' }); onClose(); }}
              className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left hover:bg-overlay-subtle"
            >
              <span className="text-[13px] text-text-secondary">Submit new task…</span>
              <span className="font-mono text-[9px] uppercase tracking-widest text-text-muted">loquela</span>
            </button>
          )}

          {filteredAgents.length > 0 && (
            <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-text-muted mt-2">Agents</div>
          )}
          {filteredAgents.map(a => {
            const idx = rowOffset++;
            return (
              <button
                key={a.id}
                onClick={() => { onAction(a); onClose(); }}
                className={rowClass(idx)}
              >
                <span className="text-[13px] text-text-secondary">{a.codename} ({a.id})</span>
                <span className="font-mono text-[9px] uppercase tracking-widest text-text-muted">{a.phase}</span>
              </button>
            );
          })}

          {filteredSkills.length > 0 && (
            <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-text-muted mt-2">Skills</div>
          )}
          {filteredSkills.map(s => {
            const idx = rowOffset++;
            return (
              <button
                key={s.command}
                onClick={() => { onAction(s); onClose(); }}
                className={rowClass(idx)}
              >
                <div className="flex flex-col">
                  <span className="text-[13px] text-text-secondary">{s.command}</span>
                  <span className="text-[11px] text-text-muted truncate max-w-[400px]">{s.about}</span>
                </div>
                <span className="font-mono text-[9px] uppercase tracking-widest text-text-muted">{s.tier}</span>
              </button>
            );
          })}

          {FED_KIND_SECTION_ORDER.map(kind => {
            const items = fedHitsByKind.get(kind);
            if (!items?.length) return null;
            return (
              <React.Fragment key={kind}>
                <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-text-muted mt-2">
                  {FED_KIND_LABELS[kind]}
                </div>
                {items.map((item, i) => {
                  const idx = rowOffset++;
                  return (
                    <button
                      key={`${kind}-${item.id}-${i}`}
                      onClick={() => activateFedEntry(item)}
                      className={rowClass(idx)}
                    >
                      <div className="flex flex-col min-w-0">
                        <span className="text-[13px] text-text-secondary truncate max-w-[440px]">
                          {item.label}
                        </span>
                        {item.detail ? (
                          <span className="text-[11px] text-text-muted truncate max-w-[440px]">
                            {item.detail}
                          </span>
                        ) : null}
                      </div>
                      <span className="font-mono text-[9px] uppercase tracking-widest text-text-muted shrink-0 ml-2">
                        {kind === 'surface' ? item.detail || 'window' : kind}
                      </span>
                    </button>
                  );
                })}
              </React.Fragment>
            );
          })}

          {/* Backend search results */}
          {hasBackendResults && (
            <>
              <div className="px-3 pt-2 pb-1.5 mt-2 mb-1 text-[10px] uppercase tracking-widest text-text-muted border-b border-border-subtle flex items-center gap-2">
                <Icon.search className="size-3" />
                Search results
              </div>
              {backendHits.map((hit, i) => {
                const idx = rowOffset++;
                const displayTitle =
                  hit.title ?? (hit.path ? hit.path.split(/[/\\]/).filter(Boolean).pop() ?? hit.path : hit.snippet.slice(0, 50));
                const isOpenable = hit.locator.kind === 'file' || hit.locator.kind === 'web';
                return (
                  <button
                    key={i}
                    onClick={() => openHit(hit)}
                    className={rowClass(idx)}
                  >
                    <div className="flex flex-col min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-[13px] text-text-secondary truncate">{displayTitle}</span>
                        <SourceBadge source={hit.source} />
                      </div>
                      {hit.snippet && (
                        <span className="text-[11px] text-text-muted truncate max-w-[440px]">
                          {hit.snippet}
                        </span>
                      )}
                    </div>
                    {isOpenable && (
                      <Icon.link className="size-3 text-text-muted shrink-0 ml-2" />
                    )}
                  </button>
                );
              })}

              <button
                onClick={() => {
                  try { localStorage.setItem(SEARCH_SEED_KEY, q); } catch { /* ignore */ }
                  onAction({ id: 'search' });
                  onClose();
                }}
                className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left hover:bg-overlay-subtle border-t border-border-subtle mt-1"
              >
                <Icon.search className="size-3 text-brass shrink-0" />
                <span className="text-[12px] text-brass">See all results for "{q}"</span>
              </button>
            </>
          )}

          {noResults && (
            <div className="px-3 py-6 text-center text-[12px] text-text-muted">
              No matches found for "{q}"
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
