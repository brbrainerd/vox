import React, { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Icon } from '../ui/Icons';
import { CommandCatalogEntry } from '../../types/catalog';
import { Agent } from '../../types/dashboard';
import { SearchResponse, UnifiedHit } from '../surfaces/Search/searchHelpers';
import { SURFACE_REGISTRY } from '../../generated/surfaceRegistry.generated';
import { SETTINGS_INDEX } from '../surfaces/Settings/settingsIndex';
import { buildPaletteItems, DocEntryLike } from './paletteSources';

const SEARCH_SEED_KEY = 'vox_search_seed';
const SETTINGS_SEED_KEY = 'vox_settings_seed';

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  onAction: (item: any) => void;
  agents: Agent[];
  skills: CommandCatalogEntry[];
}

function SourceBadge({ source }: { source: string }) {
  return (
    <span className="shrink-0 rounded border border-white/10 bg-white/[0.03] px-1.5 py-px font-mono text-[9px] uppercase tracking-widest text-zinc-500">
      {source}
    </span>
  );
}

export function CommandPalette({ open, onClose, onAction, agents, skills }: CommandPaletteProps) {
  const [q, setQ] = useState('');
  const [backendHits, setBackendHits] = useState<UnifiedHit[]>([]);
  const [backendLoading, setBackendLoading] = useState(false);
  // selectedIdx: index into the combined list of client items + backend hits.
  // We only track keyboard selection for backend hits here (client items handle their own clicks).
  const [selectedBackendIdx, setSelectedBackendIdx] = useState(-1);
  const [docs, setDocs] = useState<DocEntryLike[]>([]);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Lazy-load the docs index once the palette first opens (frontmatter walk).
  useEffect(() => {
    if (!open || docs.length > 0) return;
    invoke<DocEntryLike[]>('vox_docs_index')
      .then(setDocs)
      .catch(() => setDocs([]));
  }, [open, docs.length]);

  useEffect(() => {
    if (!open) {
      setQ('');
      setBackendHits([]);
      setSelectedBackendIdx(-1);
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (!q.trim()) {
      setBackendHits([]);
      setBackendLoading(false);
      return;
    }
    debounceRef.current = setTimeout(async () => {
      setBackendLoading(true);
      try {
        const res = await invoke<SearchResponse>('vox_search_query', { query: q, limit: 8 });
        setBackendHits(res.hits);
        setSelectedBackendIdx(-1);
      } catch {
        setBackendHits([]);
      } finally {
        setBackendLoading(false);
      }
    }, 200);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [q, open]);

  const openHit = useCallback(async (hit: UnifiedHit) => {
    if (hit.locator.kind === 'file' || hit.locator.kind === 'web') {
      try {
        await invoke('open_locator', { locator: hit.locator });
      } catch {
        // swallow — palette is closing
      }
    }
    onClose();
  }, [onClose]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      } else if (e.key === 'ArrowDown' && backendHits.length > 0) {
        e.preventDefault();
        setSelectedBackendIdx(i => Math.min(i + 1, backendHits.length - 1));
      } else if (e.key === 'ArrowUp' && backendHits.length > 0) {
        e.preventDefault();
        setSelectedBackendIdx(i => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' && selectedBackendIdx >= 0) {
        e.preventDefault();
        openHit(backendHits[selectedBackendIdx]);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose, backendHits, selectedBackendIdx, openHit]);

  if (!open) return null;

  const filteredAgents = agents.filter(a =>
    a.codename.toLowerCase().includes(q.toLowerCase()) ||
    a.id.toLowerCase().includes(q.toLowerCase())
  );

  const filteredSkills = skills.filter(s =>
    s.command.toLowerCase().includes(q.toLowerCase()) ||
    s.about.toLowerCase().includes(q.toLowerCase())
  );

  // Federated, client-side sources: windows/surfaces, settings, documentation.
  const fedItems = buildPaletteItems(q, {
    surfaces: SURFACE_REGISTRY,
    settings: SETTINGS_INDEX,
    docs,
  });
  const fedSurfaces = fedItems.filter(i => i.kind === 'surface');
  const fedSettings = fedItems.filter(i => i.kind === 'setting');
  const fedDocs = fedItems.filter(i => i.kind === 'doc');

  const activateFed = (item: (typeof fedItems)[number]) => {
    if (item.kind === 'surface') {
      onAction({ id: 'navigate', viewKey: item.viewKey });
    } else if (item.kind === 'setting') {
      try {
        localStorage.setItem(SETTINGS_SEED_KEY, JSON.stringify({ section: item.targetSection }));
      } catch { /* ignore */ }
      onAction({ id: 'navigate', viewKey: 'settings' });
      // Fire after navigation so an already-mounted SettingsView (no remount when
      // already on Settings) still consumes the seed.
      try { window.dispatchEvent(new Event('vox-settings-seed')); } catch { /* ignore */ }
    } else {
      invoke('open_locator', { locator: { kind: 'file', path: item.path } }).catch(() => {});
    }
    onClose();
  };

  const hasClientResults =
    filteredAgents.length > 0 || filteredSkills.length > 0 || fedItems.length > 0;
  const hasBackendResults = backendHits.length > 0;
  const noResults = q.length > 0 && !hasClientResults && !hasBackendResults && !backendLoading;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 backdrop-blur-sm pt-[14vh]"
      onClick={onClose}
    >
      <div
        className="w-[640px] max-w-[92vw] rounded-2xl border border-white/10 bg-zinc-950/90 shadow-[0_40px_120px_-30px_rgba(0,0,0,0.9)] backdrop-blur-2xl"
        onClick={e => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-white/5 px-4 py-3">
          <Icon.command className="size-4 text-brass" />
          <input
            autoFocus
            value={q}
            onChange={e => setQ(e.target.value)}
            placeholder="Search commands, settings, docs, windows, agents…"
            className="flex-1 bg-transparent text-[14px] text-zinc-100 placeholder:text-zinc-600 outline-none"
          />
          {backendLoading && (
            <div className="size-3 rounded-full border-2 border-brass/20 border-t-brass/80 animate-spin shrink-0" />
          )}
          <kbd className="rounded border border-white/10 bg-white/5 px-1.5 py-0.5 font-mono text-[10px] text-zinc-500">
            esc
          </kbd>
        </div>

        <div className="max-h-[460px] overflow-auto p-2 custom-scrollbar">
          {q.length === 0 && (
            <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-zinc-500 border-b border-white/5 mb-1">
              Quick Actions
            </div>
          )}

          {q.length === 0 && (
            <button
              onClick={() => { onAction({ id: 'submit' }); onClose(); }}
              className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left hover:bg-white/[0.04]"
            >
              <span className="text-[13px] text-zinc-200">Submit new task…</span>
              <span className="font-mono text-[9px] uppercase tracking-widest text-zinc-500">loquela</span>
            </button>
          )}

          {filteredAgents.length > 0 && (
            <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-zinc-500 mt-2">Agents</div>
          )}
          {filteredAgents.map(a => (
            <button
              key={a.id}
              onClick={() => { onAction(a); onClose(); }}
              className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left hover:bg-white/[0.04]"
            >
              <span className="text-[13px] text-zinc-200">{a.codename} ({a.id})</span>
              <span className="font-mono text-[9px] uppercase tracking-widest text-zinc-500">{a.phase}</span>
            </button>
          ))}

          {filteredSkills.length > 0 && (
            <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-zinc-500 mt-2">Skills</div>
          )}
          {filteredSkills.map(s => (
            <button
              key={s.command}
              onClick={() => { onAction(s); onClose(); }}
              className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left hover:bg-white/[0.04]"
            >
              <div className="flex flex-col">
                <span className="text-[13px] text-zinc-200">{s.command}</span>
                <span className="text-[11px] text-zinc-500 truncate max-w-[400px]">{s.about}</span>
              </div>
              <span className="font-mono text-[9px] uppercase tracking-widest text-zinc-500">{s.tier}</span>
            </button>
          ))}

          {/* Windows / surfaces */}
          {fedSurfaces.length > 0 && (
            <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-zinc-500 mt-2">Windows</div>
          )}
          {fedSurfaces.map((item, i) => (
            <button
              key={`surf-${i}`}
              onClick={() => activateFed(item)}
              className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left hover:bg-white/[0.04] focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40"
            >
              <span className="text-[13px] text-zinc-200">{item.label}</span>
              <span className="font-mono text-[9px] uppercase tracking-widest text-zinc-500">{item.detail || 'window'}</span>
            </button>
          ))}

          {/* Settings */}
          {fedSettings.length > 0 && (
            <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-zinc-500 mt-2">Settings</div>
          )}
          {fedSettings.map((item, i) => (
            <button
              key={`set-${i}`}
              onClick={() => activateFed(item)}
              className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left hover:bg-white/[0.04] focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40"
            >
              <div className="flex flex-col min-w-0">
                <span className="text-[13px] text-zinc-200">{item.label}</span>
                <span className="text-[11px] text-zinc-500 truncate max-w-[420px]">{item.detail}</span>
              </div>
              <span className="font-mono text-[9px] uppercase tracking-widest text-zinc-500 shrink-0 ml-2">settings</span>
            </button>
          ))}

          {/* Documentation */}
          {fedDocs.length > 0 && (
            <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-zinc-500 mt-2">Documentation</div>
          )}
          {fedDocs.map((item, i) => (
            <button
              key={`doc-${i}`}
              onClick={() => activateFed(item)}
              className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left hover:bg-white/[0.04] focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40"
            >
              <div className="flex flex-col min-w-0">
                <span className="text-[13px] text-zinc-200 truncate max-w-[440px]">{item.label}</span>
                <span className="text-[11px] text-zinc-500 truncate max-w-[440px]">{item.detail}</span>
              </div>
              <Icon.file className="size-3 text-zinc-500 shrink-0 ml-2" />
            </button>
          ))}

          {/* Backend search results */}
          {hasBackendResults && (
            <>
              <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-zinc-500 mt-2 border-t border-white/5 flex items-center gap-2">
                <Icon.search className="size-3" />
                Search results
              </div>
              {backendHits.map((hit, i) => {
                const displayTitle =
                  hit.title ?? (hit.path ? hit.path.split(/[/\\]/).filter(Boolean).pop() ?? hit.path : hit.snippet.slice(0, 50));
                const isOpenable = hit.locator.kind === 'file' || hit.locator.kind === 'web';
                return (
                  <button
                    key={i}
                    onClick={() => openHit(hit)}
                    className={`flex w-full items-center justify-between rounded-lg px-3 py-2 text-left transition ${
                      i === selectedBackendIdx
                        ? 'bg-brass/[0.08] border border-brass/20'
                        : 'hover:bg-white/[0.04]'
                    }`}
                  >
                    <div className="flex flex-col min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-[13px] text-zinc-200 truncate">{displayTitle}</span>
                        <SourceBadge source={hit.source} />
                      </div>
                      {hit.snippet && (
                        <span className="text-[11px] text-zinc-500 truncate max-w-[440px]">
                          {hit.snippet}
                        </span>
                      )}
                    </div>
                    {isOpenable && (
                      <Icon.link className="size-3 text-zinc-500 shrink-0 ml-2" />
                    )}
                  </button>
                );
              })}

              {/* "See all results" — NOTE: CommandPalette has no setView/onNavigate prop.
                  We write the seed to localStorage and dispatch a custom event that
                  SearchView (mounted in the main panel) listens for on focus/visibility.
                  Since App.tsx's handleCommandAction doesn't route on id:'search',
                  we instead write the seed and let the user navigate manually via the
                  sidebar. This is the no-navigation-prop fallback per spec. */}
              <button
                onClick={() => {
                  try { localStorage.setItem(SEARCH_SEED_KEY, q); } catch { /* ignore */ }
                  onAction({ id: 'search' });
                  onClose();
                }}
                className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left hover:bg-white/[0.04] border-t border-white/5 mt-1"
              >
                <Icon.search className="size-3 text-brass shrink-0" />
                <span className="text-[12px] text-brass">See all results for "{q}"</span>
              </button>
            </>
          )}

          {noResults && (
            <div className="px-3 py-6 text-center text-[12px] text-zinc-500">
              No matches found for "{q}"
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
