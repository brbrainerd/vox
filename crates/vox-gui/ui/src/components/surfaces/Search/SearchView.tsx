import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { EmptyState } from '../../ui/EmptyState';
import { SurfaceDecoratorProps } from '../decoratorRegistry';
import { SEARCH_TOP_K } from '../../../config/constants';
import {
  ALL_USER_SCOPES,
  backendScopesFromUserScopes,
  filterCommandCatalogHits,
  initialSearchState,
  USER_SCOPE_LABELS,
  type UserScope,
} from '../../../lib/searchController';
import type { CommandCatalog } from '../../../types/catalog';
import {
  scoreToPct,
  groupBySource,
  pathBasename,
  renderHighlights,
  UnifiedHit,
  FacetCount,
  SearchResponse,
} from './searchHelpers';

// Re-export helpers so tests can import from SearchView directly.
export { scoreToPct, groupBySource } from './searchHelpers';

const SEARCH_SEED_KEY = 'vox_search_seed';

function ScopeChip({
  scope,
  active,
  onToggle,
}: {
  scope: UserScope;
  active: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      onClick={onToggle}
      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 font-mono text-[10px] uppercase tracking-widest transition-colors ${
        active
          ? 'border-brass/40 bg-brass/10 text-brass'
          : 'border-white/5 bg-white/[0.01] text-zinc-500 hover:border-white/10 hover:text-zinc-400'
      }`}
    >
      <span className={`size-1.5 rounded-full ${active ? 'bg-brass' : 'bg-white/15'}`} />
      {USER_SCOPE_LABELS[scope]}
    </button>
  );
}

function FacetChip({
  facet,
  active,
  onToggle,
}: {
  facet: FacetCount;
  active: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      onClick={onToggle}
      className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 font-mono text-[10px] transition-colors ${
        active
          ? 'border-brass/40 bg-brass/10 text-brass'
          : 'border-white/5 bg-white/[0.01] text-zinc-500 hover:border-white/10 hover:text-zinc-400'
      }`}
    >
      {facet.value}
      <span className="rounded bg-white/10 px-1 py-px text-[9px]">{facet.count}</span>
    </button>
  );
}

function ScoreBar({ score }: { score: number }) {
  const pct = Math.max(0, Math.min(1, score)) * 100;
  return (
    <div className="flex items-center gap-1.5 shrink-0">
      <div className="h-1 w-12 rounded-full bg-white/[0.06] overflow-hidden">
        <div
          className="h-full rounded-full bg-brass/60 transition-all"
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="font-mono text-[9px] text-zinc-500">{scoreToPct(score)}</span>
    </div>
  );
}

function HighlightedSnippet({ snippet, query }: { snippet: string; query: string }) {
  const segments = renderHighlights(snippet, query);
  return (
    <span>
      {segments.map((seg, i) =>
        seg.mark ? (
          <mark key={i} className="bg-brass/20 text-brass rounded px-0.5">
            {seg.text}
          </mark>
        ) : (
          <span key={i}>{seg.text}</span>
        )
      )}
    </span>
  );
}

function HitRow({
  hit,
  query,
  selected,
  onOpen,
  pushToast,
}: {
  hit: UnifiedHit;
  query: string;
  selected: boolean;
  onOpen: () => void;
  pushToast: (t: any) => void;
}) {
  const displayTitle = hit.title ?? (hit.path ? pathBasename(hit.path) : hit.snippet.slice(0, 40));
  const provenanceStr = hit.provenance.join(' · ');
  const isOpenable = hit.locator.kind === 'file' || hit.locator.kind === 'web';

  const handleClick = async () => {
    if (isOpenable) {
      onOpen();
    } else if (hit.path) {
      try {
        await navigator.clipboard.writeText(hit.path);
        pushToast({ tone: 'ok', title: 'Path copied', body: hit.path });
      } catch {
        // Clipboard unavailable; silently ignore.
      }
    }
  };

  const copyPath = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (hit.path) {
      try {
        await navigator.clipboard.writeText(hit.path);
        pushToast({ tone: 'ok', title: 'Path copied', body: hit.path });
      } catch {
        // silently ignore
      }
    }
  };

  return (
    <div
      onClick={handleClick}
      className={`group flex items-start gap-3 rounded-xl border px-4 py-3 transition cursor-pointer ${
        selected
          ? 'border-brass/40 bg-brass/[0.05]'
          : 'border-white/5 bg-white/[0.02] hover:border-white/10 hover:bg-white/[0.035]'
      }`}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-0.5">
          <span className="font-semibold text-[13px] text-zinc-100 truncate" title={displayTitle}>
            {displayTitle}
          </span>
          <span className="shrink-0 rounded border border-white/8 bg-white/[0.03] px-1.5 py-px font-mono text-[9px] uppercase tracking-widest text-zinc-500">
            {hit.kind}
          </span>
          <span className="shrink-0 rounded border border-white/5 bg-white/[0.02] px-1.5 py-px font-mono text-[9px] text-zinc-600">
            {hit.source}
          </span>
        </div>
        {hit.path && (
          <div className="font-mono text-[10px] text-zinc-600 truncate mb-1" title={hit.path}>
            {hit.path}
          </div>
        )}
        <div className="text-[12px] leading-relaxed text-zinc-400 line-clamp-2">
          <HighlightedSnippet snippet={hit.snippet} query={query} />
        </div>
        {provenanceStr && (
          <div className="mt-1 font-mono text-[9px] text-zinc-600 truncate" title={provenanceStr}>
            {provenanceStr}
          </div>
        )}
      </div>
      <div className="flex flex-col items-end gap-2 shrink-0">
        <ScoreBar score={hit.score} />
        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition">
          {isOpenable && (
            <button
              onClick={e => { e.stopPropagation(); onOpen(); }}
              title={hit.locator.kind === 'web' ? 'Open in browser' : 'Open file'}
              className="rounded p-1 text-zinc-500 hover:text-brass hover:bg-white/[0.04] transition"
            >
              <Icon.link className="size-3" />
            </button>
          )}
          {hit.path && (
            <button
              onClick={copyPath}
              title="Copy path"
              className="rounded p-1 text-zinc-500 hover:text-zinc-300 hover:bg-white/[0.04] transition"
            >
              <Icon.file className="size-3" />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export function SearchView({ pushToast }: SurfaceDecoratorProps) {
  // Seed from CommandPalette navigation.
  const seedQuery = (() => {
    try {
      const v = localStorage.getItem(SEARCH_SEED_KEY) ?? '';
      if (v) localStorage.removeItem(SEARCH_SEED_KEY);
      return v;
    } catch {
      return '';
    }
  })();

  const [query, setQuery] = useState(seedQuery);
  const [debouncedQuery, setDebouncedQuery] = useState(seedQuery);
  const [selectedUserScopes, setSelectedUserScopes] = useState<UserScope[]>([]);
  const [selectedSourceFacets, setSelectedSourceFacets] = useState<string[]>([]);
  const [selectedKinds, setSelectedKinds] = useState<string[]>([]);
  const [pathGlob, setPathGlob] = useState('');
  const [debouncedPathGlob, setDebouncedPathGlob] = useState('');
  const [topK] = useState(SEARCH_TOP_K);
  const [loading, setLoading] = useState(false);
  const [response, setResponse] = useState<SearchResponse | null>(null);
  const [allHits, setAllHits] = useState<UnifiedHit[]>([]);
  const [selectedIdx, setSelectedIdx] = useState<number>(-1);
  const seqRef = useRef(0);

  // Debounce query.
  useEffect(() => {
    const id = setTimeout(() => setDebouncedQuery(query), 250);
    return () => clearTimeout(id);
  }, [query]);

  // Debounce pathGlob.
  useEffect(() => {
    const id = setTimeout(() => setDebouncedPathGlob(pathGlob), 300);
    return () => clearTimeout(id);
  }, [pathGlob]);

  const toggleUserScope = useCallback((scope: UserScope) => {
    setSelectedUserScopes(prev =>
      prev.includes(scope) ? prev.filter(s => s !== scope) : [...prev, scope]
    );
  }, []);

  const toggleSourceFacet = useCallback((source: string) => {
    setSelectedSourceFacets(prev =>
      prev.includes(source) ? prev.filter(s => s !== source) : [...prev, source]
    );
  }, []);

  const toggleKind = useCallback((kind: string) => {
    setSelectedKinds(prev =>
      prev.includes(kind) ? prev.filter(k => k !== kind) : [...prev, kind]
    );
  }, []);

  // Reset accumulated hits + selectedIdx whenever the primary search params change.
  useEffect(() => {
    setAllHits([]);
    setSelectedIdx(-1);
  }, [debouncedQuery, selectedUserScopes, selectedKinds, debouncedPathGlob]);

  const doSearch = useCallback(async (
    q: string,
    userScopes: UserScope[],
    kinds: string[],
    glob: string,
    limit: number,
    offset: number,
    append: boolean,
  ) => {
    if (!q.trim()) {
      setResponse(null);
      setAllHits([]);
      return;
    }
    const effectiveUserScopes =
      userScopes.length > 0 ? userScopes : initialSearchState.scopes;
    const backendScope = backendScopesFromUserScopes(effectiveUserScopes);
    const wantsCommands = effectiveUserScopes.includes('commands');
    const seq = ++seqRef.current;
    setLoading(true);
    try {
      const res = await invoke<SearchResponse>('vox_search_query', {
        query: q,
        scope: backendScope.length > 0 ? backendScope : null,
        kinds: kinds.length > 0 ? kinds : null,
        pathGlob: glob.trim() || null,
        limit,
        offset,
      });
      let mergedHits = res.hits;
      if (wantsCommands && !append) {
        try {
          const catalog = await invoke<CommandCatalog>('get_command_catalog');
          const cmdHits = filterCommandCatalogHits(catalog.entries ?? [], q) as UnifiedHit[];
          mergedHits = [...cmdHits, ...mergedHits];
        } catch {
          // catalog unavailable — backend hits only
        }
      }
      if (seq === seqRef.current) {
        setResponse(res);
        if (append) {
          setAllHits(prev => [...prev, ...mergedHits]);
        } else {
          setAllHits(mergedHits);
        }
      }
    } catch (err) {
      if (seq === seqRef.current) {
        pushToast({ tone: 'warn', title: 'Search failed', body: String(err) });
        setResponse(null);
        if (!append) setAllHits([]);
      }
    } finally {
      if (seq === seqRef.current) {
        setLoading(false);
      }
    }
  }, [pushToast]);

  // Fire initial / filter-changed search (offset=0, replace).
  useEffect(() => {
    doSearch(debouncedQuery, selectedUserScopes, selectedKinds, debouncedPathGlob, topK, 0, false);
  }, [debouncedQuery, selectedUserScopes, selectedKinds, debouncedPathGlob, topK, doSearch]);

  const loadMore = useCallback(() => {
    if (!response?.next_cursor) return;
    doSearch(debouncedQuery, selectedUserScopes, selectedKinds, debouncedPathGlob, topK, response.next_cursor, true);
  }, [response, debouncedQuery, selectedUserScopes, selectedKinds, debouncedPathGlob, topK, doSearch]);

  const displayHits = allHits.filter(h => {
    if (selectedSourceFacets.length > 0 && !selectedSourceFacets.includes(h.source)) return false;
    if (selectedKinds.length > 0 && !selectedKinds.includes(h.kind)) return false;
    return true;
  });

  const openHit = useCallback(async (hit: UnifiedHit) => {
    if (hit.locator.kind === 'file' || hit.locator.kind === 'web') {
      try {
        await invoke('open_locator', { locator: hit.locator });
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Could not open', body: String(err) });
      }
    } else if (hit.locator.kind === 'chat') {
      try {
        localStorage.setItem(SEARCH_SEED_KEY, hit.path ?? '');
      } catch {
        /* ignore */
      }
      pushToast({ tone: 'info', title: 'Chat session', body: 'Open Chat from the sidebar' });
    } else if (hit.locator.kind === 'command' || hit.kind === 'command') {
      try {
        await navigator.clipboard.writeText(hit.path ?? hit.title ?? '');
        pushToast({ tone: 'ok', title: 'Command copied', body: hit.path ?? hit.title ?? '' });
      } catch {
        pushToast({ tone: 'info', title: 'Command', body: hit.path ?? hit.title ?? '' });
      }
    } else if (hit.path) {
      try {
        await navigator.clipboard.writeText(hit.path);
        pushToast({ tone: 'ok', title: 'Path copied', body: hit.path });
      } catch {
        // silently ignore
      }
    }
  }, [pushToast]);

  // Keyboard navigation.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (displayHits.length === 0) return;
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIdx(i => Math.min(i + 1, displayHits.length - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIdx(i => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' && selectedIdx >= 0) {
        e.preventDefault();
        openHit(displayHits[selectedIdx]);
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [displayHits, selectedIdx, openHit]);

  const grouped = displayHits.length > 0 ? groupBySource(displayHits) : null;

  return (
    <div className="flex flex-col gap-5">
      {/* Header */}
      <Glass className="p-4">
        <div className="flex flex-wrap items-center justify-between gap-3 mb-4">
          <div>
            <div className="font-display text-sm tracking-widest text-zinc-200 uppercase">Unified Search</div>
            <div className="text-xs text-zinc-500 mt-1">
              {response
                ? `${response.total} result${response.total !== 1 ? 's' : ''} across ${response.corpora.join(', ')}`
                : 'Search across memory, knowledge, repo, and web'}
            </div>
          </div>
          {/* Path glob filter */}
          <div className="flex items-center gap-2">
            <span className="font-mono text-[10px] uppercase tracking-widest text-zinc-500">Path</span>
            <input
              type="text"
              value={pathGlob}
              onChange={e => setPathGlob(e.target.value)}
              placeholder="**/*.rs"
              className="rounded-lg border border-white/10 bg-white/[0.04] px-2 py-1 font-mono text-[11px] text-zinc-300 outline-none focus:border-brass/40 w-28"
            />
          </div>
        </div>

        {/* Search input */}
        <div className="relative flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.04] px-3 py-2.5 focus-within:border-brass/40 transition-colors">
          <Icon.search className="size-4 shrink-0 text-brass" />
          <input
            autoFocus
            type="text"
            value={query}
            onChange={e => setQuery(e.target.value)}
            placeholder="Search everything…"
            className="flex-1 bg-transparent text-[14px] text-zinc-100 placeholder:text-zinc-600 outline-none"
          />
          {loading && (
            <div className="size-4 shrink-0 rounded-full border-2 border-brass/20 border-t-brass/80 animate-spin" />
          )}
          {!loading && query && (
            <button
              onClick={() => {
                setQuery('');
                setDebouncedQuery('');
                setResponse(null);
                setAllHits([]);
              }}
              className="text-zinc-500 hover:text-zinc-300 transition"
            >
              <Icon.x className="size-4" />
            </button>
          )}
        </div>

        {/* Scope chips */}
        <div className="mt-3 flex flex-wrap gap-2">
          {ALL_USER_SCOPES.map(scope => (
            <ScopeChip
              key={scope}
              scope={scope}
              active={selectedUserScopes.includes(scope)}
              onToggle={() => toggleUserScope(scope)}
            />
          ))}
          {selectedUserScopes.length > 0 && (
            <button
              onClick={() => setSelectedUserScopes([])}
              className="font-mono text-[9px] uppercase tracking-widest text-zinc-600 hover:text-zinc-400 transition"
            >
              clear
            </button>
          )}
        </div>
      </Glass>

      {/* Facet sidebar + results layout */}
      <div className="flex gap-4 items-start">
        {/* Facet sidebar — only when we have facets */}
        {response && (response.facets_by_source.length > 0 || response.facets_by_kind.length > 0) && (
          <div className="w-44 shrink-0 flex flex-col gap-3">
            {response.facets_by_source.length > 0 && (
              <Glass className="p-3">
                <div className="font-mono text-[9px] uppercase tracking-widest text-zinc-500 mb-2">Source</div>
                <div className="flex flex-col gap-1">
                  {response.facets_by_source.map(f => (
                    <FacetChip
                      key={f.value}
                      facet={f}
                      active={selectedSourceFacets.includes(f.value)}
                      onToggle={() => toggleSourceFacet(f.value)}
                    />
                  ))}
                </div>
              </Glass>
            )}
            {response.facets_by_kind.length > 0 && (
              <Glass className="p-3">
                <div className="font-mono text-[9px] uppercase tracking-widest text-zinc-500 mb-2">Kind</div>
                <div className="flex flex-col gap-1">
                  {response.facets_by_kind.map(f => (
                    <FacetChip
                      key={f.value}
                      facet={f}
                      active={selectedKinds.includes(f.value)}
                      onToggle={() => toggleKind(f.value)}
                    />
                  ))}
                </div>
              </Glass>
            )}
          </div>
        )}

        {/* Results column */}
        <div className="flex-1 min-w-0 flex flex-col gap-5">
          {/* Empty / loading / no-results states */}
          {!query.trim() && (
            <Glass className="p-4">
              <EmptyState
                icon={<Icon.search className="size-8" />}
                title="Search the workspace"
                description="Code · docs · chats · commands · memory · web — toggle scopes in the sidebar."
              />
            </Glass>
          )}

          {query.trim() && loading && displayHits.length === 0 && (
            <Glass className="p-8 text-center text-zinc-500 text-sm">Searching…</Glass>
          )}

          {query.trim() && !loading && response && displayHits.length === 0 && (
            <Glass className="p-4">
              <EmptyState
                icon={<Icon.search className="size-8" />}
                title={`No results for "${query}"`}
                description={
                  selectedUserScopes.length > 0 || selectedSourceFacets.length > 0 || selectedKinds.length > 0
                    ? 'Try clearing filters to search all corpora.'
                    : 'Broaden the query or switch scopes.'
                }
              />
            </Glass>
          )}

          {grouped && grouped.size > 0 && (
            <div className="flex flex-col gap-5">
              {Array.from(grouped.entries()).map(([source, hits]) => (
                <section key={source}>
                  <div className="mb-2 flex items-center gap-2">
                    <div className="font-display text-[11px] tracking-[0.2em] uppercase text-zinc-400">
                      {source}
                    </div>
                    <span className="font-mono text-[9px] text-zinc-600">
                      {hits.length} hit{hits.length !== 1 ? 's' : ''}
                    </span>
                  </div>
                  <div className="flex flex-col gap-2">
                    {hits.map((hit, i) => {
                      const globalIdx = displayHits.indexOf(hit);
                      return (
                        <HitRow
                          key={`${source}-${i}`}
                          hit={hit}
                          query={debouncedQuery}
                          selected={globalIdx === selectedIdx}
                          onOpen={() => openHit(hit)}
                          pushToast={pushToast}
                        />
                      );
                    })}
                  </div>
                </section>
              ))}
            </div>
          )}

          {/* Load more */}
          {response?.next_cursor != null && (
            <div className="flex justify-center">
              <button
                onClick={loadMore}
                disabled={loading}
                className="rounded-lg border border-white/10 bg-white/[0.04] px-5 py-2 font-mono text-[11px] text-zinc-300 hover:border-brass/30 hover:text-brass transition disabled:opacity-50"
              >
                {loading ? 'Loading…' : 'Load more'}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
