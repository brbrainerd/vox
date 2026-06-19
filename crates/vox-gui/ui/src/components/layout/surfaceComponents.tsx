import React from 'react';
import { Dashboard } from '../surfaces/Dashboard/Dashboard';
import { AgentFlow } from '../surfaces/Flow/AgentFlow';
import { Catalog } from '../surfaces/Catalog/Catalog';
import { Matrix } from '../surfaces/Matrix/Matrix';
import { MemoryView } from '../surfaces/Memory/MemoryView';
import { ModelsView } from '../surfaces/Models/ModelsView';
import { RunsView } from '../surfaces/Runs/RunsView';
import { TasksView } from '../surfaces/Tasks/TasksView';
import { SettingsView } from '../surfaces/Settings/SettingsView';
import { RepositoryView } from '../surfaces/Repository/RepositoryView';
import { MeshView } from '../surfaces/Mesh/MeshView';
import { GamifyView } from '../surfaces/Gamify/GamifyView';
import { HarnessRedirect } from '../surfaces/Harness/HarnessRedirect';
import { BrowserView } from '../surfaces/Browser/BrowserView';
import { ApprovalsView } from '../surfaces/Approvals/ApprovalsView';
import { SkillsPluginsView } from '../surfaces/SkillsPlugins/SkillsPluginsView';
import { PoliciesView } from '../surfaces/Policies/PoliciesView';
import { ParentSurface } from './ParentSurface';
import { surfaceDecorators } from '../surfaces/decoratorRegistry';
import { ChatSurface } from '../surfaces/Chat/ChatSurface';
import type {
  ChatExecutionRailKpis,
  ChatExecutionTask,
} from '../surfaces/Chat/ChatExecutionRail';
import { Console } from '../surfaces/Console/Console';
import type { DashboardData, Agent, LudusAlert, StreamItem } from '../../types/dashboard';
import type { CatalogEntry, Toast, AttentionBudgetSnapshot } from '../../types/tauri';
import type { ChatMessage } from '../../lib/chatCorrelation';
import type { HudTilesConfig } from '../../hooks/useHudTiles';

export interface SurfaceProps {
  pushToast: (t: Toast) => void;
  data: DashboardData;
  dashboardLoading?: boolean;
  onPause?: (a: Agent) => void;
  onResume?: (a: Agent) => void;
  onDoubt?: (item: StreamItem) => void;
  onOverrule?: (item: StreamItem) => void;
  onAckLudus?: (note: LudusAlert) => void;
  filterKind?: string;
  setFilterKind?: (k: string) => void;
  selectedAgentId?: string;
  setSelectedAgentId?: (id: string) => void;
  skills?: CatalogEntry[];
  onAttachContext?: (items: Array<{ kind: 'file' | 'url' | 'image'; label: string }>) => void;
  onNavigate?: (viewKey: string) => void;
  onOpenChat?: () => void;
  onOpenInConsole?: (a: Agent) => void;
  activeChild?: string;
  onChildChange?: (viewKey: string) => void;
  activeSessionId?: string;
  onSessionChange?: (sessionId: string) => void;
  chatMessages?: ChatMessage[];
  onHydrateChatSession?: (sessionId: string) => void;
  onFocusComposer?: () => void;
  chatTasks?: ChatExecutionTask[];
  chatIntents?: string[];
  chatExecutionKpis?: ChatExecutionRailKpis;
  chatActiveModel?: string | null;
  chatOpenrouterSpendUsd?: number | null;
  chatAgentStreamItems?: StreamItem[];
  onOpenAgentInFlow?: (agentId: string) => void;
  chatComposer?: React.ReactNode;
  gamifyEnabled?: boolean;
  hudTilesConfig?: HudTilesConfig;
  onHudTilesChange?: (config: HudTilesConfig) => void;
  attention_budget?: AttentionBudgetSnapshot | null;
}

function childRenderer(props: SurfaceProps, viewKey: string): React.ReactNode {
  const Decorator = surfaceDecorators[viewKey];
  if (Decorator) {
    return <Decorator pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
  }
  switch (viewKey) {
    case 'dashboard':
      return (
        <Dashboard
          data={props.data}
          loading={props.dashboardLoading}
          onPause={props.onPause!}
          onResume={props.onResume!}
          onDoubt={props.onDoubt!}
          onOverrule={props.onOverrule!}
          onAckLudus={props.onAckLudus!}
          filterKind={props.filterKind!}
          setFilterKind={props.setFilterKind!}
          onOpenInConsole={props.onOpenInConsole}
          onOpenChat={props.onOpenChat}
          onNavigate={props.onNavigate}
          attention_budget={props.attention_budget}
        />
      );
    case 'flow':
      return (
        <AgentFlow
          agents={props.data.agents}
          selectedId={props.selectedAgentId!}
          onSelect={props.setSelectedAgentId!}
        />
      );
    case 'catalog':
      return <Catalog skills={props.data.skills} />;
    case 'matrix':
      return <Matrix pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'memory':
      return <MemoryView pushToast={props.pushToast} onAttachContext={props.onAttachContext} />;
    case 'models':
      return <ModelsView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'runs':
      return <RunsView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'tasks':
      return <TasksView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'settings':
      return (
        <SettingsView
          pushToast={props.pushToast}
          gamifyEnabled={props.gamifyEnabled}
          hudTilesConfig={props.hudTilesConfig}
          onHudTilesChange={props.onHudTilesChange}
        />
      );
    case 'repository':
      return <RepositoryView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'mesh':
      return <MeshView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'gamify':
      return <GamifyView pushToast={props.pushToast} />;
    case 'harness':
      return (
        <HarnessRedirect
          onFocusComposer={props.onFocusComposer}
          gamifyEnabled={props.gamifyEnabled}
        />
      );
    case 'browser':
      return <BrowserView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'console':
      return (
        <Console
          pushToast={props.pushToast}
          gamifyEnabled={props.gamifyEnabled}
          initialAgentId={
            props.selectedAgentId && props.selectedAgentId !== 'ROOT'
              ? props.selectedAgentId
              : null
          }
        />
      );
    case 'approvals':
      return <ApprovalsView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'policies':
      return <PoliciesView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'skills':
      return <SkillsPluginsView pushToast={props.pushToast} />;
    case 'chat':
      return (
        <ChatSurface
          pushToast={props.pushToast}
          onNavigate={props.onNavigate}
          messages={props.chatMessages}
          activeSessionId={props.activeSessionId}
          onSessionChange={props.onSessionChange}
          onHydrateSession={props.onHydrateChatSession}
          tasks={props.chatTasks}
          intents={props.chatIntents}
          executionKpis={props.chatExecutionKpis}
          activeModel={props.chatActiveModel}
          openrouterSpendUsd={props.chatOpenrouterSpendUsd}
          agentStreamItems={props.chatAgentStreamItems}
          onOpenAgentInFlow={props.onOpenAgentInFlow}
          composer={props.chatComposer}
        />
      );
    default:
      return null;
  }
}

export function renderSurfaceView(parentKey: string, props: SurfaceProps): React.ReactNode {
  if (parentKey === 'chat') {
    return childRenderer(props, 'chat');
  }
  const activeChild = props.activeChild ?? parentKey;
  return (
    <ParentSurface
      parentKey={parentKey}
      activeChild={activeChild}
      onChildChange={props.onChildChange ?? (() => {})}
      renderChild={(vk) => childRenderer(props, vk)}
    />
  );
}
