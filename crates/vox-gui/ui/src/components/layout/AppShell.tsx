import React from 'react';
import { Backdrop } from '../ui/Backdrop';
import { Sidebar, type SidebarMode } from './Sidebar';
import { SurfaceScrollHost } from './SurfaceScrollHost';
import { BreadcrumbBar } from './BreadcrumbBar';
import { BottomStatusBar } from './BottomStatusBar';
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
    <div className="flex flex-1 min-h-0 w-screen flex-col bg-bg-base text-text-muted font-sans selection:bg-brass/30 selection:text-text-primary overflow-hidden">
      <Backdrop />

      <div className="flex flex-1 min-h-0">
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
          onOpenCommandPalette={onOpenCommandPalette}
        />

        <main className="flex-1 flex flex-col min-w-0 relative">
          <div className="px-4 pt-3 pb-0">
            <BreadcrumbBar viewKey={activeView} onNavigate={onNavigate} gamifyEnabled={gamifyEnabled} />
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
        </main>
      </div>

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
        gamifyEnabled={gamifyEnabled}
        onOpenAchievements={onOpenAchievements}
      />
    </div>
  );
}
