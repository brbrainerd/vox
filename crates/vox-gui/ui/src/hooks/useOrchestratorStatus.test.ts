// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockGetBin = vi.fn();
const mockListen = vi.fn();
const mockGetVersionMismatch = vi.fn();

vi.mock('@msgpack/msgpack', () => ({
  decode: vi.fn(() => ({ agent_count: 2, agents: [], recent_events: [], alerts: [] })),
}));

vi.mock('../transport', () => ({
  listenOrchStatus: (...args: unknown[]) => mockListen(...args),
  voxTransport: {
    getOrchestratorStatusBin: () => mockGetBin(),
    getOrchestratorVersionMismatch: () => mockGetVersionMismatch(),
  },
}));

const recordGamifyMock = vi.fn(() => Promise.resolve(null));

vi.mock('../lib/gamifyGuiEvents', () => ({
  recordGamifyGuiEvent: (...args: unknown[]) => recordGamifyMock(...args),
}));

import {
  useOrchestratorStatus,
  ORCH_STATUS_QUERY_KEY,
  meshKpiFromStatus,
  orchestratorStatusErrorMessage,
  useOrchestratorFirstConnectGamify,
} from './useOrchestratorStatus';
import type { OrchestratorStatus } from '../types/tauri';

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return React.createElement(QueryClientProvider, { client }, children);
}

describe('useOrchestratorStatus', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetBin.mockResolvedValue(new Uint8Array([0x80]));
    mockListen.mockResolvedValue(() => {});
    mockGetVersionMismatch.mockResolvedValue(null);
  });

  it('exposes versionMismatch when the daemon reports a different version', async () => {
    mockGetVersionMismatch.mockResolvedValue(['0.0.1-stale', '0.6.0']);
    const { result } = renderHook(() => useOrchestratorStatus(), { wrapper });

    await waitFor(() =>
      expect(result.current.versionMismatch).toEqual({ daemon: '0.0.1-stale', gui: '0.6.0' }),
    );
  });

  it('exposes null versionMismatch when the daemon reports no mismatch', async () => {
    mockGetVersionMismatch.mockResolvedValue(null);
    const { result } = renderHook(() => useOrchestratorStatus(), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.versionMismatch).toBeNull();
  });

  it('fetches cold-start status via transport and subscribes to live updates', async () => {
    const { result } = renderHook(() => useOrchestratorStatus(), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(mockGetBin).toHaveBeenCalledTimes(1);
    expect(mockListen).toHaveBeenCalledTimes(1);
    expect(result.current.data?.agent_count).toBe(2);
    expect(result.current.isError).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('surfaces fetch errors via isError and error', async () => {
    mockGetBin.mockRejectedValue(new Error('daemon unreachable'));
    const { result } = renderHook(() => useOrchestratorStatus(), { wrapper });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(result.current.error?.message).toBe('daemon unreachable');
    expect(orchestratorStatusErrorMessage(result.current)).toBe('daemon unreachable');
  });

  it('surfaces listen failure via listenFailed and error message helper', async () => {
    mockListen.mockRejectedValue(new Error('not in tauri'));
    const { result } = renderHook(() => useOrchestratorStatus(), { wrapper });

    await waitFor(() => expect(result.current.listenFailed).toBe(true));
    expect(result.current.usesPolling).toBe(true);
    expect(orchestratorStatusErrorMessage(result.current)).toMatch(/live stream unavailable/i);
  });

  it('uses stable query key', () => {
    expect(ORCH_STATUS_QUERY_KEY).toEqual(['orchestrator', 'status']);
  });
});

describe('useOrchestratorFirstConnectGamify', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetBin.mockResolvedValue(new Uint8Array([0x80]));
    mockListen.mockResolvedValue(() => {});
  });

  it('records orchestrator_first_connect once when status query succeeds', async () => {
    const { result, rerender } = renderHook(
      () => {
        const query = useOrchestratorStatus();
        useOrchestratorFirstConnectGamify(query, true);
        return query;
      },
      { wrapper },
    );

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(recordGamifyMock).toHaveBeenCalledTimes(1);
    expect(recordGamifyMock).toHaveBeenCalledWith(
      'orchestrator_first_connect',
      undefined,
      { enabled: true },
    );

    rerender();
    expect(recordGamifyMock).toHaveBeenCalledTimes(1);
  });

  it('skips gamify event when disabled', async () => {
    renderHook(
      () => {
        const query = useOrchestratorStatus();
        useOrchestratorFirstConnectGamify(query, false);
        return query;
      },
      { wrapper },
    );

    await waitFor(() => expect(recordGamifyMock).toHaveBeenCalledWith(
      'orchestrator_first_connect',
      undefined,
      { enabled: false },
    ));
    expect(recordGamifyMock).toHaveBeenCalledTimes(1);
  });
});

const peer = (id: string) => ({
  id,
  name: id,
  backend: 'local',
  online: true,
});

describe('meshKpiFromStatus', () => {
  it('shows peer count when mesh_throughput is absent', () => {
    const status: OrchestratorStatus = {
      peers: [peer('a'), peer('b'), peer('c')],
    };
    const mesh = meshKpiFromStatus(status);
    expect(mesh.value).toBe('3 peers');
    expect(mesh.unit).toBe('');
    expect(mesh.peers).toBe(3);
    expect(String(mesh.value)).not.toContain('MB/s');
  });

  it('shows MB/s only when backend provides mesh_throughput', () => {
    const status: OrchestratorStatus = {
      mesh_throughput: 4.2,
      peers: [peer('a')],
    };
    const mesh = meshKpiFromStatus(status);
    expect(mesh.value).toBe(4.2);
    expect(mesh.unit).toBe('MB/s');
    expect(mesh.peers).toBe(1);
  });

  it('computes peer delta from previous snapshot when throughput absent', () => {
    const status: OrchestratorStatus = { peers: [peer('a'), peer('b')] };
    const mesh = meshKpiFromStatus(status, { value: '1 peer', peers: 1 });
    expect(mesh.delta).toBe(1);
  });
});
