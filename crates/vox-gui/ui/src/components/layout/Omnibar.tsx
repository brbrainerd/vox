import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { voxTransport } from '../../transport';
import { Icon } from '../ui/Icons';
import { CommandCatalogEntry } from '../../types/catalog';
import { Agent } from '../../types/dashboard';
import { UnifiedHit } from '../surfaces/Search/searchHelpers';
import { parsePaletteQuery, type PalettePrefixMode } from './paletteSources';
import type { FederatedIndexKind } from '../../lib/federatedSearchIndex';
import { useSearchController } from '../../hooks/useSearchController';
import { useFederatedSearchIndex } from '../../hooks/useFederatedSearchIndex';
import { useContentManifest } from '../../hooks/useContentManifest';
import { querySearchableRegistry } from '../../lib/searchableRegistry';
import {
  buildOmnibarFacets,
  omnibarRowsInOrder,
  queryForFederatedSearch,
  type FacetKey,
  type GraphNeighbor,
  type OmnibarGraphResult,
  type OmnibarRow,
} from '../../lib/omnibarFacets';
import { recordGamifyGuiEvent } from '../../lib/gamifyGuiEvents';
import { unwrapMcpEnvelope } from '../../lib/mcpToolResult';

const FACET_ORDER: FacetKey[] = ['surfaces', 'commands', 'onScreen', 'graph', 'docs'];

const SETTINGS_SEED_KEY = 'vox_settings_seed';

/**
 * Graph-discover MCP tool. `vox_graphify_query` was renamed to the `vox_search_*`
 * family (dispatch.rs registers vox_search_{status,structural,neighbors,path,
 * callers,callees,compare,rebuild}); this constant was left pointing at the old
 * name, so the facet errored on every keystroke. `vox_search_structural` is the
 * only graph tool that takes a lexical query string.
 */
const GRAPH_DISCOVER_TOOL = 'vox_search_structural';

/** VG-1-owned neighbor-expansion MCP tool (umbrella spec §3.1: { corpus, node_ids, max_depth }). */
const GRAPH_NEIGHBORS_TOOL = 'vox_search_neighbors';

/**
 * finding #4: preserve CommandPalette's prefix-mode → kind restriction. Moved in
 * verbatim from CommandPalette.tsx (it was a private helper, not exported). Maps
 * `>`=commands/actions, `@`=agents (no federated kinds), `/`=docs+skills so the
 * federated lane actually narrows; dropping it would make prefix modes decorative.
 */
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

/**
 * Parse a graph-tool response into rows. `invoke_mcp_tool` returns the daemon's
 * whole `ToolResult` under `result`, so the payload of both
 * `vox_search_structural` and `vox_search_neighbors` lives at `result.data`, and
 * the hits at `result.data.hits[]`. `results[]` is accepted as a legacy
 * fallback. Reading `result.hits` (no envelope unwrap) made the facet silently
 * return zero rows on every keystroke.
 */
export function parseDiscoverResults(res: unknown): GraphNeighbor[] {
  const r = res as { is_error?: boolean; result?: unknown };
  if (r?.is_error) return [];
  const data = unwrapMcpEnvelope(r?.result) as
    | { hits?: unknown[]; results?: unknown[] }
    | null
    | undefined;
  const raw = Array.isArray(data?.hits)
    ? data.hits
    : Array.isArray(data?.results)
      ? data.results
      : [];
  return raw.flatMap((raw0) => {
    const n = raw0 as { node_id?: string; id?: string; label?: string };
    const id = n.node_id ?? n.id;
    if (typeof id !== 'string' || id.length === 0) return [];
    const vk = id.startsWith('surface:') ? id.slice('surface:'.length) : undefined;
    return [{ id, label: n.label ?? vk ?? id, viewKey: vk }];
  });
}

interface OmnibarProps {
  open: boolean;
  onClose: () => void;
  onNavigate: (viewKey: string, anchorId?: string) => void;
  onRunCommand: (command: string) => void;
  onSendToChat: (query: string) => void;
  onOpenDoc: (path: string) => void;
  onSubmitTask: () => void;
  agents: Agent[];
  skills: CommandCatalogEntry[];
  gamifyEnabled?: boolean;
}

/** Always-offered quick action — not derived from any search source, so it
 *  must be spliced into the commands facet directly rather than relying on
 *  buildOmnibarFacets' query-matched rows (which are empty on an empty query). */
const SUBMIT_TASK_ROW: OmnibarRow = {
  id: 'quick-action:submit-task',
  facet: 'commands',
  label: 'Submit new task…',
  detail: 'Open Chat and focus the composer',
  provenance: 'corpus',
  activate: { type: 'submit-task' },
};

function ProvenanceBadge({ hint }: { hint: string }) {
  return (
    <span className="shrink-0 rounded-sm border border-border-subtle bg-overlay-subtle px-1.5 py-px font-mono text-[9px] uppercase tracking-widest text-text-muted">
      {hint}
    </span>
  );
}

export function Omnibar({
  open,
  onClose,
  onNavigate,
  onRunCommand,
  onSendToChat,
  onOpenDoc,
  onSubmitTask,
  agents,
  skills,
  gamifyEnabled = false,
}: OmnibarProps) {
  const [q, setQ] = useState('');
  const [selectedRowIdx, setSelectedRowIdx] = useState(-1);
  const [graph, setGraph] = useState<OmnibarGraphResult>({ rows: [], error: null });

  const { mode: prefixMode, query: effectiveQ } = parsePaletteQuery(q);
  const backendSearchEnabled = open && prefixMode === 'default';

  // Debounced copy of effectiveQ used ONLY to throttle the GRAPH discover MCP
  // call so it does not fire an `invokeMcpTool` per keystroke. Local-only facets
  // (federated/runtime/manifest) keep using effectiveQ directly — they are cheap.
  const [debouncedQ, setDebouncedQ] = useState(effectiveQ);
  useEffect(() => {
    const id = setTimeout(() => setDebouncedQ(effectiveQ), 200);
    return () => clearTimeout(id);
  }, [effectiveQ]);

  const { state: searchState, setQuery: setSearchQuery } = useSearchController({
    enabled: backendSearchEnabled,
  });
  const backendHits = useMemo(
    () => (backendSearchEnabled ? (searchState.hits as UnifiedHit[]) : []),
    [searchState.hits, backendSearchEnabled],
  );

  // finding #5: agents are not in the federated index — CommandPalette seeded them
  // from the `agents` prop. Carry them forward as activatable command-facet rows
  // (filtered in agents/default mode), routed via the `agent` activation arm.
  const agentRows = useMemo<OmnibarRow[]>(() => {
    if (prefixMode !== 'default' && prefixMode !== 'agents') return [];
    const ql = effectiveQ.trim().toLowerCase();
    if (!ql) return [];
    return agents
      .filter((a) => a.codename.toLowerCase().includes(ql) || a.id.toLowerCase().includes(ql))
      .map((a) => ({
        id: `agent:${a.id}`,
        facet: 'commands' as const,
        label: `${a.codename} (${a.id})`,
        detail: a.phase ?? '',
        provenance: 'corpus' as const,
        activate: { type: 'agent' as const, agentId: a.id },
      }));
  }, [agents, effectiveQ, prefixMode]);

  const skillSources = useMemo(
    () =>
      skills.map((s) => ({
        id: s.capability_id ?? s.command,
        name: s.command,
        description: s.about,
      })),
    [skills],
  );
  const { search: searchFederated } = useFederatedSearchIndex(skillSources);
  const fedKinds = useMemo(() => federatedKindsForMode(prefixMode), [prefixMode]);
  const federated = useMemo(
    () => {
      const q = queryForFederatedSearch(effectiveQ);
      return q && fedKinds.length > 0 ? searchFederated(q, { kinds: fedKinds }) : [];
    },
    [searchFederated, effectiveQ, fedKinds],
  );

  const manifest = useContentManifest();
  const runtimeHits = useMemo(
    () => (effectiveQ.trim() ? querySearchableRegistry(effectiveQ) : []),
    [effectiveQ],
  );

  // GRAPH facet: graph-discover MCP tool, independently fallible. The tool is
  // vox_search_structural. Its payload sits behind the daemon's ToolResult
  // envelope, at `result.data.hits[]` (the legacy `results[]` is still
  // tolerated) — see parseDiscoverResults, which unwraps it.
  // A failure here is a real search failure, not an absent capability.
  useEffect(() => {
    if (!open || !debouncedQ.trim()) {
      setGraph({ rows: [], error: null });
      return;
    }
    let cancelled = false;
    voxTransport
      .invokeMcpTool(GRAPH_DISCOVER_TOOL, { query: debouncedQ, limit: 6 })
      .then((res) => {
        if (cancelled) return;
        const r = res as { is_error?: boolean };
        if (r?.is_error) {
          setGraph({ rows: [], error: 'graph search failed' });
          return;
        }
        setGraph({ rows: parseDiscoverResults(res), error: null });
      })
      .catch(() => {
        if (!cancelled) {
          setGraph({ rows: [], error: 'graph search unreachable' });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [open, debouncedQ]);

  const facets = useMemo(() => {
    const base = buildOmnibarFacets({
      query: effectiveQ,
      prefixMode,
      federated,
      backendHits,
      manifest,
      runtimeHits,
      graph,
    });
    // Carry agents into the COMMANDS facet (finding #5) without re-capping below
    // FACET_CAP would drop real commands — agents lead, then existing commands.
    // SUBMIT_TASK_ROW is a default quick action shown only on the empty-query
    // landing state — like a real command palette's default suggestions, it
    // steps aside the moment the user starts searching for something else.
    const withSubmitTask = effectiveQ.trim() ? agentRows : [...agentRows, SUBMIT_TASK_ROW];
    if (withSubmitTask.length === 0) return base;
    return base.map((f) =>
      f.key === 'commands'
        ? { ...f, rows: [...withSubmitTask, ...f.rows] }
        : f,
    );
  }, [effectiveQ, prefixMode, federated, backendHits, manifest, runtimeHits, graph, agentRows]);
  const rows = useMemo(() => omnibarRowsInOrder(facets), [facets]);

  const activateRow = useCallback(
    (row: OmnibarRow) => {
      recordGamifyGuiEvent('palette_navigation', { facet: row.facet }, { enabled: gamifyEnabled });
      switch (row.activate.type) {
        case 'navigate':
          onNavigate(row.activate.viewKey, row.activate.anchorId);
          break;
        case 'command':
          onRunCommand(row.activate.command);
          break;
        case 'doc':
          onOpenDoc(row.activate.path);
          break;
        case 'graph':
          if (row.activate.node.viewKey) onNavigate(row.activate.node.viewKey);
          break;
        // finding #5: carried-in CommandPalette arms — route the same way.
        case 'setting':
          try {
            localStorage.setItem(
              SETTINGS_SEED_KEY,
              JSON.stringify({ section: row.activate.section, settingId: row.activate.settingId }),
            );
          } catch {
            /* ignore */
          }
          onNavigate('settings');
          try {
            window.dispatchEvent(new Event('vox-settings-seed'));
          } catch {
            /* ignore */
          }
          break;
        case 'policy':
          onNavigate('policies');
          break;
        case 'skill':
          onRunCommand(`skill:${row.activate.skillId}`);
          break;
        case 'agent':
          onNavigate('roster');
          break;
        case 'submit-task':
          onSubmitTask();
          break;
      }
      onClose();
    },
    [onNavigate, onRunCommand, onOpenDoc, onSubmitTask, onClose, gamifyEnabled],
  );

  const expandGraphNeighbors = useCallback((row: OmnibarRow) => {
    if (row.activate.type !== 'graph') return;
    const seed = row.activate.node;
    // vox_search_neighbors is the real neighbor primitive:
    // { corpus, node_ids, max_depth }. Its payload sits behind the daemon's
    // ToolResult envelope, at `result.data.hits[]` — parseDiscoverResults
    // unwraps it, so do not reach for `result.hits` here.
    // TODO(VG-1): pass `corpus` once the omnibar carries an active/seed corpus.
    // The discover call (GRAPH_DISCOVER_TOOL) also omits corpus today and relies
    // on the tool default; both should be threaded the same active corpus.
    voxTransport
      .invokeMcpTool(GRAPH_NEIGHBORS_TOOL, { node_ids: [seed.id], max_depth: 1 })
      .then((res) => {
        const added0 = parseDiscoverResults(res);
        setGraph((prev) => {
          if (prev.error) return prev;
          const seen = new Set(prev.rows.map((n) => n.id));
          const added = added0.filter((n) => !seen.has(n.id));
          return { rows: [...prev.rows, ...added], error: null };
        });
      })
      .catch(() => {
        /* facet stays as-is — honest no-op */
      });
  }, []);

  const rowsRef = useRef(rows);
  const idxRef = useRef(selectedRowIdx);
  rowsRef.current = rows;
  idxRef.current = selectedRowIdx;

  useEffect(() => {
    if (!open) {
      setQ('');
      setSearchQuery('');
      setSelectedRowIdx(-1);
    }
  }, [open, setSearchQuery]);

  useEffect(() => setSelectedRowIdx(-1), [rows.length, q]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      const list = rowsRef.current;
      let idx = idxRef.current;
      if (e.key === 'Escape') {
        onClose();
      } else if (e.key === 'Enter' && e.shiftKey) {
        e.preventDefault();
        if (q.trim()) onSendToChat(q.trim());
        onClose();
      } else if (e.key === 'ArrowDown' && list.length > 0) {
        e.preventDefault();
        idx = idx < 0 ? 0 : Math.min(idx + 1, list.length - 1);
        idxRef.current = idx;
        setSelectedRowIdx(idx);
      } else if (e.key === 'ArrowUp' && list.length > 0) {
        e.preventDefault();
        idx = idx < 0 ? list.length - 1 : Math.max(idx - 1, 0);
        idxRef.current = idx;
        setSelectedRowIdx(idx);
      } else if (e.key === 'ArrowRight' && e.altKey) {
        e.preventDefault();
        const target = idx >= 0 && idx < list.length ? list[idx] : list[0];
        if (target && target.activate.type === 'graph') expandGraphNeighbors(target);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        const target = idx >= 0 && idx < list.length ? list[idx] : list[0];
        if (target) activateRow(target);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose, onSendToChat, activateRow, expandGraphNeighbors, q]);

  if (!open) return null;

  let rowCursor = 0;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 backdrop-blur-xs pt-[14vh]"
      onClick={onClose}
    >
      <div
        className="w-[680px] max-w-[92vw] rounded-2xl border border-border-subtle bg-bg-base/90 shadow-[0_40px_120px_-30px_rgba(0,0,0,0.9)] backdrop-blur-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-border-subtle px-4 py-3">
          <Icon.search className="size-4 text-brass" />
          <input
            autoFocus
            value={q}
            onChange={(e) => {
              const v = e.target.value;
              setQ(v);
              setSearchQuery(v);
            }}
            placeholder="Search surfaces, commands, on-screen text, graph, docs…  ⇧⏎ to ask chat"
            className="flex-1 bg-transparent text-[14px] text-text-primary placeholder:text-text-muted outline-hidden"
          />
          <kbd className="rounded-sm border border-border-subtle bg-overlay-subtle px-1.5 py-0.5 font-mono text-[10px] text-text-muted">esc</kbd>
        </div>

        <div className="max-h-[480px] overflow-auto p-2 custom-scrollbar">
          {FACET_ORDER.map((key) => {
            const facet = facets.find((f) => f.key === key)!;
            if (facet.rows.length === 0 && !facet.error) return null;
            return (
              <React.Fragment key={key}>
                <div className="flex items-center justify-between px-3 py-2 mt-2 text-[10px] uppercase tracking-widest text-text-muted border-b border-border-subtle">
                  <span>{facet.label}</span>
                  <ProvenanceBadge hint={facet.provenanceHint} />
                </div>
                {facet.error ? (
                  <div className="px-3 py-2 text-[11px] text-text-muted italic">
                    {facet.label} unavailable — {facet.error}
                  </div>
                ) : (
                  facet.rows.map((row) => {
                    const idx = rowCursor++;
                    const selected = idx === selectedRowIdx;
                    return (
                      <button
                        key={row.id}
                        onClick={() => activateRow(row)}
                        className={`flex w-full items-center justify-between rounded-lg px-3 py-2 text-left transition ${
                          selected ? 'bg-brass/8 border border-brass/20' : 'hover:bg-overlay-subtle'
                        }`}
                      >
                        <div className="flex flex-col min-w-0">
                          <span className="text-[13px] text-text-secondary truncate max-w-[460px]">{row.label}</span>
                          {row.detail ? (
                            <span className="text-[11px] text-text-muted truncate max-w-[460px]">{row.detail}</span>
                          ) : null}
                        </div>
                        <span className="font-mono text-[9px] uppercase tracking-widest text-text-muted shrink-0 ml-2">
                          {row.provenance}
                        </span>
                      </button>
                    );
                  })
                )}
              </React.Fragment>
            );
          })}

          {q.length > 0 && rows.length === 0 && !facets.some((f) => f.error) && (
            <div className="px-3 py-6 text-center text-[12px] text-text-muted">
              No matches for "{q}" — press ⇧⏎ to ask chat
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
