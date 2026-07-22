import { useEffect, useRef, useState } from 'react';
import { useQueryClient, type UseQueryResult } from '@tanstack/react-query';
import { recordGamifyGuiEvent } from '../lib/gamifyGuiEvents';
import { decode } from '@msgpack/msgpack';
import type { OrchestratorStatus } from '../types/tauri';
import { listenOrchStatus, voxTransport } from '../transport';
import { useVoxQuery } from './useVoxQuery';

/** Daemon-vs-GUI version mismatch, surfaced by `orchestrator_version_mismatch`. */
export interface VersionMismatch {
  daemon: string;
  gui: string;
}

/** TanStack query result plus orchestrator-specific transport metadata. */
export type OrchestratorStatusHookResult = UseQueryResult<OrchestratorStatus, Error> & {
  /**
   * The live event stream is not currently delivering updates — either the
   * initial `listenOrchStatus` registration failed, or no status snapshot
   * has arrived recently (T3.1: the backend `PersistentDaemon` reconnect
   * loop self-heals a mid-session daemon death, so this is a "reconnecting /
   * stale" signal for the UI rather than a trigger for a client-side poll).
   */
  usesPolling: boolean;
  /** The `listenOrchStatus` subscription failed (fetch may still succeed). */
  listenFailed: boolean;
  /** Daemon/GUI version mismatch reported by the backend, or `null` if none. */
  versionMismatch: VersionMismatch | null;
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
  const [versionMismatch, setVersionMismatch] = useState<VersionMismatch | null>(null);

  const query = useVoxQuery<OrchestratorStatus>(
    ORCH_STATUS_QUERY_KEY,
    async () => {
      const raw = await voxTransport.getOrchestratorStatusBin();
      return decode(raw) as OrchestratorStatus;
    },
    { staleTime: 5_000, retry: false },
  );

  // T3.1: the backend `PersistentDaemon` reconnect loop
  // (`spawn_orchestrator_status_stream` in `crates/vox-gui/src/commands/orchestrator.rs`)
  // now self-heals a mid-session daemon death — it detects the dead daemon,
  // re-resolves (adopt/respawn), and resubscribes on its own. The frontend no
  // longer needs a timer-driven polling fallback to paper over a stream that
  // silently went quiet forever; it only needs to detect and surface
  // staleness/registration failure so the UI can show a reconnecting/stale
  // indicator (see `useFreshness`/`freshnessTone`).
  useEffect(() => {
    let unlisten: (() => void) | undefined;
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
          // Registration itself failed (e.g. non-Tauri/browser context).
          // Surface it; no client-side poll loop — the backend reconnect
          // loop is what recovers a live daemon, not this hook.
          setListenFailed(true);
          setUsesPolling(true);
        }
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [queryClient]);

  // Poll the backend's cached daemon/GUI version-mismatch state (T2/Task 2).
  // This mirrors the `PersistentDaemon.last_version_mismatch` cache updated
  // as a side effect of the daemon connect path; a lightweight interval poll
  // (rather than a dedicated event) is sufficient since a mismatch, once
  // true, does not need sub-second freshness to be useful to the user.
  useEffect(() => {
    let cancelled = false;
    const check = () => {
      voxTransport
        .getOrchestratorVersionMismatch()
        .then((result) => {
          if (cancelled) return;
          setVersionMismatch(result ? { daemon: result[0], gui: result[1] } : null);
        })
        .catch(() => {
          // Non-Tauri/browser preview context or command failure: leave
          // versionMismatch as-is rather than surfacing a false warning.
        });
    };
    check();
    const id = setInterval(check, 5_000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  return { ...query, usesPolling, listenFailed, versionMismatch };
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
