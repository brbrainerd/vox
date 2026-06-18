// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import type { TaskRow } from '../components/surfaces/Tasks/tasksHelpers';
import type { RoutingSummary } from '../types/tauri';
import {
  useChatExecutionData,
  mapOrchestratorTasksForSession,
  intentsFromRoutingSummary,
  CHAT_EXECUTION_POLL_MS,
} from './useChatExecutionData';

const mockListOrchestratorTasks = vi.fn();
const mockGetRoutingSummaryLive = vi.fn();
const mockGetOrchestratorStatusBin = vi.fn();

vi.mock('@msgpack/msgpack', () => ({
  decode: vi.fn(() => ({ peers: [{ id: 'p1' }, { id: 'p2' }] })),
}));

vi.mock('../transport', () => ({
  voxTransport: {
    listOrchestratorTasks: () => mockListOrchestratorTasks(),
    getRoutingSummaryLive: () => mockGetRoutingSummaryLive(),
    getOrchestratorStatusBin: () => mockGetOrchestratorStatusBin(),
  },
}));

const taskRow = (over: Partial<TaskRow>): TaskRow => ({
  id: 1,
  description: 'Fix CI',
  priority: 'normal',
  lifecycle: 'in_progress',
  agent_id: null,
  session_id: 'sess-a',
  estimated_complexity: 1,
  depends_on: [],
  write_files: [],
  remote_node: null,
  ...over,
});

const routingSummary = (preview: RoutingSummary['decision_preview']): RoutingSummary => ({
  active_model: 'claude-sonnet-4',
  exploration_spent_usd: 0,
  exploration_budget_usd: 1,
  routing_priority: {
    efficiency: 50,
    precision: 50,
    latency: 50,
    availability: 50,
    balance: 50,
    mobile: 50,
  },
  arm_count: 1,
  model_count: 1,
  decision_preview: preview,
});

describe('mapOrchestratorTasksForSession', () => {
  it('filters list_orchestrator_tasks rows by session_id when provided', () => {
    const rows = [
      taskRow({ id: 1, session_id: 'sess-a', description: 'Task A' }),
      taskRow({ id: 2, session_id: 'sess-b', description: 'Task B' }),
      taskRow({ id: 3, session_id: 'sess-a', description: 'Task C', lifecycle: 'completed' }),
    ];

    expect(mapOrchestratorTasksForSession(rows, 'sess-a')).toEqual([
      { id: '1', title: 'Task A', status: 'in_progress' },
    ]);
  });

  it('returns no tasks when session_id is empty', () => {
    const rows = [taskRow({ id: 1 })];
    expect(mapOrchestratorTasksForSession(rows, '')).toEqual([]);
  });
});

describe('intentsFromRoutingSummary', () => {
  it('maps routing summary decision_preview into intent labels for rail', () => {
    const summary = routingSummary({
      selected_model: 'claude-sonnet-4',
      discovery_state: 'exploit',
      alternatives: ['gpt-4o', 'gemini-pro', 'mistral-large'],
      rejection_reasons: ['latency cap'],
      intelligence_score: 0.82,
      efficiency_score: 0.71,
      latency_score: 0.9,
    });

    expect(intentsFromRoutingSummary(summary)).toEqual([
      'claude-sonnet-4 · exploit',
      'Alt: gpt-4o',
      'Alt: gemini-pro',
    ]);
  });

  it('returns empty intents when decision_preview is null', () => {
    expect(intentsFromRoutingSummary(routingSummary(null))).toEqual([]);
  });
});

describe('useChatExecutionData', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
    mockListOrchestratorTasks.mockResolvedValue([
      taskRow({ id: 10, session_id: 'sess-a', description: 'Rail task' }),
    ]);
    mockGetRoutingSummaryLive.mockResolvedValue(
      routingSummary({
        selected_model: 'mens-v1',
        discovery_state: 'explore',
        alternatives: [],
        rejection_reasons: [],
        intelligence_score: 0.5,
        efficiency_score: 0.5,
        latency_score: 0.5,
      }),
    );
    mockGetOrchestratorStatusBin.mockResolvedValue(new Uint8Array([0x80]));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('loads tasks and intents for the active session', async () => {
    const { result } = renderHook(() => useChatExecutionData('sess-a'));

    await waitFor(() => expect(result.current.tasks).toHaveLength(1));
    expect(result.current.tasks[0]).toEqual({
      id: '10',
      title: 'Rail task',
      status: 'in_progress',
    });
    expect(result.current.intents).toEqual(['mens-v1 · explore']);
    expect(result.current.meshPeers).toBe(2);
    expect(mockListOrchestratorTasks).toHaveBeenCalled();
    expect(mockGetRoutingSummaryLive).toHaveBeenCalled();
  });

  it('does not poll when sessionId is empty', async () => {
    renderHook(() => useChatExecutionData(''));
    await act(async () => {
      await Promise.resolve();
    });
    expect(mockListOrchestratorTasks).not.toHaveBeenCalled();
  });

  it(`polls every ${CHAT_EXECUTION_POLL_MS / 1000}s when sessionId is set`, async () => {
    vi.useFakeTimers();
    renderHook(() => useChatExecutionData('sess-a'));
    await act(async () => {
      await Promise.resolve();
    });
    expect(mockListOrchestratorTasks).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(CHAT_EXECUTION_POLL_MS);
    });
    expect(mockListOrchestratorTasks).toHaveBeenCalledTimes(2);
    expect(mockGetRoutingSummaryLive).toHaveBeenCalledTimes(2);
  });
});
