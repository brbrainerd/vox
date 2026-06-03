import React, { useCallback, useEffect, useReducer, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { decode } from '@msgpack/msgpack';
import { Backdrop } from './components/ui/Backdrop';
import { Sidebar, SidebarMode } from './components/layout/Sidebar';
import { TopHud } from './components/layout/TopHud';
import { CommandPalette } from './components/layout/CommandPalette';
import { Toasts, ToastItem } from './components/ui/Toasts';
import { Dashboard } from './components/surfaces/Dashboard/Dashboard';
import { Loquela } from './components/surfaces/Loquela/Loquela';
import { Transcript } from './components/surfaces/Loquela/Transcript';
import { chatReducer, initialChatState } from './lib/chatCorrelation';
import { Catalog } from './components/surfaces/Catalog/Catalog';
import { Matrix } from './components/surfaces/Matrix/Matrix';
import { AgentFlow } from './components/surfaces/Flow/AgentFlow';
import { MemoryView } from './components/surfaces/Memory/MemoryView';
import { SettingsView } from './components/surfaces/Settings/SettingsView';
import { ModelsView } from './components/surfaces/Models/ModelsView';
import { RunsView } from './components/surfaces/Runs/RunsView';
import { RepositoryView } from './components/surfaces/Repository/RepositoryView';
import { MeshView } from './components/surfaces/Mesh/MeshView';
import { GamifyView } from './components/surfaces/Gamify/GamifyView';
import { HarnessView } from './components/surfaces/Harness/HarnessView';
import { surfaceDecorators } from './components/surfaces/decoratorRegistry';
import { ApprovalsView } from './components/surfaces/Approvals/ApprovalsView';
import { SkillsPluginsView } from './components/surfaces/SkillsPlugins/SkillsPluginsView';
import { voxTransport, listenOrchStatus, listenAgentEvents, type AgentEventFrame } from './transport';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useLocalStorage } from './hooks/useLocalStorage';
import { usePersistedSparkWindow } from './hooks/useSparkWindow';
import { DashboardData, Agent, StreamItem, LudusAlert } from './types/dashboard';
import { INITIAL_DATA, INITIAL_KPIS } from './data/initialState';

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
  | 'scientia'
  | 'claims'
  | 'mens'
  | 'populi'
  | 'research'
  | 'oratio'
  | 'approvals'
  | 'skills'
  | 'settings'
  | 'coverage'
  | 'publications'
  | 'search';

// ─── Agent mapper — shared between EventBus and polling fallback ─────────────
function mapAgent(a: any): Agent {
  return {
    id: `A-${String(a.id).padStart(2, '0')}`,
    codename: a.codename ?? 'Agent',
    phase: a.paused ? 'Paused' : (a.in_progress ? (a.current_phase ?? 'Executing') : 'Idle'),
    progress: a.progress ?? (a.in_progress ? 0.45 : 0),
    task: a.task_description ?? (a.in_progress ? 'Processing…' : 'Idle'),
    cost: a.cost ?? 0,
    budget: a.budget ?? 5.0,
    eta: a.eta ?? '—',
    skill: a.active_skill,
  };
}

function mapStream(e: any): StreamItem {
  return {
    id: e.id ?? Math.random().toString(36).substring(7),
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

function mapAlert(a: any): LudusAlert {
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
  const [chips, setChips] = useState<any[]>([]);
  const [activeSkill, setActiveSkill] = useState<any>(null);
  const [deployedSet, setDeployedSet] = useState(new Set<string>());
  const [selectedAgentId, setSelectedAgentId] = useState('ROOT');
  const [appVersion, setAppVersion] = useState<string>('loading…');

  // ── B4-chat: pure-reducer transcript state for the Loquela composer ────────
  const [chat, dispatchChat] = useReducer(chatReducer, initialChatState);

  // ── 5-minute rolling sparkline windows ──────────────────────────────────
  // Each hook persists its window to localStorage under a namespaced key.
  const agentCountWindow = usePersistedSparkWindow('kpi.agentCount', kpis.activeAgents.value);
  const queueDepthWindow = usePersistedSparkWindow('kpi.queueDepth', kpis.queueDepth.value);
  const budgetBurnWindow = usePersistedSparkWindow('kpi.budgetBurn', kpis.budgetBurn.value);
  const meshWindow       = usePersistedSparkWindow('kpi.mesh', typeof kpis.mesh.value === 'number' ? kpis.mesh.value : 0);

  // ── Toast helper ─────────────────────────────────────────────────────────
  const pushToast = useCallback((t: Omit<ToastItem, 'id'>) => {
    const id = Math.random().toString(36).substring(7);
    setToasts(curr => [...curr, { ...t, id }]);
    setTimeout(() => setToasts(curr => curr.filter(x => x.id !== id)), 5000);
  }, []);

  // ── KPI update — shared logic used by both EventBus listener and fallback ─
  const applyStatus = useCallback((status: any) => {
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
        value: status.total_cost ?? 0,
        cap: status.budget_cap ?? 50.0,
        delta: (status.total_cost ?? 0) - prev.budgetBurn.value,
        spark: budgetBurnWindow,
      },
      mesh: {
        value: status.mesh_throughput ?? 0,
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
    voxTransport.getCatalog().then((catalog: any) => {
      if (catalog?.entries) setData(prev => ({ ...prev, skills: catalog.entries }));
    });
    invoke<{ display?: string; version?: string }>('get_build_info')
      .then((info) => setAppVersion(info.display ?? info.version ?? 'unknown'))
      .catch(() => setAppVersion('unknown'));

    invoke('get_initial_view').then((view: any) => {
      if (view && (['dashboard', 'flow', 'catalog', 'matrix', 'memory', 'models', 'runs', 'repository', 'mesh', 'gamify', 'harness', 'scientia', 'claims', 'mens', 'populi', 'research', 'oratio', 'approvals', 'skills', 'settings', 'coverage', 'publications', 'search'] as string[]).includes(view)) {
        setActiveView(view as View);
      }
    }).catch(() => {});
  // eslint-disable-next-line react-hooks/exhaustive-deps
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
        applyStatus(decode(rawBytes));
      } catch (err) {
        // Silently ignore if backend is down or not ready.
      }
    };

    const startFallbackPolling = () => {
      if (fallbackInterval !== undefined) return;
      fallbackInterval = setInterval(fetchSnapshot, 2000);
    };

    // Initial snapshot for first paint.
    fetchSnapshot();

    // Subscribe to the pushed event stream; fall back to polling on failure.
    listenOrchStatus((status) => applyStatus(status))
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
        if (!cancelled) startFallbackPolling();
      });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
      if (fallbackInterval !== undefined) clearInterval(fallbackInterval);
    };
  }, [applyStatus]);

  // ── Pushed live agent-event stream (B4: "vox://agent-events" Tauri event) ──
  // Each AgentEvent is prepended to the dashboard activity feed as a StreamItem.
  // We keep this independent from the status subscription above; the list is
  // capped so it does not grow unbounded.
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listenAgentEvents((frame) => {
      // One listener, two consumers: the dashboard activity feed and the
      // B4-chat transcript reducer.
      const item = mapAgentEvent(frame);
      setData(prev => ({
        ...prev,
        stream: [item, ...prev.stream].slice(0, 100),
      }));
      dispatchChat({ type: 'agentEvent', event: frame });
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

  const executeWithRun = useCallback(async (operationName: string, payload: any, workflowName: string) => {
    const runId = `gui-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    await invoke('start_gui_run', {
      input: {
        run_id: runId,
        workflow_name: workflowName,
        planned_steps: 1,
      }
    });
    try {
      const result = await voxTransport.callTool(operationName, payload);
      const success = result.exit_code === 0;
      await invoke('finish_gui_run', {
        run_id: runId,
        success,
        completed_steps: success ? 1 : 0,
        error: success ? null : (result.stderr || `exit_code=${result.exit_code}`)
      });
      if (!success) {
        throw new Error(result.stderr || `Command failed with exit code ${result.exit_code}`);
      }
      return result;
    } catch (err) {
      await invoke('finish_gui_run', {
        run_id: runId,
        success: false,
        completed_steps: 0,
        error: String(err),
      }).catch(() => {});
      throw err;
    }
  }, []);

  const executeIpcWithRun = useCallback(async <T,>(command: string, payload: any, workflowName: string, onRun?: (runId: string) => void): Promise<T> => {
    const runId = `gui-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    onRun?.(runId);
    await invoke('start_gui_run', {
      input: {
        run_id: runId,
        workflow_name: workflowName,
        planned_steps: 1,
      }
    });
    try {
      const result = await invoke<T>(command, payload);
      await invoke('finish_gui_run', {
        run_id: runId,
        success: true,
        completed_steps: 1,
        error: null
      });
      return result;
    } catch (err) {
      await invoke('finish_gui_run', {
        run_id: runId,
        success: false,
        completed_steps: 0,
        error: String(err),
      }).catch(() => {});
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
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Action handlers ───────────────────────────────────────────────────────
  const handleLoquelaSubmit = useCallback(async (payload: any) => {
    pushToast({ tone: 'info', title: 'Task Dispatched', body: payload.description, cmd: 'vox submit-task' });
    let runId = '';
    await executeIpcWithRun<{ ok: boolean; message: string; task_id: string | null }>(
      'submit_orchestrator_task',
      {
        input: {
          description: payload.description,
          files: payload.files ?? [],
          priority: payload.priority ?? null,
          session_id: payload.session_id ?? 'gui-loquela',
        }
      },
      'gui.loquela.submit',
      // Mint the runId and create the user/assistant bubbles BEFORE the invoke
      // resolves so streamed tokens correlate to a live transcript entry.
      (id) => {
        runId = id;
        dispatchChat({ type: 'submit', runId: id, prompt: String(payload.description ?? '') });
      },
    )
      .then((result) => {
        if (runId && result?.task_id != null) {
          dispatchChat({ type: 'submitResolved', runId, taskId: String(result.task_id) });
        }
      })
      .catch(err => pushToast({ tone: 'warn', title: 'Dispatch Failed', body: String(err) }));
  }, [executeIpcWithRun, pushToast]);

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

  const handleCommandAction = useCallback((cmd: any) => {
    if (cmd.id === 'submit') document.querySelector('textarea')?.focus();
    else if (cmd.id === 'pause-all') data.agents.forEach(handlePause);
    else if (cmd.id === 'resume-all') data.agents.filter(a => a.phase === 'Paused').forEach(handleResume);
    else if (cmd.id === 'ack-all') data.alerts.forEach(handleAckAlert);
    else if (cmd.id?.startsWith('agent:')) { setActiveView('flow'); setSelectedAgentId(cmd.id.slice(6)); }
    else if (cmd.id?.startsWith('skill:')) {
      const s = data.skills.find((x: any) => x.id === cmd.id.slice(6));
      if (s) {
        setDeployedSet(prev => new Set([...prev, s.id]));
        handleLoquelaSubmit({ description: `Deploy skill: ${s.command}`, active_skill: s.id });
      }
    } else if (cmd.id === 'search') {
      setActiveView('search');
    } else {
      pushToast({ tone: 'info', title: 'Command', body: cmd.label });
    }
  }, [data, handlePause, handleResume, handleAckAlert, handleLoquelaSubmit, pushToast]);

  // ── View renderer ─────────────────────────────────────────────────────────
  const renderView = () => {
    // Decorator registry: a surface may override its default view (SP-4 seam).
    const Decorator = surfaceDecorators[activeView];
    if (Decorator) return <Decorator pushToast={pushToast} />;
    switch (activeView) {
      case 'dashboard':
        return (
          <Dashboard
            data={data}
            onPause={handlePause}
            onResume={handleResume}
            onDoubt={handleDoubt}
            onOverrule={handleOverrule}
            onAckLudus={handleAckAlert}
            filterKind={filterKind}
            setFilterKind={setFilterKind}
          />
        );
      case 'flow':
        return (
          <AgentFlow
            agents={data.agents}
            selectedId={selectedAgentId}
            onSelect={setSelectedAgentId}
          />
        );
      case 'catalog':
        return (
          <Catalog
            skills={data.skills}
            onDeploy={(s: any) => {
              setDeployedSet(prev => new Set([...prev, s.id ?? s.command]));
              handleLoquelaSubmit({ description: `Deploy skill: ${s.command}`, active_skill: s.id });
            }}
            deployedSet={deployedSet}
          />
        );
      case 'matrix':
        return (
          <Matrix
            intentions={data.intentions}
            onDoubt={(i: any) => executeWithRun('vox_doubt_policy', { id: i.id }, 'gui.policy.doubt')
              .catch((err: any) => pushToast({ tone: 'warn', title: 'Policy doubt failed', body: String(err) }))}
            onOverrule={(i: any) => executeWithRun('vox_promote_policy', { id: i.id }, 'gui.policy.promote')
              .catch((err: any) => pushToast({ tone: 'warn', title: 'Policy promote failed', body: String(err) }))}
          />
        );
      case 'memory':
        return <MemoryView pushToast={pushToast} />;
      case 'models':
        return <ModelsView pushToast={pushToast} />;
      case 'runs':
        return <RunsView pushToast={pushToast} />;
      case 'settings':
        return <SettingsView pushToast={pushToast} />;
      case 'repository':
        return <RepositoryView pushToast={pushToast} />;
      case 'mesh':
        return <MeshView pushToast={pushToast} />;
      case 'gamify':
        return <GamifyView pushToast={pushToast} />;
      case 'harness':
        return <HarnessView pushToast={pushToast} />;
      case 'approvals':
        return <ApprovalsView pushToast={pushToast} />;
      case 'skills':
        return <SkillsPluginsView pushToast={pushToast} />;
      default:
        return null;
    }
  };

  return (
    <div className="flex h-screen w-screen bg-void text-zinc-400 font-sans selection:bg-brass/30 selection:text-zinc-100 overflow-hidden">
      <Backdrop />

      <Sidebar
        view={activeView}
        setView={setActiveView as any}
        agentsCount={data.agents.filter(a => a.phase !== 'Idle').length}
        data={data}
        mode={sidebarMode}
        setMode={setSidebarMode}
        pushToast={pushToast}
        appVersion={appVersion}
      />

      <main className="flex-1 flex flex-col min-w-0 relative">
        <div className="p-4 pb-0">
          <TopHud kpis={kpis} onCommand={() => setIsCommandOpen(true)} />
        </div>

        <div className="flex-1 overflow-y-auto overflow-x-hidden custom-scrollbar p-5 pb-[180px]">
          {renderView()}
        </div>

        {/* Loquela — fixed to the bottom of main, tracks sidebar width */}
        <div className="p-4 pt-0 mt-auto">
          <Transcript messages={chat.messages} />
          <Loquela
            chips={chips}
            setChips={setChips}
            onSubmit={handleLoquelaSubmit}
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
