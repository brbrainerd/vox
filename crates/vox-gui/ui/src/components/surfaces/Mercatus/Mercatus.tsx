import React, { useEffect, useState, useCallback } from 'react';
import { voxTransport } from '../../../transport';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { useLabel } from '../../../hooks/useLanguage';

interface SourceMeta {
  enabled: boolean;
  costUsd: number;
  cadenceHours: number;
  tier: string;
}

interface WatchlistPart {
  id: string;
  role: string;
  model: string;
  sources: string[];
  ids: Record<string, string>;
  target_usd?: number;
  condition?: string;
}

interface PriceWatchConfig {
  _meta: { sources: Record<string, SourceMeta> };
  watchlist: WatchlistPart[];
}

type LoadState = 'loading' | 'ok' | 'error';

/**
 * First-run / absent-config shape. The backend already returns this on
 * ENOENT (see mercatus_load_config), but any other null-ish payload used to
 * fall through the render guard and leave the surface completely blank, so
 * the coercion happens here too rather than trusting one caller.
 */
const EMPTY_CONFIG: PriceWatchConfig = { _meta: { sources: {} }, watchlist: [] };

export function Mercatus({ condensed }: { condensed?: boolean }) {
  const navLabel = useLabel('mercatus');
  const [cfg, setCfg] = useState<PriceWatchConfig | null>(null);
  const [state, setState] = useState<LoadState>('loading');
  const [err, setErr] = useState('');
  const [configPath, setConfigPath] = useState<string | null>(null);

  const reload = useCallback(() => {
    setState('loading');
    voxTransport.mercatusLoadConfig()
      .then((c) => { setCfg((c as PriceWatchConfig | null) ?? EMPTY_CONFIG); setState('ok'); })
      .catch((e) => { setErr(sanitizeErrorForToast(e)); setState('error'); });
  }, []);

  useEffect(() => { reload(); }, [reload]);

  // Best-effort: the empty state is more useful with the path, but a failure
  // here must not take the surface down with it.
  useEffect(() => {
    voxTransport.mercatusConfigPath()
      .then((p) => setConfigPath(p))
      .catch(() => setConfigPath(null));
  }, []);

  const sources = cfg?._meta?.sources ?? {};
  const parts = cfg?.watchlist ?? [];
  const allSourceKeys = Object.keys(sources).filter(k => sources[k].enabled);

  if (condensed) {
    return (
      <section className="space-y-2 text-[11px] text-text-muted">
        <h2 className="font-display text-xs text-text-primary tracking-wider uppercase">{navLabel}</h2>
        {state === 'loading' && <div className="animate-pulse">Loading…</div>}
        {state === 'error' && <div className="text-red-400">{err}</div>}
        {state === 'ok' && <div>{parts.length} parts · {allSourceKeys.length} enabled sources</div>}
      </section>
    );
  }

  return (
    <section className="space-y-4">
      <div className="flex items-baseline gap-3">
        <h2 className="font-display text-lg text-text-primary tracking-wider uppercase">
          {navLabel} — Price Watch
        </h2>
        <button
          type="button"
          onClick={reload}
          className="rounded border border-border-subtle bg-overlay-subtle px-2 py-0.5 font-mono text-[10px] text-text-muted hover:text-text-secondary"
        >
          {state === 'loading' ? 'loading…' : 'refresh'}
        </button>
      </div>

      {state === 'error' && (
        <div className="rounded-lg border border-dashed border-red-400/30 bg-red-400/5 px-4 py-2 text-[11px] text-red-400">
          {err}
        </div>
      )}

      {state === 'ok' && parts.length === 0 && (
        <div
          data-testid="mercatus-empty"
          className="rounded-lg border border-dashed border-border-subtle bg-overlay-subtle px-4 py-6 text-center text-[11px] text-text-muted"
        >
          <p className="text-text-secondary">No parts are being tracked yet.</p>
          <p className="mt-1">
            Add entries to the price-watch config to start comparing prices across sources.
          </p>
          {configPath && (
            <p className="mt-2 font-mono text-[10px] text-text-muted/80 break-all" data-testid="mercatus-config-path">
              {configPath}
            </p>
          )}
        </div>
      )}

      {state === 'ok' && parts.length > 0 && (
        <>
          {/* Coverage matrix */}
          <div className="overflow-x-auto rounded-lg border border-border-subtle">
            <table className="w-full border-collapse text-[11px]">
              <thead>
                <tr className="border-b border-border-subtle bg-overlay-subtle">
                  <th className="px-3 py-2 text-left font-display text-[10px] uppercase tracking-widest text-text-muted">
                    Part
                  </th>
                  <th className="px-3 py-2 text-left font-display text-[10px] uppercase tracking-widest text-text-muted">
                    Role
                  </th>
                  <th className="px-3 py-2 text-left font-display text-[10px] uppercase tracking-widest text-text-muted">
                    Target
                  </th>
                  {allSourceKeys.map((src) => (
                    <th
                      key={src}
                      className="px-2 py-2 text-center font-display text-[10px] uppercase tracking-widest text-text-muted"
                    >
                      {src}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {parts.map((part, i) => {
                  return (
                    <tr
                      key={part.id}
                      className={`border-b border-border-subtle last:border-0 ${i % 2 === 0 ? '' : 'bg-overlay-subtle/30'}`}
                    >
                      <td className="px-3 py-1.5 font-mono text-[11px] text-text-primary">
                        {part.model || part.id}
                      </td>
                      <td className="px-3 py-1.5 text-text-muted">{part.role}</td>
                      <td className="px-3 py-1.5 font-mono text-text-secondary">
                        {part.target_usd != null ? `$${part.target_usd}` : '—'}
                      </td>
                      {allSourceKeys.map((src) => {
                        const hasId = !!(part.ids ?? {})[src];
                        const inSources = (part.sources ?? []).includes(src);
                        const dot = hasId
                          ? '●'
                          : inSources
                          ? '○'
                          : '·';
                        const cls = hasId
                          ? 'text-accent-secondary'
                          : inSources
                          ? 'text-brass/60'
                          : 'text-text-muted/30';
                        return (
                          <td key={src} className={`px-2 py-1.5 text-center font-mono ${cls}`} title={hasId ? `id: ${part.ids[src]}` : inSources ? 'in sources[], no id' : 'not tracked'}>
                            {dot}
                          </td>
                        );
                      })}
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          <div className="text-[10px] text-text-muted flex gap-4">
            <span><span className="font-mono text-accent-secondary">●</span> id pinned</span>
            <span><span className="font-mono text-brass/60">○</span> in sources[], no id</span>
            <span><span className="font-mono text-text-muted/30">·</span> not tracked</span>
            <span className="ml-auto">{parts.length} parts · {allSourceKeys.length} enabled sources</span>
          </div>

          {/* Source cost table */}
          <details className="rounded-lg border border-border-subtle">
            <summary className="cursor-pointer px-3 py-2 font-display text-[10px] uppercase tracking-widest text-text-muted hover:text-text-secondary">
              Source registry ({Object.keys(sources).length})
            </summary>
            <div className="overflow-x-auto">
              <table className="w-full border-collapse text-[11px]">
                <thead>
                  <tr className="border-b border-border-subtle bg-overlay-subtle">
                    {['Source', 'Tier', 'Cost/run', 'Cadence', 'Enabled'].map(h => (
                      <th key={h} className="px-3 py-1.5 text-left font-display text-[10px] uppercase tracking-widest text-text-muted">{h}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {Object.entries(sources).map(([src, meta]) => (
                    <tr key={src} className="border-b border-border-subtle last:border-0">
                      <td className="px-3 py-1.5 font-mono text-text-primary">{src}</td>
                      <td className="px-3 py-1.5 text-text-muted">{meta.tier}</td>
                      <td className="px-3 py-1.5 font-mono text-text-secondary">${meta.costUsd.toFixed(3)}</td>
                      <td className="px-3 py-1.5 text-text-muted">{meta.cadenceHours}h</td>
                      <td className={`px-3 py-1.5 font-mono ${meta.enabled ? 'text-accent-secondary' : 'text-text-muted/40'}`}>
                        {meta.enabled ? 'yes' : 'no'}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </details>
        </>
      )}

      {state === 'loading' && (
        <div className="text-[11px] text-text-muted animate-pulse">Loading config…</div>
      )}
    </section>
  );
}
