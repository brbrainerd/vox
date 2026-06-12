import React, { useCallback, useEffect, useReducer, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { decode } from '@msgpack/msgpack';
import { Backdrop } from './components/ui/Backdrop';
import { Sidebar, SidebarMode } from './components/layout/Sidebar';
import { TopHud, type HudMode } from './components/layout/TopHud';
import { DockShell } from './components/layout/DockShell';
import { renderSurfaceView } from './components/layout/surfaceComponents';
import { resolveNavigation } from './lib/navigation';
import { CommandPalette } from './components/layout/CommandPalette';
import { Toasts, ToastItem } from './components/ui/Toasts';
import { SurfaceErrorBoundary } from './components/ui/ErrorBoundary';
import { Loquela } from './components/surfaces/Loquela/Loquela';
import { Transcript } from './components/surfaces/Loquela/Transcript';
import { DiffReview } from './components/surfaces/Loquela/DiffReview';
import { InlineApprovals } from './components/surfaces/Loquela/InlineApprovals';
import { parsePendingApprovals, type McpInvokeResult } from './lib/mcpToolResult';
import {
  assistantMessagesReadyToPersist,
  assistantPersistContent,
} from './lib/chatCorrelation';
import {
  getSessionMessages,
  initialSessionChatStore,
  sessionChatReducer,
} from './lib/sessionChatStore';
import { contextRefsFromPayload } from './lib/loquelaContext';
import { overallWorst, worstCount } from './components/surfaces/Policies/policyTree';
import type { PolicyRow, PolicyStatus, BranchInfo, RunStatus } from './components/surfaces/Policies/types';
import { voxTransport, listenOrchStatus, listenAgentEvents, type AgentEventFrame } from './transport';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useLocalStorage } from './hooks/useLocalStorage';
import { usePersistedSparkWindow } from './hooks/useSparkWindow';
import { DashboardData, Agent, StreamItem, LudusAlert } from './types/dashboard';
import type {
  ActiveSkill,
  ChatPayload,
  CommandCatalog,
  CommandPaletteAction,
  ContextChip,
  OrchestratorStatus,
  RawAgentSummary,
  RawLudusAlert,
  RawStreamEvent,
  Session,
  SubmitTaskResult,
  Toast,
} from './types/tauri';
import { INITIAL_DATA, INITIAL_KPIS } from './data/initialState';
import {
  ORCH_POLL_FALLBACK_MS,
  POLICY_BADGE_POLL_MS,
  APPROVALS_POLL_MS,
  STREAM_CAP,
  LIVE_EVENT_FRESH_MS,
} from './config/constants';
import { budgetStateFromStatus, DEFAULT_BUDGET_CAP_USD } from './config/budget';
import { nextId, nextGuiRunId } from './lib/ids';
import { slashCommandBase } from './lib/slashRouter';

type View =
  | 'dashboard'
  | 'flow'
  | 'catalog'
  | 'matrix'
  | 'memory'
  | 'models'
  | 'runs'
  | 'repository'
  | 'mesh'
  | 'gamify'
  | 'harness'
  | 'browser'
  | 'console'
  | 'scientia'
  | 'discovery-review'
  | 'discovery-inbox'
  | 'archive-panel'
  | 'claims'
  | 'mens'
  | 'populi'
  | 'research'
  | 'oratio'
  | 'approvals'
  | 'policies'
  | 'skills'
  | 'settings'
  | 'coverage'
  | 'publications'
  | 'search'
  | 'chat'
  | 'agents'
  | 'workspace'
  | 'commands'
  | 'knowledge'
  | 'compute';

const LEGACY_VIEWS: string[] = [
  'dashboard', 'flow', 'catalog', 'matrix', 'memory', 'models', 'runs', 'repository',
  'mesh', 'gamify', 'harness', 'browser', 'console', 'scientia', 'discovery-review', 'discovery-inbox', 'archive-panel', 'claims', 'mens',
  'populi', 'research', 'oratio', 'approvals', 'policies', 'skills', 'settings', 'coverage',
  'publications', 'search', 'chat', 'agents', 'workspace', 'commands', 'knowledge', 'compute',
  'review',
];

// Single source of truth for valid view ids (deep-link validation + initial-view).
const KNOWN_VIEWS: string[] = [
  'dashboard',
  'flow',
  'catalog',
  'matrix',
  'memory',
  'models',
  'runs',
  'repository',
  'mesh',
  'gamify',
  'harness',
  'scientia',
  'discovery-review',
  'discovery-inbox',
  'archive-panel',
  'claims',
  'mens',
  'populi',
  'research',
  'oratio',
  'approvals',
  'policies',
  'skills',
  'settings',
  'coverage',
  'publications',
  'search',
];

function isKnownView(v: unknown): v is View {
  return typeof v === 'string' && KNOWN_VIEWS.includes(v);
}

// ─── Agent mapper — shared between EventBus and polling fallback ─────────────
function mapAgent(a: RawAgentSummary): Agent {
  const inProgress = a.in_progress ?? false;
  const rawProgress = a.progress;
  const progress =
    typeof rawProgress === 'number' && Number.isFinite(rawProgress)
      ? rawProgress
      : inProgress
        ? null
        : 0;
  const rawBudget = a.budget;
  const budget =
    typeof rawBudget === 'number' && Number.isFinite(rawBudget) ? rawBudget : null;
  return {
    id: `A-${String(a.id).padStart(2, '0')}`,
    codename: a.codename ?? 'Agent',
    phase: a.paused ? 'Paused' : (inProgress ? (a.current_phase ?? 'Executing') : 'Idle'),
    progress,
    task: a.task_description ?? (inProgress ? 'Processing…' : 'Idle'),
    cost: typeof a.cost === 'number' ? a.cost : 0,
    budget,
    eta: a.eta ?? '—',
    skill: a.active_skill,
  };
}

function mapStream(e: RawStreamEvent): StreamItem {
  return {
    id: e.id != null ? String(e.id) : nextId('stream'),
    kind: e.kind ?? 'system',
    tag: e.tag ?? 'SYSTEM',
    title: e.title ?? 'Event',
    body: e.body ?? '',
    ts: e.timestamp ?? 'now',
  };
}

// ─── Live AgentEvent (B4 "vox://agent-events") → dashboard StreamItem ─────────
const AGENT_EVENT_LABELS: Record<string, string> = {
  token_streamed: 'TOKEN',
  task_started: 'TASK',
  task_phase_changed: 'PHASE',
  task_completed: 'DONE',
  task_failed: 'FAILED',
  agent_spawned: 'SPAWN',
  agent_retired: 'RETIRE',
  cost_incurred: 'COST',
  snapshot_captured: 'CHECKPOINT',
  activity_changed: 'ACTIVITY',
  tool_timed_out: 'TOOL',
};

function mapAgentEvent(e: AgentEventFrame): StreamItem {
  const kind = e.kind ?? ({ type: 'unknown' } as AgentEventFrame['kind']);
  const type = kind.type ?? 'unknown';
  const tag = AGENT_EVENT_LABELS[type] ?? type.replace(/_/g, ' ').toUpperCase();

  // Build a human-readable title/body per variant.
  let title = type.replace(/_/g, ' ');
  let body = '';
  switch (type) {
    case 'token_streamed':
      title = `Token · ${kind.agent_id ?? '?'}`;
      body = String(kind.text ?? '');
      break;
    case 'task_started':
    case 'task_phase_changed':
    case 'task_completed':
    case 'task_failed':
      title = `${tag} · task ${kind.task_id ?? '?'}`;
      body = kind.phase
        ? `phase: ${kind.phase}`
        : kind.error
          ? `error: ${kind.error}`
          : kind.agent_id
            ? `agent ${kind.agent_id}`
            : '';
      break;
    case 'agent_spawned':
    case 'agent_retired':
      title = `${tag} · agent ${kind.agent_id ?? '?'}`;
      break;
    case 'snapshot_captured':
      title = `${tag} · ${kind.snapshot_id ?? 'snapshot'}`;
      body = typeof kind.description === 'string' ? kind.description : '';
      break;
    case 'activity_changed':
      title = `${tag} · agent ${kind.agent_id ?? '?'}`;
      body = typeof kind.activity === 'string' ? kind.activity : '';
      break;
    default:
      body = '';
  }

  const isFailed = type === 'task_failed';
  return {
    // Numeric event id correlates with orchestrator doubt/overrule task ids.
    id: String(e.id),
    kind: isFailed ? 'doubted' : 'agent',
    tag,
    title,
    body,
    ts: e.timestamp_ms
      ? new Date(e.timestamp_ms).toLocaleTimeString()
      : 'now',
  };
}

function mapAlert(a: RawLudusAlert): LudusAlert {
  return { id: a.id, level: a.level, title: a.title, body: a.body };
}

export default function App() {
  const [data, setData] = useState<DashboardData>(INITIAL_DATA);
  const [kpis, setKpis] = useState(INITIAL_KPIS);
  const [activeView, setActiveView] = useLocalStorage<View>('vox_active_view', 'dashboard');
  const [sidebarMode, setSidebarMode] = useLocalStorage<SidebarMode>('vox_sidebar_mode', 'default');
  const [isCommandOpen, setIsCommandOpen] = useState(false);
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const [filterKind, setFilterKind] = useState('all');
  const [chips, setChips] = useState<ContextChip[]>([]);
  const [activeSkill, setActiveSkill] = useState<ActiveSkill | null>(null);
  const [deployedSet, setDeployedSet] = useState(new Set<string>());
  const [selectedAgentId, setSelectedAgentId] = useState('ROOT');
  const [appVersion, setAppVersion] = useState<string>('loading…');
  // Master-sidebar Policies badge: worst-status count for the current branch.
  const [policyBadge, setPolicyBadge] = useState<{ count: number; status: RunStatus } | null>(null);
  const [lastOrchEventAt, setLastOrchEventAt] = useState<number | null>(null);
  const [orchUsesPolling, setOrchUsesPolling] = useState(false);
  const [hudMode, setHudMode] = useLocalStorage<HudMode>('vox_hud_mode', 'full');
  const [activeSessionId, setActiveSessionId] = useState<string>('');
  const [approvalsPending, setApprovalsPending] = useState(0);
  const [diffOpen, setDiffOpen] = useState(false);
  const [diffText, setDiffText] = useState('');
  const [diffLoading, setDiffLoading] = useState(false);

  // ── B4-chat: pure-reducer transcript state for the Loquela composer ────────
  const [chatStore, dispatchSessionChat] = useReducer(sessionChatReducer, initialSessionChatStore);
  const persistedAssistantIdsRef = useRef<Map<string, Set<string>>>(new Map());
  const activeChatMessages = getSessionMessages(chatStore, activeSessionId);

  // ── 5-minute rolling sparkline windows ──────────────────────────────────
  // Each hook persists its window to localStorage under a namespaced key.
  const agentCountWindow = usePersistedSparkWindow('kpi.agentCount', kpis.activeAgents.value);
  const queueDepthWindow = usePersistedSparkWindow('kpi.queueDepth', kpis.queueDepth.value);
  const budgetBurnWindow = usePersistedSparkWindow('kpi.budgetBurn', kpis.budgetBurn.value);
  const meshWindow       = usePersistedSparkWindow('kpi.mesh', typeof kpis.mesh.value === 'number' ? kpis.mesh.value : 0);

  // ── Toast helper ─────────────────────────────────────────────────────────
  const pushToast = useCallback((t: Toast) => {
    const id = nextId('toast');
    setToasts(curr => [...curr, { ...t, id }]);
    setTimeout(() => setToasts(curr => curr.filter(x => x.id !== id)), 5000);
  }, []);

  // ── KPI update — shared logic used by both EventBus listener and fallback ─
  const applyStatus = useCallback((status: OrchestratorStatus) => {
    const agents: Agent[] = (status.agents ?? []).map(mapAgent);
    const stream: StreamItem[] = (status.recent_events ?? []).map(mapStream);
    const alerts: LudusAlert[] = (status.alerts ?? []).map(mapAlert);

    setData(prev => ({
      ...prev,
      agents,
      stream: stream.length > 0 ? stream : prev.stream,
      alerts,
      peers: (status.peers ?? []).length > 0 ? status.peers : prev.peers,
    }));

    const budget = budgetStateFromStatus(status.total_cost, status.budget_cap);
    setKpis(prev => ({
      activeAgents: {
        value: status.agent_count ?? 0,
        delta: (status.agent_count ?? 0) - prev.activeAgents.value,
        spark: agentCountWindow,
      },
      queueDepth: {
        value: status.total_queued ?? 0,
        delta: (status.total_queued ?? 0) - prev.queueDepth.value,
        spark: queueDepthWindow,
      },
      budgetBurn: {
        value: budget.spent,
        cap: budget.cap ?? DEFAULT_BUDGET_CAP_USD,
        source: budget.source,
        delta: budget.spent - prev.budgetBurn.value,
        spark: budgetBurnWindow,
      },
      mesh: {
        value: `${status.mesh_throughput ?? 0} MB/s`,
        unit: 'MB/s',
        delta: 0,
        spark: meshWindow,
        peers: (status.peers ?? []).length,
        vramGb: status.total_vram_gb ?? 0,
      },
    }));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [agentCountWindow, queueDepthWindow, budgetBurnWindow, meshWindow]);

  // ── Bootstrap: catalog, initial view ─────────────────────────────────────
  useEffect(() => {
    voxTransport.getCatalog().then((catalog: CommandCatalog) => {
      if (catalog?.entries) setData(prev => ({ ...prev, skills: catalog.entries }));
    });
    invoke<{ display?: string; version?: string }>('get_build_info')
      .then((info) => setAppVersion(info.display ?? info.version ?? 'unknown'))
      .catch(() => setAppVersion('unknown'));

    invoke<string>('get_initial_view').then((view) => {
      if (view && LEGACY_VIEWS.includes(view)) {
        setActiveView(view as View);
      }
    }).catch(() => {});

    invoke<Session[]>('chat_list_sessions', { limit: 1 })
      .then((sessions) => {
        if (sessions?.[0]?.session_id) {
          setActiveSessionId(sessions[0].session_id);
        } else {
          invoke<Session>('chat_create_session', { title: 'Chat' })
            .then((s) => setActiveSessionId(s.session_id))
            .catch((err) => pushToast({ tone: 'warn', title: 'Chat session', body: String(err) }));
        }
      })
      .catch((err) => pushToast({ tone: 'warn', title: 'Chat sessions', body: String(err) }));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Master-sidebar Policies badge: poll the catalog + current-branch status,
  // compute the worst status + count of rules at that tier, color the nav badge.
  // Lightweight (in-process IPC) and resilient: any failure clears the badge.
  useEffect(() => {
    let cancelled = false;
    const refresh = async () => {
      try {
        const [rows, branchInfos] = await Promise.all([
          invoke<PolicyRow[]>('policy_list', { domain: null, group: null }),
          invoke<BranchInfo[]>('list_branches').catch(() => [] as BranchInfo[]),
        ]);
        const branches = branchInfos.filter(b => b.isCurrent).map(b => b.branch);
        const sel = branches.length ? branches : ['HEAD'];
        const status = await invoke<PolicyStatus[]>('policy_status', { branches: sel }).catch(() => [] as PolicyStatus[]);
        if (cancelled) return;
        setPolicyBadge({ status: overallWorst(rows, status, sel), count: worstCount(rows, status, sel) });
      } catch {
        if (!cancelled) setPolicyBadge(null);
      }
    };
    refresh();
    const id = setInterval(refresh, POLICY_BADGE_POLL_MS);
    return () => { cancelled = true; clearInterval(id); };
  }, []);

  // ── Pushed status stream (B1: "vox://orch-status" Tauri event) ─────────────
  // Primary path is the daemon-pushed event stream. We keep one initial snapshot
  // fetch on mount, and fall back to polling ONLY if the listener can't be set
  // up (e.g. running outside Tauri in a plain browser dev session).
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let fallbackInterval: ReturnType<typeof setInterval> | undefined;
    let cancelled = false;

    // One-shot initial snapshot via the existing binary status command.
    // We use `get_orchestrator_status_bin` to fetch raw MessagePack payloads,
    // bypassing Tauri's default JSON string-escaping overhead for large states.
    const fetchSnapshot = async () => {
      try {
        const rawBytes = await invoke<Uint8Array>('get_orchestrator_status_bin');
        setLastOrchEventAt(Date.now());
        applyStatus(decode(rawBytes));
      } catch (err) {
        // Silently ignore if backend is down or not ready.
      }
    };

    const startFallbackPolling = () => {
      if (fallbackInterval !== undefined) return;
      fallbackInterval = setInterval(fetchSnapshot, ORCH_POLL_FALLBACK_MS);
    };

    // Initial snapshot for first paint.
    fetchSnapshot();

    // Subscribe to the pushed event stream; fall back to polling on failure.
    listenOrchStatus((status) => {
      setLastOrchEventAt(Date.now());
      setOrchUsesPolling(false);
      applyStatus(status);
    })
      .then((fn) => {
        if (cancelled) {
          // Effect already cleaned up before subscription resolved.
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch(() => {
        // listen() unavailable (e.g. plain browser) — degrade to polling.
        if (!cancelled) {
          setOrchUsesPolling(true);
          startFallbackPolling();
        }
      });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
      if (fallbackInterval !== undefined) clearInterval(fallbackInterval);
    };
  }, [applyStatus]);

  // ── Approvals badge for Runs nav ───────────────────────────────────────────
  useEffect(() => {
    let cancelled = false;
    const refresh = async () => {
      try {
        const res = await invoke<McpInvokeResult>('invoke_mcp_tool', {
          tool: 'vox_pending_approvals',
          args: {},
        });
        const pending = parsePendingApprovals(res);
        if (!cancelled) setApprovalsPending(pending.length);
      } catch {
        if (!cancelled) setApprovalsPending(0);
      }
    };
    refresh();
    const id = setInterval(refresh, APPROVALS_POLL_MS);
    return () => { cancelled = true; clearInterval(id); };
  }, []);

  // ── Pushed live agent-event stream (B4: "vox://agent-events" Tauri event) ──
  // Each AgentEvent is prepended to the dashboard activity feed as a StreamItem.
  // We keep this independent from the status subscription above; the list is
  // capped so it does not grow unbounded.
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listenAgentEvents((frame) => {
      const kindType = frame.kind?.type ?? '';
      if (kindType !== 'token_streamed') {
        const item = mapAgentEvent(frame);
        setData(prev => ({
          ...prev,
          stream: [item, ...prev.stream].slice(0, STREAM_CAP),
        }));
      }
      dispatchSessionChat({ type: 'agentEvent', event: frame });
    })
      .then((fn) => {
        if (cancelled) {
          // Effect already cleaned up before subscription resolved.
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch(() => {
        // listen() unavailable (e.g. plain browser dev) — no live feed.
      });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const executeIpcWithRun = useCallback(async <T,>(command: string, payload: Record<string, unknown>, workflowName: string, onRun?: (runId: string) => void): Promise<T> => {
    const runId = nextGuiRunId();
    onRun?.(runId);
    await invoke('start_gui_run', {
      input: {
        run_id: runId,
        workflow_name: workflowName,
        planned_steps: 1,
      }
    });
    let finished = false;
    try {
      const result = await invoke<T>(command, payload);
      await invoke('finish_gui_run', {
        run_id: runId,
        success: true,
        completed_steps: 1,
        error: null
      });
      finished = true;
      return result;
    } catch (err) {
      if (!finished) {
        await invoke('finish_gui_run', {
          run_id: runId,
          success: false,
          completed_steps: 0,
          error: String(err),
        }).catch(() => {});
      }
      throw err;
    }
  }, []);

  // ── Global keybinds ───────────────────────────────────────────────────────
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === 'k') { e.preventDefault(); setIsCommandOpen(true); }
      if (mod && e.key.toLowerCase() === 'b') {
        e.preventDefault();
        setSidebarMode(m => m === 'rail' ? 'default' : m === 'default' ? 'wide' : 'rail');
      }
      if (mod && e.shiftKey && e.key.toLowerCase() === 'h') {
        e.preventDefault();
        setHudMode(m => (m === 'full' ? 'slim' : m === 'slim' ? 'hidden' : 'full'));
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Cross-surface deep-link: a surface may request navigation to another
  // surface (optionally seeding a value) by dispatching a `vox://navigate-surface`
  // CustomEvent. Used by the Discovery Inbox "Open review" action to jump to the
  // Discovery Review surface with the publication id pre-filled.
  useEffect(() => {
    const onNavigate = (e: Event) => {
      const detail = (e as CustomEvent<{ view?: string; publicationId?: string }>).detail;
      if (!detail?.view || !isKnownView(detail.view)) return;
      if (detail.publicationId != null) {
        try {
          window.localStorage.setItem('vox_discovery_review_seed', detail.publicationId);
        } catch { /* localStorage unavailable — surface still switches */ }
      }
      setActiveView(detail.view);
    };
    window.addEventListener('vox://navigate-surface', onNavigate as EventListener);
    return () => window.removeEventListener('vox://navigate-surface', onNavigate as EventListener);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Action handlers ───────────────────────────────────────────────────────
  const navigateTo = useCallback((viewKey: string) => {
    const { child } = resolveNavigation(viewKey);
    setActiveView(child as View);
  }, [setActiveView]);

  const hydrateChatSession = useCallback(async (sessionId: string) => {
    if (!sessionId) return;
    try {
      const rows = await invoke<
        Array<{ id: number; role: string; content: string; task_id?: string }>
      >('chat_get_messages', { sessionId, limit: 500 });
      dispatchSessionChat({
        type: 'hydrate',
        sessionId,
        messages: rows.map(r => ({
          id: String(r.id),
          role: r.role as 'user' | 'assistant' | 'system',
          text: r.content,
          status: 'done' as const,
          runId: r.task_id ?? `persist-${r.id}`,
          taskId: r.task_id ?? undefined,
        })),
      });
    } catch {
      // keep live state when DB read fails
    }
  }, []);

  useEffect(() => {
    if (activeSessionId) hydrateChatSession(activeSessionId);
  }, [activeSessionId, hydrateChatSession]);

  const loadTaskDiff = useCallback(async (path?: string) => {
    setDiffOpen(true);
    setDiffLoading(true);
    try {
      const text = await invoke<string>('get_task_diff', { path: path ?? null });
      setDiffText(text);
    } catch (err) {
      setDiffText('');
      pushToast({ tone: 'warn', title: 'Diff failed', body: String(err) });
    } finally {
      setDiffLoading(false);
    }
  }, [pushToast]);

  const handleLoquelaSubmit = useCallback(async (payload: ChatPayload) => {
    pushToast({ tone: 'info', title: 'Task Dispatched', body: payload.description, cmd: 'vox submit-task' });
    let runId = '';
    const sessionId = payload.session_id ?? activeSessionId;
    if (!sessionId) {
      pushToast({ tone: 'warn', title: 'No chat session', body: 'Create or select a chat session first.' });
      return;
    }
    invoke('chat_append_message', {
      input: { session_id: sessionId, role: 'user', content: String(payload.description ?? ''), task_id: null },
    }).catch((err) => pushToast({ tone: 'warn', title: 'Message not saved', body: String(err) }));
    const contextFiles = contextRefsFromPayload(payload);

    // One submit attempt. `allowDuplicate=false` lets the daemon refuse a
    // near-duplicate (returning `duplicate_of` with a null task_id) so we can
    // ask the user instead of silently enqueuing the same work twice.
    const dispatchAttempt = (allowDuplicate: boolean) =>
      executeIpcWithRun<SubmitTaskResult>(
        'submit_orchestrator_task',
        {
          input: {
            description: payload.description,
            files: contextFiles,
            priority: payload.priority ?? null,
            session_id: sessionId || null,
            mode: payload.mode ?? null,
            model_hint: payload.model_hint ?? payload.tier ?? null,
            dry_run: payload.dry_run ?? null,
            active_skill: payload.active_skill ?? activeSkill?.id ?? null,
            allow_duplicate: allowDuplicate,
          }
        },
        'gui.loquela.submit',
        // Mint the runId and create the user/assistant bubbles BEFORE the invoke
        // resolves so streamed tokens correlate to a live transcript entry.
        (id) => {
          runId = id;
          dispatchSessionChat({
            type: 'submit',
            sessionId,
            runId: id,
            prompt: String(payload.description ?? ''),
          });
        },
      );

    try {
      let result = await dispatchAttempt(false);
      // Refused as a near-duplicate: retract the optimistic bubble and ask.
      if (result?.task_id == null && result?.duplicate_of) {
        if (runId) {
          dispatchSessionChat({
            type: 'failRun',
            sessionId,
            runId,
            error: `Skipped — near-duplicate of task #${result.duplicate_of}`,
          });
        }
        const proceed = window.confirm(
          `This looks like a near-duplicate of task #${result.duplicate_of}.\n\nSubmit it anyway?`,
        );
        if (!proceed) {
          pushToast({ tone: 'info', title: 'Duplicate skipped', body: `Kept existing task #${result.duplicate_of}.` });
          return;
        }
        result = await dispatchAttempt(true);
      }
      if (runId && result?.task_id != null && sessionId) {
        dispatchSessionChat({
          type: 'submitResolved',
          sessionId,
          runId,
          taskId: String(result.task_id),
        });
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Dispatch Failed', body: String(err) });
    }
  }, [executeIpcWithRun, pushToast, activeSessionId, activeSkill]);

  const handleLoquelaSlash = useCallback(async (
    cmd: string,
    ctx: { setText: (text: string) => void },
  ): Promise<boolean> => {
    const base = slashCommandBase(cmd);

    if (base === '/diff') {
      loadTaskDiff();
      return true;
    }
    if (base === '/memory') {
      navigateTo('memory');
      return true;
    }
    if (base === '/spawn') {
      void handleLoquelaSubmit({
        description: 'Spawn a sub-agent on the current branch to pursue this task in parallel.',
        mode: 'act',
      });
      return true;
    }
    if (base === '/rollback') {
      void (async () => {
        try {
          const res = await invoke<McpInvokeResult>('invoke_mcp_tool', { tool: 'vox_undo', args: {} });
          pushToast({
            tone: res.is_error ? 'warn' : 'ok',
            title: res.is_error ? 'Rollback failed' : 'Rollback complete',
            body: res.is_error
              ? (typeof res.result === 'string' ? res.result : JSON.stringify(res.result))
              : 'Reverted to last durable checkpoint.',
          });
        } catch (err) {
          pushToast({ tone: 'warn', title: 'Rollback failed', body: String(err) });
        }
      })();
      return true;
    }
    if (base === '/doubt') {
      ctx.setText('/doubt ');
      pushToast({
        tone: 'info',
        title: 'Doubt injection',
        body: 'Add a threshold (e.g. /doubt 3) and submit, or doubt a stream item from the feed.',
      });
      return true;
    }
    if (base === '/audit') {
      void (async () => {
        try {
          const out = await invoke<{ exit_code: number; stdout: string; stderr: string }>('execute_command', {
            path: ['check'],
            args: { __argv: [] },
          });
          const text = [out.stdout, out.stderr].filter(Boolean).join('\n').trim();
          pushToast({
            tone: out.exit_code === 0 ? 'ok' : 'warn',
            title: out.exit_code === 0 ? 'Audit passed' : 'Audit findings',
            body: text.slice(0, 400) || 'vox check completed',
          });
        } catch {
          try {
            const res = await invoke<McpInvokeResult>('invoke_mcp_tool', { tool: 'vox_check', args: {} });
            const body = typeof res.result === 'string'
              ? res.result.slice(0, 400)
              : JSON.stringify(res.result).slice(0, 400);
            pushToast({
              tone: res.is_error ? 'warn' : 'ok',
              title: res.is_error ? 'Audit failed' : 'Audit complete',
              body: body || 'vox_check finished',
            });
          } catch (err) {
            pushToast({ tone: 'warn', title: 'Audit unavailable', body: String(err) });
          }
        }
      })();
      return true;
    }
    return false;
  }, [loadTaskDiff, navigateTo, handleLoquelaSubmit, pushToast]);

  // Persist assistant transcript rows when a run completes (user rows persist on submit).
  useEffect(() => {
    for (const [sessionId, state] of Object.entries(chatStore.sessions)) {
      let persisted = persistedAssistantIdsRef.current.get(sessionId);
      if (!persisted) {
        persisted = new Set();
        persistedAssistantIdsRef.current.set(sessionId, persisted);
      }
      const ready = assistantMessagesReadyToPersist(state.messages, persisted);
      for (const m of ready) {
        persisted.add(m.id);
        const content = assistantPersistContent(m);
        if (!content.trim()) continue;
        invoke('chat_append_message', {
          input: {
            session_id: sessionId,
            role: 'assistant',
            content,
            task_id: m.taskId ?? null,
          },
        }).catch(() => {});
      }
    }
  }, [chatStore]);

  // Attach one or more locators to the shared Loquela context set. These chips
  // become the next task's file manifest (see handleLoquelaSubmit), so this is
  // a real "pin to context" — not a toast-only gesture. Deduped by chip id.
  const attachContext = useCallback((items: Array<{ kind: 'file' | 'url' | 'image'; label: string }>) => {
    if (items.length === 0) return;
    setChips(prev => {
      const seen = new Set(prev.map(c => c.id));
      const next = [...prev];
      for (const it of items) {
        const id = `ctx-${it.kind}-${it.label}`;
        if (seen.has(id)) continue;
        seen.add(id);
        next.push({ id, kind: it.kind, label: it.label });
      }
      return next;
    });
    pushToast({
      tone: 'ok',
      title: items.length === 1 ? 'Pinned to context' : `${items.length} pinned to context`,
      body: items.length === 1 ? items[0].label : `${items.length} citations → Loquela`,
      cmd: 'context.attach',
    });
  }, [pushToast]);

  const handlePause = useCallback(async (a: Agent) => {
    setData(prev => ({ ...prev, agents: prev.agents.map(x => x.id === a.id ? { ...x, phase: 'Paused' } : x) }));
    pushToast({ tone: 'warn', title: `${a.codename} paused`, cmd: `vox pause-agent ${a.id}` });
    const id = Number(a.id.replace('A-', ''));
    if (!Number.isFinite(id)) {
      pushToast({ tone: 'warn', title: 'Pause unavailable', body: 'Selected agent has non-numeric id; cannot route pause command.' });
      return;
    }
    await executeIpcWithRun('pause_orchestrator_agent', { agentId: id }, 'gui.agent.pause')
      .catch((err) => pushToast({ tone: 'warn', title: 'Pause failed', body: String(err) }));
  }, [executeIpcWithRun, pushToast]);

  const handleResume = useCallback(async (a: Agent) => {
    setData(prev => ({ ...prev, agents: prev.agents.map(x => x.id === a.id ? { ...x, phase: 'Executing' } : x) }));
    pushToast({ tone: 'ok', title: `${a.codename} resumed`, cmd: `vox resume-agent ${a.id}` });
    const id = Number(a.id.replace('A-', ''));
    if (!Number.isFinite(id)) {
      pushToast({ tone: 'warn', title: 'Resume unavailable', body: 'Selected agent has non-numeric id; cannot route resume command.' });
      return;
    }
    await executeIpcWithRun('resume_orchestrator_agent', { agentId: id }, 'gui.agent.resume')
      .catch((err) => pushToast({ tone: 'warn', title: 'Resume failed', body: String(err) }));
  }, [executeIpcWithRun, pushToast]);

  const handleDoubt = useCallback(async (item: StreamItem) => {
    setData(prev => ({ ...prev, stream: prev.stream.map(x => x.id === item.id ? { ...x, kind: 'doubted' } : x) }));
    pushToast({ tone: 'warn', title: 'Doubt injected', body: item.title, cmd: `vox doubt-task ${item.id}` });
    const taskId = Number(String(item.id).replace(/\D+/g, ''));
    if (!Number.isFinite(taskId) || taskId <= 0) {
      pushToast({
        tone: 'warn',
        title: 'Doubt unavailable',
        body: 'This stream event has no numeric task id, so orchestrator doubt cannot be applied.',
      });
      return;
    }
    await executeIpcWithRun('doubt_orchestrator_task', {
      taskId,
      reason: `GUI doubt on stream event ${item.id}`,
    }, 'gui.stream.doubt')
      .catch((err) => pushToast({ tone: 'warn', title: 'Doubt failed', body: String(err) }));
  }, [executeIpcWithRun, pushToast]);

  const handleOverrule = useCallback(async (item: StreamItem) => {
    setData(prev => ({ ...prev, stream: prev.stream.map(x => x.id === item.id ? { ...x, kind: 'validated' } : x) }));
    pushToast({ tone: 'ok', title: 'Doubt overruled', body: item.title, cmd: `vox overrule-task ${item.id}` });
    const taskId = Number(String(item.id).replace(/\D+/g, ''));
    if (!Number.isFinite(taskId) || taskId <= 0) {
      pushToast({
        tone: 'warn',
        title: 'Overrule unavailable',
        body: 'This stream event has no numeric task id, so orchestrator overrule cannot be applied.',
      });
      return;
    }
    await executeIpcWithRun('overrule_orchestrator_task', {
      taskId,
      reason: `GUI overrule on stream event ${item.id}`,
    }, 'gui.stream.overrule')
      .catch((err) => pushToast({ tone: 'warn', title: 'Overrule failed', body: String(err) }));
  }, [executeIpcWithRun, pushToast]);

  const handleAckAlert = useCallback(async (note: LudusAlert) => {
    setData(prev => ({ ...prev, alerts: prev.alerts.filter(x => x.id !== note.id) }));
    await invoke('ack_ludus_notification', { notificationId: note.id })
      .catch((err) => pushToast({ tone: 'warn', title: 'Alert ack failed', body: String(err) }));
  }, [pushToast]);

  const focusComposer = useCallback(() => {
    const el = document.getElementById('loquela-composer') as HTMLTextAreaElement | null;
    if (el) {
      el.focus();
      el.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    }
  }, []);

  const handleCommandAction = useCallback((cmd: CommandPaletteAction) => {
    if ('type' in cmd && cmd.type === 'navigate' && cmd.viewKey) navigateTo(cmd.viewKey);
    else if ('type' in cmd && cmd.type === 'agent' && 'id' in cmd) { navigateTo('flow'); setSelectedAgentId(String(cmd.id)); }
    else if ('type' in cmd && cmd.type === 'command') navigateTo('catalog');
    else if ('type' in cmd && cmd.type === 'hit' && cmd.locator) {
      if (cmd.locator.kind === 'chat') navigateTo('chat');
      else if (cmd.locator.kind === 'file' || cmd.locator.kind === 'web') invoke('open_locator', { locator: cmd.locator }).catch(() => {});
    }
    else if ('id' in cmd && cmd.id === 'submit') focusComposer();
    else if ('id' in cmd && cmd.id === 'pause-all') data.agents.forEach(handlePause);
    else if ('id' in cmd && cmd.id === 'resume-all') data.agents.filter(a => a.phase === 'Paused').forEach(handleResume);
    else if ('id' in cmd && cmd.id === 'ack-all') data.alerts.forEach(handleAckAlert);
    else if ('id' in cmd && typeof cmd.id === 'string' && cmd.id.startsWith('agent:')) { navigateTo('flow'); setSelectedAgentId(cmd.id.slice(6)); }
    else if ('id' in cmd && typeof cmd.id === 'string' && cmd.id.startsWith('skill:')) {
      const skillId = cmd.id.slice(6);
      const s = data.skills.find((x) => x.command === skillId || x.capability_id === skillId);
      if (s) {
        const deployId = s.capability_id ?? s.command;
        setDeployedSet(prev => new Set([...prev, deployId]));
        handleLoquelaSubmit({ description: `Deploy skill: ${s.command}`, active_skill: deployId });
      }
    } else if ('id' in cmd && cmd.id === 'search') {
      navigateTo('search');
    } else if ('codename' in cmd) {
      navigateTo('flow');
      setSelectedAgentId(cmd.id);
    } else if ('command' in cmd) {
      navigateTo('catalog');
    } else if ('label' in cmd || 'type' in cmd) {
      const action = cmd as { label?: string; type?: string };
      pushToast({ tone: 'info', title: 'Command', body: action.label ?? action.type ?? 'action' });
    }
  }, [data, handlePause, handleResume, handleAckAlert, handleLoquelaSubmit, pushToast, navigateTo, focusComposer]);

  const nav = resolveNavigation(activeView);
  const surfaceProps = {
    pushToast,
    data,
    onPause: handlePause,
    onResume: handleResume,
    onDoubt: handleDoubt,
    onOverrule: handleOverrule,
    onAckLudus: handleAckAlert,
    filterKind,
    setFilterKind,
    selectedAgentId,
    setSelectedAgentId,
    skills: data.skills,
    onAttachContext: attachContext,
    onNavigate: navigateTo,
    onOpenInConsole: (a: Agent) => {
      setSelectedAgentId(a.id);
      navigateTo('console');
    },
    activeChild: nav.child,
    onChildChange: (vk: string) => setActiveView(vk as View),
    activeSessionId,
    onSessionChange: setActiveSessionId,
    chatMessages: activeChatMessages,
    onHydrateChatSession: hydrateChatSession,
    onFocusComposer: focusComposer,
  };

  const mainSurface = renderSurfaceView(nav.parent, surfaceProps);

  return (
    <div className="flex h-screen w-screen bg-void text-zinc-400 font-sans selection:bg-brass/30 selection:text-zinc-100 overflow-hidden">
      <Backdrop />

      <Sidebar
        view={activeView}
        setView={(v) => setActiveView(resolveNavigation(v).child as View)}
        agentsCount={data.agents.filter(a => a.phase !== 'Idle').length}
        data={data}
        mode={sidebarMode}
        setMode={setSidebarMode}
        pushToast={pushToast}
        appVersion={appVersion}
        policyBadge={policyBadge}
        approvalsPending={approvalsPending}
      />

      <main className="flex-1 flex flex-col min-w-0 relative">
        <div className="p-4 pb-0">
          <TopHud
            kpis={kpis}
            onCommand={() => setIsCommandOpen(true)}
            lastOrchEventAt={lastOrchEventAt}
            orchUsesPolling={orchUsesPolling}
            liveFreshMs={LIVE_EVENT_FRESH_MS}
            onNavigate={navigateTo}
            hudMode={hudMode}
            setHudMode={setHudMode}
          />
        </div>

        <div className="flex-1 min-h-0 overflow-hidden p-5 pb-[180px]">
          <SurfaceErrorBoundary key={`${nav.parent}-${nav.child}`} surface={nav.child}>
            <DockShell panelId="main-surface" panelTitle={nav.parent}>
              {mainSurface}
            </DockShell>
          </SurfaceErrorBoundary>
        </div>

        {/* Loquela — fixed to the bottom of main, tracks sidebar width */}
        <div className="p-4 pt-0 mt-auto">
          <InlineApprovals pushToast={pushToast} onViewAll={() => navigateTo('approvals')} />
          {diffOpen && (
            <DiffReview
              diff={diffText}
              loading={diffLoading}
              onClose={() => setDiffOpen(false)}
            />
          )}
          <Transcript messages={activeChatMessages} />
          <Loquela
            chips={chips}
            setChips={setChips}
            onSubmit={(p) => handleLoquelaSubmit({ ...p, session_id: activeSessionId })}
            onSlashCommand={handleLoquelaSlash}
            sessionBudget={{
              spent: kpis.budgetBurn.value,
              cap: kpis.budgetBurn.cap,
              source: kpis.budgetBurn.source,
            }}
            activeSkill={activeSkill}
            setActiveSkill={setActiveSkill}
            skills={data.skills}
            toast={pushToast}
            agents={data.agents}
          />
        </div>
      </main>

      <CommandPalette
        open={isCommandOpen}
        onClose={() => setIsCommandOpen(false)}
        onAction={cmd => { handleCommandAction(cmd); setIsCommandOpen(false); }}
        agents={data.agents}
        skills={data.skills}
      />

      <Toasts
        items={toasts}
        onClose={id => setToasts(curr => curr.filter(x => x.id !== id))}
      />
    </div>
  );
}
