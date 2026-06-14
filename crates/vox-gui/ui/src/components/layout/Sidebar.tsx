import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { DashboardData } from '../../types/dashboard';
import { SURFACE_REGISTRY } from '../../generated/surfaceRegistry.generated';
import { useLocalStorage } from '../../hooks/useLocalStorage';
import { TOP_LEVEL_VIEWS, resolveNavigation } from '../../lib/navigation';
import { STATUS_BADGE_CLASS, STATUS_RAIL_BADGE_CLASS } from '../../styles/tokens';

export type SidebarMode = 'rail' | 'default' | 'wide';

type PolicyBadgeStatus = 'pass' | 'fail' | 'warn' | 'not_run';
export interface PolicyBadge { count: number; status: PolicyBadgeStatus; }

interface NavItemProps {
  active: boolean;
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  badge?: number | string | null;
  badgeClass?: string;
  railBadgeClass?: string;
  collapsed: boolean;
  innerRef?: React.Ref<HTMLButtonElement>;
}

function NavItem({ active, icon, label, onClick, badge, badgeClass, railBadgeClass, collapsed, innerRef }: NavItemProps) {
  return (
    <button type="button" ref={innerRef} onClick={onClick} title={collapsed ? label : undefined} aria-label={collapsed ? label : undefined}
      className={`group relative flex w-full items-center ${collapsed ? "justify-center px-0" : "gap-3 px-3"} py-2.5 rounded-xl transition ${active ? "bg-white/[0.04] text-zinc-100" : "text-zinc-500 hover:bg-white/[0.025] hover:text-zinc-200"}`}>
      {active && <span className="absolute left-0 top-1/2 -translate-y-1/2 h-5 w-[2px] rounded-r bg-brass shadow-[0_0_12px_2px_rgb(var(--brass)_/_0.5)]" />}
      <span className={`flex size-7 items-center justify-center rounded-lg shrink-0 ${active ? "bg-brass/10 text-brass ring-1 ring-brass/30" : "bg-white/[0.02] ring-1 ring-white/5"}`}>{icon}</span>
      {!collapsed && <span className="flex-1 text-left font-display text-[12px] tracking-[0.12em] uppercase whitespace-nowrap overflow-hidden">{label}</span>}
      {!collapsed && badge != null && <span className={`rounded-full px-1.5 py-0.5 font-mono text-[9px] ${badgeClass ?? 'bg-white/[0.05] text-zinc-400'}`}>{badge}</span>}
      {collapsed && badge != null && <span className={`absolute right-1 top-1 rounded-full px-1 font-mono text-[8px] ${railBadgeClass ?? 'bg-brass/80 text-zinc-950'}`}>{badge}</span>}
    </button>
  );
}

const SIDEBAR_WIDTHS = { rail: 64, default: 212, wide: 280 };
const SIDEBAR_ORDER: SidebarMode[] = ["rail", "default", "wide"];

const TOP_NAV_META: Record<string, { label: string; icon: string }> = {
  chat: { label: 'Chat', icon: 'message' },
  agents: { label: 'Agents', icon: 'users' },
  runs: { label: 'Runs & Approvals', icon: 'scale' },
  workspace: { label: 'Workspace', icon: 'folder' },
  commands: { label: 'Commands', icon: 'terminal' },
  search: { label: 'Search', icon: 'search' },
  knowledge: { label: 'Knowledge', icon: 'book' },
  compute: { label: 'Compute', icon: 'cpu' },
  settings: { label: 'Settings', icon: 'settings' },
};

interface SidebarProps {
  view: string;
  setView: (v: string) => void;
  agentsCount: number;
  data: DashboardData;
  mode: SidebarMode;
  setMode: (m: SidebarMode) => void;
  pushToast: (t: any) => void;
  appVersion?: string;
  policyBadge?: PolicyBadge | null;
  approvalsPending?: number;
}

export function Sidebar({
  view,
  setView,
  agentsCount,
  mode,
  setMode,
  appVersion,
  policyBadge,
  approvalsPending,
}: SidebarProps) {
  const w = SIDEBAR_WIDTHS[mode];
  const collapsed = mode === "rail";
  const { parent: activeParent } = resolveNavigation(view);
  const [identity, setIdentity] = useState('operator@vox');

  useEffect(() => {
    invoke<{ display_name: string }>('get_identity_summary')
      .then(i => setIdentity(i.display_name))
      .catch(() => {});
  }, []);

  const cycle = (dir: number) => {
    const i = SIDEBAR_ORDER.indexOf(mode);
    const ni = Math.max(0, Math.min(SIDEBAR_ORDER.length - 1, i + dir)) as number;
    setMode(SIDEBAR_ORDER[ni]);
  };

  const activeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: 'nearest' });
  }, [view]);

  const settingsEntry = SURFACE_REGISTRY.find(e => e.viewKey === 'settings');

  return (
    <aside className="shrink-0 flex flex-col transition-[width] duration-200 ease-out h-screen overflow-hidden sticky top-0" style={{ width: w }}>
      <Glass className="flex h-full flex-col p-3 rounded-none border-y-0 border-l-0">
        <div className={`flex items-center ${collapsed ? "justify-center" : "justify-between"} pb-3 shrink-0`}>
          {!collapsed && (
            <div className="flex items-center gap-2 px-1">
              <div className="relative size-6 rounded-md bg-gradient-to-br from-brass via-amber-600 to-zinc-900 ring-1 ring-brass/40">
                <span className="absolute inset-0 grid place-items-center font-display text-[11px] font-bold text-zinc-950">V</span>
              </div>
              <div className="leading-tight">
                <div className="font-display text-[11px] tracking-[0.22em] text-zinc-200">VOX</div>
              </div>
            </div>
          )}
          <div className={`flex items-center ${collapsed ? "flex-col gap-1" : "gap-0.5"}`}>
            <button type="button" onClick={() => cycle(-1)} disabled={mode === "rail"} title="Collapse" aria-label="Collapse sidebar"
              className={`flex size-6 items-center justify-center rounded-md border border-white/5 ${mode === "rail" ? "opacity-30 cursor-not-allowed" : "hover:bg-white/5 text-zinc-400 hover:text-zinc-100"}`}>
              <Icon.chevL className="size-3" aria-hidden="true"/>
            </button>
            <button type="button" onClick={() => cycle(1)} disabled={mode === "wide"} title="Expand" aria-label="Expand sidebar"
              className={`flex size-6 items-center justify-center rounded-md border border-white/5 ${mode === "wide" ? "opacity-30 cursor-not-allowed" : "hover:bg-white/5 text-zinc-400 hover:text-zinc-100"}`}>
              <Icon.chevR className="size-3" aria-hidden="true"/>
            </button>
          </div>
        </div>

        <nav className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden custom-scrollbar flex flex-col gap-0.5 -mr-1 pr-1">
          {TOP_LEVEL_VIEWS.filter(k => k !== 'settings').map(key => {
            const meta = TOP_NAV_META[key] ?? { label: key, icon: 'file' };
            const IconCmp = (Icon as Record<string, any>)[meta.icon] ?? Icon.file;
            const isActive = activeParent === key;
            const badge =
              key === 'agents' ? agentsCount
              : key === 'runs' && approvalsPending != null && approvalsPending > 0 ? approvalsPending
              : undefined;
            return (
              <NavItem
                key={key}
                innerRef={isActive ? activeRef : undefined}
                collapsed={collapsed}
                active={isActive}
                onClick={() => setView(key)}
                icon={<IconCmp className="size-4" />}
                label={meta.label}
                badge={badge}
              />
            );
          })}
        </nav>

        <div className="flex flex-col gap-2 pt-3 shrink-0">
          {settingsEntry && (
            <div className="flex flex-col gap-0.5 border-t border-white/5 pt-2">
              {!collapsed && <div className="px-2 pb-0.5 font-display text-[9px] uppercase tracking-[0.32em] text-zinc-600">System</div>}
              <NavItem
                collapsed={collapsed}
                active={activeParent === 'settings'}
                onClick={() => setView('settings')}
                icon={<Icon.settings className="size-4" />}
                label={settingsEntry.navLabel as string}
                badge={policyBadge && policyBadge.count > 0 ? policyBadge.count : undefined}
                badgeClass={policyBadge ? STATUS_BADGE_CLASS[policyBadge.status] : undefined}
                railBadgeClass={policyBadge ? STATUS_RAIL_BADGE_CLASS[policyBadge.status] : undefined}
              />
            </div>
          )}

          <div className={`flex items-center ${collapsed ? "justify-center" : "gap-2 px-2"} pb-1 pt-1`}>
            <div className="relative size-7 shrink-0 rounded-full bg-gradient-to-br from-violet-500 to-cyan-500">
              <span className="absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full bg-emerald-400 ring-2 ring-zinc-950"/>
            </div>
            {!collapsed && (
              <div className="flex-1 leading-tight overflow-hidden">
                <div className="font-display text-[11px] text-zinc-200 truncate">{identity}</div>
                <div className="font-mono text-[9px] text-zinc-500">build {appVersion ?? 'unknown'} · tauri 2</div>
              </div>
            )}
          </div>
        </div>
      </Glass>
    </aside>
  );
}
