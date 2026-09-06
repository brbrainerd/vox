import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { AxisMark } from '../ui/AxisMark';
import { DashboardData } from '../../types/dashboard';
import { SURFACE_REGISTRY } from '../../generated/surfaceRegistry.generated';
import { TOP_LEVEL_VIEWS, resolveNavigation, CHILD_ORDER_BY_PARENT, labelForNavKey } from '../../lib/navigation';
import { STATUS_BADGE_CLASS, STATUS_RAIL_BADGE_CLASS } from '../../styles/tokens';
import { useFreshness } from '../../hooks/useFreshness';
import { useLang } from '../../hooks/useLanguage';
import { LEXICON, labelFor, sidebarParentLabel } from '../../lib/lexicon';
import { SessionSidebarSection } from './SessionSidebarSection';
import type { ChatSession } from '../../lib/useChatSessions';

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
}

function NavItem({ active, icon, label, onClick, badge, badgeClass, railBadgeClass, collapsed, innerRef, ariaLabel }: NavItemProps) {
  const effectiveAriaLabel = ariaLabel ?? (collapsed ? label : undefined);
  return (
    <button type="button" ref={innerRef} onClick={onClick} title={collapsed ? label : undefined} aria-label={effectiveAriaLabel}
      className={`group relative flex w-full items-center ${collapsed ? "justify-center px-0" : "gap-3 px-3"} py-2.5 rounded-xl transition ${active ? "bg-overlay-subtle text-text-primary" : "text-text-muted hover:bg-overlay-hover hover:text-text-secondary"}`}>
      {active && <span className="absolute left-0 top-1/2 -translate-y-1/2 h-5 w-[2px] rounded-r bg-brass" />}
      <span className={`flex size-7 items-center justify-center rounded-lg shrink-0 ${active ? "bg-brass/10 text-brass ring-1 ring-brass/30" : "bg-overlay-subtle ring-1 ring-border-subtle"}`}>{icon}</span>
      {!collapsed && <span className="flex-1 min-w-0 text-left font-display text-[12px] tracking-[0.12em] uppercase whitespace-nowrap overflow-hidden text-ellipsis">{label}</span>}
      {!collapsed && badge != null && <span className={`rounded-full px-1.5 py-0.5 font-mono text-[9px] ${badgeClass ?? 'bg-overlay-subtle text-text-muted'}`}>{badge}</span>}
      {collapsed && badge != null && <span className={`absolute right-1 top-1 rounded-full px-1 font-mono text-[8px] ${railBadgeClass ?? 'bg-brass/80 text-bg-base'}`}>{badge}</span>}
    </button>
  );
}

const SIDEBAR_WIDTHS = { rail: 64, default: 212, wide: 280 };
const SIDEBAR_ORDER: SidebarMode[] = ["rail", "default", "wide"];

// Curated nav icons (labels now come from the lexicon by viewKey).
// ponytail: nav English now lives in lexicon.ts; the generated registry navLabel still feeds
// federated search keywords. Reconcile only if they drift. Upgrade path: thread nav_label_la
// through crates/vox-cli/src/commands/ci/gui_surface_registry.rs so the registry carries la natively.
const TOP_NAV_ICON: Record<string, string> = {
  chat: 'message',
  runs: 'shield',
  agents: 'users',
  knowledge: 'book',
  workspace: 'folder',
  commands: 'terminal',
  compute: 'cpu',
  mercatus: 'scale',
  settings: 'settings',
};

const navLabelFor = (key: string, lang: 'en' | 'la') => sidebarParentLabel(key, lang);

interface SidebarProps {
  view: string;
  onOpenParent: (parentKey: string) => void;
  onOpenTab: (viewKey: string) => void;
  agentsCount: number;
  data: DashboardData;
  mode: SidebarMode;
  setMode: (m: SidebarMode) => void;
  pushToast: (t: any) => void;
  appVersion?: string;
  policyBadge?: PolicyBadge | null;
  needsYouCount?: number;
  lastOrchEventAt?: number | null;
  orchUsesPolling?: boolean;
  liveFreshMs?: number;
  onOpenCommandPalette?: () => void;
  chatSessions?: ChatSession[];
  activeSessionId?: string | null;
  chatTaskCounts?: Record<string, number>;
  archivedChatSessions?: ChatSession[];
  showArchivedChatSessions?: boolean;
  /** session_ids with at least one pending scientia_harness_issues row (App.tsx polls). */
  pendingHarnessIssueSessionIds?: Set<string>;
  onSessionChange?: (sessionId: string) => void;
  onCreateSession?: () => void;
  onRenameSession?: (sessionId: string, title: string) => void;
  onArchiveSession?: (sessionId: string) => void;
  onUnarchiveSession?: (sessionId: string) => void;
  onToggleArchivedSessions?: () => void;
  onTaskBadgeClick?: (sessionId: string) => void;
}

export function Sidebar({
  view,
  onOpenParent,
  onOpenTab,
  agentsCount,
  mode,
  setMode,
  appVersion,
  policyBadge,
  needsYouCount,
  lastOrchEventAt = null,
  orchUsesPolling = false,
  liveFreshMs = 10_000,
  onOpenCommandPalette,
  chatSessions,
  activeSessionId,
  chatTaskCounts,
  archivedChatSessions,
  showArchivedChatSessions,
  pendingHarnessIssueSessionIds,
  onSessionChange,
  onCreateSession,
  onRenameSession,
  onArchiveSession,
  onUnarchiveSession,
  onToggleArchivedSessions,
  onTaskBadgeClick,
}: SidebarProps) {
  const w = SIDEBAR_WIDTHS[mode];
  const collapsed = mode === "rail";
  const { parent: activeParent } = resolveNavigation(view);
  const [identity, setIdentity] = useState('operator@vox');
  const tone = useFreshness(lastOrchEventAt, {
    freshMs: liveFreshMs,
    usesPolling: orchUsesPolling,
  });
  const orchDotClass =
    tone === 'live' ? 'bg-accent-secondary' : tone === 'poll' ? 'bg-brass' : 'bg-text-muted';

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

  const { lang } = useLang();
  const activeRef = useRef<HTMLButtonElement>(null);

  const visibleTopLevel = TOP_LEVEL_VIEWS.filter(k => k !== 'settings');

  // null = no override (show active parent's children).
  // '' (empty string) = user explicitly collapsed the active parent.
  const [peekedParent, setPeekedParent] = useState<string | null>(null);

  useEffect(() => {
    setPeekedParent(null);
  }, [activeParent]);

  const expandedParent = peekedParent === '' ? null : peekedParent ?? activeParent;

  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: 'nearest' });
  }, [view]);

  const settingsEntry = SURFACE_REGISTRY.find(e => e.viewKey === 'settings');
  const coverageEntry = SURFACE_REGISTRY.find(e => e.viewKey === 'coverage');

  return (
    <aside aria-label="Sidebar" className="shrink-0 flex flex-col transition-[width] duration-200 ease-out h-full overflow-hidden sticky top-0" style={{ width: w }}>
      <Glass className="flex h-full flex-col p-3 rounded-none border-y-0 border-l-0">
        <div className={`flex items-center ${collapsed ? "justify-center" : "justify-between"} pb-3 shrink-0`}>
          {collapsed && (
            <div className="grid size-6 place-items-center rounded-md bg-white/4 ring-1 ring-brass/40 shrink-0">
              <AxisMark className="size-4 text-brass" />
            </div>
          )}
          {!collapsed && (
            <div className="flex items-center gap-2 px-1">
              <AxisMark size={22} />
              <div className="leading-tight">
                <div className="font-display text-[11px] tracking-[0.22em] text-text-secondary">AXIS</div>
              </div>
            </div>
          )}
          <div className={`flex items-center ${collapsed ? "flex-col gap-1" : "gap-0.5"}`}>
            <button type="button" onClick={() => cycle(-1)} disabled={mode === "rail"} title="Collapse" aria-label="Collapse sidebar"
              className={`flex size-6 items-center justify-center rounded-md border border-border-subtle ${mode === "rail" ? "opacity-30 cursor-not-allowed" : "hover:bg-overlay-hover text-text-muted hover:text-text-primary"}`}>
              <Icon.chevL className="size-3" aria-hidden="true"/>
            </button>
            <button type="button" onClick={() => cycle(1)} disabled={mode === "wide"} title="Expand" aria-label="Expand sidebar"
              className={`flex size-6 items-center justify-center rounded-md border border-border-subtle ${mode === "wide" ? "opacity-30 cursor-not-allowed" : "hover:bg-overlay-hover text-text-muted hover:text-text-primary"}`}>
              <Icon.chevR className="size-3" aria-hidden="true"/>
            </button>
          </div>
        </div>

        {onOpenCommandPalette && (
          <button
            type="button"
            data-testid="omnisearch-trigger"
            onClick={onOpenCommandPalette}
            title={collapsed ? 'Search or jump…' : undefined}
            aria-label="Search or jump to…"
            className={`group relative flex w-full items-center ${collapsed ? "justify-center px-0" : "gap-3 px-3"} py-2.5 mb-1 rounded-xl text-text-muted transition hover:bg-overlay-hover hover:text-text-secondary shrink-0`}
          >
            <span className="flex size-7 items-center justify-center rounded-lg shrink-0 bg-overlay-subtle ring-1 ring-border-subtle">
              <Icon.search className="size-4" aria-hidden="true" />
            </span>
            {!collapsed && (
              <>
                <span className="flex-1 min-w-0 text-left font-display text-[12px] tracking-[0.12em] uppercase whitespace-nowrap overflow-hidden text-ellipsis">
                  Search
                </span>
                <span className="rounded-sm border border-border-subtle bg-overlay-subtle px-1 text-[9px] tracking-widest text-text-muted">⌘K</span>
              </>
            )}
          </button>
        )}

        <nav aria-label="Primary navigation" className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden custom-scrollbar flex flex-col gap-0.5 -mr-1 pr-1">
          {visibleTopLevel.map(key => {
            const label = navLabelFor(key, lang);
            const IconCmp = (Icon as Record<string, any>)[TOP_NAV_ICON[key] ?? 'file'] ?? Icon.file;
            const isActive = activeParent === key;
            const isExpanded = expandedParent === key && mode === 'wide';
            const children = CHILD_ORDER_BY_PARENT[key];
            const badge =
              key === 'agents' ? agentsCount
              : key === 'runs' && needsYouCount != null && needsYouCount > 0 ? needsYouCount
              : undefined;
            const navAriaLabel =
              key === 'runs'
                ? needsYouCount != null && needsYouCount > 0
                  ? `Review, ${needsYouCount} items need you`
                  : 'Review'
                : undefined;
            return (
              <React.Fragment key={key}>
                <div className="flex items-center gap-0.5">
                  <div className="flex-1 min-w-0">
                    <NavItem
                      innerRef={isActive ? activeRef : undefined}
                      collapsed={collapsed}
                      active={isActive}
                      onClick={() => onOpenParent(key)}
                      icon={<IconCmp className="size-4" />}
                      label={label}
                      badge={badge}
                      ariaLabel={navAriaLabel}
                    />
                  </div>
                  {children && mode === 'wide' && (
                    <button
                      type="button"
                      aria-label={`${isExpanded ? 'Collapse' : 'Expand'} ${label}`}
                      aria-expanded={isExpanded}
                      onClick={() => setPeekedParent(isExpanded ? '' : key)}
                      className="flex size-6 shrink-0 items-center justify-center rounded-md text-text-muted hover:bg-overlay-hover hover:text-text-primary"
                    >
                      <Icon.chevR className={`size-3 transition-transform ${isExpanded ? 'rotate-90' : ''}`} aria-hidden="true" />
                    </button>
                  )}
                </div>
                {isExpanded && key === 'chat' && chatSessions && (
                  <div className="ml-4 border-l border-border-subtle pl-2 max-h-[50vh] overflow-y-auto custom-scrollbar">
                    <SessionSidebarSection
                      sessions={chatSessions}
                      activeSessionId={activeSessionId ?? null}
                      taskCounts={chatTaskCounts ?? {}}
                      archivedSessions={archivedChatSessions ?? []}
                      showArchived={showArchivedChatSessions ?? false}
                      pendingIssueSessionIds={pendingHarnessIssueSessionIds}
                      onSessionChange={onSessionChange ?? (() => {})}
                      onCreateSession={onCreateSession ?? (() => {})}
                      onRenameSession={onRenameSession ?? (() => {})}
                      onArchiveSession={onArchiveSession ?? (() => {})}
                      onUnarchiveSession={onUnarchiveSession ?? (() => {})}
                      onToggleArchivedView={onToggleArchivedSessions ?? (() => {})}
                      onTaskBadgeClick={onTaskBadgeClick ?? (() => {})}
                    />
                  </div>
                )}
                {isExpanded && key !== 'chat' && children && (
                  <div className="ml-4 flex flex-col gap-0.5 border-l border-border-subtle pl-2">
                    {children.map(childKey => (
                      <button
                        key={childKey}
                        type="button"
                        onClick={() => onOpenTab(childKey)}
                        aria-current={view === childKey ? 'page' : undefined}
                        className={`w-full rounded-lg px-2 py-1.5 text-left font-display text-[11px] tracking-widest uppercase transition ${
                          view === childKey
                            ? 'bg-brass/10 text-brass'
                            : 'text-text-muted hover:bg-overlay-hover hover:text-text-secondary'
                        }`}
                      >
                        {labelForNavKey(childKey)}
                      </button>
                    ))}
                  </div>
                )}
              </React.Fragment>
            );
          })}
        </nav>

        <div className="flex flex-col gap-2 pt-3 shrink-0">
          {settingsEntry && (
            <div className={`flex flex-col gap-0.5 pt-2 ${collapsed ? 'border-t border-border-subtle' : ''}`}>
              {!collapsed && <div className="mx-2 mb-1 border-b border-border-subtle px-0 pb-1 font-display text-[9px] uppercase tracking-[0.32em] text-text-muted">{labelFor('group:system', lang)}</div>}
              <NavItem
                collapsed={collapsed}
                active={activeParent === 'settings'}
                onClick={() => onOpenTab('settings')}
                icon={<Icon.settings className="size-4" />}
                label={labelFor('settings', lang)}
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
                  onClick={() => onOpenTab('coverage')}
                  icon={<Icon.check className="size-4" />}
                  label={labelFor('coverage', lang)}
                  ariaLabel="Coverage, CI surface gaps"
                />
              )}
            </div>
          )}

          <div className={`flex items-center ${collapsed ? "justify-center" : "gap-2 px-2"} pb-1 pt-1`}>
            <div className="relative size-7 shrink-0 rounded-full bg-linear-to-br from-violet-500 to-cyan-500">
              <span
                data-testid="sidebar-orch-freshness-dot"
                aria-hidden="true"
                className={`absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full ring-2 ring-bg-base ${orchDotClass}`}
              />
            </div>
            {!collapsed && (
              <div className="flex-1 leading-tight overflow-hidden">
                <div className="font-display text-[11px] text-text-secondary truncate">{identity}</div>
                <div className="font-mono text-[9px] text-text-muted">Vox Axis · build {appVersion ?? 'unknown'}</div>
              </div>
            )}
          </div>
        </div>
      </Glass>
    </aside>
  );
}
