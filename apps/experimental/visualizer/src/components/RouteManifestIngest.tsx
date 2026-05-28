import React, { useMemo, useState } from 'react';
import { FileJson, Plug } from 'lucide-react';
import { extractRoutePathsFromManifest, extractVoxClientEndpoints } from '../ingest/artifacts';

export function RouteManifestIngest() {
  const [manifest, setManifest] = useState('');
  const [client, setClient] = useState('');
  const paths = useMemo(() => extractRoutePathsFromManifest(manifest), [manifest]);
  const endpoints = useMemo(() => extractVoxClientEndpoints(client), [client]);

  return (
    <div className="p-10 h-full overflow-y-auto bg-[#09090b] text-zinc-200">
      <h2 className="text-2xl font-bold text-white mb-2 flex items-center gap-2">
        <FileJson className="text-blue-400" size={28} /> Web artifacts
      </h2>
      <p className="text-zinc-500 text-sm max-w-3xl mb-8">
        Paste <code className="text-blue-300">routes.manifest.ts</code> and optional{' '}
        <code className="text-blue-300">vox-client.ts</code> from a <code className="text-blue-300">vox build</code>{' '}
        output tree. Parsing is regex-based for quick inspection only.
      </p>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
        <div className="flex flex-col gap-2">
          <label className="text-xs font-bold uppercase tracking-widest text-zinc-500">routes.manifest.ts</label>
          <textarea
            className="min-h-[200px] rounded-xl border border-white/10 bg-black/40 p-4 font-mono text-xs text-zinc-300 focus:outline-none focus:ring-2 focus:ring-blue-500/40"
            placeholder="// Exported voxRoutes + path: entries"
            value={manifest}
            onChange={(e) => setManifest(e.target.value)}
          />
          <div className="text-xs text-zinc-500">Paths found: {paths.length}</div>
          <ul className="text-xs font-mono text-emerald-400/90 space-y-1 max-h-40 overflow-y-auto">
            {paths.map((p) => (
              <li key={p}>{p || '(empty)'}</li>
            ))}
          </ul>
        </div>

        <div className="flex flex-col gap-2">
          <label className="text-xs font-bold uppercase tracking-widest text-zinc-500 flex items-center gap-1">
            <Plug size={14} /> vox-client.ts
          </label>
          <textarea
            className="min-h-[200px] rounded-xl border border-white/10 bg-black/40 p-4 font-mono text-xs text-zinc-300 focus:outline-none focus:ring-2 focus:ring-blue-500/40"
            placeholder="// export async function myQuery(...)"
            value={client}
            onChange={(e) => setClient(e.target.value)}
          />
          <div className="text-xs text-zinc-500">Endpoints: {endpoints.length}</div>
          <ul className="text-xs font-mono text-amber-400/90 space-y-1 max-h-40 overflow-y-auto">
            {endpoints.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}
