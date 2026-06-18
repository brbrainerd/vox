import { useEffect, useRef, useState } from 'react';
import { useQueryClient, type UseQueryResult } from '@tanstack/react-query';
import { recordGamifyGuiEvent } from '../lib/gamifyGuiEvents';
import { decode } from '@msgpack/msgpack';
import type { OrchestratorStatus } from '../types/tauri';
import { listenOrchStatus, voxTransport } from '../transport';
import { useVoxQuery } from './useVoxQuery';
import { ORCH_POLL_FALLBACK_MS } from '../config/constants';

/** TanStack query result plus orchestrator-specific transport metadata. */
export type OrchestratorStatusHookResult = UseQueryResult<OrchestratorStatus, Error> & {
  /** Polling fallback is active because the live event stream is unavailable. */
  usesPolling: boolean;
  /** The `listenOrchStatus` subscription failed (fetch may still succeed). */
  listenFailed: boolean;
};

export interface OrchestratorStatusErrorFields {
  isError: boolean;
  error: Error | null;
  listenFailed: boolean;
}

/** Human-readable orchestrator error for HUD / console surfaces. */
export function orchestratorStatusErrorMessage(
  result: OrchestratorStatusErrorFields,
): string | null {
  if (result.isError && result.error) {
    return result.error.message;
  }
  if (result.listenFailed) {
    return 'Orchestrator live stream unavailable; polling for status';
  }
  return null;
}

export const ORCH_STATUS_QUERY_KEY = ['orchestrator', 'status'] as const;

export interface MeshKpiFields {
  value: number | string;
  unit: string;
  delta: number;
  peers: number;
  vramGb: number;
}

export interface MeshKpiPrev {
  value: number | string;
  peers: number;
}

/** Mesh HUD fields: throughput when daemon reports it, else honest peer count. */
export function meshKpiFromStatus(
  status: OrchestratorStatus,
  prev?: MeshKpiPrev,
): MeshKpiFields {
  const peerCount = (status.peers ?? []).length;
  const throughput = status.mesh_throughput;
  const hasThroughput = throughput != null && Number.isFinite(throughput);

  if (hasThroughput) {
    const prevThroughput = typeof prev?.value === 'number' ? prev.value : 0;
    return {
      value: throughput,
      unit: 'MB/s',
      delta: throughput - prevThroughput,
      peers: peerCount,
      vramGb: status.total_vram_gb ?? 0,
    };
  }

  const prevPeers = prev?.peers ?? 0;
  const peerLabel = peerCount === 1 ? '1 peer' : `${peerCount} peers`;
  return {
    value: peerLabel,
    unit: '',
    delta: peerCount - prevPeers,
    peers: peerCount,
    vramGb: status.total_vram_gb ?? 0,
  };
}

export function useOrchestratorStatus(): OrchestratorStatusHookResult {
  const queryClient = useQueryClient();
  const [usesPolling, setUsesPolling] = useState(false);
  const [listenFailed, setListenFailed] = useState(false);

  const query = useVoxQuery<OrchestratorStatus>(
    ORCH_STATUS_QUERY_KEY,
    async () => {
      const raw = await voxTransport.getOrchestratorStatusBin();
      return decode(raw) as OrchestratorStatus;
    },
    { staleTime: 5_000, retry: false },
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let fallbackInterval: ReturnType<typeof setInterval> | undefined;
    let cancelled = false;

    listenOrchStatus((status) => {
      setUsesPolling(false);
      setListenFailed(false);
      queryClient.setQueryData(ORCH_STATUS_QUERY_KEY, status);
    })
      .then((fn) => {
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch(() => {
        if (!cancelled) {
          setListenFailed(true);
          setUsesPolling(true);
          fallbackInterval = setInterval(() => {
            void queryClient.invalidateQueries({ queryKey: ORCH_STATUS_QUERY_KEY });
          }, ORCH_POLL_FALLBACK_MS);
        }
      });

    return () => {
      cancelled = true;
      unlisten?.();
      if (fallbackInterval !== undefined) clearInterval(fallbackInterval);
    };
  }, [queryClient]);

  return { ...query, usesPolling, listenFailed };
}

/** Fires Appendix C gamify hook once per session on first successful status fetch. */
export function useOrchestratorFirstConnectGamify(
  query: Pick<UseQueryResult<OrchestratorStatus, Error>, 'isSuccess' | 'data'>,
  gamifyEnabled: boolean,
): void {
  const firedRef = useRef(false);

  useEffect(() => {
    if (firedRef.current || !query.isSuccess || !query.data) return;
    firedRef.current = true;
    void recordGamifyGuiEvent('orchestrator_first_connect', undefined, {
      enabled: gamifyEnabled,
    });
  }, [query.isSuccess, query.data, gamifyEnabled]);
}
