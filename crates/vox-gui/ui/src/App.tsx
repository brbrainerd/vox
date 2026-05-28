import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { decode } from '@msgpack/msgpack';
import { Backdrop } from './components/ui/Backdrop';
import { Sidebar, SidebarMode } from './components/layout/Sidebar';
import { TopHud } from './components/layout/TopHud';
import { CommandPalette } from './components/layout/CommandPalette';
import { Toasts, ToastItem } from './components/ui/Toasts';
import { Dashboard } from './components/surfaces/Dashboard/Dashboard';
import { Loquela } from './components/surfaces/Loquela/Loquela';
import { Catalog } from './components/surfaces/Catalog/Catalog';
import { Matrix } from './components/surfaces/Matrix/Matrix';
import { AgentFlow } from './components/surfaces/Flow/AgentFlow';
import { MemoryView } from './components/surfaces/Memory/MemoryView';
import { SettingsView } from './components/surfaces/Settings/SettingsView';
import { voxTransport } from './transport';
import { useLocalStorage } from './hooks/useLocalStorage';
import { usePersistedSparkWindow } from './hooks/useSparkWindow';
import { DashboardData, Agent, StreamItem, LudusAlert } from './types/dashboard';
import { INITIAL_DATA, INITIAL_KPIS } from './data/initialState';

type View = 'dashboard' | 'flow' | 'catalog' | 'matrix' | 'memory' | 'settings';

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

    invoke('get_initial_view').then((view: any) => {
      if (view && (['dashboard', 'flow', 'catalog', 'matrix', 'memory', 'settings'] as string[]).includes(view)) {
        setActiveView(view as View);
      }
    }).catch(() => {});
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Reactive EventBus — zero-copy IPC polling path ────────────────────────
  // We use `get_orchestrator_status_bin` to fetch raw MessagePack payloads,
  // bypassing Tauri's default JSON string-escaping overhead for large states.
  useEffect(() => {
    let fallbackInterval: ReturnType<typeof setInterval> | undefined;

    const poll = async () => {
      try {
        const rawBytes = await invoke<Uint8Array>('get_orchestrator_status_bin');
        const status = decode(rawBytes);
        applyStatus(status);
      } catch (err) {
        // Silently ignore if backend is down or not ready
      }
    };

    poll();
    fallbackInterval = setInterval(poll, 500);

    return () => {
      if (fallbackInterval !== undefined) clearInterval(fallbackInterval);
    };
  }, [applyStatus]);

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
    await voxTransport.callTool('vox_submit_task', payload)
      .catch(err => pushToast({ tone: 'warn', title: 'Dispatch Failed', body: String(err) }));
  }, [pushToast]);

  const handlePause = useCallback(async (a: Agent) => {
    setData(prev => ({ ...prev, agents: prev.agents.map(x => x.id === a.id ? { ...x, phase: 'Paused' } : x) }));
    pushToast({ tone: 'warn', title: `${a.codename} paused`, cmd: `vox dei pause-agent ${a.id}` });
    await voxTransport.callTool('vox_pause_agent', { agent_id: a.id.replace('A-', '') })
      .catch(() => {});
  }, [pushToast]);

  const handleResume = useCallback(async (a: Agent) => {
    setData(prev => ({ ...prev, agents: prev.agents.map(x => x.id === a.id ? { ...x, phase: 'Executing' } : x) }));
    pushToast({ tone: 'ok', title: `${a.codename} resumed`, cmd: `vox dei resume-agent ${a.id}` });
    await voxTransport.callTool('vox_resume_agent', { agent_id: a.id.replace('A-', '') })
      .catch(() => {});
  }, [pushToast]);

  const handleDoubt = useCallback(async (item: StreamItem) => {
    setData(prev => ({ ...prev, stream: prev.stream.map(x => x.id === item.id ? { ...x, kind: 'doubted' } : x) }));
    pushToast({ tone: 'warn', title: 'Doubt injected', body: item.title, cmd: `vox doubt-task ${item.id}` });
    await voxTransport.callTool('vox_doubt_task', { task_id: item.id }).catch(() => {});
  }, [pushToast]);

  const handleOverrule = useCallback(async (item: StreamItem) => {
    setData(prev => ({ ...prev, stream: prev.stream.map(x => x.id === item.id ? { ...x, kind: 'validated' } : x) }));
    pushToast({ tone: 'ok', title: 'Doubt overruled', body: item.title, cmd: `vox overrule-task ${item.id}` });
    await voxTransport.callTool('vox_overrule_task', { task_id: item.id }).catch(() => {});
  }, [pushToast]);

  const handleAckAlert = useCallback(async (note: LudusAlert) => {
    setData(prev => ({ ...prev, alerts: prev.alerts.filter(x => x.id !== note.id) }));
    await voxTransport.callTool('vox_gamify_notification_ack', { notification_id: note.id }).catch(() => {});
  }, []);

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
    } else {
      pushToast({ tone: 'info', title: 'Command', body: cmd.label });
    }
  }, [data, handlePause, handleResume, handleAckAlert, handleLoquelaSubmit, pushToast]);

  // ── View renderer ─────────────────────────────────────────────────────────
  const renderView = () => {
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
            onDoubt={(i: any) => voxTransport.callTool('vox_doubt_policy', { id: i.id }).catch(() => {})}
            onOverrule={(i: any) => voxTransport.callTool('vox_promote_policy', { id: i.id }).catch(() => {})}
          />
        );
      case 'memory':
        return <MemoryView pushToast={pushToast} />;
      case 'settings':
        return <SettingsView pushToast={pushToast} />;
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
