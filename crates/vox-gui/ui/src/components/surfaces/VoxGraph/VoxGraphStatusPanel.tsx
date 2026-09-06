import React, { useCallback, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { voxTransport } from '../../../transport';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { useVoxGraphStatus, VOX_GRAPH_STATUS_QUERY_KEY } from '../../../hooks/useVoxGraphStatus';
import { useLabel } from '../../../hooks/useLanguage';

/**
 * Render an RFC3339 `built_at` timestamp as a coarse relative time
 * ("3h ago"). Returns an em dash for missing/unparseable input.
 */
function relativeBuiltAt(iso: string | null): string {
  if (!iso) return '—';
  const ts = Date.parse(iso);
  if (!Number.isFinite(ts)) return '—';
  const deltaSec = Math.round((Date.now() - ts) / 1000);
  if (deltaSec < 0) return 'just now';
  if (deltaSec < 60) return `${deltaSec}s ago`;
  if (deltaSec < 3600) return `${Math.floor(deltaSec / 60)}m ago`;
  if (deltaSec < 86_400) return `${Math.floor(deltaSec / 3600)}h ago`;
  return `${Math.floor(deltaSec / 86_400)}d ago`;
}

/**
 * Per-corpus rebuild button. Treats freshness as a progress/health signal:
 * a stale corpus needs attention, and this is the action affordance.
 *
 * The `vox_search_rebuild` MCP tool is wired here by name (the registered
 * dispatch name). If it errors at runtime, a 404/error surfaces as an inline
 * message and the panel keeps working.
 */
function RebuildButton({ corpusId }: { corpusId: string }) {
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleRebuild = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await voxTransport.invokeMcpTool('vox_search_rebuild', { corpus: corpusId });
      // Refresh freshness so the card flips fresh once the rebuild completes.
      await queryClient.invalidateQueries({ queryKey: VOX_GRAPH_STATUS_QUERY_KEY });
    } catch (e) {
      setError(sanitizeErrorForToast((e as Error)?.message ?? e));
    } finally {
      setBusy(false);
    }
  }, [corpusId, queryClient]);

  return (
    <div className="mt-2 flex items-center gap-2">
      <button
        type="button"
        disabled={busy}
        aria-label={`Rebuild ${corpusId}`}
        onClick={handleRebuild}
        className="rounded-md border border-amber-500/30 bg-amber-500/10 px-2.5 py-1 text-[11px] font-medium text-amber-300 transition hover:bg-amber-500/20 disabled:opacity-50"
      >
        {busy ? 'Rebuilding…' : 'Rebuild'}
      </button>
      {error && (
        <span role="alert" className="text-[10px] text-red-400">
          {error}
        </span>
      )}
    </div>
  );
}

/** Range mirrors validate_ttl_days in crates/vox-config/src/graphify.rs. */
const TTL_DAYS_MIN = 1;
const TTL_DAYS_MAX = 3650;

/**
 * Editable staleness TTL. TTL is a global registry setting, so this lives in the
 * panel header rather than on a corpus card. Writes go through
 * `vox_search_set_ttl`, which edits `ttl_days_default` in the tracked contract
 * `contracts/retrieval/vox-graph-corpora.v1.yaml` — the same value the CLI and
 * the CI freshness gate read, so the save leaves an uncommitted change.
 */
function TtlEditor({
  ttlDays,
  effectiveTtlDays,
  envForced,
}: {
  /** The CONTRACT value — what Save writes, so what the control must show. */
  ttlDays: number;
  /** The value actually in force after env precedence, shown when it differs. */
  effectiveTtlDays: number;
  envForced: boolean;
}) {
  const queryClient = useQueryClient();
  const [value, setValue] = useState(String(ttlDays));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [wrotePath, setWrotePath] = useState<string | null>(null);

  // Resync when a refetch brings a different TTL (a save, or an env-forced
  // value winning). Comparing against the last prop seen rather than syncing in
  // an effect means typing is never clobbered: the input only resets when the
  // incoming number actually changes.
  const [lastTtlDays, setLastTtlDays] = useState(ttlDays);
  if (ttlDays !== lastTtlDays) {
    setLastTtlDays(ttlDays);
    setValue(String(ttlDays));
  }

  const handleSave = useCallback(async () => {
    const parsed = Number(value);
    if (!Number.isInteger(parsed) || parsed < TTL_DAYS_MIN || parsed > TTL_DAYS_MAX) {
      setError(`TTL must be a whole number between ${TTL_DAYS_MIN} and ${TTL_DAYS_MAX}`);
      return;
    }
    setBusy(true);
    setError(null);
    setWrotePath(null);
    try {
      // `invoke_mcp_tool` returns `{ is_error, result }` where `result` is the
      // daemon's own `{ success, data, error? }` envelope — both already parsed
      // objects (see crates/vox-gui/src/commands/mcp.rs).
      const res = (await voxTransport.invokeMcpTool('vox_search_set_ttl', {
        ttl_days: parsed,
      })) as {
        result?: {
          success?: boolean;
          error?: string;
          data?: { requires_commit?: boolean; contract_path?: string };
        };
      };
      // A failed write reports in-band rather than throwing; silence here would
      // look exactly like a successful save.
      if (res?.result?.success === false) {
        setError(sanitizeErrorForToast(res.result.error ?? 'Failed to set TTL'));
        return;
      }
      // The tool edits a TRACKED contract file, so the save dirties the working
      // tree. Saying so is not decoration: a user who is not told will not commit,
      // and CI will keep enforcing the old TTL.
      if (res?.result?.data?.requires_commit) {
        setWrotePath(res.result.data.contract_path ?? null);
      }
      await queryClient.invalidateQueries({ queryKey: VOX_GRAPH_STATUS_QUERY_KEY });
    } catch (e) {
      setError(sanitizeErrorForToast((e as Error)?.message ?? e));
    } finally {
      setBusy(false);
    }
  }, [value, queryClient]);

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-2">
        <label htmlFor="vg-ttl-days" className="text-[9px] uppercase tracking-wider text-zinc-500">
          TTL (days)
        </label>
        {/* h-7 (28px) clears the WCAG 2.2 SC 2.5.8 24x24 target minimum; do not
            shrink it to match the 10px type scale around it. */}
        <input
          id="vg-ttl-days"
          type="number"
          inputMode="numeric"
          min={TTL_DAYS_MIN}
          max={TTL_DAYS_MAX}
          value={value}
          disabled={busy}
          aria-label="Staleness TTL in days"
          // The confirmation describes the value that was written, so it must
          // not outlive an edit to that value.
          onChange={(e) => {
            setValue(e.target.value);
            setWrotePath(null);
            setError(null);
          }}
          className="h-7 w-20 rounded-md border border-white/10 bg-zinc-950/40 px-2 font-mono text-[11px] text-zinc-200 disabled:opacity-50"
        />
        <button
          type="button"
          disabled={busy}
          aria-label="Save TTL"
          onClick={handleSave}
          className="h-7 rounded-md border border-white/10 bg-white/5 px-3 text-[11px] font-medium text-zinc-200 transition hover:bg-white/10 disabled:opacity-50"
        >
          {busy ? 'Saving…' : 'Save'}
        </button>
      </div>
      {envForced && (
        <span className="text-[10px] text-amber-400">
          VOX_GRAPHIFY_TTL_DAYS is set and overrides this value.
        </span>
      )}
      {effectiveTtlDays !== ttlDays && (
        <span className="text-[10px] text-zinc-400">
          Currently in force: {effectiveTtlDays} days.
        </span>
      )}
      {wrotePath && (
        <span className="text-[10px] text-zinc-400">
          Wrote <code className="font-mono">{wrotePath}</code> — commit it so the CLI and CI
          use this TTL too.
        </span>
      )}
      {error && (
        <span role="alert" className="text-[10px] text-red-400">
          {error}
        </span>
      )}
    </div>
  );
}

export function VoxGraphStatusPanel({ condensed }: { condensed?: boolean } = {}) {
  const { data, isLoading, isError, error } = useVoxGraphStatus();
  const corpusHealthLabel = useLabel('vg-corpus-health');

  if (condensed) {
    if (isLoading) {
      return <div className="p-2 text-[11px] text-zinc-400 animate-pulse">Loading…</div>;
    }
    if (isError) {
      return <div className="p-2 text-[11px] text-red-400" role="alert">Graphify status unavailable</div>;
    }
    if (!data) return <div className="p-2 text-[11px] text-zinc-400">No graphify data</div>;
    const freshCount = data.corpora.filter(c => c.is_fresh).length;
    return (
      <div className="p-2 text-[11px] text-zinc-400">
        <div className="mb-1 text-xs font-semibold uppercase tracking-wider text-zinc-200">{corpusHealthLabel}</div>
        <div>{freshCount}/{data.corpora.length} fresh</div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-sm text-zinc-400">
        Loading graphify status…
      </div>
    );
  }

  if (isError) {
    return (
      <div className="p-4" role="alert">
        <div className="flex items-center gap-2 rounded-lg border border-red-900/50 bg-red-950/20 p-3 text-sm text-red-400">
          <span>Graphify status unavailable: {(error as Error)?.message ?? 'unknown error'}</span>
        </div>
      </div>
    );
  }

  if (!data) return <div className="p-4 text-zinc-400">No graphify data available</div>;

  return (
    <div className="flex flex-col gap-4 p-4">
      <div className="flex items-center justify-between gap-4">
        <h2 className="ds-section-head">{corpusHealthLabel}</h2>
        <div className="flex items-center gap-4">
          {typeof data.ttl_days === 'number' && (
            <TtlEditor
              // Prefill what Save WRITES (the contract), not the env-resolved
              // effective value: prefilling the effective TTL meant one Save
              // rewrote the tracked contract to a number the user never chose.
              ttlDays={data.ttl_days_contract ?? data.ttl_days}
              effectiveTtlDays={data.ttl_days}
              envForced={data.ttl_days_env_forced === true}
            />
          )}
          <span className="font-mono text-[10px] text-zinc-500">
            Default: {data.default_corpus_id}
          </span>
        </div>
      </div>

      <div className="grid gap-3 sm:grid-cols-1 md:grid-cols-2">
        {data.corpora.map((c) => (
          <div
            key={c.corpus_id}
            className={`group rounded-lg border p-4 transition-all duration-200 ${
              c.is_fresh
                ? 'border-emerald-500/10 bg-emerald-500/2 hover:border-emerald-500/20'
                : 'border-amber-500/10 bg-amber-500/2 hover:border-amber-500/20'
            }`}
          >
            <div className="flex items-start justify-between">
              <div>
                <h3 className="font-medium text-zinc-200">{c.title}</h3>
                <p className="font-mono text-[10px] text-zinc-500">{c.corpus_id}</p>
              </div>
              <span
                className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-medium ${
                  c.is_fresh
                    ? 'bg-emerald-500/10 text-emerald-400'
                    : 'bg-amber-500/10 text-amber-400'
                }`}
              >
                <span
                  className={`h-1.5 w-1.5 rounded-full ${
                    c.is_fresh ? 'bg-emerald-400' : 'bg-amber-400'
                  }`}
                />
                {c.is_fresh ? 'Fresh' : 'Stale'}
              </span>
            </div>

            <div className="mt-3 grid grid-cols-3 gap-2 border-t border-white/5 pt-3 font-mono text-[11px] text-zinc-400">
              <div>
                <span className="text-zinc-500 block text-[9px] uppercase">Nodes</span>
                <span className="font-semibold text-zinc-300">
                  {c.node_count?.toLocaleString() ?? '—'}
                </span>
              </div>
              <div>
                <span className="text-zinc-500 block text-[9px] uppercase">Edges</span>
                <span className="font-semibold text-zinc-300">
                  {c.edge_count?.toLocaleString() ?? '—'}
                </span>
              </div>
              <div>
                <span className="text-zinc-500 block text-[9px] uppercase">Built</span>
                <span
                  className="font-semibold text-zinc-300"
                  title={c.built_at ?? undefined}
                >
                  {relativeBuiltAt(c.built_at)}
                </span>
              </div>
            </div>

            {!c.is_fresh && (
              <div className="mt-3 space-y-2 border-t border-white/5 pt-3">
                <div className="text-[11px] text-zinc-400">
                  <span className="text-zinc-500 text-[9px] block uppercase">Stale Reasons</span>
                  <div className="flex flex-wrap gap-1 mt-1">
                    {c.stale_reasons.map((r) => (
                      <span
                        key={r}
                        className="rounded-sm bg-amber-500/10 px-1.5 py-0.5 text-[9px] font-mono text-amber-400"
                      >
                        {r}
                      </span>
                    ))}
                  </div>
                </div>

                <RebuildButton corpusId={c.corpus_id} />

                <div className="relative mt-2 rounded-sm bg-zinc-950/40 p-2 border border-white/5">
                  <span className="text-[9px] text-zinc-500 block uppercase mb-1">Rebuild Command</span>
                  <code className="block select-all font-mono text-[10px] text-zinc-300 break-all leading-normal">
                    vox graphify rebuild --corpus {c.corpus_id}
                  </code>
                </div>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
