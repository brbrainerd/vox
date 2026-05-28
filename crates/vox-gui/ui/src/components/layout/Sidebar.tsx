import React from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { DashboardData } from '../../types/dashboard';

export type SidebarMode = 'rail' | 'default' | 'wide';

interface NavItemProps {
  active: boolean;
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  badge?: number | string | null;
  collapsed: boolean;
}

function NavItem({ active, icon, label, onClick, badge, collapsed }: NavItemProps) {
  return (
    <button onClick={onClick} title={collapsed ? label : undefined}
      className={`group relative flex w-full items-center ${collapsed ? "justify-center px-0" : "gap-3 px-3"} py-2.5 rounded-xl transition ${active ? "bg-white/[0.04] text-zinc-100" : "text-zinc-500 hover:bg-white/[0.025] hover:text-zinc-200"}`}>
      {active && <span className="absolute left-0 top-1/2 -translate-y-1/2 h-5 w-[2px] rounded-r bg-brass shadow-[0_0_12px_2px_rgba(212,175,55,0.5)]" />}
      <span className={`flex size-7 items-center justify-center rounded-lg shrink-0 ${active ? "bg-brass/10 text-brass ring-1 ring-brass/30" : "bg-white/[0.02] ring-1 ring-white/5"}`}>{icon}</span>
      {!collapsed && <span className="flex-1 text-left font-display text-[12px] tracking-[0.12em] uppercase whitespace-nowrap overflow-hidden">{label}</span>}
      {!collapsed && badge != null && <span className="rounded-full bg-white/[0.05] px-1.5 py-0.5 font-mono text-[9px] text-zinc-400">{badge}</span>}
      {collapsed && badge != null && <span className="absolute right-1 top-1 rounded-full bg-brass/80 px-1 font-mono text-[8px] text-zinc-950">{badge}</span>}
    </button>
  );
}

const SIDEBAR_WIDTHS = { rail: 64, default: 212, wide: 280 };
const SIDEBAR_ORDER: SidebarMode[] = ["rail", "default", "wide"];

interface SidebarProps {
  view: string;
  setView: (v: any) => void;
  agentsCount: number;
  data: DashboardData;
  mode: SidebarMode;
  setMode: (m: SidebarMode) => void;
  pushToast: (t: any) => void;
}

export function Sidebar({ view, setView, agentsCount, data, mode, setMode, pushToast }: SidebarProps) {
  const w = SIDEBAR_WIDTHS[mode];
  const collapsed = mode === "rail";
  const wide = mode === "wide";
  
  const cycle = (dir: number) => {
    const i = SIDEBAR_ORDER.indexOf(mode);
    const ni = Math.max(0, Math.min(SIDEBAR_ORDER.length - 1, i + dir)) as number;
    setMode(SIDEBAR_ORDER[ni]);
  };

  return (
    <aside className="shrink-0 flex flex-col transition-[width] duration-200 ease-out h-screen overflow-hidden sticky top-0" style={{ width: w }}>
      <Glass className="flex h-full flex-col p-3 rounded-none border-y-0 border-l-0">
        {/* Brand + collapse handles */}
        <div className={`flex items-center ${collapsed ? "justify-center" : "justify-between"} pb-3`}>
          {!collapsed && (
            <div className="flex items-center gap-2 px-1">
              <div className="relative size-6 rounded-md bg-gradient-to-br from-brass via-amber-600 to-zinc-900 ring-1 ring-brass/40">
                <span className="absolute inset-0 grid place-items-center font-display text-[11px] font-bold text-zinc-950">V</span>
              </div>
              <div className="leading-tight">
                <div className="font-display text-[11px] tracking-[0.22em] text-zinc-200">VOX</div>
                <div className="font-mono text-[8px] tracking-widest text-zinc-500">IMPERIUM</div>
              </div>
            </div>
          )}
          <div className={`flex items-center ${collapsed ? "flex-col gap-1" : "gap-0.5"}`}>
            <button onClick={() => cycle(-1)} disabled={mode === "rail"} title="Collapse"
              className={`flex size-6 items-center justify-center rounded-md border border-white/5 ${mode === "rail" ? "opacity-30 cursor-not-allowed" : "hover:bg-white/5 text-zinc-400 hover:text-zinc-100"}`}>
              <Icon.chevL className="size-3"/>
            </button>
            <button onClick={() => cycle(1)} disabled={mode === "wide"} title="Expand"
              className={`flex size-6 items-center justify-center rounded-md border border-white/5 ${mode === "wide" ? "opacity-30 cursor-not-allowed" : "hover:bg-white/5 text-zinc-400 hover:text-zinc-100"}`}>
              <Icon.chevR className="size-3"/>
            </button>
          </div>
        </div>

        {!collapsed && (
          <div className="px-2 pb-3">
            <div className="font-display text-[9px] uppercase tracking-[0.32em] text-zinc-500">Sectors</div>
          </div>
        )}
        <nav className="flex flex-col gap-1">
          <NavItem collapsed={collapsed} active={view === "dashboard"} onClick={() => setView("dashboard")} icon={<Icon.dashboard className="size-4"/>} label="Imperium" />
          <NavItem collapsed={collapsed} active={view === "flow"}      onClick={() => setView("flow")}      icon={<Icon.flow className="size-4"/>}      label="Agents" badge={agentsCount} />
          <NavItem collapsed={collapsed} active={view === "catalog"}   onClick={() => setView("catalog")}   icon={<Icon.catalog className="size-4"/>}   label="Skills" />
          <NavItem collapsed={collapsed} active={view === "matrix"}    onClick={() => setView("matrix")}    icon={<Icon.matrix className="size-4"/>}    label="Policies" />
          <NavItem collapsed={collapsed} active={view === "memory"}    onClick={() => setView("memory")}    icon={<Icon.memory className="size-4"/>}    label="Memory" />
        </nav>

        <div className="mt-auto flex flex-col gap-2 pt-3">
          {!collapsed && (
            <div className="rounded-xl border border-white/5 bg-gradient-to-br from-violet-500/[0.06] via-zinc-900/40 to-zinc-950 p-3">
              <div className="flex items-center gap-2">
                <div className="flex size-7 items-center justify-center rounded-md bg-violet-400/15 text-violet-300"><Icon.cpu className="size-3.5"/></div>
                <div className="leading-tight">
                  <div className="font-display text-[11px] tracking-wider text-violet-300">Mesh</div>
                  <div className="font-mono text-[10px] text-zinc-400">{(data.peers||[]).filter(p=>p.online).length}/{(data.peers||[]).length} peers</div>
                </div>
              </div>
              <div className="mt-2.5 space-y-1">
                {(data.peers||[]).slice(0, wide ? 5 : 3).map(p => (
                  <div key={p.id} className="flex items-center justify-between font-mono text-[9px]">
                    <span className="flex items-center gap-1.5 text-zinc-400"><span className={`size-1 rounded-full ${p.online?"bg-emerald-400":"bg-zinc-600"}`}/>{p.name}</span>
                    <span className="text-zinc-500">{p.backend}</span>
                  </div>
                ))}
              </div>
              {wide && (
                <button onClick={() => pushToast({ tone: "ok", title: "Mesh refreshed", cmd: "mesh_refresh_peers" })}
                  className="mt-3 flex w-full items-center justify-center gap-1.5 rounded-md border border-white/10 bg-white/[0.02] py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5">
                  <Icon.refresh className="size-3"/> rescan peers
                </button>
              )}
            </div>
          )}
          <NavItem collapsed={collapsed} active={view === "settings"} onClick={() => setView("settings")} icon={<Icon.settings className="size-4"/>} label="Settings" />
          <div className={`flex items-center ${collapsed ? "justify-center" : "gap-2 px-2"} pb-1 pt-1`}>
            <div className="relative size-7 shrink-0 rounded-full bg-gradient-to-br from-violet-500 to-cyan-500">
              <span className="absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full bg-emerald-400 ring-2 ring-zinc-950"/>
            </div>
            {!collapsed && (
              <div className="flex-1 leading-tight overflow-hidden">
                <div className="font-display text-[11px] text-zinc-200 truncate">archon@vox</div>
                <div className="font-mono text-[9px] text-zinc-500">build 0.5.0 · tauri 2</div>
              </div>
            )}
          </div>
        </div>
      </Glass>
    </aside>
  );
}
