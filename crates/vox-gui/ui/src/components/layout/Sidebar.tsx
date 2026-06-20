import React, { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { AxisMark } from '../brand/AxisMark';
import { DashboardData } from '../../types/dashboard';
import { SURFACE_REGISTRY } from '../../generated/surfaceRegistry.generated';
import { useLocalStorage } from '../../hooks/useLocalStorage';
import { TOP_LEVEL_VIEWS, resolveNavigation } from '../../lib/navigation';
import { STATUS_BADGE_CLASS, STATUS_RAIL_BADGE_CLASS } from '../../styles/tokens';
import { useFreshness } from '../../hooks/useFreshness';
import { SHELL_PREFERENCE_KEYS } from '../../lib/shellPersistence';
import { SidebarResizer } from './SidebarResizer';

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
  ariaLabel?: string;
  onMouseDown?: (e: React.MouseEvent<HTMLButtonElement>) => void;
  onOpenInNewPanel?: () => void;
}

function NavItem({ active, icon, label, onClick, onMouseDown, onOpenInNewPanel, badge, badgeClass, railBadgeClass, collapsed, innerRef, ariaLabel }: NavItemProps) {
  const effectiveAriaLabel = ariaLabel ?? (collapsed ? label : undefined);
  // Wrap in a `group` div so the sibling ⊞ button can use group-hover — a
  // focusable button must NOT be nested inside another button (invalid HTML).
  return (
    <div className="group relative flex w-full items-center">
      <button type="button" ref={innerRef} onClick={onClick} onMouseDown={onMouseDown} title={collapsed ? label : undefined} aria-label={effectiveAriaLabel}
        className={`relative flex flex-1 items-center ${collapsed ? "justify-center px-0" : "gap-3 px-3"} py-2.5 rounded-xl transition ${active ? "bg-white/[0.04] text-zinc-100" : "text-zinc-500 hover:bg-white/[0.025] hover:text-zinc-200"}`}>
        {active && <span className="absolute left-0 top-1/2 -translate-y-1/2 h-5 w-[2px] rounded-r bg-brass shadow-[0_0_12px_2px_rgb(var(--brass)_/_0.5)]" />}
        <span className={`flex size-7 items-center justify-center rounded-lg shrink-0 ${active ? "bg-brass/10 text-brass ring-1 ring-brass/30" : "bg-white/[0.02] ring-1 ring-white/5"}`}>{icon}</span>
        {!collapsed && <span className={`flex-1 text-left font-display text-[12px] tracking-[0.12em] uppercase whitespace-nowrap overflow-hidden ${!collapsed && onOpenInNewPanel ? 'pr-5' : ''}`}>{label}</span>}
        {!collapsed && badge != null && <span className={`rounded-full px-1.5 py-0.5 font-mono text-[9px] ${badgeClass ?? 'bg-white/[0.05] text-zinc-400'}`}>{badge}</span>}
        {collapsed && badge != null && <span className={`absolute right-1 top-1 rounded-full px-1 font-mono text-[8px] ${railBadgeClass ?? 'bg-brass/80 text-zinc-950'}`}>{badge}</span>}
      </button>
      {!collapsed && onOpenInNewPanel && (
        <button
          type="button"
          onClick={(e) => { e.stopPropagation(); onOpenInNewPanel(); }}
          onKeyDown={(e) => {
            if (e.key === ' ') { e.preventDefault(); e.stopPropagation(); onOpenInNewPanel(); }
            else if (e.key === 'Enter') { e.stopPropagation(); onOpenInNewPanel(); }
          }}
          title="Open in new panel"
          aria-label={`Open ${label} in new panel`}
          className="opacity-0 group-hover:opacity-100 absolute right-1 top-1/2 -translate-y-1/2 z-10 p-0.5 rounded hover:bg-white/10 text-zinc-400 hover:text-zinc-100 transition-opacity shrink-0"
        >
          ⊞
        </button>
      )}
    </div>
  );
}

const SIDEBAR_WIDTHS = { rail: 64, default: 212, wide: 280 };
const SIDEBAR_ORDER: SidebarMode[] = ["rail", "default", "wide"];
const SIDEBAR_FILTER_COLLAPSED_KEY = 'gui.sidebar.filter_collapsed.v1';

function matchesNavFilter(label: string, query: string): boolean {
  return label.toLowerCase().includes(query.toLowerCase());
}

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
  lastOrchEventAt?: number | null;
  orchUsesPolling?: boolean;
  liveFreshMs?: number;
  onOpenPanel?: (viewKey: string) => void;
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
  lastOrchEventAt = null,
  orchUsesPolling = false,
  liveFreshMs = 10_000,
  onOpenPanel,
}: SidebarProps) {
  const handleMouseDown = (e: React.MouseEvent, key: string) => {
    if (e.button === 1) {
      e.preventDefault();
      onOpenPanel?.(key);
    }
  };
  const [width, setWidth] = useLocalStorage<number>(SHELL_PREFERENCE_KEYS.sidebarWidth, SIDEBAR_WIDTHS[mode]);
  const [dragWidth, setDragWidth] = useState<number | null>(null);

  // Sync width to preset only when the user actually cycles the mode; skip
  // the initial mount so a persisted custom drag-width is not overwritten.
  const prevModeRef = useRef<SidebarMode>(mode);
  useEffect(() => {
    if (prevModeRef.current === mode) return;
    prevModeRef.current = mode;
    if (mode !== 'rail') {
      setWidth(SIDEBAR_WIDTHS[mode]);
    }
  }, [mode, setWidth]);

  const collapsed = mode === "rail";
  const { parent: activeParent } = resolveNavigation(view);
  const [identity, setIdentity] = useState('operator@vox');
  const [osUser, setOsUser] = useState<string | null>(null);
  const [identityOpen, setIdentityOpen] = useState(false);
  const tone = useFreshness(lastOrchEventAt, {
    freshMs: liveFreshMs,
    usesPolling: orchUsesPolling,
  });
  const orchDotClass =
    tone === 'live' ? 'bg-emerald-400' : tone === 'poll' ? 'bg-amber-400' : 'bg-zinc-500';

  useEffect(() => {
    invoke<{ display_name: string; os_user: string | null }>('get_identity_summary')
      .then(i => { setIdentity(i.display_name); setOsUser(i.os_user ?? null); })
      .catch(() => {});
  }, []);

  // Jump to a specific Settings section (seed + navigate). SettingsView reads the
  // JSON `{ section }` seed on mount and on the `vox-settings-seed` event.
  const openSettingsSection = (section: string) => {
    try {
      localStorage.setItem('vox_settings_seed', JSON.stringify({ section }));
      window.dispatchEvent(new Event('vox-settings-seed'));
    } catch { /* localStorage unavailable — Settings still opens */ }
    setView('settings');
    setIdentityOpen(false);
  };

  const cycle = (dir: number) => {
    const i = SIDEBAR_ORDER.indexOf(mode);
    const ni = Math.max(0, Math.min(SIDEBAR_ORDER.length - 1, i + dir)) as number;
    setMode(SIDEBAR_ORDER[ni]);
  };

  const activeRef = useRef<HTMLButtonElement>(null);
  const [navFilter, setNavFilter] = useState('');
  const [filterCollapsed, setFilterCollapsed] = useLocalStorage<boolean>(
    SIDEBAR_FILTER_COLLAPSED_KEY,
    false,
  );
  const filterQuery = navFilter.trim();

  const childTabsByParent = useMemo(() => {
    const map = new Map<string, Array<{ viewKey: string; label: string }>>();
    for (const entry of SURFACE_REGISTRY) {
      if (!entry.parentSurface || !entry.viewKey || !entry.navLabel) continue;
      const parent = entry.parentSurface as string;
      const list = map.get(parent) ?? [];
      list.push({ viewKey: entry.viewKey as string, label: entry.navLabel as string });
      map.set(parent, list);
    }
    return map;
  }, []);

  const visibleTopLevel = useMemo(() => {
    const keys = TOP_LEVEL_VIEWS.filter(k => k !== 'settings');
    if (!filterQuery) return keys;
    return keys.filter(key => {
      const label = TOP_NAV_META[key]?.label ?? key;
      if (matchesNavFilter(label, filterQuery)) return true;
      const children = childTabsByParent.get(key) ?? [];
      return children.some(child => matchesNavFilter(child.label, filterQuery));
    });
  }, [childTabsByParent, filterQuery]);

  const visibleChildTabs = (parentKey: string) => {
    const children = childTabsByParent.get(parentKey) ?? [];
    if (!filterQuery) return [];
    const parentLabel = TOP_NAV_META[parentKey]?.label ?? parentKey;
    if (matchesNavFilter(parentLabel, filterQuery)) {
      return children.filter(
        child =>
          matchesNavFilter(child.label, filterQuery) ||
          matchesNavFilter(parentLabel, filterQuery),
      );
    }
    return children.filter(child => matchesNavFilter(child.label, filterQuery));
  };

  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: 'nearest' });
  }, [view]);

  const settingsEntry = SURFACE_REGISTRY.find(e => e.viewKey === 'settings');
  const coverageEntry = SURFACE_REGISTRY.find(e => e.viewKey === 'coverage');

  const w = collapsed ? SIDEBAR_WIDTHS.rail : (dragWidth ?? width);

  return (
    <aside className={`shrink-0 flex flex-col relative h-screen overflow-hidden sticky top-0 ${dragWidth === null ? "transition-[width] duration-200 ease-out" : ""}`} style={{ width: w }}>
      <Glass className="flex h-full flex-col p-3 rounded-none border-y-0 border-l-0">
        <div className={`flex items-center ${collapsed ? "justify-center" : "justify-between"} pb-3 shrink-0`}>
          {collapsed && (
            <div className="grid size-6 place-items-center rounded-md bg-white/[0.04] ring-1 ring-brass/40 shrink-0">
              <AxisMark className="size-4 text-brass" />
            </div>
          )}
          {!collapsed && (
            <div className="flex items-center gap-2 px-1">
              <div className="relative grid size-6 place-items-center rounded-md bg-white/[0.04] ring-1 ring-brass/40">
                <AxisMark className="size-4 text-brass" />
              </div>
              <div className="leading-tight">
                <div className="font-display text-[11px] tracking-[0.22em] text-zinc-200">AXIS</div>
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

        {!collapsed && (
          <div className="pb-2 shrink-0">
            <button
              type="button"
              onClick={() => setFilterCollapsed(!filterCollapsed)}
              aria-expanded={!filterCollapsed}
              aria-label="Filter navigation"
              className="flex w-full items-center justify-between rounded-lg border border-white/5 px-2 py-1.5 text-[10px] uppercase tracking-[0.18em] text-zinc-500 hover:bg-white/[0.02] hover:text-zinc-300 transition"
            >
              <span>Filter nav…</span>
              <Icon.chevronDown className={`size-3 transition ${filterCollapsed ? '' : 'rotate-180'}`} aria-hidden="true" />
            </button>
            {!filterCollapsed && (
              <input
                data-testid="sidebar-nav-filter"
                aria-label="Filter navigation"
                type="search"
                value={navFilter}
                onChange={e => setNavFilter(e.target.value)}
                placeholder="Filter nav…"
                className="mt-1.5 w-full rounded-lg border border-white/5 bg-white/[0.02] px-2.5 py-1.5 text-[11px] text-zinc-200 placeholder:text-zinc-600 focus:border-brass/30 focus:outline-none"
              />
            )}
          </div>
        )}

        <nav className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden custom-scrollbar flex flex-col gap-0.5 -mr-1 pr-1">
          {visibleTopLevel.map(key => {
            const meta = TOP_NAV_META[key] ?? { label: key, icon: 'file' };
            const IconCmp = (Icon as Record<string, any>)[meta.icon] ?? Icon.file;
            const isActive = activeParent === key;
            const badge =
              key === 'agents' ? agentsCount
              : key === 'runs' && approvalsPending != null && approvalsPending > 0 ? approvalsPending
              : undefined;
            const navAriaLabel =
              key === 'runs'
                ? approvalsPending != null && approvalsPending > 0
                  ? `Runs and Approvals, ${approvalsPending} pending`
                  : 'Runs and Approvals'
                : undefined;
            return (
              <React.Fragment key={key}>
                <NavItem
                  innerRef={isActive ? activeRef : undefined}
                  collapsed={collapsed}
                  active={isActive}
                  onClick={() => setView(key)}
                  onMouseDown={(e) => handleMouseDown(e, key)}
                  onOpenInNewPanel={onOpenPanel ? () => onOpenPanel(key) : undefined}
                  icon={<IconCmp className="size-4" />}
                  label={meta.label}
                  badge={badge}
                  ariaLabel={navAriaLabel}
                />
                {!collapsed &&
                  visibleChildTabs(key).map(child => (
                    <button
                      key={child.viewKey}
                      type="button"
                      onClick={() => setView(child.viewKey)}
                      onMouseDown={(e) => handleMouseDown(e, child.viewKey)}
                      className={`ml-6 flex w-[calc(100%-1.5rem)] items-center rounded-lg px-2.5 py-1.5 text-left text-[11px] transition ${
                        view === child.viewKey
                          ? 'bg-white/[0.04] text-zinc-100'
                          : 'text-zinc-500 hover:bg-white/[0.025] hover:text-zinc-200'
                      }`}
                    >
                      {child.label}
                    </button>
                  ))}
              </React.Fragment>
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
                onMouseDown={(e) => handleMouseDown(e, 'settings')}
                onOpenInNewPanel={onOpenPanel ? () => onOpenPanel('settings') : undefined}
                icon={<Icon.settings className="size-4" />}
                label={settingsEntry.navLabel as string}
                badge={policyBadge && policyBadge.count > 0 ? policyBadge.count : undefined}
                badgeClass={policyBadge ? STATUS_BADGE_CLASS[policyBadge.status] : undefined}
                railBadgeClass={policyBadge ? STATUS_RAIL_BADGE_CLASS[policyBadge.status] : undefined}
                ariaLabel={
                  policyBadge && policyBadge.count > 0
                    ? `Settings, ${policyBadge.count} policy failures`
                    : 'Settings'
                }
              />
              {coverageEntry && (
                <NavItem
                  collapsed={collapsed}
                  active={view === 'coverage'}
                  onClick={() => setView('coverage')}
                  onMouseDown={(e) => handleMouseDown(e, 'coverage')}
                  onOpenInNewPanel={onOpenPanel ? () => onOpenPanel('coverage') : undefined}
                  icon={<Icon.check className="size-4" />}
                  label={coverageEntry.navLabel as string}
                  ariaLabel="Coverage, CI surface gaps"
                />
              )}
            </div>
          )}

          <div className="relative">
            <button
              type="button"
              data-testid="sidebar-identity-button"
              aria-haspopup="menu"
              aria-expanded={identityOpen}
              aria-label="Identity, keys & federation"
              onClick={() => setIdentityOpen(o => !o)}
              className={`flex w-full items-center rounded-lg ${collapsed ? "justify-center px-0" : "gap-2 px-2"} pb-1 pt-1 transition hover:bg-white/[0.03]`}
            >
              <div className="relative size-7 shrink-0 rounded-full bg-gradient-to-br from-violet-500 to-cyan-500">
                <span
                  data-testid="sidebar-orch-freshness-dot"
                  aria-hidden="true"
                  className={`absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full ring-2 ring-zinc-950 ${orchDotClass}`}
                />
              </div>
              {!collapsed && (
                <div className="flex-1 leading-tight overflow-hidden text-left">
                  <div className="font-display text-[11px] text-zinc-200 truncate">{identity}</div>
                  <div className="font-mono text-[9px] text-zinc-500">Vox Axis · build {appVersion ?? 'unknown'} · tauri 2</div>
                </div>
              )}
            </button>

            {identityOpen && (
              <>
                {/* Click-away backdrop */}
                <button
                  type="button"
                  aria-hidden="true"
                  tabIndex={-1}
                  onClick={() => setIdentityOpen(false)}
                  className="fixed inset-0 z-40 cursor-default"
                />
                <div
                  role="menu"
                  data-testid="sidebar-identity-menu"
                  className="absolute bottom-full z-50 mb-2 w-60 left-0 rounded-xl border border-white/10 bg-zinc-950/95 p-2 shadow-2xl backdrop-blur"
                >
                  <div className="px-2 py-1.5">
                    <div className="font-display text-[12px] text-zinc-100 truncate">{identity}</div>
                    <div className="font-mono text-[10px] text-zinc-500 truncate">
                      {osUser ? `os user · ${osUser}` : 'local identity'}
                    </div>
                    <div className="mt-1 font-mono text-[9px] text-zinc-600">
                      Local identity — Vox has no cloud login. Federation is peer trust on the mesh.
                    </div>
                  </div>
                  <div className="my-1 h-px bg-white/5" />
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => openSettingsSection('mesh')}
                    className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[12px] text-zinc-300 hover:bg-white/[0.05] hover:text-zinc-100"
                  >
                    <Icon.link className="size-3.5 text-zinc-500" aria-hidden="true" />
                    Mesh &amp; peers (federation)
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => openSettingsSection('signing')}
                    className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[12px] text-zinc-300 hover:bg-white/[0.05] hover:text-zinc-100"
                  >
                    <Icon.shield className="size-3.5 text-zinc-500" aria-hidden="true" />
                    Signing keys
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => openSettingsSection('secrets')}
                    className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[12px] text-zinc-300 hover:bg-white/[0.05] hover:text-zinc-100"
                  >
                    <Icon.shield className="size-3.5 text-zinc-500" aria-hidden="true" />
                    Keys &amp; secrets
                  </button>
                  <div className="my-1 h-px bg-white/5" />
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => { setView('settings'); setIdentityOpen(false); }}
                    className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[12px] text-zinc-300 hover:bg-white/[0.05] hover:text-zinc-100"
                  >
                    <Icon.settings className="size-3.5 text-zinc-500" aria-hidden="true" />
                    All settings
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </Glass>
      {!collapsed && (
        <SidebarResizer
          onResize={setDragWidth}
          onCommit={(px) => {
            setWidth(px);
            setDragWidth(null);
          }}
          onReset={() => {
            setWidth(SIDEBAR_WIDTHS.default);
          }}
        />
      )}
    </aside>
  );
}
