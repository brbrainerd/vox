import React from 'react';
import { Dashboard } from '../surfaces/Dashboard/Dashboard';
import { AgentFlow } from '../surfaces/Flow/AgentFlow';
import { Catalog } from '../surfaces/Catalog/Catalog';
import { Matrix } from '../surfaces/Matrix/Matrix';
import { MemoryView } from '../surfaces/Memory/MemoryView';
import { ModelsView } from '../surfaces/Models/ModelsView';
import { RunsView } from '../surfaces/Runs/RunsView';
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
import type { DashboardData, Agent, LudusAlert, StreamItem } from '../../types/dashboard';
import type { CatalogEntry, Toast } from '../../types/tauri';
import type { ChatMessage } from '../../lib/chatCorrelation';

export interface SurfaceProps {
  pushToast: (t: Toast) => void;
  data: DashboardData;
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
  activeChild?: string;
  onChildChange?: (viewKey: string) => void;
  activeSessionId?: string;
  onSessionChange?: (sessionId: string) => void;
  chatMessages?: ChatMessage[];
  onHydrateChatSession?: (sessionId: string) => void;
  onFocusComposer?: () => void;
}

function childRenderer(props: SurfaceProps, viewKey: string): React.ReactNode {
  const Decorator = surfaceDecorators[viewKey];
  if (Decorator) return <Decorator pushToast={props.pushToast} />;
  switch (viewKey) {
    case 'dashboard':
      return (
        <Dashboard
          data={props.data}
          onPause={props.onPause!}
          onResume={props.onResume!}
          onDoubt={props.onDoubt!}
          onOverrule={props.onOverrule!}
          onAckLudus={props.onAckLudus!}
          filterKind={props.filterKind!}
          setFilterKind={props.setFilterKind!}
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
      return <Matrix pushToast={props.pushToast} />;
    case 'memory':
      return <MemoryView pushToast={props.pushToast} onAttachContext={props.onAttachContext} />;
    case 'models':
      return <ModelsView pushToast={props.pushToast} />;
    case 'runs':
      return <RunsView pushToast={props.pushToast} />;
    case 'settings':
      return <SettingsView pushToast={props.pushToast} />;
    case 'repository':
      return <RepositoryView pushToast={props.pushToast} />;
    case 'mesh':
      return <MeshView pushToast={props.pushToast} />;
    case 'gamify':
      return <GamifyView pushToast={props.pushToast} />;
    case 'harness':
      return <HarnessRedirect onFocusComposer={props.onFocusComposer} />;
    case 'browser':
      return <BrowserView pushToast={props.pushToast} />;
    case 'approvals':
      return <ApprovalsView pushToast={props.pushToast} />;
    case 'policies':
      return <PoliciesView pushToast={props.pushToast} />;
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
