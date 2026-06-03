import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { PUBLICATION_STAGES, groupByStage, PublicationManifest } from '../../../lib/pipeline';

export function PublicationsView({ pushToast }: SurfaceDecoratorProps) {
  const [manifests, setManifests] = useState<PublicationManifest[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setManifests(await invoke<PublicationManifest[]>('list_publication_manifests', { limit: 200 }));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Publications load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => { refresh(); }, [refresh]);

  const groups = groupByStage(manifests);

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Publication Pipeline</h2>
        <button onClick={refresh} disabled={loading}
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs hover:bg-white/[0.06]">
          {loading ? 'Loading…' : 'Refresh'}
        </button>
      </div>
      <div className="flex gap-3 overflow-x-auto pb-2">
        {PUBLICATION_STAGES.map(stage => (
          <div key={stage} className="w-56 shrink-0">
            <div className="mb-2 flex items-center justify-between">
              <span className="font-mono text-[10px] uppercase tracking-wide text-zinc-400">{stage.replace(/_/g, ' ')}</span>
              <span className="rounded-full bg-white/[0.05] px-1.5 font-mono text-[9px] text-zinc-500">{groups[stage].length}</span>
            </div>
            <div className="space-y-2">
              {groups[stage].map(m => (
                <div key={m.publication_id} className="rounded-lg border border-white/10 bg-white/[0.02] p-2">
                  <div className="truncate font-mono text-[11px] text-zinc-200">{m.publication_id}</div>
                  <div className="text-[10px] text-zinc-500">{m.content_type}</div>
                </div>
              ))}
              {groups[stage].length === 0 && <div className="rounded-lg border border-dashed border-white/5 p-2 text-center text-[10px] text-zinc-600">—</div>}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
