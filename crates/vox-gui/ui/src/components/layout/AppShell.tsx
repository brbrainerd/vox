import React from 'react';
import { Backdrop } from '../ui/Backdrop';
import { Sidebar, type SidebarMode } from './Sidebar';
import { SurfaceScrollHost } from './SurfaceScrollHost';
import { BreadcrumbBar } from './BreadcrumbBar';
import { BottomStatusBar } from './BottomStatusBar';
import { Icon } from '../ui/Icons';
import { SurfaceErrorBoundary } from '../ui/ErrorBoundary';
import type { DashboardData } from '../../types/dashboard';
import type { PolicyBadge } from './Sidebar';
import type { HudTilesConfig } from '../../hooks/useHudTiles';
import type { Toast } from '../../types/tauri';
import type { MeshNode } from '../surfaces/Mesh/MeshView';
import { INITIAL_KPIS } from '../../data/initialState';

type KpiState = typeof INITIAL_KPIS;

export interface AppShellProps {
  activeView: string;
  onNavigate: (view: string) => void;
  onOpenParent: (parentKey: string) => void;
  onOpenTab: (viewKey: string) => void;
  sidebarMode: SidebarMode;
  setSidebarMode: (mode: SidebarMode) => void;
  agentsCount: number;
  data: DashboardData;
  pushToast: (t: Toast) => void;
  appVersion: string;
  policyBadge: PolicyBadge;
  needsYouCount: number;
  pendingApprovals: number;
  kpis: KpiState;
  onOpenCommandPalette: () => void;
  lastOrchEventAt: number | null;
  orchUsesPolling: boolean;
  liveFreshMs: number;
  surfaceKey: string;
  surfaceLabel: string;
  /** When false, Loquela / transcript stack is omitted (Chat surface hosts composer). */
  chatDocked: boolean;
  chatDock?: React.ReactNode;
  children: React.ReactNode;
  workspaceTitle?: string;
  activeModel?: string | null;
  openrouterSpendUsd?: number | null;
  gamifyEnabled?: boolean;
  onOpenAchievements?: () => void;
  hudTilesConfig: HudTilesConfig;
  onHudTilesChange: (config: HudTilesConfig) => void;
  meshNodes: MeshNode[] | undefined;
}

export function AppShell({
  activeView,
  onNavigate,
  onOpenParent,
  onOpenTab,
  sidebarMode,
  setSidebarMode,
  agentsCount,
  data,
  pushToast,
  appVersion,
  policyBadge,
  needsYouCount,
  pendingApprovals,
  kpis,
  onOpenCommandPalette,
  lastOrchEventAt,
  orchUsesPolling,
  liveFreshMs,
  surfaceKey,
  surfaceLabel,
  chatDocked,
  chatDock,
  children,
  activeModel,
  openrouterSpendUsd,
  gamifyEnabled,
  onOpenAchievements,
  hudTilesConfig,
  onHudTilesChange,
  meshNodes,
}: AppShellProps) {
  const mainPaddingBottom = chatDocked ? 'pb-[180px]' : 'pb-5';

  return (
    <div className="flex flex-1 min-h-0 w-screen bg-bg-base text-text-muted font-sans selection:bg-brass/30 selection:text-text-primary overflow-hidden">
      <Backdrop />

      <Sidebar
        view={activeView}
        onOpenParent={onOpenParent}
        onOpenTab={onOpenTab}
        agentsCount={agentsCount}
        data={data}
        mode={sidebarMode}
        setMode={setSidebarMode}
        pushToast={pushToast}
        appVersion={appVersion}
        policyBadge={policyBadge}
        needsYouCount={needsYouCount}
        lastOrchEventAt={lastOrchEventAt}
        orchUsesPolling={orchUsesPolling}
        liveFreshMs={liveFreshMs}
      />

      <main className="flex-1 flex flex-col min-w-0 relative">
        <div className="p-4 pb-0 flex items-center justify-between gap-2">
          <div className="min-w-0 flex-1 truncate">
            <BreadcrumbBar viewKey={activeView} onNavigate={onNavigate} gamifyEnabled={gamifyEnabled} />
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            {gamifyEnabled && onOpenAchievements && (
              <button
                type="button"
                data-testid="achievements-trigger"
                aria-label="Open achievements"
                onClick={onOpenAchievements}
                className="inline-flex items-center justify-center rounded px-1.5 py-0.5 text-amber-300/80 hover:bg-overlay-subtle hover:text-amber-200 transition"
              >
                <Icon.trophy className="size-3.5" aria-hidden="true" />
              </button>
            )}
            <button
              type="button"
              data-testid="omnisearch-trigger"
              onClick={onOpenCommandPalette}
              className="inline-flex items-center gap-1.5 rounded border border-border-subtle bg-overlay-subtle px-2 py-0.5 text-xs text-text-muted hover:border-brass/40 hover:text-brass transition"
            >
              <span>Search or jump…</span>
              <span className="rounded border border-border-subtle bg-overlay-subtle px-1 text-[9px] tracking-widest text-text-muted">⌘K</span>
            </button>
          </div>
        </div>

        <div className={`flex-1 min-h-0 flex flex-col overflow-hidden p-5 ${mainPaddingBottom}`}>
          <SurfaceErrorBoundary key={surfaceKey} surface={surfaceLabel}>
            <SurfaceScrollHost>{children}</SurfaceScrollHost>
          </SurfaceErrorBoundary>
        </div>

        {chatDocked && chatDock != null && (
          <div className="p-4 pt-0 mt-auto" data-testid="loquela-dock">
            {chatDock}
          </div>
        )}

        <div className="px-4 pb-2">
          <BottomStatusBar
            kpis={kpis}
            hudTilesConfig={hudTilesConfig}
            onHudTilesChange={onHudTilesChange}
            onNavigate={onNavigate}
            lastOrchEventAt={lastOrchEventAt}
            orchUsesPolling={orchUsesPolling}
            liveFreshMs={liveFreshMs}
            activeModel={activeModel}
            openrouterSpendUsd={openrouterSpendUsd}
            pendingApprovals={pendingApprovals}
            meshNodes={meshNodes}
          />
        </div>
      </main>
    </div>
  );
}
