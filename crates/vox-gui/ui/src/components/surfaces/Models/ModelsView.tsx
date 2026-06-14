import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';

interface ModelCard {
  id: string;
  provider: string;
  tier: string;
  cost_per_1k: number;
  max_tokens: number;
  is_free: boolean;
  latency_p50_ms?: number | null;
  success_rate?: number | null;
  quality_score?: number | null;
}

interface RoutingSummary {
  active_model?: string | null;
  exploration_spent_usd: number;
  exploration_budget_usd: number;
  arm_count: number;
  model_count: number;
  decision_preview?: {
    selected_model: string;
    discovery_state: string;
    alternatives: string[];
    rejection_reasons: string[];
    intelligence_score: number;
    efficiency_score: number;
    latency_score: number;
  } | null;
}

interface ModelsViewProps {
  pushToast: (t: any) => void;
}

export function ModelsView({ pushToast }: ModelsViewProps) {
  const [models, setModels] = useState<ModelCard[]>([]);
  const [summary, setSummary] = useState<RoutingSummary | null>(null);
  const [activeModel, setActiveModel] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [cards, routing, active] = await Promise.all([
        invoke<ModelCard[]>('list_model_cards', { limit: 120 }),
        invoke<RoutingSummary>('get_routing_summary_live'),
        invoke<string | null>('get_active_model'),
      ]);
      setModels(cards);
      setSummary(routing);
      setActiveModel(active);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Models load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 8000);
    return () => clearInterval(id);
  }, [refresh]);

  const setDefault = async (id: string) => {
    try {
      await invoke('set_active_model', { modelId: id });
      setActiveModel(id);
      pushToast({ tone: 'ok', title: 'Active model set', body: id });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Set active failed', body: String(err) });
    }
  };

  const hosted = models.filter(m => !m.id.includes('ollama') && !m.id.startsWith('mesh/') && !m.id.startsWith('mens/'));
  const local = models.filter(m => m.id.includes('ollama') || m.id.startsWith('mesh/') || m.id.startsWith('mens/'));

  return (
    <div className="flex flex-col gap-5">
      <Glass className="p-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <div className="font-display text-sm tracking-widest text-zinc-200 uppercase">Model Registry</div>
            <div className="text-xs text-zinc-500 mt-1">
              {summary ? `${summary.model_count} models · ${summary.arm_count} routing arms · explore $${(summary.exploration_spent_usd ?? 0).toFixed(2)} / $${(summary.exploration_budget_usd ?? 0).toFixed(0)}` : 'Loading routing summary…'}
            </div>
          </div>
          <div className="text-right">
            <div className="text-[10px] uppercase tracking-widest text-zinc-500">Active</div>
            <div className="font-mono text-xs text-brass">{activeModel ?? 'auto-route'}</div>
          </div>
        </div>
      </Glass>
      {summary?.decision_preview && (
        <Glass className="p-4">
          <div className="font-display text-[11px] tracking-[0.2em] uppercase text-zinc-400">Decision Preview</div>
          <div className="mt-2 text-xs text-zinc-200 font-mono">{summary.decision_preview.selected_model}</div>
          <div className="text-[10px] text-zinc-500 mt-1">
            state={summary.decision_preview.discovery_state} · intel={(summary.decision_preview.intelligence_score ?? 0).toFixed(2)} · eff={(summary.decision_preview.efficiency_score ?? 0).toFixed(2)} · lat={(summary.decision_preview.latency_score ?? 0).toFixed(2)}
          </div>
          {summary.decision_preview.alternatives?.length ? (
            <div className="mt-2 text-[10px] text-zinc-500">
              alternatives: {summary.decision_preview.alternatives.slice(0, 3).join(', ')}
            </div>
          ) : null}
        </Glass>
      )}
      {loading && models.length === 0 ? (
        <Glass className="p-8 text-center text-zinc-500 text-sm">Loading model catalog…</Glass>
      ) : (
        <>
          <ModelGrid title="Hosted" items={hosted} activeModel={activeModel} onSetDefault={setDefault} />
          <ModelGrid title="Local / Mesh / MENS" items={local} activeModel={activeModel} onSetDefault={setDefault} />
        </>
      )}
    </div>
  );
}

function ModelGrid({ title, items, activeModel, onSetDefault }: {
  title: string; items: ModelCard[]; activeModel: string | null; onSetDefault: (id: string) => void;
}) {
  if (items.length === 0) return null;
  return (
    <section>
      <div className="mb-2 font-display text-[11px] tracking-[0.2em] uppercase text-zinc-400">{title}</div>
      <div role="list" className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
        {items.slice(0, 48).map(m => {
          const isActive = activeModel === m.id;
          return (
            <Glass key={m.id} role="listitem" className={`p-4 flex flex-col gap-3 ${isActive ? 'ring-1 ring-brass/40' : ''}`}>
              <div className="flex justify-between gap-2">
                <div className="min-w-0">
                  <div className="font-mono text-xs text-zinc-100 truncate" title={m.id}>{m.id}</div>
                  <div className="text-[10px] text-zinc-500">{m.provider} · {m.tier}</div>
                </div>
                {m.is_free && <span className="text-[9px] uppercase tracking-widest text-emerald-400">free</span>}
              </div>
              <div className="grid grid-cols-3 gap-2 text-[10px] font-mono text-zinc-400">
                <div><span className="text-zinc-600">ctx</span> {Math.round(m.max_tokens / 1000)}k</div>
                <div><span className="text-zinc-600">$/1k</span> {m.cost_per_1k.toFixed(4)}</div>
                <div><span className="text-zinc-600">p50</span> {m.latency_p50_ms ?? '—'}</div>
              </div>
              <button
                type="button"
                onClick={() => onSetDefault(m.id)}
                aria-pressed={isActive}
                aria-current={isActive ? 'true' : undefined}
                aria-label={`Set ${m.id} as active model${isActive ? ' (currently active)' : ''}`}
                className="mt-auto rounded-lg border border-white/10 px-3 py-1.5 text-[10px] uppercase tracking-widest hover:bg-white/5"
              >
                {isActive ? 'Active' : 'Set active'}
              </button>
            </Glass>
          );
        })}
      </div>
    </section>
  );
}
