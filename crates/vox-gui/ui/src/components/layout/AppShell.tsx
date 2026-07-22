import React from 'react';
import { Backdrop } from '../ui/Backdrop';
import { Sidebar, type SidebarMode } from './Sidebar';
import { TopHud, type HudMode } from './TopHud';
import { SurfaceScrollHost } from './SurfaceScrollHost';
import { BreadcrumbBar } from './BreadcrumbBar';
import { StatusBar } from './StatusBar';
import { SurfaceErrorBoundary } from '../ui/ErrorBoundary';
import type { DashboardData } from '../../types/dashboard';
import type { PolicyBadge } from './Sidebar';
import type { HudTileKind } from '../../hooks/useHudTiles';
import type { Toast } from '../../types/tauri';
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
  onCommand: () => void;
  onOpenCommandPalette?: () => void;
  lastOrchEventAt: number | null;
  orchUsesPolling: boolean;
  liveFreshMs: number;
  hudMode: HudMode;
  setHudMode: (mode: HudMode) => void;
  surfaceKey: string;
  surfaceLabel: string;
  /** When false, Loquela / transcript stack is omitted (Chat surface hosts composer). */
  chatDocked: boolean;
  chatDock?: React.ReactNode;
  tabBar?: React.ReactNode;
  children: React.ReactNode;
  /** Workspace display name for TopHud (defaults to Operator in TopHud). */
  workspaceTitle?: string;
  visibleTiles?: HudTileKind[];
  activeModel?: string | null;
  openrouterSpendUsd?: number | null;
  gamifyEnabled?: boolean;
  onOpenAchievements?: () => void;
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
  onCommand,
  onOpenCommandPalette,
  lastOrchEventAt,
  orchUsesPolling,
  liveFreshMs,
  hudMode,
  setHudMode,
  surfaceKey,
  surfaceLabel,
  chatDocked,
  chatDock,
  tabBar,
  children,
  workspaceTitle,
  visibleTiles,
  activeModel,
  openrouterSpendUsd,
  gamifyEnabled,
  onOpenAchievements,
}: AppShellProps) {
  const mainPaddingBottom = chatDocked ? 'pb-[180px]' : 'pb-5';

  return (
    <div className="flex h-full w-screen bg-bg-base text-text-muted font-sans selection:bg-brass/30 selection:text-text-primary overflow-hidden">
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
        <div className="p-4 pb-0">
          <TopHud
            kpis={kpis}
            onCommand={onCommand}
            onOpenCommandPalette={onOpenCommandPalette ?? onCommand}
            lastOrchEventAt={lastOrchEventAt}
            orchUsesPolling={orchUsesPolling}
            liveFreshMs={liveFreshMs}
            onNavigate={onNavigate}
            hudMode={hudMode}
            setHudMode={setHudMode}
            workspaceTitle={workspaceTitle}
            visibleTiles={visibleTiles}
            activeModel={activeModel}
            openrouterSpendUsd={openrouterSpendUsd}
            pendingApprovals={pendingApprovals}
          />
          <BreadcrumbBar viewKey={activeView} onNavigate={onNavigate} gamifyEnabled={gamifyEnabled} />
          <StatusBar
            kpis={kpis}
            lastOrchEventAt={lastOrchEventAt}
            orchUsesPolling={orchUsesPolling}
            liveFreshMs={liveFreshMs}
            onNavigate={onNavigate}
            gamifyEnabled={gamifyEnabled}
            onOpenAchievements={onOpenAchievements}
          />
        </div>

        <div className={`flex-1 min-h-0 flex flex-col overflow-hidden p-5 ${mainPaddingBottom}`}>
          {tabBar}
          <SurfaceErrorBoundary key={surfaceKey} surface={surfaceLabel}>
            <SurfaceScrollHost>{children}</SurfaceScrollHost>
          </SurfaceErrorBoundary>
        </div>

        {chatDocked && chatDock != null && (
          <div className="p-4 pt-0 mt-auto" data-testid="loquela-dock">
            {chatDock}
          </div>
        )}
      </main>
    </div>
  );
}
