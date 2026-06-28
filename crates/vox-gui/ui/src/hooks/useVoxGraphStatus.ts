import { useQuery } from '@tanstack/react-query';
import { voxTransport } from '../transport';
import { parseGraphifyStatus, type McpInvokeResult } from '../lib/mcpToolResult';
import type { GraphifyStatusDto } from '../types/tauri';

// VG-1 G9: renamed from useGraphifyStatus / GRAPHIFY_STATUS_QUERY_KEY.
// The transport seam (vs1 T8: read freshness through the shared MCP dispatch
// `vox_search_status`, ending the GUI/MCP split-brain) is preserved verbatim;
// this file only renames the hook and query key. All four useQuery options
// (queryKey / queryFn / staleTime / refetchInterval) are kept — dropping
// staleTime/refetchInterval would be a polling regression hidden in a rename.
export const VOX_GRAPH_STATUS_QUERY_KEY = ['vox-graph', 'status'];

/**
 * T8: read graphify/vox-search freshness through the shared MCP dispatch
 * (`vox_search_status`) rather than a dedicated `vox_graphify_status` Tauri
 * command, ending the GUI/MCP split-brain (spec §4). The Tauri command stays
 * registered for non-GUI callers; only this call path moves to the seam.
 */
async function fetchVoxSearchStatus(): Promise<GraphifyStatusDto> {
  const res = await voxTransport.invokeMcpTool('vox_search_status', {});
  return parseGraphifyStatus(res as McpInvokeResult) as GraphifyStatusDto;
}

export function useVoxGraphStatus() {
  return useQuery<GraphifyStatusDto, Error>({
    queryKey: VOX_GRAPH_STATUS_QUERY_KEY,
    queryFn: fetchVoxSearchStatus,
    staleTime: 30_000,
    refetchInterval: 60_000,
  });
}

/** Forward-compat alias for the eventual VoxSearchPanel rename (P5). */
export const useVoxSearchStatus = useVoxGraphStatus;
