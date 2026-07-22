import { useEffect, useState } from 'react';
import { voxTransport } from '../transport';
import type { MeshNode } from '../components/surfaces/Mesh/MeshView';

interface NodesResult {
  nodes?: MeshNode[];
}

/**
 * Polls the `vox_mesh_nodes` MCP tool for a bare node list, at a
 * caller-supplied cadence. Kept intentionally separate from `MeshView.tsx`'s
 * own fetch (which also pulls queue stats, dispatch-availability metadata,
 * and supports an embedded one-shot mode) — that fetch is tangled with
 * MeshView-only state, so this hook duplicates the minimal polling logic
 * rather than forcing a refactor. Callers mounted for the whole session
 * (e.g. the always-mounted BottomStatusBar) should pass a slower cadence
 * than MeshView's own `REFRESH_MS` to avoid adding steady-state load on the
 * orchestrator daemon for a one-line summary figure.
 */
export function useMeshNodes(cadenceMs: number): MeshNode[] | undefined {
  const [nodes, setNodes] = useState<MeshNode[] | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;

    const refresh = async () => {
      try {
        const res = await voxTransport.invokeMcpTool('vox_mesh_nodes', {});
        if (cancelled) return;
        const result = (res?.result ?? {}) as NodesResult;
        setNodes(Array.isArray(result.nodes) ? result.nodes : []);
      } catch {
        // Silent — leave prior value (or undefined) in place rather than
        // flashing an error for an always-mounted, low-priority status tile.
      }
    };

    refresh();
    const id = window.setInterval(refresh, cadenceMs);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [cadenceMs]);

  return nodes;
}
