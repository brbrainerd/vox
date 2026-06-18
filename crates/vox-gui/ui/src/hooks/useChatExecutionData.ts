import { useEffect, useState } from 'react';
import { decode } from '@msgpack/msgpack';
import { filterBySession, type TaskRow } from '../components/surfaces/Tasks/tasksHelpers';
import type { ChatExecutionTask } from '../components/surfaces/Chat/ChatExecutionRail';
import type { OrchestratorStatus, RoutingSummary } from '../types/tauri';
import { voxTransport } from '../transport';

export const CHAT_EXECUTION_POLL_MS = 5_000;

export function mapOrchestratorTasksForSession(
  rows: TaskRow[],
  sessionId: string | undefined,
): ChatExecutionTask[] {
  if (!sessionId) return [];
  return filterBySession(rows, sessionId)
    .filter(t => t.lifecycle !== 'completed')
    .map(t => ({
      id: String(t.id),
      title: t.description,
      status: t.lifecycle,
    }));
}

export function intentsFromRoutingSummary(summary: RoutingSummary | null): string[] {
  const preview = summary?.decision_preview;
  if (!preview) return [];

  const intents: string[] = [];
  if (preview.selected_model) {
    const state = preview.discovery_state ? ` · ${preview.discovery_state}` : '';
    intents.push(`${preview.selected_model}${state}`);
  }

  for (const alt of preview.alternatives ?? []) {
    if (intents.length >= 3) break;
    intents.push(`Alt: ${alt}`);
  }

  return intents.slice(0, 3);
}

function meshPeersFromStatusBin(statusBin: Uint8Array | null): number {
  if (!statusBin) return 0;
  try {
    const status = decode(statusBin) as OrchestratorStatus;
    return (status.peers ?? []).length;
  } catch {
    return 0;
  }
}

export interface ChatExecutionData {
  tasks: ChatExecutionTask[];
  intents: string[];
  meshPeers: number;
}

export function useChatExecutionData(sessionId: string | undefined): ChatExecutionData {
  const [tasks, setTasks] = useState<ChatExecutionTask[]>([]);
  const [intents, setIntents] = useState<string[]>([]);
  const [meshPeers, setMeshPeers] = useState(0);

  useEffect(() => {
    if (!sessionId) {
      setTasks([]);
      setIntents([]);
      setMeshPeers(0);
      return;
    }

    let cancelled = false;

    const refresh = async () => {
      try {
        const [rows, summary, statusBin] = await Promise.all([
          voxTransport.listOrchestratorTasks(),
          voxTransport.getRoutingSummaryLive(),
          voxTransport.getOrchestratorStatusBin().catch(() => null),
        ]);
        if (cancelled) return;
        setTasks(mapOrchestratorTasksForSession(rows, sessionId));
        setIntents(intentsFromRoutingSummary(summary));
        setMeshPeers(meshPeersFromStatusBin(statusBin));
      } catch {
        if (!cancelled) {
          setTasks([]);
          setIntents([]);
          setMeshPeers(0);
        }
      }
    };

    refresh();
    const id = window.setInterval(refresh, CHAT_EXECUTION_POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [sessionId]);

  return { tasks, intents, meshPeers };
}
