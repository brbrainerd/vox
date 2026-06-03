import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { SurfaceDecoratorProps } from '../decoratorRegistry';
import { scoreToPct, groupBySource, pathBasename, UnifiedHit } from './searchHelpers';

// Re-export helpers so tests can import from SearchView directly.
export { scoreToPct, groupBySource } from './searchHelpers';

interface SearchResponse {
  hits: UnifiedHit[];
  total: number;
  corpora: string[];
}

const ALL_SCOPES = ['memory', 'knowledge', 'chunk', 'repo', 'web'] as const;
type Scope = typeof ALL_SCOPES[number];

const SCOPE_LABELS: Record<Scope, string> = {
  memory: 'Memory',
  knowledge: 'Knowledge',
  chunk: 'Chunk',
  repo: 'Repo',
  web: 'Web',
};

function ScopeChip({
  scope,
  active,
  onToggle,
}: {
  scope: Scope;
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
      {SCOPE_LABELS[scope]}
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

function HitRow({ hit }: { hit: UnifiedHit }) {
  const displayTitle = hit.title ?? (hit.path ? pathBasename(hit.path) : hit.snippet.slice(0, 40));
  const provenanceStr = hit.provenance.join(' · ');

  const copyPath = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (hit.path) {
      try {
        await navigator.clipboard.writeText(hit.path);
      } catch {
        // Clipboard unavailable (e.g. Tauri sandbox); silently ignore.
      }
    }
  };

  return (
    <div className="group flex items-start gap-3 rounded-xl border border-white/5 bg-white/[0.02] px-4 py-3 hover:border-white/10 hover:bg-white/[0.035] transition">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-0.5">
          <span className="font-semibold text-[13px] text-zinc-100 truncate" title={displayTitle}>
            {displayTitle}
          </span>
          <span className="shrink-0 rounded border border-white/8 bg-white/[0.03] px-1.5 py-px font-mono text-[9px] uppercase tracking-widest text-zinc-500">
            {hit.kind}
          </span>
        </div>
        {hit.path && (
          <div className="font-mono text-[10px] text-zinc-600 truncate mb-1" title={hit.path}>
            {hit.path}
          </div>
        )}
        <div className="text-[12px] leading-relaxed text-zinc-400 line-clamp-2">
          {hit.snippet}
        </div>
        {provenanceStr && (
          <div className="mt-1 font-mono text-[9px] text-zinc-600 truncate" title={provenanceStr}>
            {provenanceStr}
          </div>
        )}
      </div>
      <div className="flex flex-col items-end gap-2 shrink-0">
        <ScoreBar score={hit.score} />
        {hit.path && (
          <button
            onClick={copyPath}
            title="Copy path"
            className="opacity-0 group-hover:opacity-100 rounded p-1 text-zinc-500 hover:text-zinc-300 hover:bg-white/[0.04] transition"
          >
            <Icon.file className="size-3" />
          </button>
        )}
      </div>
    </div>
  );
}

export function SearchView({ pushToast }: SurfaceDecoratorProps) {
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [selectedScopes, setSelectedScopes] = useState<Scope[]>([]);
  const [topK, setTopK] = useState(30);
  const [loading, setLoading] = useState(false);
  const [response, setResponse] = useState<SearchResponse | null>(null);
  const seqRef = useRef(0);

  // Debounce: update debouncedQuery ~250ms after typing stops.
  useEffect(() => {
    const id = setTimeout(() => setDebouncedQuery(query), 250);
    return () => clearTimeout(id);
  }, [query]);

  const toggleScope = useCallback((scope: Scope) => {
    setSelectedScopes(prev =>
      prev.includes(scope) ? prev.filter(s => s !== scope) : [...prev, scope]
    );
  }, []);

  const doSearch = useCallback(async (q: string, scopes: Scope[], limit: number) => {
    if (!q.trim()) {
      setResponse(null);
      return;
    }
    const seq = ++seqRef.current;
    setLoading(true);
    try {
      const res = await invoke<SearchResponse>('vox_search_query', {
        query: q,
        scope: scopes.length > 0 ? scopes : null,
        limit,
      });
      // Drop stale responses: only apply if this is still the latest request.
      if (seq === seqRef.current) {
        setResponse(res);
      }
    } catch (err) {
      if (seq === seqRef.current) {
        pushToast({ tone: 'warn', title: 'Search failed', body: String(err) });
        setResponse(null);
      }
    } finally {
      if (seq === seqRef.current) {
        setLoading(false);
      }
    }
  }, [pushToast]);

  useEffect(() => {
    doSearch(debouncedQuery, selectedScopes, topK);
  }, [debouncedQuery, selectedScopes, topK, doSearch]);

  const grouped = response ? groupBySource(response.hits) : null;

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
          {/* Top-K control */}
          <div className="flex items-center gap-2">
            <span className="font-mono text-[10px] uppercase tracking-widest text-zinc-500">Top</span>
            <select
              value={topK}
              onChange={e => setTopK(Number(e.target.value))}
              className="rounded-lg border border-white/10 bg-white/[0.04] px-2 py-1 font-mono text-[11px] text-zinc-300 outline-none focus:border-brass/40"
            >
              {[10, 20, 30, 50, 100].map(n => (
                <option key={n} value={n}>{n}</option>
              ))}
            </select>
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
              onClick={() => { setQuery(''); setDebouncedQuery(''); setResponse(null); }}
              className="text-zinc-500 hover:text-zinc-300 transition"
            >
              <Icon.x className="size-4" />
            </button>
          )}
        </div>

        {/* Scope chips */}
        <div className="mt-3 flex flex-wrap gap-2">
          {ALL_SCOPES.map(scope => (
            <ScopeChip
              key={scope}
              scope={scope}
              active={selectedScopes.includes(scope)}
              onToggle={() => toggleScope(scope)}
            />
          ))}
          {selectedScopes.length > 0 && (
            <button
              onClick={() => setSelectedScopes([])}
              className="font-mono text-[9px] uppercase tracking-widest text-zinc-600 hover:text-zinc-400 transition"
            >
              clear
            </button>
          )}
        </div>
      </Glass>

      {/* Empty / loading / results */}
      {!query.trim() && (
        <Glass className="p-10 text-center">
          <Icon.search className="mx-auto size-8 text-zinc-700 mb-3" />
          <div className="text-sm text-zinc-500">Type to search across all vox corpora</div>
          <div className="mt-1 font-mono text-[10px] text-zinc-600">memory · knowledge · chunk · repo · web</div>
        </Glass>
      )}

      {query.trim() && loading && !response && (
        <Glass className="p-8 text-center text-zinc-500 text-sm">Searching…</Glass>
      )}

      {query.trim() && !loading && response && response.hits.length === 0 && (
        <Glass className="p-10 text-center">
          <div className="text-sm text-zinc-500">No results for "{query}"</div>
          {selectedScopes.length > 0 && (
            <div className="mt-1 font-mono text-[10px] text-zinc-600">
              Try clearing scope filters to search all corpora
            </div>
          )}
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
                {hits.map((hit, i) => (
                  <HitRow key={`${source}-${i}`} hit={hit} />
                ))}
              </div>
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
