import React from 'react';
import { Dashboard } from '../surfaces/Dashboard/Dashboard';
import { AgentFlow } from '../surfaces/Flow/AgentFlow';
import { Catalog } from '../surfaces/Catalog/Catalog';
import { MemoryView } from '../surfaces/Memory/MemoryView';
import { ModelsView } from '../surfaces/Models/ModelsView';
import { RunsView } from '../surfaces/Runs/RunsView';
import { TasksView } from '../surfaces/Tasks/TasksView';
import { SettingsView } from '../surfaces/Settings/SettingsView';
import { RepositoryView } from '../surfaces/Repository/RepositoryView';
import { MeshView } from '../surfaces/Mesh/MeshView';
import { GamifyView } from '../surfaces/Gamify/GamifyView';
import { HarnessRedirect } from '../surfaces/Harness/HarnessRedirect';
import { HarnessHealthView } from '../surfaces/HarnessHealth/HarnessHealthView';
import { BrowserView } from '../surfaces/Browser/BrowserView';
import { ApprovalsView } from '../surfaces/Approvals/ApprovalsView';
import { CodeRabbitView } from '../surfaces/CodeRabbit/CodeRabbitView';
import { DiscoverySurface } from '../surfaces/Discovery/DiscoverySurface';
import { SkillsPluginsView } from '../surfaces/SkillsPlugins/SkillsPluginsView';
import { PoliciesView } from '../surfaces/Policies/PoliciesView';
import { NeedsYouSurface } from '../surfaces/NeedsYou/NeedsYouSurface';
import { surfaceDecorators } from '../surfaces/decoratorRegistry';
import { VoxGraphStatusPanel } from '../surfaces/VoxGraph/VoxGraphStatusPanel';
import { Mercatus } from '../surfaces/Mercatus';
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
import type { AttentionInbox } from '../../hooks/useAttentionInbox';

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
  onOpenFeedbackContext?: (id: string) => void;
  focusedFeedbackId?: string | null;
  attention?: AttentionInbox;
  chatPlanSessionId?: string | null;
  chatPlanVersion?: number | null;
  onDiscardPlan?: () => void;
  chatActiveSkillId?: string | null;
  onExcludeSkill?: (skillId: string) => void;
}

export function childRenderer(props: SurfaceProps, viewKey: string): React.ReactNode {
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
          onDoubt={props.onDoubt}
          onOverrule={props.onOverrule}
          onAckLudus={props.onAckLudus!}
          filterKind={props.filterKind!}
          setFilterKind={props.setFilterKind!}
          onOpenInConsole={props.onOpenInConsole}
          onOpenChat={props.onOpenChat}
          onNavigate={props.onNavigate}
          attention_budget={props.attention_budget}
          pushToast={props.pushToast}
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
    case 'memory':
      return <MemoryView pushToast={props.pushToast} onAttachContext={props.onAttachContext} />;
    case 'vox-search':
    // `graphify` retained as a one-release alias falling through to the same panel.
    case 'graphify':
      return <VoxGraphStatusPanel />;
    case 'mercatus':
      return <Mercatus />;
    case 'models':
      return <ModelsView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'runs':
      return <RunsView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'tasks':
      return <TasksView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} attention={props.attention} />;
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
    case 'harness-health':
      return <HarnessHealthView />;
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
    case 'coderabbit':
      return <CodeRabbitView pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'activity':
      return <DiscoverySurface pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
    case 'needs-you':
      return (
        <NeedsYouSurface
          onOpenContext={props.onOpenFeedbackContext!}
          pushToast={props.pushToast}
          attention={props.attention}
        />
      );
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
          tasks={props.chatTasks}
          intents={props.chatIntents}
          executionKpis={props.chatExecutionKpis}
          activeModel={props.chatActiveModel}
          openrouterSpendUsd={props.chatOpenrouterSpendUsd}
          agentStreamItems={props.chatAgentStreamItems}
          onOpenAgentInFlow={props.onOpenAgentInFlow}
          flowAgents={props.data.agents}
          flowSelectedAgentId={props.selectedAgentId}
          onFlowSelectAgent={props.setSelectedAgentId}
          composer={props.chatComposer}
          focusedFeedbackId={props.focusedFeedbackId}
          gamifyEnabled={props.gamifyEnabled}
          attention_budget={props.attention_budget}
          waitingQuestions={props.attention?.needsYou.length}
          blockedTasks={props.attention?.blockedTasksCount}
          planSessionId={props.chatPlanSessionId}
          planVersion={props.chatPlanVersion}
          onDiscardPlan={props.onDiscardPlan}
          activeSkillId={props.chatActiveSkillId}
          onExcludeSkill={props.onExcludeSkill}
        />
      );
    default:
      return null;
  }
}

export function renderSurfaceContent(viewKey: string, props: SurfaceProps): React.ReactNode {
  return childRenderer(props, viewKey);
}

/** @deprecated Prefer {@link renderSurfaceContent} with a leaf view key. */
export function renderSurfaceView(_parentKey: string, props: SurfaceProps): React.ReactNode {
  const viewKey = props.activeChild ?? _parentKey;
  return renderSurfaceContent(viewKey, props);
}
