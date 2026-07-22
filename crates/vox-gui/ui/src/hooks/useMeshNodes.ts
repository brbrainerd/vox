import { useCallback, useEffect, useState } from 'react';
import { voxTransport } from '../transport';
import type { MeshNode } from '../components/surfaces/Mesh/MeshView';

/** Shape of the `vox_mesh_nodes` MCP tool result. */
export interface NodesResult {
  source?: string;
  control_url?: string;
  control_plane_error?: string;
  nodes?: MeshNode[];
  queue_depth?: number | null;
  node_count?: number;
}

/**
 * Fetches and normalizes the `vox_mesh_nodes` MCP tool result. This is the
 * single place that calls the tool and coerces `nodes` into an array — both
 * `useMeshNodes` and `useMeshNodesFull` poll via this helper so the
 * fetch/parse logic isn't duplicated between them.
 */
async function fetchMeshNodesResult(): Promise<NodesResult> {
  const res = await voxTransport.invokeMcpTool('vox_mesh_nodes', {});
  if (res?.is_error) {
    const msg =
      (res.result as { error?: string } | undefined)?.error ?? 'Failed to fetch mesh nodes';
    throw new Error(msg);
  }
  const result = (res?.result ?? {}) as NodesResult;
  return { ...result, nodes: Array.isArray(result.nodes) ? result.nodes : [] };
}

/**
 * Polls the `vox_mesh_nodes` MCP tool for a bare node list, at a
 * caller-supplied cadence. Silent on error — leaves the prior value (or
 * undefined) in place rather than flashing an error, which suits an
 * always-mounted, low-priority status tile (e.g. `BottomStatusBar`).
 * Callers that need the full result (source/error metadata, loading state,
 * an imperative refresh trigger) should use `useMeshNodesFull` instead.
 */
export function useMeshNodes(cadenceMs: number): MeshNode[] | undefined {
  const [nodes, setNodes] = useState<MeshNode[] | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;

    const refresh = async () => {
      try {
        const result = await fetchMeshNodesResult();
        if (cancelled) return;
        setNodes(result.nodes ?? []);
      } catch {
        // Silent — see doc comment above.
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

export interface MeshNodesFullResult {
  nodes: MeshNode[];
  meta: NodesResult;
  loading: boolean;
  /** Imperative re-fetch, for a manual "Refresh" action or post-mutation reload. */
  refresh: () => Promise<void>;
}

export interface UseMeshNodesFullOptions {
  /** When true, fetch once on mount and never start a repeating poll. */
  embedded?: boolean;
  onError?: (err: unknown) => void;
}

/**
 * Like `useMeshNodes`, but exposes the full `vox_mesh_nodes` result
 * (dispatch-availability source, control-plane error/url, queue depth),
 * a loading flag distinguishing "never loaded" from "loaded but empty",
 * and an imperative `refresh()` for manual/post-mutation reloads. Used by
 * `MeshView`, which needs this richer surface; `BottomStatusBar` uses the
 * plain `useMeshNodes` above since it only needs the node list.
 */
export function useMeshNodesFull(
  cadenceMs: number,
  opts?: UseMeshNodesFullOptions,
): MeshNodesFullResult {
  const embedded = opts?.embedded ?? false;
  const onError = opts?.onError;
  const [nodes, setNodes] = useState<MeshNode[]>([]);
  const [meta, setMeta] = useState<NodesResult>({});
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const result = await fetchMeshNodesResult();
      setMeta(result);
      setNodes(result.nodes ?? []);
    } catch (err) {
      onError?.(err);
    } finally {
      setLoading(false);
    }
  }, [onError]);

  useEffect(() => {
    refresh();
    // Embedded mini-render: one initial fetch only, no repeating poll.
    if (embedded) return;
    const id = window.setInterval(refresh, cadenceMs);
    return () => window.clearInterval(id);
  }, [refresh, embedded, cadenceMs]);

  return { nodes, meta, loading, refresh };
}
