import React, { useCallback, useEffect, useState } from 'react';
import { Glass } from '../../ui/Glass';
import { voxTransport, type ModelPoolDto, type PoolRule } from '../../../transport';

const RULE_LABELS: Record<string, string> = {
  free: 'Free models only',
  provider: 'Provider',
  max_cost_per_1k: 'Max cost / 1k tokens',
  tier: 'Tier',
  min_context: 'Min context window',
};

function ruleLabel(r: PoolRule): string {
  if (r.kind === 'free') return 'Free models only';
  if (r.kind === 'provider') return `Provider = ${(r as any).value}`;
  if (r.kind === 'max_cost_per_1k') return `Max cost ≤ $${(r as any).value}/1k`;
  if (r.kind === 'tier') return `Tier = ${(r as any).value}`;
  if (r.kind === 'min_context') return `Context ≥ ${((r as any).value as number).toLocaleString()} tokens`;
  return r.kind;
}

interface ModelPoolViewProps {
  pushToast: (t: { message: string; variant?: string }) => void;
}

export function ModelPoolView({ pushToast }: ModelPoolViewProps) {
  const [pool, setPool] = useState<ModelPoolDto | null>(null);
  const [providers, setProviders] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    try {
      const [p, prov] = await Promise.all([
        voxTransport.getModelPool(),
        voxTransport.listEnabledProviders(),
      ]);
      setPool(p);
      setProviders(prov);
    } catch (e) {
      pushToast({ message: `Failed to load model pool: ${e}`, variant: 'error' });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => { load(); }, [load]);

  const removeExclude = useCallback(async (id: string) => {
    if (!pool) return;
    const next: ModelPoolDto = { ...pool, excludes: pool.excludes.filter(x => x !== id) };
    setSaving(true);
    try {
      await voxTransport.setModelPool(next);
      setPool(next);
    } catch (e) {
      pushToast({ message: `Save failed: ${e}`, variant: 'error' });
    } finally {
      setSaving(false);
    }
  }, [pool, pushToast]);

  const removeRule = useCallback(async (idx: number) => {
    if (!pool) return;
    const next: ModelPoolDto = { ...pool, rules: pool.rules.filter((_, i) => i !== idx) };
    setSaving(true);
    try {
      await voxTransport.setModelPool(next);
      setPool(next);
    } catch (e) {
      pushToast({ message: `Save failed: ${e}`, variant: 'error' });
    } finally {
      setSaving(false);
    }
  }, [pool, pushToast]);

  if (loading) {
    return (
      <Glass className="flex items-center justify-center p-8 text-zinc-500 text-sm">
        Loading model pool…
      </Glass>
    );
  }

  if (!pool) return null;

  const memberCount = pool.member_ids.length;
  const fallbackNote = pool.fell_open
    ? 'Pool resolved empty — showing all enabled models as fallback.'
    : null;

  return (
    <section className="flex flex-col gap-4">
      <header className="flex items-baseline gap-3">
        <h2 className="font-display text-sm font-semibold text-zinc-200 uppercase tracking-widest">
          Model Pool
        </h2>
        <span className="text-xs text-zinc-500">
          {memberCount} model{memberCount !== 1 ? 's' : ''} in pool
          {providers.length > 0 && ` · ${providers.length} provider${providers.length !== 1 ? 's' : ''} enabled`}
        </span>
        {saving && <span className="ml-auto text-xs text-zinc-500">Saving…</span>}
      </header>

      {fallbackNote && (
        <Glass className="px-4 py-2 text-xs text-amber-400 border border-amber-400/20">
          {fallbackNote}
        </Glass>
      )}

      {/* Rules */}
      <Glass className="flex flex-col gap-2 p-4">
        <p className="text-[10px] uppercase tracking-widest text-zinc-500 mb-1">Rules</p>
        {pool.rules.length === 0 && (
          <p className="text-xs text-zinc-500 italic">No rules — all enabled models are candidates.</p>
        )}
        {pool.rules.map((r, i) => (
          <div key={i} className="flex items-center gap-2 text-xs text-zinc-300">
            <span className="flex-1">{ruleLabel(r)}</span>
            <button
              type="button"
              onClick={() => removeRule(i)}
              className="text-zinc-600 hover:text-rose-400 transition text-[11px]"
              aria-label={`Remove rule ${ruleLabel(r)}`}
            >
              ✕
            </button>
          </div>
        ))}
      </Glass>

      {/* Excludes */}
      {pool.excludes.length > 0 && (
        <Glass className="flex flex-col gap-2 p-4">
          <p className="text-[10px] uppercase tracking-widest text-zinc-500 mb-1">Excluded Models</p>
          {pool.excludes.map(id => (
            <div key={id} className="flex items-center gap-2 text-xs">
              <span className="flex-1 font-mono text-zinc-400">{id}</span>
              <button
                type="button"
                onClick={() => removeExclude(id)}
                className="text-zinc-600 hover:text-rose-400 transition text-[11px]"
                aria-label={`Remove exclusion ${id}`}
              >
                ✕
              </button>
            </div>
          ))}
        </Glass>
      )}

      {/* Pool members preview */}
      {memberCount > 0 && (
        <Glass className="flex flex-col gap-1 p-4">
          <p className="text-[10px] uppercase tracking-widest text-zinc-500 mb-1">
            Current Members ({memberCount})
          </p>
          <div className="flex flex-wrap gap-1.5">
            {pool.member_ids.slice(0, 24).map(id => (
              <span
                key={id}
                className="rounded border border-white/10 bg-white/[0.02] px-2 py-0.5 font-mono text-[10px] text-zinc-400"
              >
                {id}
              </span>
            ))}
            {memberCount > 24 && (
              <span className="text-[10px] text-zinc-500">+{memberCount - 24} more</span>
            )}
          </div>
        </Glass>
      )}
    </section>
  );
}
