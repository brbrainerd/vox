import React, { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { invoke } from '@tauri-apps/api/core';
import { type UnlistenFn } from '@tauri-apps/api/event';
import { ChatTranscript } from './ChatTranscript';
import type { StreamItem } from '../../../types/dashboard';
import { EmptyState } from '../../ui/EmptyState';
import { Icon } from '../../ui/Icons';
import {
  ChatExecutionRail,
  type ChatExecutionRailKpis,
  type ChatExecutionTask,
} from './ChatExecutionRail';
import { PlanPanel, type PlanNodeView } from './PlanPanel';
import { listPlanNodes } from '../../../transport';
import { labelForNavKey } from '../../../lib/navigation';
import { WORKBENCH_TABBAR_TRAILING_SLOT_ID } from '../../../lib/domIds';
import type { ChatMessage } from '../../../lib/chatCorrelation';
import type { AttentionBudgetSnapshot } from '../../../types/tauri';
import { AttentionBudgetMeter } from '../AttentionBudgetMeter';
import { SecretaryToast } from './SecretaryToast';
import { listenSecretaryProposed, type SecretaryProposedPayload, feedbackList } from '../../../transport';
import { Matrix } from '../Matrix/Matrix';
import { DockWorkspaceShell, layoutStorageKeyFor } from '../../dock/DockWorkspaceShell';
import type { DockviewApi, IDockviewPanelProps, IDockviewPanelHeaderProps } from 'dockview';
import { AgentFlow } from '../Flow/AgentFlow';
import type { Agent } from '../../../types/dashboard';
import { NeedsYouSurface } from '../NeedsYou/NeedsYouSurface';
import type { AttentionInbox } from '../../../hooks/useAttentionInbox';
import { VoxGraphStatusPanel } from '../VoxGraph/VoxGraphStatusPanel';
import { DiscoverySurface } from '../Discovery/DiscoverySurface';
import { RepositoryView } from '../Repository/RepositoryView';
import { Mercatus } from '../Mercatus/Mercatus';
import { HarnessRedirect } from '../Harness/HarnessRedirect';
import { getPermissionMode } from '../../../transport';



const CORE_PANEL_IDS = ['transcript', 'executionRail', 'flow', 'todos'] as const;
type CorePanelId = (typeof CORE_PANEL_IDS)[number];

// Chat transcript is the primary work surface (and, since Task 9 removed the
// Sessions dockview panel, the dock's leftmost/anchor panel too). It gets a
// hard floor (`minimumWidth`) so opening more panels to its right can never
// squeeze it below a usable width, and no `maximumWidth` so it keeps
// dockview's default fill/grow behavior for whatever room the row leaves it.
//
// dockview-core@6.6.1's real constraint mechanism, read from the shipped
// type defs (not guessed):
//   - `AddPanelOptions` = `... & Partial<Contraints>` — dockview-core
//     dist/esm/dockview/options.d.ts:303, where `Contraints` (dockview's own
//     spelling) is imported from gridview/gridviewPanel.d.ts:9-14:
//     `{ minimumWidth?: number; maximumWidth?: number; minimumHeight?: number;
//     maximumHeight?: number }` — plain pixel numbers, not percentages.
//   - The same fields are settable post-creation via
//     `panel.api.setConstraints({ minimumWidth, maximumWidth })` —
//     dockview-core dist/esm/api/gridviewPanelApi.d.ts:22-23
//     (`setConstraints(value: GridConstraintChangeEvent2): void`, where
//     GridConstraintChangeEvent2 at gridviewPanelApi.d.ts:11-14 allows
//     `FunctionOrValue<number>`, still resolving to a concrete pixel number
//     at layout time — not a live "% of container" binding). DockviewPanelApi
//     inherits this from GridviewPanelApi (dockview/dockviewPanelApi.d.ts:20).
const TRANSCRIPT_MIN_WIDTH_PX = 460;

// Passed straight into `addPanel`'s `Partial<Contraints>` fields (see above)
// for every call site that creates this panel — onReady's initial create and
// addDefaultPanel's Panels-menu/Reset-layout create both funnel through this
// single map so the constraint values live in exactly one place.
const PANEL_SIZE_CONSTRAINTS: Partial<
  Record<string, { initialWidth?: number; maximumWidth?: number; minimumWidth?: number }>
> = {
  transcript: { minimumWidth: TRANSCRIPT_MIN_WIDTH_PX },
};

// Tasks 2.2-2.6/3.1-3.8 append one id each. Opt-in panels get NO auto-create
// branch in the refresh effect — only a guarded `.update()` if already
// present — so they can only ever be (re)created via the Panels menu's Add
// section, never resurrected on an unrelated re-render.
const OPT_IN_PANEL_IDS = ['needs-you', 'voxgraph', 'activity', 'repository', 'mercatus', 'harness', 'approvals'] as const;
type OptInPanelId = (typeof OPT_IN_PANEL_IDS)[number];

function TranscriptPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-transcript" className="flex h-full min-w-0 flex-col gap-4 overflow-y-auto p-2">{props.params.node}</div>;
}

function ExecutionRailPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-execution-rail" className="h-full overflow-y-auto">{props.params.node}</div>;
}

function FlowPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-flow" className="h-full overflow-y-auto p-2">{props.params.node}</div>;
}

function TodosDockPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-todos" className="h-full overflow-y-auto p-2">{props.params.node}</div>;
}

// Needs You's approval/question/withheld lists need real room per the
// original audit (~260-280px); condensed content reuses the same
// needsYou+withheld+approvals total NeedsYouSurface already tallies.
const NEEDS_YOU_FULL_WIDTH_PX = 270;

export function NeedsYouDockPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  const [width, setWidth] = React.useState(props.api.width);
  React.useEffect(() => {
    const disposable = props.api.onDidDimensionsChange(() => setWidth(props.api.width));
    return () => disposable.dispose();
  }, [props.api]);
  const condensed = width < NEEDS_YOU_FULL_WIDTH_PX;
  return (
    <div data-testid="chat-dock-needs-you" className="h-full overflow-y-auto p-2">
      {React.isValidElement(props.params.node)
        ? React.cloneElement(props.params.node as React.ReactElement<any>, { condensed })
        : props.params.node}
    </div>
  );
}

// Search Index's per-corpus cards need real room per the original audit
// (~220-260px); condensed content reuses the same is_fresh flag each card
// already renders as its Fresh/Stale pill, rolled up into "N/M fresh".
const VOXGRAPH_FULL_WIDTH_PX = 240;

export function VoxGraphDockPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  const [width, setWidth] = React.useState(props.api.width);
  React.useEffect(() => {
    const disposable = props.api.onDidDimensionsChange(() => setWidth(props.api.width));
    return () => disposable.dispose();
  }, [props.api]);
  const condensed = width < VOXGRAPH_FULL_WIDTH_PX;
  return (
    <div data-testid="chat-dock-voxgraph" className="h-full overflow-y-auto p-2">
      {React.isValidElement(props.params.node)
        ? React.cloneElement(props.params.node as React.ReactElement<any>, { condensed })
        : props.params.node}
    </div>
  );
}

// Discovery's four nested surfaces (Timeline/Inbox/Review/Archive) each need
// real room per the original audit (~360px); condensed content reuses the
// same active-preset state the tab strip already tracks, naming which
// preset is selected instead of mounting its (wide) nested surface.
const ACTIVITY_FULL_WIDTH_PX = 360;

export function ActivityDockPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  const [width, setWidth] = React.useState(props.api.width);
  React.useEffect(() => {
    const disposable = props.api.onDidDimensionsChange(() => setWidth(props.api.width));
    return () => disposable.dispose();
  }, [props.api]);
  const condensed = width < ACTIVITY_FULL_WIDTH_PX;
  return (
    <div data-testid="chat-dock-activity" className="h-full overflow-y-auto p-2">
      {React.isValidElement(props.params.node)
        ? React.cloneElement(props.params.node as React.ReactElement<any>, { condensed })
        : props.params.node}
    </div>
  );
}

// Repository's action-button grid + command output pane need real room per
// the original audit (~260-300px); condensed content reuses the same
// active-conflict count IsolationPanel's own "Active conflicts" section
// already computes via conflictRows(status).
const REPOSITORY_FULL_WIDTH_PX = 280;

export function RepositoryDockPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  const [width, setWidth] = React.useState(props.api.width);
  React.useEffect(() => {
    const disposable = props.api.onDidDimensionsChange(() => setWidth(props.api.width));
    return () => disposable.dispose();
  }, [props.api]);
  const condensed = width < REPOSITORY_FULL_WIDTH_PX;
  return (
    <div data-testid="chat-dock-repository" className="h-full overflow-y-auto p-2">
      {React.isValidElement(props.params.node)
        ? React.cloneElement(props.params.node as React.ReactElement<any>, { condensed })
        : props.params.node}
    </div>
  );
}

// Mercatus's coverage matrix + source registry tables need real horizontal
// room per the original audit (~320-360px); condensed content reuses the
// "{parts.length} parts · {N} enabled sources" line Mercatus itself already
// computes and shows in its full view.
const MERCATUS_FULL_WIDTH_PX = 340;

export function MercatusDockPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  const [width, setWidth] = React.useState(props.api.width);
  React.useEffect(() => {
    const disposable = props.api.onDidDimensionsChange(() => setWidth(props.api.width));
    return () => disposable.dispose();
  }, [props.api]);
  const condensed = width < MERCATUS_FULL_WIDTH_PX;
  return (
    <div data-testid="chat-dock-mercatus" className="h-full overflow-y-auto p-2">
      {React.isValidElement(props.params.node)
        ? React.cloneElement(props.params.node as React.ReactElement<any>, { condensed })
        : props.params.node}
    </div>
  );
}

function HarnessDockPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-harness" className="h-full overflow-y-auto p-2">{props.params.node}</div>;
}

// Approvals' working table needs ~850-1000px (4 fixed columns sum to 710px
// before the description column gets room). Condensed content:
// pending-approval count + current permission mode.
const APPROVALS_FULL_WIDTH_PX = 850; // from the audit: 4 fixed columns sum to 710px before the description column gets room

export function ApprovalsDockPanel(props: IDockviewPanelProps<{ pendingApprovals: number; permissionMode: string; onNavigate?: (viewKey: string) => void }>) {
  const [width, setWidth] = React.useState(props.api.width);
  React.useEffect(() => {
    const disposable = props.api.onDidDimensionsChange(() => setWidth(props.api.width));
    return () => disposable.dispose();
  }, [props.api]);

  return (
    <div data-testid="chat-dock-approvals" className="h-full overflow-y-auto p-2 text-xs">
      <div className="mb-2">{props.params.pendingApprovals} pending · {props.params.permissionMode}</div>
      {width >= APPROVALS_FULL_WIDTH_PX ? (
        <a
          role="link"
          href="#"
          onClick={e => { e.preventDefault(); props.params.onNavigate?.('approvals'); }}
          className="text-brass hover:underline"
        >
          Open full view
        </a>
      ) : null}
    </div>
  );
}

const CHAT_DOCK_COMPONENTS = {
  transcript: TranscriptPanel,
  executionRail: ExecutionRailPanel,
  flow: FlowPanel,
  todos: TodosDockPanel,
  'needs-you': NeedsYouDockPanel,
  voxgraph: VoxGraphDockPanel,
  activity: ActivityDockPanel,
  repository: RepositoryDockPanel,
  mercatus: MercatusDockPanel,
  harness: HarnessDockPanel,
  approvals: ApprovalsDockPanel,
};

// Marker rendered inside the transcript panel's dockview tab element. Task
// 1.8 pinned the transcript panel by giving it this empty tab component,
// which removes the visible tab/close chrome — but dockview-core 6.6.1 sets
// `draggable = true` unconditionally on every tab's DOM element
// (dockview-core/dist/esm/dockview/components/tab/tab.js), with no per-panel
// API to disable it. The marker below gives the capture-phase `dragstart`
// listener (see the `dockRootRef` effect in ChatSurface) a way to identify
// "this drag started from the transcript panel's tab" purely from the DOM,
// without a blanket `disableDnd` that would break dragging every other panel.
// Renders a real, visible, non-interactive title (matching the default tab's
// text styling) in place of dockview's own close/drag chrome — Task 1.8
// suppressed that chrome entirely (see the comment block above) but left
// nothing standing in for it, so the transcript's tab looked blank/broken.
// The marker span (data-testid) is unchanged and still the thing the
// dragstart-suppression listener below keys off of via `.dv-tab`
// `querySelector`.
function EmptyTab(props: IDockviewPanelHeaderProps) {
  return (
    <span
      data-testid="chat-dock-transcript-tab-marker"
      className="flex h-full items-center px-3 text-xs font-medium text-text-secondary"
    >
      {props.api.title ?? 'Chat'}
    </span>
  );
}
const CHAT_DOCK_TAB_COMPONENTS = { transcript: EmptyTab };

interface ChatSurfaceProps {
  pushToast: (t: any) => void;
  onNavigate?: (viewKey: string) => void;
  messages?: ChatMessage[];
  activeSessionId?: string;
  onSessionChange?: (sessionId: string) => void;
  tasks?: ChatExecutionTask[];
  intents?: string[];
  executionKpis?: ChatExecutionRailKpis;
  activeModel?: string | null;
  openrouterSpendUsd?: number | null;
  agentStreamItems?: StreamItem[];
  onOpenAgentInFlow?: (agentId: string) => void;
  /**
   * Same agent-graph data source that feeds the top-level Flow tab
   * (`props.data.agents` in surfaceComponents.tsx `case 'flow'`) — reused
   * here, not refetched, so the dockable Flow panel stays in sync with the
   * real dashboard state.
   */
  flowAgents?: Agent[];
  flowSelectedAgentId?: string;
  onFlowSelectAgent?: (id: string) => void;
  /** Primary Loquela composer — embedded when global shell dock is hidden on Chat. */
  composer?: React.ReactNode;
  focusedFeedbackId?: string | null;
  gamifyEnabled?: boolean;
  attention_budget?: AttentionBudgetSnapshot | null;
  waitingQuestions?: number;
  blockedTasks?: number;
  /** Live plan-DAG identity for this session, if the current task has synthesized a plan. */
  planSessionId?: string | null;
  planVersion?: number | null;
  onDiscardPlan?: () => void;
  /** Shared attention inbox (App owns polling) — sources the opt-in Needs You dock panel. */
  attention?: AttentionInbox;
  onOpenFeedbackContext?: (id: string) => void;
  /** Pending-approval count — sources the opt-in Approvals dock panel's condensed badge (App.tsx: `attention.approvals.length`). */
  pendingApprovals?: number;
  /** Currently pinned skill id (App.tsx `activeSkill`), threaded into secretary task submission. */
  activeSkillId?: string | null;
  /** "not this one" on a skill-activation chip — see `ChatTranscript`/`ChatTurnEventRow`. */
  onExcludeSkill?: (skillId: string) => void;
}

export function ChatSurface({
  pushToast,
  onNavigate,
  messages = [],
  activeSessionId,
  onSessionChange,
  tasks = [],
  intents,
  executionKpis,
  activeModel,
  openrouterSpendUsd,
  agentStreamItems,
  onOpenAgentInFlow,
  flowAgents = [],
  flowSelectedAgentId,
  onFlowSelectAgent,
  composer,
  focusedFeedbackId,
  gamifyEnabled,
  attention_budget,
  waitingQuestions = 0,
  blockedTasks = 0,
  planSessionId,
  planVersion,
  onDiscardPlan,
  attention,
  onOpenFeedbackContext,
  pendingApprovals = 0,
  activeSkillId,
  onExcludeSkill,
}: ChatSurfaceProps) {
  // Task 9: session switching lives in the global Sidebar
  // (SessionSidebarSection) now, backed by App.tsx's single owning call to
  // useChatSessions. ChatSurface no longer needs session CRUD locally.
  const [secretaryToast, setSecretaryToast] = useState<SecretaryProposedPayload | null>(null);
  const activeId = activeSessionId ?? '';
  const [planNodes, setPlanNodes] = useState<PlanNodeView[]>([]);
  // dockview's `addPanel` captures `params` at call time — it does not track
  // React re-renders. Keep a handle to the ready API so each panel's `node`
  // param can be refreshed whenever the underlying content changes (new
  // sessions loaded, transcript updated, execution rail data changed, etc).
  const dockApiRef = useRef<DockviewApi | null>(null);
  // Tracks panels removed (by the user's tab-close, or by the reset action
  // below) so the refresh effect below can tell "not yet ready to create"
  // apart from "the user closed this — leave it closed until something
  // explicitly asks for it back." Cleared per-id by the reopen/reset actions
  // (Tasks 4/5), not by this effect.
  const closedPanelIds = useRef<Set<string>>(new Set());
  // Mirrors which dock panels are currently open, purely to drive the Panels
  // menu's checkbox `checked` state. dockApiRef is a live ref, not React
  // state — reading `!!dockApiRef.current?.getPanel(id)` directly at render
  // time only stays correct if a render happens to observe it. Two calls to
  // dockApiRef mutators (addPanel/removePanel) back-to-back with no React
  // render in between never get reconciled onto the controlled checkbox's
  // DOM node, and panels closed via a route other than the checkbox (tab-close
  // action, Reset layout) never trigger a render at all, leaving a stale
  // checked=true. This state is the single source of truth the checkboxes
  // read from; it is kept in sync both by the onChange handler (synchronously,
  // for the common case) and by dockview's own onDidAddPanel/onDidRemovePanel
  // events (for external changes).
  const [openPanelIds, setOpenPanelIds] = useState<Set<string>>(() => new Set());
  // Tracks the order opt-in panels were (most-recently) activated in, so a
  // newly-opened opt-in panel positions itself next to the last one the user
  // opened rather than always inserting at the same fixed referenceChain
  // slot. Core panels keep their existing fixed-position behavior — this
  // only affects OPT_IN_PANEL_IDS.
  const activationOrderRef = useRef<string[]>([]);

  const [routingOpen, setRoutingOpen] = useState(false);
  const [panelsMenuOpen, setPanelsMenuOpen] = useState(false);
  const panelsTriggerRef = useRef<HTMLButtonElement | null>(null);
  // The popover content itself (checkboxes, Reset layout) — separate from
  // panelsTriggerRef, which only covers the trigger button. The outside-close
  // listener below must treat clicks inside *either* as "inside": it
  // previously checked only the trigger, so a mousedown on any checkbox (not
  // contained by the trigger) closed the menu before the click's onChange
  // could fire, making every checkbox in the popover unclickable.
  const panelsMenuRef = useRef<HTMLDivElement | null>(null);
  const dockRootRef = useRef<HTMLDivElement | null>(null);

  // The Panels ▾ trigger used to render in its own shrink-0 row above the
  // dock workspace, costing a full row of vertical height for one small
  // control. It was then portaled into WorkbenchTabBar's own trailing slot,
  // but that coupled the trigger's visibility to the tab bar's wrap state:
  // with many top-level tabs open the tablist wraps to multiple lines and
  // the trigger could end up on whichever line wrapped last, effectively
  // "scrolled off"/unreachable. BottomStatusBar (rendered by the app shell
  // above ChatSurface, one level up the tree — not a descendant, and *not* inside
  // the tab bar's own wrapping flex row) now exposes a fixed DOM node
  // (`#workbench-tabbar-trailing-slot`) as a portal target so the button
  // sits inline with persistent app chrome instead — a single-line, never-
  // wrapping row that exists independently of the tab bar (which is slated
  // for eventual removal). Looked up on mount rather than assumed present so
  // ChatSurface still renders correctly standalone (e.g. in tests that don't
  // mount the app shell) — falling back to its own row in that case.
  const [tabBarTrailingSlot, setTabBarTrailingSlot] = useState<HTMLElement | null>(null);
  useEffect(() => {
    setTabBarTrailingSlot(document.getElementById(WORKBENCH_TABBAR_TRAILING_SLOT_ID));
  }, []);

  // Task 1.8 follow-up: dockview-core sets `draggable = true` on every tab
  // element unconditionally, regardless of the (empty) tab component the
  // transcript panel uses to hide its chrome. There is no dockview API to
  // disable dragging for a single panel. A capture-phase `dragstart`
  // listener on the dock's own root container preventDefault()s only when
  // the drag originated from within a `.dv-tab` that contains the
  // transcript panel's marker element (rendered by `EmptyTab` above) — every
  // other panel's tab is untouched.
  //
  // Task 1 (drag-and-drop-to-reorder fix) follow-up: DockWorkspaceShell now
  // sets `dndStrategy="pointer"` on `<DockviewReact>` to work around a
  // WebView2 HTML5-drag-and-drop reliability issue. Under that strategy
  // dockview never sets `draggable = true` and never dispatches a native
  // `dragstart` — every drag (mouse included) is driven by dockview-core's
  // `PointerDragSource` (see `dnd/pointer/pointerDragSource.js`), which:
  //   1. on `pointerdown` (bubble listener on the tab element itself) just
  //      *arms* — this same `pointerdown` is also how dockview activates a
  //      clicked tab (`tabs.js`'s `tab.onPointerDown` calls
  //      `group.model.openPanel(panel)`), so it must NOT be suppressed or
  //      clicking the transcript tab to activate it would break too;
  //   2. only becomes a real drag once a subsequent `pointermove` (a
  //      *bubble* listener PointerDragSource adds dynamically to `window`)
  //      exceeds a small distance threshold.
  // So instead of blocking step 1, this tracks the `pointerId` of a
  // pointerdown that started on the transcript tab, then swallows just that
  // pointer's `pointermove` events via a *capture-phase* listener on
  // `window`. Capture-phase listeners on `window` always run before
  // same-type bubble-phase listeners also registered on `window` (capture
  // is window's first pass over the event; bubble is its last), so
  // `stopImmediatePropagation()` here reliably runs before — and prevents —
  // PointerDragSource's own move handler, starving the drag of the motion
  // it needs to cross its arm threshold. Tab click-to-activate is
  // unaffected since pointerdown itself is never touched.
  useEffect(() => {
    const root = dockRootRef.current;
    if (!root) return;
    const isTranscriptTabEvent = (event: Event) => {
      const target = event.target;
      if (!(target instanceof Element)) return false;
      const tabEl = target.closest('.dv-tab');
      if (!tabEl) return false;
      return Boolean(tabEl.querySelector('[data-testid="chat-dock-transcript-tab-marker"]'));
    };
    const handleDragStart = (event: DragEvent) => {
      if (isTranscriptTabEvent(event)) {
        event.preventDefault();
      }
    };
    let suppressedPointerId: number | null = null;
    const handlePointerDown = (event: PointerEvent) => {
      suppressedPointerId = isTranscriptTabEvent(event) ? event.pointerId : null;
    };
    const clearSuppressed = (event: PointerEvent) => {
      if (event.pointerId === suppressedPointerId) {
        suppressedPointerId = null;
      }
    };
    const handlePointerMove = (event: PointerEvent) => {
      if (suppressedPointerId !== null && event.pointerId === suppressedPointerId) {
        event.stopImmediatePropagation();
        event.preventDefault();
      }
    };
    root.addEventListener('dragstart', handleDragStart, true);
    root.addEventListener('pointerdown', handlePointerDown, true);
    window.addEventListener('pointermove', handlePointerMove, true);
    window.addEventListener('pointerup', clearSuppressed, true);
    window.addEventListener('pointercancel', clearSuppressed, true);
    return () => {
      root.removeEventListener('dragstart', handleDragStart, true);
      root.removeEventListener('pointerdown', handlePointerDown, true);
      window.removeEventListener('pointermove', handlePointerMove, true);
      window.removeEventListener('pointerup', clearSuppressed, true);
      window.removeEventListener('pointercancel', clearSuppressed, true);
    };
  }, []);

  useEffect(() => {
    if (!routingOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setRoutingOpen(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [routingOpen]);

  useEffect(() => {
    if (!panelsMenuOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setPanelsMenuOpen(false);
        panelsTriggerRef.current?.focus();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [panelsMenuOpen]);

  useEffect(() => {
    if (!panelsMenuOpen) return;
    const onPointerDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (panelsTriggerRef.current?.contains(target)) return;
      if (panelsMenuRef.current?.contains(target)) return;
      setPanelsMenuOpen(false);
    };
    document.addEventListener('mousedown', onPointerDown);
    return () => document.removeEventListener('mousedown', onPointerDown);
  }, [panelsMenuOpen]);

  // Task 9: "auto-select the first session" now lives in App.tsx, the sole
  // owner of useChatSessions (see its initial-load effect around
  // chat_list_sessions/{limit:1}) — this component no longer has its own
  // sessions list to auto-select from.

  useEffect(() => {
    if (!planSessionId || planVersion == null) {
      setPlanNodes([]);
      return;
    }
    let cancelled = false;
    listPlanNodes(planSessionId, planVersion)
      .then((rows: PlanNodeView[]) => {
        if (!cancelled) setPlanNodes(rows);
      })
      .catch(() => {
        if (!cancelled) setPlanNodes([]);
      });
    return () => {
      cancelled = true;
    };
  }, [planSessionId, planVersion]);

  useEffect(() => {
    const sub = listenSecretaryProposed((payload) => {
      setSecretaryToast(payload);
    });
    return () => {
      sub.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (!focusedFeedbackId) return;
    
    feedbackList().then((data) => {
      const allFeedback = [...data.needsYou, ...data.withheld];
      const item = allFeedback.find((f) => f.feedbackId === focusedFeedbackId);
      if (!item) return;

      const match = messages.find((msg) => {
        if (msg.taskId) {
          const mTaskId = Number(msg.taskId);
          if (item.gates.includes(mTaskId) || mTaskId === item.doubtedTaskId) {
            return true;
          }
        }
        return msg.text.includes(item.prompt);
      });

      if (match) {
        const el = document.getElementById(`msg-${match.id}`);
        if (el) {
          el.scrollIntoView({ behavior: 'smooth', block: 'center' });
          el.classList.add('ring-2', 'ring-amber-400', 'ring-offset-2', 'ring-offset-zinc-950');
          setTimeout(() => {
            el.classList.remove('ring-2', 'ring-amber-400', 'ring-offset-2', 'ring-offset-zinc-950');
          }, 3000);
        }
      }
    }).catch(() => {});
  }, [focusedFeedbackId, messages]);



  /**
   * Submit a secretary-proposed task (Task 0.2: propose-only). This is the
   * ONLY path by which a secretary classification becomes a live orchestrator
   * task — invoked exclusively from the "Add task" button on `SecretaryToast`.
   */
  const confirmSecretaryTask = async (payload: SecretaryProposedPayload) => {
    setSecretaryToast(null);
    try {
      await invoke('secretary_confirm_task', {
        sessionId: payload.session_id,
        intent: payload.intent,
        activeSkill: activeSkillId ?? null,
      });
      onNavigate?.('tasks');
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Add task failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  };

  const railKpis = executionKpis ?? {
    activeAgents: { value: 0 },
    queueDepth: { value: 0 },
    mesh: { peers: 0 },
  };

  const executionRailNode = onNavigate ? (
    <ChatExecutionRail
      tasks={tasks}
      kpis={railKpis}
      intents={intents}
      activeModel={activeModel}
      openrouterSpendUsd={openrouterSpendUsd}
      onNavigate={onNavigate}
      sessionId={activeSessionId}
      onOpenRouting={() => setRoutingOpen(true)}
    />
  ) : null;

  const flowNode = (
    <AgentFlow
      agents={flowAgents}
      selectedId={flowSelectedAgentId}
      onSelect={onFlowSelectAgent}
    />
  );

  const todosNode = (
    <PlanPanel
      planSessionId={planSessionId}
      planVersion={planVersion}
      nodes={planNodes}
      onDiscard={onDiscardPlan}
    />
  );

  const needsYouNode = (
    <NeedsYouSurface
      onOpenContext={onOpenFeedbackContext ?? (() => {})}
      pushToast={pushToast}
      attention={attention}
    />
  );

  const voxGraphNode = <VoxGraphStatusPanel />;

  const activityNode = <DiscoverySurface pushToast={pushToast} gamifyEnabled={gamifyEnabled} />;

  const repositoryNode = <RepositoryView pushToast={pushToast} gamifyEnabled={gamifyEnabled} />;

  const mercatusNode = <Mercatus />;

  const harnessNode = <HarnessRedirect gamifyEnabled={gamifyEnabled} />;

  const approvalsParams = {
    pendingApprovals,
    permissionMode: getPermissionMode() ?? 'ask',
    onNavigate,
  };

  const centerContent = (
    <>
      {messages.length === 0 && !(agentStreamItems?.length ?? 0) ? (
        <EmptyState
          icon={<Icon.spark className="size-8 text-brass" aria-hidden="true" />}
          title="No messages yet"
          description="Describe a task in the composer below to start this session."
        />
      ) : (
        <ChatTranscript
          messages={messages}
          agentStreamItems={agentStreamItems}
          sessionId={activeId}
          onExcludeSkill={onExcludeSkill}
        />
      )}
      {composer != null ? (
        <div
          className="mt-auto shrink-0 border-t border-border-subtle pt-3"
          data-testid="chat-composer-dock"
        >
          {attention_budget ? (
            <div className="mb-2 px-1" data-testid="chat-attention-meter">
              <AttentionBudgetMeter
                budget={attention_budget}
                waitingQuestions={waitingQuestions}
                blockedTasks={blockedTasks}
                // Realistic-content audit (Task 5, panels-density plan): a
                // 32-message conversation left only ~2 messages visible
                // above this card's ~110-120px fixed footprint. Default to
                // the compact summary row once a session has real content
                // (more than a couple of turns) — but never when something
                // urgent needs the full card up front: near-cap spend,
                // waiting questions, or blocked tasks.
                defaultCollapsed={
                  messages.length > 4 &&
                  !waitingQuestions &&
                  !blockedTasks &&
                  (attention_budget.max_attention_ms > 0
                    ? attention_budget.spent_ms / attention_budget.max_attention_ms
                    : 1) < 0.8
                }
              />
            </div>
          ) : null}
          {/* The model-route picker now renders inside the composer's own
              toolbar row (Loquela's `trailingSlot`, wired in App.tsx),
              right-aligned alongside the cost display, rather than as a
              separate row below it. */}
          {composer}
        </div>
      ) : null}
    </>
  );

  // Supersedes the Task 1.4 CorePanelId-only alias now that opt-in panels exist.
  type ChatDockPanelId = CorePanelId | OptInPanelId;

  // `params` is each panel's dock-component params bag, uniformly. Most
  // panels just wrap a single React node (`{ node: someNode }`), matched by
  // their DockPanel component reading `props.params.node` — but a panel is
  // free to define whatever shape its own DockPanel component expects
  // (Approvals passes structured `{ pendingApprovals, permissionMode,
  // onNavigate }` instead). This is the single source of truth for "what
  // params does panel X get" — addDefaultPanel and the refresh effect below
  // both read it, so there is exactly one definition per panel, not two.
  const panelDefs: Record<ChatDockPanelId, { title: string; params: Record<string, unknown>; referenceChain: ChatDockPanelId[] }> = {
    transcript: { title: 'Chat', params: { node: centerContent }, referenceChain: [] },
    executionRail: { title: 'Execution', params: { node: executionRailNode }, referenceChain: ['transcript'] },
    flow: { title: 'Flow', params: { node: flowNode }, referenceChain: ['executionRail', 'transcript'] },
    todos: { title: 'To-dos', params: { node: todosNode }, referenceChain: ['flow', 'executionRail', 'transcript'] },
    'needs-you': { title: labelForNavKey('needs-you'), params: { node: needsYouNode }, referenceChain: ['todos', 'flow', 'executionRail', 'transcript'] },
    // Titles for surfaces that also exist as top-level app tabs come from
    // labelForNavKey (src/lib/navigation.ts), the app-wide breadcrumb/label
    // SSOT — not hand-typed guesses. voxgraph's real label is "Search
    // Index" (not "VoxGraph") and activity's is "Discovery" (not
    // "Activity"); using the SSOT directly prevents this drifting again.
    voxgraph: { title: labelForNavKey('vox-search'), params: { node: voxGraphNode }, referenceChain: ['todos', 'flow', 'executionRail', 'transcript'] },
    activity: { title: labelForNavKey('activity'), params: { node: activityNode }, referenceChain: ['todos', 'flow', 'executionRail', 'transcript'] },
    repository: { title: labelForNavKey('repository'), params: { node: repositoryNode }, referenceChain: ['todos', 'flow', 'executionRail', 'transcript'] },
    mercatus: { title: labelForNavKey('mercatus'), params: { node: mercatusNode }, referenceChain: ['todos', 'flow', 'executionRail', 'transcript'] },
    harness: { title: labelForNavKey('harness'), params: { node: harnessNode }, referenceChain: ['todos', 'flow', 'executionRail', 'transcript'] },
    // ApprovalsDockPanel takes structured params (pendingApprovals/
    // permissionMode/onNavigate), never a rendered node, so it never mounts
    // a second live <ApprovalsView> poll loop.
    approvals: { title: labelForNavKey('approvals'), params: approvalsParams, referenceChain: ['todos', 'flow', 'executionRail', 'transcript'] },
  };

  // Positions a newly-activated opt-in panel next to whichever opt-in panel
  // was activated immediately before it, instead of always inserting at a
  // fixed referenceChain slot. Falls back to the anchor chain (the same
  // chain core panels use) only when no opt-in panel has been activated yet
  // this session — or when the last-activated one is no longer open.
  // Once this many opt-in panels are already sharing the row, a further one
  // stacks BELOW the most-recently-activated panel instead of splitting the
  // row right again — otherwise every additional opt-in panel keeps halving
  // an already-thin row indefinitely. 2 keeps the existing "positions after
  // the most-recently-activated opt-in panel" behavior intact for the first
  // couple of opt-ins (see the addPanel-position regression test) and only
  // changes behavior once a 3rd (and beyond) opt-in panel opens.
  const OPT_IN_ROW_STACK_THRESHOLD = 2;

  const positionForActivation = (
    api: DockviewApi,
    def: (typeof panelDefs)[ChatDockPanelId],
  ): { direction: 'right' | 'below'; referencePanel: string } | undefined => {
    const last = activationOrderRef.current[activationOrderRef.current.length - 1];
    if (last && api.getPanel(last)) {
      const direction = activationOrderRef.current.length >= OPT_IN_ROW_STACK_THRESHOLD ? 'below' : 'right';
      return { direction, referencePanel: last };
    }
    const anchor = def.referenceChain.find(candidateId => api.getPanel(candidateId));
    return anchor ? { direction: 'right', referencePanel: anchor } : undefined;
  };

  // Plain function, not useCallback: panelDefs is a fresh object every render.
  const addDefaultPanel = (api: DockviewApi, id: ChatDockPanelId) => {
    const def = panelDefs[id];
    const isOptIn = (OPT_IN_PANEL_IDS as readonly string[]).includes(id);
    const position = isOptIn
      ? positionForActivation(api, def)
      : (() => {
          const referencePanel = def.referenceChain.find(candidateId => api.getPanel(candidateId));
          return referencePanel ? ({ direction: 'right', referencePanel } as const) : undefined;
        })();
    api.addPanel({
      id,
      component: id,
      ...(id === 'transcript' ? { tabComponent: 'transcript' } : {}),
      title: def.title,
      params: def.params,
      position,
      ...(PANEL_SIZE_CONSTRAINTS[id] ?? {}),
    });
    closedPanelIds.current.delete(id);
    if (isOptIn) {
      activationOrderRef.current = [...activationOrderRef.current.filter(existing => existing !== id), id];
    }
  };

  // Refresh each panel's `node` param on every render so dockview reflects
  // the latest sessions/transcript/execution-rail content (addPanel only
  // captures params once, at panel-creation time).
  useEffect(() => {
    const api = dockApiRef.current;
    if (!api) return;
    api.getPanel('transcript')?.update({ params: panelDefs.transcript.params });
    const executionPanel = api.getPanel('executionRail');
    if (executionRailNode) {
      if (executionPanel) {
        executionPanel.update({ params: panelDefs.executionRail.params });
      } else if (!closedPanelIds.current.has('executionRail')) {
        api.addPanel({
          id: 'executionRail',
          component: 'executionRail',
          title: 'Execution',
          params: panelDefs.executionRail.params,
          position: { direction: 'right', referencePanel: 'transcript' },
        });
      }
    } else if (executionPanel) {
      api.removePanel(executionPanel);
    }
    const flowPanel = api.getPanel('flow');
    if (flowPanel) {
      flowPanel.update({ params: panelDefs.flow.params });
    } else if (!closedPanelIds.current.has('flow')) {
      api.addPanel({
        id: 'flow',
        component: 'flow',
        title: 'Flow',
        params: panelDefs.flow.params,
        // Note: the plan's original sketch tabbed Flow `within` the
        // execution rail group. dockview-react only mounts the active tab's
        // panel body, so that hid `chat-dock-execution-rail` from the DOM
        // whenever Flow's tab was active — a regression against the B2 test
        // asserting the execution rail is present without any tab
        // interaction. Placing Flow as its own group (to the right of
        // whichever panel is currently last) keeps all panels simultaneously
        // present, matching how sessions/transcript/executionRail already
        // coexist.
        position: {
          direction: 'right',
          referencePanel: api.getPanel('executionRail') ? 'executionRail' : 'transcript',
        },
      });
    }
    const todosPanel = api.getPanel('todos');
    if (todosPanel) {
      todosPanel.update({ params: panelDefs.todos.params });
    } else if (!closedPanelIds.current.has('todos')) {
      api.addPanel({
        id: 'todos',
        component: 'todos',
        title: 'To-dos',
        params: panelDefs.todos.params,
        position: {
          direction: 'right',
          referencePanel: api.getPanel('flow') ? 'flow' : api.getPanel('executionRail') ? 'executionRail' : 'transcript',
        },
      });
    }
    // Opt-in panels: update-only, no create branch. They can only be
    // (re)created via the Panels menu's Add section (Step 4 below) — this is
    // what makes the "resurrects after close" bug structurally impossible.
    api.getPanel('needs-you')?.update({ params: panelDefs['needs-you'].params });
    api.getPanel('voxgraph')?.update({ params: panelDefs.voxgraph.params });
    api.getPanel('activity')?.update({ params: panelDefs.activity.params });
    api.getPanel('repository')?.update({ params: panelDefs.repository.params });
    api.getPanel('mercatus')?.update({ params: panelDefs.mercatus.params });
    api.getPanel('harness')?.update({ params: panelDefs.harness.params });
    api.getPanel('approvals')?.update({ params: panelDefs.approvals.params });
  });

  const panelsMenu = (
    <div className="relative">
            <button
              ref={panelsTriggerRef}
              type="button"
              aria-label="Panels"
              aria-expanded={panelsMenuOpen}
              onClick={() => setPanelsMenuOpen(o => !o)}
              className="rounded-lg border border-border-subtle bg-overlay-subtle p-2 text-text-muted transition hover:border-brass/40 hover:text-brass"
            >
              <span className="font-mono text-xs">Panels ▾</span>
            </button>
            {panelsMenuOpen ? (
              <div
                ref={panelsMenuRef}
                className="absolute right-0 top-full z-50 mt-1 w-56 rounded-lg border border-border-subtle bg-bg-base p-1 shadow-2xl"
              >
                {[
                  ...CORE_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null),
                  ...OPT_IN_PANEL_IDS,
                ].map(id => {
                  const isOpen = openPanelIds.has(id);
                  return (
                    <label
                      key={id}
                      className="flex items-center gap-2 rounded-sm px-2 py-1.5 text-xs text-text-muted hover:bg-overlay-hover hover:text-text-primary"
                    >
                      <input
                        type="checkbox"
                        className="rounded-sm border-border-subtle bg-bg-base text-brass focus:ring-brass/40 focus:ring-offset-bg-base size-3.5"
                        checked={isOpen}
                        onChange={() => {
                          const api = dockApiRef.current;
                          if (!api) return;
                          const panel = api.getPanel(id);
                          if (panel) {
                            api.removePanel(panel);
                            closedPanelIds.current.add(id); // core panels still need this guard
                          } else {
                            addDefaultPanel(api, id);
                          }
                          // Synchronous, not reliant on dockview's own event
                          // round-trip — so a second checkbox toggled
                          // immediately after (no await in between) still
                          // reads a correct, freshly-committed openPanelIds.
                          setOpenPanelIds(prev => {
                            const next = new Set(prev);
                            if (panel) {
                              next.delete(id);
                            } else {
                              next.add(id);
                            }
                            return next;
                          });
                        }}
                      />
                      {panelDefs[id].title}
                    </label>
                  );
                })}
                <div className="my-1 border-t border-border-subtle" />
                <button
                  type="button"
                  onClick={() => {
                    window.localStorage.removeItem(layoutStorageKeyFor('gui.chat'));
                    const api = dockApiRef.current;
                    if (api) {
                      api.panels.forEach(p => api.removePanel(p)); // .panels is a fresh snapshot per access — safe to iterate while removePanel mutates the underlying model
                      closedPanelIds.current.clear();
                      // Only CORE_PANEL_IDS — opt-in panels (Phase 2/3) are intentionally
                      // NOT recreated by Reset, even if they were open before the reset.
                      CORE_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null).forEach(id =>
                        addDefaultPanel(api, id),
                      );
                      // addDefaultPanel doesn't update openPanelIds itself (it's
                      // also called from onReady, before openPanelIds even
                      // exists as a concept) — resync explicitly from the
                      // dockview API's actual panel set post-reset.
                      setOpenPanelIds(new Set(api.panels.map(p => p.id)));
                    }
                    setPanelsMenuOpen(false);
                    panelsTriggerRef.current?.focus();
                  }}
                  className="block w-full rounded-sm px-2 py-1.5 text-left text-xs text-text-muted hover:bg-overlay-hover hover:text-text-primary"
                >
                  Reset layout
                </button>
              </div>
            ) : null}
    </div>
  );

  return (
    <div
      className="relative flex h-full gap-4"
      data-testid="chat-surface-layout"
    >
      {/* Axe page-has-heading-one: surfaces render inside a heading-less shell.
          NOTE: if chatDocked (App.tsx, currently hardcoded false) is ever
          enabled, a docked ChatSurface adds a second h1 to the page. */}
      <h1 className="sr-only">Chat</h1>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        {tabBarTrailingSlot
          ? createPortal(panelsMenu, tabBarTrailingSlot)
          : <div className="relative mb-2 flex shrink-0 justify-end">{panelsMenu}</div>}
        <div ref={dockRootRef} className="min-h-0 flex-1">
        <DockWorkspaceShell
          storageKeyPrefix="gui.chat"
          components={CHAT_DOCK_COMPONENTS}
          tabComponents={CHAT_DOCK_TAB_COMPONENTS}
          onReady={(event) => {
            dockApiRef.current = event.api;
            // Persisted-layout fallback (Task 9). Any user who used the Chat
            // surface before the Sessions dockview panel was removed has a
            // persisted `gui.chat` layout that still references a `sessions`
            // panel whose `contentComponent` no longer exists in
            // CHAT_DOCK_COMPONENTS. Verified empirically (not assumed) against
            // dockview-core@6.6.1: DockWorkspaceShell's `event.api.fromJSON`
            // call, which runs BEFORE this onReady handler, throws
            // ("Dockview: Only React.memo(...), React.ForwardRef(...) and
            // functional components are accepted as components") the instant
            // it tries to construct a panel for an unregistered component id
            // — it does NOT silently skip just that one panel. That throw is
            // already caught by DockWorkspaceShell's own try/catch, which logs
            // a warning and leaves the API on its fresh default (empty) grid,
            // so `event.api.getPanel('sessions')` is always undefined by the
            // time this handler runs — dockview's own error-handling already
            // guarantees the stale panel can never surface. The explicit check
            // below is kept as defense-in-depth (a future dockview version
            // could change fromJSON to skip only the bad node instead of
            // reverting the whole restore) and is exercised by the regression
            // test "closes a stale 'sessions' panel restored from a persisted
            // layout" in ChatSurface.test.tsx, which asserts on the
            // user-visible outcome (no chat-dock-sessions node, transcript
            // still mounts) rather than on this line executing.
            const staleSessionsPanel = event.api.getPanel('sessions');
            if (staleSessionsPanel) {
              staleSessionsPanel.api.close();
            }
            event.api.onDidRemovePanel(panel => {
              closedPanelIds.current.add(panel.id);
              // Keeps the Panels menu checkbox in sync when a panel closes
              // through a route other than its own checkbox — dragging a tab
              // closed, the tab's native close action, or Reset layout's bulk
              // removePanel loop above. None of those call setOpenPanelIds
              // directly, so without this listener the checkbox would stay
              // visibly checked after the panel is actually gone.
              setOpenPanelIds(prev => {
                if (!prev.has(panel.id)) return prev;
                const next = new Set(prev);
                next.delete(panel.id);
                return next;
              });
            });
            event.api.onDidAddPanel(panel => {
              // Symmetric case: a panel added by something other than the
              // checkbox's own onChange (e.g. a restored layout from
              // localStorage on mount, or the sessions/transcript/etc.
              // auto-create branches below).
              setOpenPanelIds(prev => {
                if (prev.has(panel.id)) return prev;
                const next = new Set(prev);
                next.add(panel.id);
                return next;
              });
            });
            setOpenPanelIds(new Set(event.api.panels.map(p => p.id)));
            // Guarded against duplicate-add: a restored dockview layout
            // (ChatDockShell's localStorage persistence) already recreates
            // these panels, so onReady must not re-add them.
            if (!event.api.getPanel('transcript')) {
              event.api.addPanel({
                id: 'transcript',
                component: 'transcript',
                tabComponent: 'transcript',
                title: 'Chat',
                params: panelDefs.transcript.params,
                // No `position` — Task 9 removed the Sessions panel that
                // transcript used to anchor off of, so transcript is now
                // added with no positional anchor, matching how Sessions
                // itself used to be added (dockview places a panel with no
                // `position` as the layout's default/leftmost panel).
                ...PANEL_SIZE_CONSTRAINTS.transcript,
              });
            }
            if (executionRailNode && !event.api.getPanel('executionRail')) {
              event.api.addPanel({
                id: 'executionRail',
                component: 'executionRail',
                title: 'Execution',
                params: panelDefs.executionRail.params,
                position: { direction: 'right', referencePanel: 'transcript' },
              });
            }
          }}
        />
        </div>
      </div>

      {secretaryToast && (
        <div className="absolute bottom-4 left-1/2 z-60 w-[min(480px,90%)] -translate-x-1/2">
          <SecretaryToast
            intent={secretaryToast.intent}
            itemId={secretaryToast.item_id}
            onDismiss={() => setSecretaryToast(null)}
            onConfirm={() => confirmSecretaryTask(secretaryToast)}
          />
        </div>
      )}

      {routingOpen && (
        <div className="fixed inset-0 z-60" role="dialog" aria-modal="true" aria-label="Routing">
          <div className="absolute inset-0 bg-black/60" onClick={() => setRoutingOpen(false)} />
          <div className="absolute right-0 top-0 h-full w-[760px] max-w-full overflow-y-auto border-l border-border-subtle bg-bg-base shadow-2xl">
            <div className="flex items-center justify-between px-5 pt-4">
              <h2 className="font-display text-[13px] uppercase tracking-[0.2em] text-text-secondary">Routing</h2>
              <button
                type="button"
                aria-label="Close routing panel"
                onClick={() => setRoutingOpen(false)}
                className="rounded-md border border-border-subtle px-2 py-1 font-mono text-xs text-text-muted hover:bg-overlay-hover hover:text-text-primary"
              >
                ✕
              </button>
            </div>
            <Matrix pushToast={pushToast} gamifyEnabled={gamifyEnabled} />
          </div>
        </div>
      )}
    </div>
  );
}
