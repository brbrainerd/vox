import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useLudusStore } from '../../gamify/store';
import { invoke } from '@tauri-apps/api/core';
import { voxTransport } from '../../../transport';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { EmptyState } from '../../ui/EmptyState';
import { Skeleton } from '../../ui/Skeleton';
import { SurfaceDecoratorProps } from '../decoratorRegistry';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';
import {
  ALL_USER_SCOPES,
  filterCommandCatalogHits,
  filterSettingsIndexHits,
  initialSearchState,
  USER_SCOPE_LABELS,
  type UserScope,
} from '../../../lib/searchController';
import { useSearchController } from '../../../hooks/useSearchController';
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
const SETTINGS_SEED_KEY = 'vox_settings_seed';

function facetCounts(hits: UnifiedHit[], field: 'source' | 'kind'): FacetCount[] {
  const counts = new Map<string, number>();
  for (const hit of hits) {
    const value = field === 'source' ? hit.source : hit.kind;
    counts.set(value, (counts.get(value) ?? 0) + 1);
  }
  return [...counts.entries()].map(([value, count]) => ({ value, count }));
}

/**
 * Match `path` against a glob pattern.
 *
 * Rules:
 * - `**` matches any sequence of characters including path separators (`/`)
 * - `*`  matches any sequence of characters NOT including path separators
 * - `?`  matches exactly one character that is not a path separator
 * - All other characters match literally (dot, plus, parens, etc. are NOT regex wildcards here)
 *
 * Exported so unit tests can import it directly from this module.
 */
export function pathMatchesGlob(path: string | null, glob: string): boolean {
  const pattern = glob.trim();
  if (!pattern) return true;
  if (!path) return false;

  // Build regex source by processing ** and * in separate passes.
  //
  // We split on '**' first (double-star = match anything including '/'),
  // then within each segment we split on '*' (single-star = match non-slash chars).
  // Between steps we regex-escape any literal special characters.
  const escapeLiteral = (s: string) => s.replace(/[.+^${}()|[\]\\]/g, '\\$&');

  const regexSource = pattern
    .split('**')
    .map(segment =>
      segment
        .split('*')
        .map(part => escapeLiteral(part))
        .join('[^/]*')    // single * → zero-or-more non-separator chars
    )
    .join('.*');          // double ** → zero-or-more of anything (including /)

  // Replace ? placeholders with [^/] (one non-separator char)
  const finalSource = regexSource.replace(/\?/g, '[^/]');

  try {
    return new RegExp(`^${finalSource}$`).test(path);
  } catch {
    // Malformed pattern — fall back to safe substring match
    return path.includes(pattern);
  }
}

function SearchResultsSkeleton() {
  return (
    <Glass className="p-4 flex flex-col gap-2" role="status" aria-label="Searching">
      {[0, 1, 2].map(i => (
        <div key={i} data-testid="search-skeleton">
          <Skeleton className="h-16 w-full" />
        </div>
      ))}
    </Glass>
  );
}

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
      type="button"
      onClick={onToggle}
      aria-pressed={active}
      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 font-mono text-[10px] uppercase tracking-widest transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40 ${
        active
          ? 'border-brass/40 bg-brass/10 text-brass'
          : 'border-border-subtle bg-overlay-subtle text-text-muted hover:border-border-subtle hover:text-text-muted'
      }`}
    >
      <span aria-hidden="true" className={`size-1.5 rounded-full ${active ? 'bg-brass' : 'bg-white/15'}`} />
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
      type="button"
      onClick={onToggle}
      aria-pressed={active}
      className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 font-mono text-[10px] transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40 ${
        active
          ? 'border-brass/40 bg-brass/10 text-brass'
          : 'border-border-subtle bg-overlay-subtle text-text-muted hover:border-border-subtle hover:text-text-muted'
      }`}
    >
      {facet.value}
      <span className="rounded bg-overlay-subtle px-1 py-px text-[9px]">{facet.count}</span>
    </button>
  );
}

function ScoreBar({ score }: { score: number }) {
  const pct = Math.max(0, Math.min(1, score)) * 100;
  return (
    <div className="flex items-center gap-1.5 shrink-0">
      <div className="h-1 w-12 rounded-full bg-overlay-subtle overflow-hidden">
        <div
          className="h-full rounded-full bg-brass/60 transition-all"
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="font-mono text-[9px] text-text-muted">{scoreToPct(score)}</span>
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
    if (hit.path) {
      useLudusStore.getState().setFocusedFile(hit.path);
    }
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
          : 'border-border-subtle bg-overlay-subtle hover:border-border-subtle hover:bg-overlay-subtle'
      }`}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-0.5">
          <span className="font-semibold text-[13px] text-text-primary truncate" title={displayTitle}>
            {displayTitle}
          </span>
          <span className="shrink-0 rounded border border-white/8 bg-overlay-subtle px-1.5 py-px font-mono text-[9px] uppercase tracking-widest text-text-muted">
            {hit.kind}
          </span>
          <span className="shrink-0 rounded border border-border-subtle bg-overlay-subtle px-1.5 py-px font-mono text-[9px] text-text-muted">
            {hit.source}
          </span>
        </div>
        {hit.path && (
          <div className="font-mono text-[10px] text-text-muted truncate mb-1" title={hit.path}>
            {hit.path}
          </div>
        )}
        <div className="text-[12px] leading-relaxed text-text-muted line-clamp-2">
          <HighlightedSnippet snippet={hit.snippet} query={query} />
        </div>
        {provenanceStr && (
          <div className="mt-1 font-mono text-[9px] text-text-muted truncate" title={provenanceStr}>
            {provenanceStr}
          </div>
        )}
      </div>
      <div className="flex flex-col items-end gap-2 shrink-0">
        <ScoreBar score={hit.score} />
        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition">
          {isOpenable && (
            <button
              type="button"
              onClick={e => { e.stopPropagation(); onOpen(); }}
              title={hit.locator.kind === 'web' ? 'Open in browser' : 'Open file'}
              aria-label={hit.locator.kind === 'web' ? 'Open in browser' : 'Open file'}
              className="rounded p-1 text-text-muted hover:text-brass hover:bg-overlay-subtle transition"
            >
              <Icon.link className="size-3" aria-hidden="true" />
            </button>
          )}
          {hit.path && (
            <button
              type="button"
              onClick={copyPath}
              title="Copy path"
              aria-label="Copy path"
              className="rounded p-1 text-text-muted hover:text-text-secondary hover:bg-overlay-subtle transition"
            >
              <Icon.file className="size-3" aria-hidden="true" />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export function SearchView({ pushToast, gamifyEnabled = false }: SurfaceDecoratorProps) {
  const lastRecordedQueryRef = useRef('');
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

  const { state: searchState, setQuery: setSearchQuery, setScopes: setSearchScopes } =
    useSearchController();

  const [selectedUserScopes, setSelectedUserScopes] = useState<UserScope[]>([]);
  const [selectedSourceFacets, setSelectedSourceFacets] = useState<string[]>([]);
  const [selectedKinds, setSelectedKinds] = useState<string[]>([]);
  const [pathGlob, setPathGlob] = useState('');
  const [debouncedPathGlob, setDebouncedPathGlob] = useState('');
  const [response, setResponse] = useState<SearchResponse | null>(null);
  const [allHits, setAllHits] = useState<UnifiedHit[]>([]);
  const [selectedIdx, setSelectedIdx] = useState<number>(-1);

  useEffect(() => {
    if (seedQuery) setSearchQuery(seedQuery);
  }, [seedQuery, setSearchQuery]);

  useEffect(() => {
    const scopes = selectedUserScopes.length > 0 ? selectedUserScopes : initialSearchState.scopes;
    setSearchScopes(scopes);
  }, [selectedUserScopes, setSearchScopes]);

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

  // Reset selectedIdx whenever the primary search params change.
  useEffect(() => {
    setAllHits([]);
    setSelectedIdx(-1);
  }, [searchState.query, selectedUserScopes, selectedKinds, debouncedPathGlob]);

  useEffect(() => {
    let cancelled = false;
    const q = searchState.query.trim();
    if (!q) {
      setResponse(null);
      setAllHits([]);
      return;
    }

    async function mergeHits() {
      let merged = searchState.hits as UnifiedHit[];
      const scopes = selectedUserScopes.length > 0 ? selectedUserScopes : initialSearchState.scopes;
      if (scopes.includes('commands') && !searchState.loading) {
        try {
          const catalog = await invoke<CommandCatalog>('get_command_catalog');
          const cmdHits = filterCommandCatalogHits(catalog.entries ?? [], q) as UnifiedHit[];
          merged = [...cmdHits, ...merged];
        } catch {
          // catalog unavailable — backend hits only
        }
      }
      if (scopes.includes('settings') && !searchState.loading) {
        const settingsHits = filterSettingsIndexHits(q) as UnifiedHit[];
        merged = [...settingsHits, ...merged];
      }
      if (cancelled) return;
      setAllHits(merged);
      setResponse({
        hits: merged,
        facets_by_source: facetCounts(merged, 'source'),
        facets_by_kind: facetCounts(merged, 'kind'),
        total: merged.length,
        next_cursor: null,
        corpora: [...new Set(merged.map(h => h.source))],
        repo_truncated: searchState.repoTruncated,
      });

      if (searchState.repoTruncated) {
        pushToast({
          tone: 'warn',
          title: 'Repo scan truncated',
          body: `Results show the first ${(20_000).toLocaleString()} repo files. Narrow your search or use a path glob to find files in deep directories.`,
        });
      }
    }

    void mergeHits();
    return () => {
      cancelled = true;
    };
  }, [searchState.hits, searchState.query, searchState.loading, selectedUserScopes]);

  useEffect(() => {
    const q = searchState.query.trim();
    if (!q || searchState.loading) return;
    if (lastRecordedQueryRef.current === q) return;
    lastRecordedQueryRef.current = q;
    recordGamifyGuiEvent('search_query_executed', { query: q }, { enabled: gamifyEnabled });
  }, [searchState.query, searchState.loading, gamifyEnabled]);

  const loading = searchState.loading;
  const query = searchState.query;
  const displayHits = allHits.filter(h => {
    if (selectedSourceFacets.length > 0 && !selectedSourceFacets.includes(h.source)) return false;
    if (selectedKinds.length > 0 && !selectedKinds.includes(h.kind)) return false;
    if (!pathMatchesGlob(h.path, debouncedPathGlob)) return false;
    return true;
  });

  const openHit = useCallback(async (hit: UnifiedHit) => {
    if (hit.locator.kind === 'file' || hit.locator.kind === 'web') {
      try {
        await voxTransport.openLocator(hit.locator);
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
    } else if (hit.locator.kind === 'setting' || hit.kind === 'setting') {
      try {
        localStorage.setItem(SETTINGS_SEED_KEY, hit.locator.value);
        window.dispatchEvent(new Event('vox-settings-seed'));
      } catch {
        /* ignore */
      }
      pushToast({ tone: 'info', title: 'Settings', body: 'Open Settings from the sidebar' });
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
        setSelectedIdx(i => (i + 1) % displayHits.length);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIdx(i => (i <= 0 ? displayHits.length - 1 : i - 1));
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
            <div className="font-display text-sm tracking-widest text-text-secondary uppercase">Unified Search</div>
            <div
              className="text-xs text-text-muted mt-1"
              aria-live="polite"
              aria-atomic="true"
            >
              {response
                ? `${response.total} result${response.total !== 1 ? 's' : ''} across ${response.corpora.join(', ')}`
                : 'Search across memory, knowledge, repo, and web'}
            </div>
          </div>
          {/* Path glob filter */}
          <div className="flex items-center gap-2">
            <span className="font-mono text-[10px] uppercase tracking-widest text-text-muted">Path</span>
            <input
              type="text"
              value={pathGlob}
              onChange={e => setPathGlob(e.target.value)}
              placeholder="**/*.rs"
              aria-label="Filter results by path glob"
              className="rounded-lg border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[11px] text-text-secondary outline-none focus:border-brass/40 w-28"
            />
          </div>
        </div>

        {/* Search input */}
        <div className="relative flex items-center gap-2 rounded-xl border border-border-subtle bg-overlay-subtle px-3 py-2.5 focus-within:border-brass/40 transition-colors">
          <Icon.search className="size-4 shrink-0 text-brass" aria-hidden="true" />
          <input
            autoFocus
            type="text"
            value={query}
            onChange={e => setSearchQuery(e.target.value)}
            placeholder="Search everything…"
            aria-label="Search query"
            className="flex-1 bg-transparent text-[14px] text-text-primary placeholder:text-text-muted outline-none"
          />
          {loading && (
            <div className="size-4 shrink-0 rounded-full border-2 border-brass/20 border-t-brass/80 animate-spin" />
          )}
          {!loading && query && (
            <button
              type="button"
              aria-label="Clear search"
              onClick={() => {
                setSearchQuery('');
                setResponse(null);
                setAllHits([]);
              }}
              className="text-text-muted hover:text-text-secondary transition"
            >
              <Icon.x className="size-4" aria-hidden="true" />
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
              type="button"
              onClick={() => setSelectedUserScopes([])}
              className="font-mono text-[9px] uppercase tracking-widest text-text-muted hover:text-text-muted transition"
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
                <div className="font-mono text-[9px] uppercase tracking-widest text-text-muted mb-2">Source</div>
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
                <div className="font-mono text-[9px] uppercase tracking-widest text-text-muted mb-2">Kind</div>
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

          {query.trim() && loading && displayHits.length === 0 && <SearchResultsSkeleton />}

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
                    <div className="font-display text-[11px] tracking-[0.2em] uppercase text-text-muted">
                      {source}
                    </div>
                    <span className="font-mono text-[9px] text-text-muted">
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
                          query={query}
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
        </div>
      </div>
    </div>
  );
}
