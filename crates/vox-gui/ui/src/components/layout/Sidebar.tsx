import React, { useEffect, useRef } from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { DashboardData } from '../../types/dashboard';
import { SURFACE_REGISTRY, SurfaceRegistryEntry } from '../../generated/surfaceRegistry.generated';
import { useLocalStorage } from '../../hooks/useLocalStorage';

export type SidebarMode = 'rail' | 'default' | 'wide';

interface NavItemProps {
  active: boolean;
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  badge?: number | string | null;
  collapsed: boolean;
  innerRef?: React.Ref<HTMLButtonElement>;
}

function NavItem({ active, icon, label, onClick, badge, collapsed, innerRef }: NavItemProps) {
  return (
    <button ref={innerRef} onClick={onClick} title={collapsed ? label : undefined}
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

// Sidebar sections in display order. `nav_group` on each surface (the SSOT) decides membership;
// this list only fixes the order + label of the sections themselves.
const NAV_SECTIONS: { id: string; label: string }[] = [
  { id: 'operate', label: 'Operate' },
  { id: 'develop', label: 'Develop' },
  { id: 'knowledge', label: 'Knowledge' },
  { id: 'compute', label: 'Compute' },
];
const SYSTEM_GROUP = 'system';

// Curated order within each section. Surfaces not listed here append after, so a newly
// registered surface still shows up (drift-safe) without editing this file.
const SECTION_ORDER: Record<string, string[]> = {
  operate: ['dashboard', 'flow', 'approvals', 'runs', 'matrix'],
  develop: ['harness', 'catalog', 'repository', 'skills'],
  knowledge: ['search', 'memory', 'research', 'scientia', 'discovery-review', 'claims', 'publications'],
  compute: ['models', 'mens', 'populi', 'oratio', 'mesh'],
  system: ['coverage', 'gamify', 'settings'],
};

const KNOWN_GROUPS = new Set([...NAV_SECTIONS.map(s => s.id), SYSTEM_GROUP]);

const orderIndex = (group: string, viewKey: string) => {
  const i = (SECTION_ORDER[group] ?? []).indexOf(viewKey);
  return i === -1 ? Number.MAX_SAFE_INTEGER : i;
};

const navigableSurfaces = (): SurfaceRegistryEntry[] =>
  SURFACE_REGISTRY.filter(e => e.viewKey && e.navLabel);

const itemsForGroup = (group: string): SurfaceRegistryEntry[] =>
  navigableSurfaces()
    .filter(e => e.navGroup === group)
    .sort((a, b) => orderIndex(group, a.viewKey as string) - orderIndex(group, b.viewKey as string));

// Any surface whose nav_group isn't one of the known sections — rendered under "More" so it is
// never silently dropped when the registry changes.
const orphanSurfaces = (): SurfaceRegistryEntry[] =>
  navigableSurfaces().filter(e => !KNOWN_GROUPS.has(e.navGroup ?? ''));

interface SidebarProps {
  view: string;
  setView: (v: any) => void;
  agentsCount: number;
  data: DashboardData;
  mode: SidebarMode;
  setMode: (m: SidebarMode) => void;
  pushToast: (t: any) => void;
  appVersion?: string;
}

export function Sidebar({ view, setView, agentsCount, data, mode, setMode, pushToast, appVersion }: SidebarProps) {
  const w = SIDEBAR_WIDTHS[mode];
  const collapsed = mode === "rail";
  const wide = mode === "wide";

  // Per-section collapse state, persisted. Primary workflows (Operate/Develop) start open; the
  // larger secondary groups start collapsed so the first paint shows every section without
  // scrolling. A section auto-expands when it holds the active surface (see renderSection).
  const [collapsedSections, setCollapsedSections] = useLocalStorage<Record<string, boolean>>('vox_nav_sections', { knowledge: true, compute: true });
  const toggleSection = (id: string) =>
    setCollapsedSections(prev => ({ ...prev, [id]: !prev[id] }));

  const cycle = (dir: number) => {
    const i = SIDEBAR_ORDER.indexOf(mode);
    const ni = Math.max(0, Math.min(SIDEBAR_ORDER.length - 1, i + dir)) as number;
    setMode(SIDEBAR_ORDER[ni]);
  };

  // Keep the active surface's nav item in view — its section may be far down the scrollable list.
  const activeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: 'nearest' });
  }, [view]);

  const renderItem = (e: SurfaceRegistryEntry) => {
    const IconCmp = (Icon as Record<string, any>)[e.navIcon ?? 'file'] ?? Icon.file;
    const isActive = view === e.viewKey;
    return (
      <NavItem
        key={e.viewKey as string}
        innerRef={isActive ? activeRef : undefined}
        collapsed={collapsed}
        active={isActive}
        onClick={() => setView(e.viewKey)}
        icon={<IconCmp className="size-4" />}
        label={e.navLabel as string}
        badge={e.viewKey === 'flow' ? agentsCount : undefined}
      />
    );
  };

  const renderSection = (id: string, label: string, items: SurfaceRegistryEntry[]) => {
    if (items.length === 0) return null;
    if (collapsed) {
      // Rail mode: icons only, sections separated by a hairline divider.
      return (
        <div key={id} className="flex flex-col">
          <div className="mx-2 my-1 h-px bg-white/5" />
          {items.map(renderItem)}
        </div>
      );
    }
    // A collapsed section still opens when it holds the active surface, so the current view is
    // never hidden behind a collapsed header.
    const containsActive = items.some(e => e.viewKey === view);
    const open = !collapsedSections[id] || containsActive;
    return (
      <div key={id} className="flex flex-col">
        <button onClick={() => toggleSection(id)}
          className="group flex items-center justify-between rounded-md px-2 pb-1 pt-2.5 text-zinc-500 hover:text-zinc-300">
          <span className="font-display text-[9px] uppercase tracking-[0.32em]">{label}</span>
          <Icon.chevronDown className={`size-3 transition-transform ${open ? '' : '-rotate-90'}`} />
        </button>
        {open && <div className="flex flex-col gap-0.5">{items.map(renderItem)}</div>}
      </div>
    );
  };

  const systemItems = itemsForGroup(SYSTEM_GROUP);
  const orphans = orphanSurfaces();

  return (
    <aside className="shrink-0 flex flex-col transition-[width] duration-200 ease-out h-screen overflow-hidden sticky top-0" style={{ width: w }}>
      <Glass className="flex h-full flex-col p-3 rounded-none border-y-0 border-l-0">
        {/* Brand + collapse handles */}
        <div className={`flex items-center ${collapsed ? "justify-center" : "justify-between"} pb-3 shrink-0`}>
          {!collapsed && (
            <div className="flex items-center gap-2 px-1">
              <div className="relative size-6 rounded-md bg-gradient-to-br from-brass via-amber-600 to-zinc-900 ring-1 ring-brass/40">
                <span className="absolute inset-0 grid place-items-center font-display text-[11px] font-bold text-zinc-950">V</span>
              </div>
              <div className="leading-tight">
                <div className="font-display text-[11px] tracking-[0.22em] text-zinc-200">VOX</div>
                  <div className="font-mono text-[8px] tracking-widest text-zinc-500">OPERATOR CONSOLE</div>
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

        {/* Grouped, scrollable navigation. Footer below stays pinned. */}
        <nav className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden custom-scrollbar flex flex-col gap-0.5 -mr-1 pr-1">
          {NAV_SECTIONS.map(s => renderSection(s.id, s.label, itemsForGroup(s.id)))}
          {renderSection('__more__', 'More', orphans)}
        </nav>

        <div className="flex flex-col gap-2 pt-3 shrink-0">
          {/* Compact mesh status — navigates to the Mesh surface. The full peer card lives in wide mode. */}
          {!collapsed && !wide && (data.peers || []).length > 0 && (
            <button onClick={() => setView('mesh')}
              className="flex items-center justify-between rounded-lg border border-white/5 bg-white/[0.02] px-2.5 py-1.5 hover:bg-white/[0.04]">
              <span className="flex items-center gap-1.5 font-display text-[10px] uppercase tracking-[0.18em] text-violet-300/90">
                <Icon.link className="size-3" /> Mesh
              </span>
              <span className="font-mono text-[10px] text-zinc-400">{(data.peers || []).filter(p => p.online).length}/{(data.peers || []).length} peers</span>
            </button>
          )}
          {wide && (
            <div className="rounded-xl border border-white/5 bg-gradient-to-br from-violet-500/[0.06] via-zinc-900/40 to-zinc-950 p-3">
              <div className="space-y-1">
                {(data.peers||[]).slice(0, 5).map(p => (
                  <div key={p.id} className="flex items-center justify-between font-mono text-[9px]">
                    <span className="flex items-center gap-1.5 text-zinc-400"><span className={`size-1 rounded-full ${p.online?"bg-emerald-400":"bg-zinc-600"}`}/>{p.name}</span>
                    <span className="text-zinc-500">{p.backend}</span>
                  </div>
                ))}
              </div>
              <button onClick={() => pushToast({ tone: "ok", title: "Mesh refreshed", cmd: "mesh_refresh_peers" })}
                className="mt-3 flex w-full items-center justify-center gap-1.5 rounded-md border border-white/10 bg-white/[0.02] py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5">
                <Icon.refresh className="size-3"/> rescan peers
              </button>
            </div>
          )}

          {/* System cluster — Coverage / Gamify / Settings (instrumentation + account). */}
          {systemItems.length > 0 && (
            <div className="flex flex-col gap-0.5 border-t border-white/5 pt-2">
              {!collapsed && <div className="px-2 pb-0.5 font-display text-[9px] uppercase tracking-[0.32em] text-zinc-600">System</div>}
              {systemItems.map(renderItem)}
            </div>
          )}

          <div className={`flex items-center ${collapsed ? "justify-center" : "gap-2 px-2"} pb-1 pt-1`}>
            <div className="relative size-7 shrink-0 rounded-full bg-gradient-to-br from-violet-500 to-cyan-500">
              <span className="absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full bg-emerald-400 ring-2 ring-zinc-950"/>
            </div>
            {!collapsed && (
              <div className="flex-1 leading-tight overflow-hidden">
                <div className="font-display text-[11px] text-zinc-200 truncate">archon@vox</div>
                <div className="font-mono text-[9px] text-zinc-500">build {appVersion ?? 'unknown'} · tauri 2</div>
              </div>
            )}
          </div>
        </div>
      </Glass>
    </aside>
  );
}
