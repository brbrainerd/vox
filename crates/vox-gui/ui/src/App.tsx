import React, { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react';
import {
  sanitizeErrorForToast,
  isBudgetExceededError,
  isRateLimitedError,
  isContextExceededError,
  stripRateLimitedPrefix,
} from './lib/backendGuard';
import { invoke } from '@tauri-apps/api/core';
import { AppShell } from './components/layout/AppShell';
import { SidebarMode } from './components/layout/Sidebar';
import { renderSurfaceContent } from './components/layout/surfaceComponents';
import { resolveNavigation, parseViewFromLocation, syncViewToLocation, seedDiscoveryPresetForLegacyKey, labelForNavKey, DEFAULT_CHILD_BY_PARENT } from './lib/navigation';
import { useActiveView } from './hooks/useActiveView';
import { useDocViewer } from './hooks/useDocViewer';
import { DocViewerDrawer } from './components/layout/DocViewerDrawer';
import { Omnibar } from './components/layout/Omnibar';
import { redirectSearchViewToOmnibar } from './components/layout/omnibarRedirect';
import { Loquela } from './components/surfaces/Loquela/Loquela';
import { ChatModelPicker } from './components/surfaces/Chat/ChatModelPicker';
import { GroundingCheckToggle } from './components/surfaces/Chat/GroundingCheckToggle';
import { useGroundingCheck } from './hooks/useGroundingCheck';
import { Toasts, ToastItem } from './components/ui/Toasts';
import { coalesceToast } from './lib/toastQueue';
import { scrollAndFocusAnchor } from './lib/anchorFocus';
import { BackendBanner } from './components/ui/BackendBanner';
import { VersionMismatchBanner } from './components/layout/VersionMismatchBanner';
import { OnboardingWizard } from './components/surfaces/Onboarding/OnboardingWizard';
import { userAppendInput } from './lib/composerSubmit';
import { Transcript } from './components/surfaces/Loquela/Transcript';
import { DiffReview } from './components/surfaces/Loquela/DiffReview';
import { InlineApprovals } from './components/surfaces/Loquela/InlineApprovals';
import { type McpInvokeResult } from './lib/mcpToolResult';
import {
  assistantMessagesReadyToPersist,
  assistantPersistContent,
  chatReducer,
  PENDING_TIMEOUT_MS,
} from './lib/chatCorrelation';
import {
  getSessionMessages,
  initialSessionChatStore,
  resolveSessionForEvent,
  sessionChatReducer,
  type SessionChatAction,
  type SessionChatStore,
} from './lib/sessionChatStore';
import { mapAgentEvent } from './lib/mapAgentEvent';
import { sendChatTurn } from './lib/chatSend';
import type { ChatTurnDto } from './lib/chatSend';
import { buildChatTurn } from './lib/buildChatTurn';
import { contextRefsFromPayload } from './lib/loquelaContext';
import { overallWorst, worstCount } from './components/surfaces/Policies/policyTree';
import type { PolicyRow, PolicyStatus, BranchInfo, RunStatus } from './components/surfaces/Policies/types';
import { voxTransport, listenAgentEvents, chatTurn as sendChatTurnRaw, type AgentEventFrame } from './transport';
import { useAttentionInbox } from './hooks/useAttentionInbox';
import { useKeybinds } from './hooks/useKeybinds';
import { parseBindings, DEFAULT_BINDINGS, type Bindings } from './lib/keybinds';
import { type UnlistenFn } from '@tauri-apps/api/event';
import { useLocalStorage } from './hooks/useLocalStorage';
import { SHELL_PREFERENCE_KEYS } from './lib/shellPersistence';
import { usePersistedSparkWindow } from './hooks/useSparkWindow';
import { useOrchestratorStatus, meshKpiFromStatus, useOrchestratorFirstConnectGamify } from './hooks/useOrchestratorStatus';
import { useInstalledSkills } from './hooks/useInstalledSkills';
import { useLlmSpend } from './hooks/useLlmSpend';
import { useChatExecutionData } from './hooks/useChatExecutionData';
import { useHudTilesConfig } from './hooks/useHudTilesConfig';
import { useMeshNodes } from './hooks/useMeshNodes';
import { useGamifySettings } from './hooks/useGamifySettings';
import { useAchievementToasts } from './hooks/useAchievementToasts';
import { recordGamifyGuiEvent, setGamifyGuiEventResultListener } from './lib/gamifyGuiEvents';
import { AchievementToast } from './components/gamify/AchievementToast';
import { installedSkillToCatalogEntry } from './lib/installedSkills';
import { handleSubmitTaskAction } from './lib/commandPaletteActions';
import { DashboardData, Agent, StreamItem, LudusAlert } from './types/dashboard';
import type {
  ActiveSkill,
  ChatPayload,
  CommandCatalog,
  CommandPaletteAction,
  ContextChip,
  OrchestratorStatus,
  RawAgentSummary,
  RawLudusAlert,
  RawStreamEvent,
  Session,
  Toast,
} from './types/tauri';
import { INITIAL_DATA, INITIAL_KPIS } from './data/initialState';
import {
  POLICY_BADGE_POLL_MS,
  STREAM_CAP,
  LIVE_EVENT_FRESH_MS,
} from './config/constants';
import { budgetStateFromStatus, DEFAULT_BUDGET_CAP_USD } from './config/budget';
import { nextId, nextGuiRunId, newBackgroundSessionId } from './lib/ids';
import { slashCommandBase } from './lib/slashRouter';
import { viewKeyForLocator } from './lib/locatorNavigation';
import { AchievementsDrawer } from './components/gamify/AchievementsDrawer';
import type { LudusProfile } from './lib/ludus';
import { useChatSessions, type ChatSession } from './lib/useChatSessions';

type View =
  | 'dashboard'
  | 'flow'
  | 'catalog'
  | 'matrix'
  | 'memory'
  | 'models'
  | 'runs'
  | 'repository'
  | 'mesh'
  | 'gamify'
  | 'harness'
  | 'harness-health'
  | 'browser'
  | 'console'
  | 'coderabbit'
  | 'scientia'
  | 'discovery-review'
  | 'discovery-inbox'
  | 'archive-panel'
  | 'claims'
  | 'mens'
  | 'populi'
  | 'research'
  | 'oratio'
  | 'approvals'
  | 'needs-you'
  | 'activity'
  | 'policies'
  | 'skills'
  | 'settings'
  | 'coverage'
  | 'publications'
  | 'search'
  | 'vox-search'
  | 'chat'
  | 'agents'
  | 'workspace'
  | 'commands'
  | 'knowledge'
  | 'compute'
  | 'mercatus'
  | 'sub-agents';

const LEGACY_VIEWS: string[] = [
  'dashboard', 'flow', 'catalog', 'matrix', 'memory', 'models', 'runs', 'repository',
  'mesh', 'gamify', 'harness', 'harness-health', 'browser', 'console', 'coderabbit', 'scientia', 'discovery-review', 'discovery-inbox', 'archive-panel', 'claims', 'mens',
  'populi', 'research', 'oratio', 'approvals', 'policies', 'skills', 'settings', 'coverage',
  'publications', 'search', 'vox-search', 'chat', 'agents', 'workspace', 'commands', 'knowledge', 'compute', 'mercatus',
  'review', 'tasks', 'sub-agents',
];

// Single source of truth for valid view ids (deep-link validation + initial-view).
const KNOWN_VIEWS: string[] = LEGACY_VIEWS;

function isKnownView(v: unknown): v is View {
  return typeof v === 'string' && KNOWN_VIEWS.includes(v);
}

/**
 * Task C1: `chat_turn`'s typed `ChatTurnError` (see `dispatchErrorToast`
 * below) carries the human-readable text in its `message` field, not in
 * `String(err)` (which would stringify the whole `{kind, message}` object to
 * `"[object Object]"`). Used both by `dispatchErrorToast` and by the
 * chat-store `error:` field so the failed transcript bubble shows the same
 * text as the toast.
 */
function chatTurnErrorMessage(err: unknown): string {
  if (typeof err === 'object' && err !== null && 'kind' in err) {
    const message = (err as { message?: unknown }).message;
    if (typeof message === 'string') return message;
  }
  return sanitizeErrorForToast(err);
}

/**
 * Both chat lifecycles (the synchronous reply and the background task — one
 * `chat_turn` command, two store lifecycles; see the two catch blocks in
 * `handleLoquelaSubmit` below) funnel through
 * `vox-orchestrator-mcp`'s budget guard. A budget-exceeded failure is
 * actionable in a way a generic backend error isn't (the user can raise
 * their cap in Settings), so it gets a distinct title/body instead of the
 * generic dispatch-failure toast — same `pushToast` shape, just a
 * budget-aware title/body when `isBudgetExceededError` matches.
 *
 * Task 12b: the same two dispatch mechanisms also funnel through the live LLM
 * egress paths (`vox-actor-runtime`'s `llm_chat` and `vox-orchestrator-mcp`'s
 * `mcp_infer_tool_completion`), both of which now prepend `RATE_LIMITED_PREFIX`
 * to a rate-limited (e.g. OpenRouter free-tier 50/day cap) terminal failure.
 * That's actionable too (add your own API key, or wait for the cap to reset),
 * so it gets its own distinct toast — checked alongside (not instead of) the
 * budget check, since the two conditions are mutually exclusive.
 *
 * Task C1: `chat_turn` (crates/vox-gui/src/commands/chat_turn.rs) now returns
 * `Result<ChatTurnDto, ChatTurnError>` — a `#[serde(tag = "kind", ...)]`
 * enum — so Tauri v2 rejects the invoke() promise with the deserialized
 * `{kind, message}` object itself, not a display string. `err` is matched on
 * `kind` first; the string-pattern checks below are a fallback for the
 * (still-`Result<_, String>`) other commands and for non-chat_turn error
 * shapes. `sanitizeErrorForToast` is only invoked on the fully-unrecognized
 * fallthrough — every known kind above is an already-safe, app-authored
 * string (never raw IPC internals), so there's nothing to strip.
 */
function dispatchErrorToast(err: unknown, fallbackTitle: string): Toast {
  if (typeof err === 'object' && err !== null && 'kind' in err) {
    const { kind, message } = err as { kind: string; message?: string };
    const text = typeof message === 'string' ? message : String(err);
    switch (kind) {
      case 'budget_exceeded':
        return {
          tone: 'warn',
          title: 'Budget limit reached',
          body: `${text} Adjust your daily/session budget caps in Settings.`,
          cause: 'backend-error',
        };
      case 'rate_limited':
        return {
          tone: 'warn',
          title: 'Free tier limit reached',
          body: `${stripRateLimitedPrefix(text)} Add your own API key or wait for the limit to reset.`,
          cause: 'backend-error',
        };
      case 'context_exceeded':
        return {
          tone: 'warn',
          title: 'Message too long',
          body: `${text} Trim your context or start a new session.`,
          cause: 'backend-error',
        };
      default:
        return { tone: 'warn', title: fallbackTitle, body: sanitizeErrorForToast(text), cause: 'backend-error' };
    }
  }
  const errorText = String(err);
  if (isBudgetExceededError(errorText)) {
    return {
      tone: 'warn',
      title: 'Budget limit reached',
      body: `${errorText} Adjust your daily/session budget caps in Settings.`,
      cause: 'backend-error',
    };
  }
  if (isRateLimitedError(errorText)) {
    return {
      tone: 'warn',
      title: 'Free tier limit reached',
      body: `${stripRateLimitedPrefix(errorText)} Add your own API key or wait for the limit to reset.`,
      cause: 'backend-error',
    };
  }
  if (isContextExceededError(errorText)) {
    return {
      tone: 'warn',
      title: 'Message too long',
      body: `${errorText} Trim your context or start a new session.`,
      cause: 'backend-error',
    };
  }
  return { tone: 'warn', title: fallbackTitle, body: sanitizeErrorForToast(err), cause: 'backend-error' };
}

// ─── Agent mapper — shared between EventBus and polling fallback ─────────────
function mapAgent(a: RawAgentSummary): Agent {
  const inProgress = a.in_progress ?? false;
  const rawProgress = a.progress;
  const progress =
    typeof rawProgress === 'number' && Number.isFinite(rawProgress)
      ? rawProgress
      : inProgress
        ? null
        : 0;
  const rawBudget = a.budget;
  const budget =
    typeof rawBudget === 'number' && Number.isFinite(rawBudget) ? rawBudget : null;
  return {
    id: `A-${String(a.id).padStart(2, '0')}`,
    codename: typeof a.codename === 'object' && a.codename !== null ? JSON.stringify(a.codename) : String(a.codename ?? 'Agent'),
    phase: a.paused ? 'Paused' : (inProgress ? String(a.current_phase ?? 'Executing') : 'Idle'),
    progress,
    task: typeof a.task_description === 'object' && a.task_description !== null ? JSON.stringify(a.task_description) : String(a.task_description ?? (inProgress ? 'Processing…' : 'Idle')),
    cost: typeof a.cost === 'number' ? a.cost : 0,
    budget,
    eta: typeof a.eta === 'object' && a.eta !== null ? JSON.stringify(a.eta) : String(a.eta ?? '—'),
    skill: a.active_skill ? String(a.active_skill) : undefined,
  };
}

function mapStream(e: RawStreamEvent): StreamItem {
  const tagStr = typeof e.tag === 'object' && e.tag !== null ? JSON.stringify(e.tag) : String(e.tag ?? 'SYSTEM');
  const titleStr = typeof e.title === 'object' && e.title !== null ? JSON.stringify(e.title) : String(e.title ?? 'Event');
  const bodyStr = typeof e.body === 'object' && e.body !== null ? JSON.stringify(e.body) : String(e.body ?? '');
  return {
    id: e.id != null ? String(e.id) : nextId('stream'),
    kind: e.kind ?? 'system',
    tag: tagStr,
    title: titleStr,
    body: bodyStr,
    ts: e.timestamp != null ? String(e.timestamp) : 'now',
  };
}

function mapAlert(a: RawLudusAlert): LudusAlert {
  const titleStr = typeof a.title === 'object' && a.title !== null ? JSON.stringify(a.title) : String(a.title ?? '');
  const bodyStr = typeof a.body === 'object' && a.body !== null ? JSON.stringify(a.body) : String(a.body ?? '');
  return {
    id: String(a.id),
    level: a.level,
    title: titleStr,
    body: bodyStr,
  };
}

// ── Chat pending-bubble watchdog ─────────────────────────────────────────────
// `pendingTimeout` is a store-level sweep (every session) layered on top of
// sessionChatReducer; the per-bubble expiry logic lives in chatCorrelation's
// chatReducer so it stays pure and unit-testable.
type AppChatAction = SessionChatAction | { type: 'pendingTimeout'; nowMs: number };

/** How often the watchdog sweeps for stale pending bubbles (a fraction of
 *  PENDING_TIMEOUT_MS so expiry lands close to the 90s mark). */
const PENDING_WATCHDOG_SWEEP_MS = Math.max(1_000, Math.floor(PENDING_TIMEOUT_MS / 18));

function appChatReducer(store: SessionChatStore, action: AppChatAction): SessionChatStore {
  if (action.type === 'pendingTimeout') {
    let changed = false;
    const sessions: SessionChatStore['sessions'] = {};
    for (const [sid, state] of Object.entries(store.sessions)) {
      const next = chatReducer(state, action);
      if (next !== state) changed = true;
      sessions[sid] = next;
    }
    return changed ? { ...store, sessions } : store;
  }
  return sessionChatReducer(store, action);
}

export default function App() {
  const [data, setData] = useState<DashboardData>(INITIAL_DATA);
  const [kpis, setKpis] = useState(INITIAL_KPIS);
  const { activeView, navigateTo: openTab } = useActiveView();
  const { activeDoc, openDoc: openDocTab, closeDoc: closeDocViewer } = useDocViewer();
  const [sidebarMode, setSidebarMode] = useLocalStorage<SidebarMode>(
    SHELL_PREFERENCE_KEYS.sidebarMode,
    'default',
  );
  const [isCommandOpen, setIsCommandOpen] = useState(false);
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const [filterKind, setFilterKind] = useState('all');
  const [chips, setChips] = useState<ContextChip[]>([]);
  const [activeSkill, setActiveSkill] = useState<ActiveSkill | null>(null);
  // Skills the user rejected via "not this one" (ChatTurnEventRow). Session-scoped
  // in the same (global-not-per-session) way `activeSkill` already is — see
  // `excludeSkillAndRetry` below for the append-and-redispatch wiring.
  const [skillExclusions, setSkillExclusions] = useState<string[]>([]);
  const skillExclusionsRef = useRef<string[]>([]);
  useEffect(() => {
    skillExclusionsRef.current = skillExclusions;
  }, [skillExclusions]);
  // Last submitted chat payload, so `excludeSkillAndRetry` can re-dispatch the
  // same turn immediately after excluding a skill.
  const lastChatPayloadRef = useRef<ChatPayload | null>(null);
  const [deployedSet, setDeployedSet] = useState(new Set<string>());
  const [selectedAgentId, setSelectedAgentId] = useState('ROOT');
  const [appVersion, setAppVersion] = useState<string>('loading…');
  // Master-sidebar Policies badge: worst-status count for the current branch.
  const [policyBadge, setPolicyBadge] = useState<{ count: number; status: RunStatus } | null>(null);
  const [lastOrchEventAt, setLastOrchEventAt] = useState<number | null>(null);
  const [bindings, setBindings] = useState<Bindings>(DEFAULT_BINDINGS);
  useEffect(() => {
    voxTransport.getGuiPreference('gui.keybinds')
      .then(json => setBindings(parseBindings(json)))
      .catch(() => setBindings(DEFAULT_BINDINGS));
  }, []);
  const orchQuery = useOrchestratorStatus();
  const orchUsesPolling = orchQuery.usesPolling;
  const { totalUsd: openrouterSpendUsd } = useLlmSpend();
  const { config: hudTilesConfig, setConfig: setHudTilesConfig } = useHudTilesConfig();
  // Slower than MeshView's own 5s poll — BottomStatusBar is mounted for the
  // whole session on every view, so a one-line online/total summary doesn't
  // need MeshView's fast cadence, which would add permanent steady-state
  // load on the orchestrator daemon.
  const meshNodes = useMeshNodes(20_000);
  const activeModel = useMemo(() => {
    const status = orchQuery.data as (OrchestratorStatus & { active_model?: string | null }) | undefined;
    return status?.active_model ?? null;
  }, [orchQuery.data]);
  const installedSkills = useInstalledSkills(true);
  const installedSkillEntries = useMemo(
    () => installedSkills.map(installedSkillToCatalogEntry),
    [installedSkills],
  );
  const [activeSessionId, setActiveSessionId] = useState<string>('');
  const [openPlanSessionId, setOpenPlanSessionId] = useState<string | null>(null);
  const [openPlanVersion, setOpenPlanVersion] = useState<number | null>(null);
  const [chatModelOverride, setChatModelOverride] = useLocalStorage<string | null>(
    SHELL_PREFERENCE_KEYS.chatModelOverride,
    null,
  );
  // Phase B / Task B2: a pin persisted from a previous session may name a model
  // that has since left the registry — validate once on mount and clear it
  // rather than letting `SelectionSource::classify` silently read `Fallback`
  // forever with no way for the user to see why. Runs once; ChatModelPicker's
  // own listModels() calls stay independent (its own on-demand fetch).
  useEffect(() => {
    if (!chatModelOverride) return;
    let cancelled = false;
    voxTransport
      .listModels(120)
      .then((models: any) => {
        if (cancelled || !Array.isArray(models)) return;
        const stillPresent = models.some((m: any) => m.id === chatModelOverride || m.model_id === chatModelOverride);
        if (!stillPresent) setChatModelOverride(null);
      })
      .catch(() => {
        // Transport failure here is not this effect's problem to report — leave
        // the pin as-is and let the next real chat turn surface a resolver error.
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- validate once on mount only
  }, []);
  const [groundingCheckEnabled, setGroundingCheckEnabled] = useGroundingCheck(activeSessionId);
  const {
    tasks: chatTasks,
    intents: chatIntents,
    meshPeers: chatMeshPeers,
  } = useChatExecutionData(activeSessionId);
  const attention = useAttentionInbox();
  const [diffOpen, setDiffOpen] = useState(false);
  const [diffText, setDiffText] = useState('');
  const [diffLoading, setDiffLoading] = useState(false);
  const [achievementsOpen, setAchievementsOpen] = useState(false);
  const [ludusProfile, setLudusProfile] = useState<LudusProfile | null>(null);
  const gamifySettings = useGamifySettings();
  useOrchestratorFirstConnectGamify(orchQuery, gamifySettings.enabled);
  const achievementToasts = useAchievementToasts(
    gamifySettings.enabled,
    gamifySettings.mode,
  );

  useEffect(() => {
    setGamifyGuiEventResultListener(achievementToasts.handleGuiEventResult);
    return () => setGamifyGuiEventResultListener(null);
  }, [achievementToasts.handleGuiEventResult]);

  // ── B4-chat: pure-reducer transcript state for the Loquela composer ────────
  const [chatStore, dispatchSessionChat] = useReducer(appChatReducer, initialSessionChatStore);
  const chatStoreRef = useRef(chatStore);
  chatStoreRef.current = chatStore;
  const [sessionAgentStreams, setSessionAgentStreams] = useState<Record<string, StreamItem[]>>({});
  const persistedAssistantIdsRef = useRef<Map<string, Set<string>>>(new Map());
  // Fix Task 1 (gui-chat-harness-fixes): guards the synchronous chat_send_message
  // path against overlapping sends for the same session (e.g. a fast double-Enter
  // while a prior reply is still in flight), which used to spawn two independent
  // tempId lifecycles that could settle out of order with no user-visible sign.
  const chatSendInFlightRef = useRef<Set<string>>(new Set());
  // Guards "+ New session" against a rapid double-click firing two concurrent
  // chat_create_session calls before the first's setActiveSessionId lands.
  const creatingSessionRef = useRef(false);
  const activeChatMessages = getSessionMessages(chatStore, activeSessionId);
  const activeChatAgentItems = sessionAgentStreams[activeSessionId] ?? [];

  // ── 5-minute rolling sparkline windows ──────────────────────────────────
  // Each hook persists its window to localStorage under a namespaced key.
  const agentCountWindow = usePersistedSparkWindow('kpi.agentCount', kpis.activeAgents.value);
  const queueDepthWindow = usePersistedSparkWindow('kpi.queueDepth', kpis.queueDepth.value);
  const budgetBurnWindow = usePersistedSparkWindow('kpi.budgetBurn', kpis.budgetBurn.value);
  const meshWindow       = usePersistedSparkWindow('kpi.mesh', typeof kpis.mesh.value === 'number' ? kpis.mesh.value : kpis.mesh.peers);

  // ── Toast helper ─────────────────────────────────────────────────────────
  // Same-group toasts (see Toast.groupKey, defaults to title) coalesce into a
  // single entry with a count rather than pushing separate entries; once at
  // capacity, distinct-group arrivals fold into an "N more notifications"
  // overflow toast instead of silently dropping an unseen one. See
  // src/lib/toastQueue.ts.
  const toastTimers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const pushToast = useCallback((t: Toast) => {
    const id = nextId('toast');
    setToasts(curr => {
      const { items, touchedId } = coalesceToast(curr, t, id);
      const prevTimer = toastTimers.current.get(touchedId);
      if (prevTimer) clearTimeout(prevTimer);
      toastTimers.current.set(
        touchedId,
        setTimeout(() => {
          setToasts(c => c.filter(x => x.id !== touchedId));
          toastTimers.current.delete(touchedId);
        }, 5000),
      );
      return items;
    });
  }, []);

  // ── Harness issue polling: badge data + toast on newly-detected issues ──
  // Only issues detected in a poll *after* the first are toasted — the first
  // poll just establishes the current pending baseline, so restarting the
  // app doesn't re-toast the entire existing backlog.
  const [pendingHarnessIssueSessionIds, setPendingHarnessIssueSessionIds] = useState<Set<string>>(
    new Set(),
  );
  const seenHarnessIssueIdsRef = useRef<Set<number>>(new Set());
  const harnessIssueBaselineEstablishedRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const { listHarnessIssues } = await import(
          './components/surfaces/Scientia/harnessIssuesApi'
        );
        const pending = await listHarnessIssues('pending', 'chat_session');
        if (cancelled) return;
        const sessionIds = new Set(
          pending.map(i => i.session_key).filter((k): k is string => Boolean(k)),
        );
        setPendingHarnessIssueSessionIds(sessionIds);

        const isFirstPoll = !harnessIssueBaselineEstablishedRef.current;
        harnessIssueBaselineEstablishedRef.current = true;
        for (const issue of pending) {
          if (seenHarnessIssueIdsRef.current.has(issue.id)) continue;
          seenHarnessIssueIdsRef.current.add(issue.id);
          // Skip toasting the pre-existing backlog on first mount/restart —
          // only genuinely new detections (found on later polls) toast.
          if (isFirstPoll) continue;
          pushToast({
            tone: 'warn',
            title: 'Harness issue detected',
            body: issue.summary,
            cause: 'backend-ok',
          });
        }
      } catch {
        // polling failure is non-fatal — next tick retries
      }
    };
    poll();
    const id = window.setInterval(poll, 8_000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [pushToast]);

  // ── Chat sessions (Task 9: App.tsx is the sole owner; Sidebar renders the
  // list via SessionSidebarSection). The hook's initial mount-time load has
  // no built-in error handling (Task 7 follow-up) — this is the one place
  // that owns it, so it's the right place to close that gap with a toast,
  // matching the existing pushToast pattern used for every other session
  // action below.
  const chatSessionsApi = useChatSessions((err) => {
    pushToast({ tone: 'warn', title: 'Chat sessions', body: sanitizeErrorForToast(err), cause: 'backend-error' });
  });
  const [showArchivedSessions, setShowArchivedSessions] = useState(false);
  const [archivedSessions, setArchivedSessions] = useState<ChatSession[]>([]);
  const [chatTaskCounts, setChatTaskCounts] = useState<Record<string, number>>({});
  // Batched open-task counts per chat session, feeding the sidebar's
  // per-session task badges (SessionSidebarSection). Re-fetches whenever the
  // visible session list changes; a failure just leaves badges at their
  // last-known counts rather than surfacing a toast for a purely cosmetic
  // feature.
  useEffect(() => {
    let cancelled = false;
    const ids = (chatSessionsApi.sessions ?? []).map((s) => s.session_id);
    if (ids.length === 0) {
      setChatTaskCounts({});
      return () => { cancelled = true; };
    }
    invoke<Record<string, number>>('plan_open_task_counts', { sessionIds: ids })
      .then((counts) => { if (!cancelled) setChatTaskCounts(counts); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [chatSessionsApi.sessions]);

  // ── Budget warn toast (non-blocking, distinct from the hard-block "Budget
  // limit reached" toast in `dispatchErrorToast`) ─────────────────────────
  // `budget_warn_threshold_pct` is read once (VoxConfig, via the same
  // `get_user_config` command Settings/Onboarding already use) and cached —
  // it rarely changes mid-session and isn't worth a refetch per message.
  const budgetWarnThresholdPctRef = useRef<number | null>(null);
  useEffect(() => {
    invoke<Array<{ key: string; currentValue: string }>>('get_user_config')
      .then((fields) => {
        const field = fields?.find((f) => f.key === 'budget_warn_threshold_pct');
        if (field) {
          const pct = Number(field.currentValue);
          if (Number.isFinite(pct)) budgetWarnThresholdPctRef.current = pct;
        }
      })
      .catch(() => { /* nice-to-have; leave threshold unset on failure */ });
  }, []);
  // Fires at most once per session — dedup flag reset when the active
  // session changes, since a fresh session starts with $0 spend anyway and
  // shouldn't inherit a previous session's "already warned" state.
  const budgetWarnedRef = useRef(false);
  useEffect(() => { budgetWarnedRef.current = false; }, [activeSessionId]);

  const checkBudgetWarn = useCallback((sessionId: string) => {
    if (budgetWarnedRef.current) return;
    const threshold = budgetWarnThresholdPctRef.current;
    if (threshold == null) return;
    invoke<{ sessionUsd: number; dayUsd: number; totalUsd: number; dailyBudgetUsd: number; perSessionBudgetUsd: number }>(
      'get_llm_spend',
      { sessionId },
    )
      .then((spend) => {
        if (!spend || budgetWarnedRef.current) return;
        const dayRatio = spend.dailyBudgetUsd > 0 ? spend.dayUsd / spend.dailyBudgetUsd : 0;
        const sessionRatio = spend.perSessionBudgetUsd > 0 ? spend.sessionUsd / spend.perSessionBudgetUsd : 0;
        const dayWarn = dayRatio >= threshold && dayRatio < 1.0;
        const sessionWarn = sessionRatio >= threshold && sessionRatio < 1.0;
        if (!dayWarn && !sessionWarn) return;
        budgetWarnedRef.current = true;
        // Prefer whichever cap is closer to its limit when both cross.
        const useDayCap = dayWarn && (!sessionWarn || dayRatio >= sessionRatio);
        const body = useDayCap
          ? `You've used ${Math.round(dayRatio * 100)}% of your daily budget ($${spend.dayUsd.toFixed(2)} of $${spend.dailyBudgetUsd.toFixed(2)}).`
          : `You've used ${Math.round(sessionRatio * 100)}% of your session budget ($${spend.sessionUsd.toFixed(2)} of $${spend.perSessionBudgetUsd.toFixed(2)}).`;
        pushToast({ tone: 'warn', title: 'Approaching budget limit', body, cause: 'backend-ok' });
      })
      .catch(() => { /* nice-to-have; never surface an error for this check */ });
  }, [pushToast]);

  const openAchievements = useCallback(() => {
    setAchievementsOpen(true);
  }, []);

  const closeAchievements = useCallback(() => {
    setAchievementsOpen(false);
  }, []);

  // ── KPI update — shared logic used by both EventBus listener and fallback ─
  const applyStatus = useCallback((status: OrchestratorStatus) => {
    const agents: Agent[] = (status.agents ?? []).map(mapAgent);
    const stream: StreamItem[] = (status.recent_events ?? []).map(mapStream);
    const alerts: LudusAlert[] = (status.alerts ?? []).map(mapAlert);

    const budget = budgetStateFromStatus(status.total_cost, status.budget_cap);

    setData(prev => ({
      ...prev,
      agents,
      stream: stream.length > 0 ? stream : prev.stream,
      alerts,
      peers: (status.peers ?? []).length > 0 ? status.peers : prev.peers,
      kpis: {
        ...prev.kpis,
        budgetBurn: {
          ...prev.kpis.budgetBurn,
          value: budget.spent,
          spark: budgetBurnWindow,
        },
        queueDepth: {
          value: status.total_queued ?? 0,
          spark: queueDepthWindow,
        },
      },
    }));
    setKpis(prev => ({
      activeAgents: {
        value: status.agent_count ?? 0,
        delta: (status.agent_count ?? 0) - prev.activeAgents.value,
        spark: agentCountWindow,
      },
      queueDepth: {
        value: status.total_queued ?? 0,
        delta: (status.total_queued ?? 0) - prev.queueDepth.value,
        spark: queueDepthWindow,
      },
      budgetBurn: {
        value: budget.spent,
        cap: budget.cap ?? DEFAULT_BUDGET_CAP_USD,
        source: budget.source,
        delta: budget.spent - prev.budgetBurn.value,
        spark: budgetBurnWindow,
      },
      mesh: (() => {
        const fields = meshKpiFromStatus(status, {
          value: prev.mesh.value,
          peers: prev.mesh.peers,
        });
        return {
          ...fields,
          spark: meshWindow,
        };
      })(),
    }));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [agentCountWindow, queueDepthWindow, budgetBurnWindow, meshWindow]);

  useEffect(() => {
    if (orchQuery.data) {
      setLastOrchEventAt(Date.now());
      applyStatus(orchQuery.data);
    }
  }, [orchQuery.data, applyStatus]);

  // ── Bootstrap: catalog, initial view ─────────────────────────────────────
  useEffect(() => {
    voxTransport.getCatalog().then((catalog: CommandCatalog) => {
      if (catalog?.entries) setData(prev => ({ ...prev, skills: catalog.entries }));
    }).catch((e: unknown) => { console.debug('[App] getCatalog failed (skills palette disabled):', e); });
    invoke<{ display?: string; version?: string }>('get_build_info')
      .then((info) => setAppVersion(info.display ?? info.version ?? 'unknown'))
      .catch(() => setAppVersion('unknown'));

    invoke<string>('get_initial_view').then((view) => {
      const fromHash = parseViewFromLocation(window.location);
      // VG-2: a fresh load on #view=search opens the Omnibar instead of the
      // retired Search surface.
      if (
        fromHash &&
        redirectSearchViewToOmnibar(fromHash, {
          openOmnibar: () => setIsCommandOpen(true),
          navigateTo: (vk) => {
            openTab(vk);
            syncViewToLocation(vk);
          },
          fallbackChild: 'memory',
        })
      ) {
        return;
      }
      if (fromHash && isKnownView(fromHash)) {
        seedDiscoveryPresetForLegacyKey(fromHash);
        const { child } = resolveNavigation(fromHash);
        openTab(child);
        syncViewToLocation(child);
        return;
      }
      if (view && isKnownView(view)) {
        seedDiscoveryPresetForLegacyKey(view);
        const { child } = resolveNavigation(view);
        openTab(child);
        syncViewToLocation(child);
      }
    }).catch(() => {});

    invoke<Session[]>('chat_list_sessions', { limit: 1 })
      .then((sessions) => {
        if (sessions?.[0]?.session_id) {
          setActiveSessionId(sessions[0].session_id);
        } else {
          invoke<Session>('chat_create_session', { title: 'Chat' })
            .then((s) => { if (s?.session_id) setActiveSessionId(s.session_id); })
            .catch((err) => pushToast({ tone: 'warn', title: 'Chat session', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
        }
      })
      .catch((err) => pushToast({ tone: 'warn', title: 'Chat sessions', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Master-sidebar Policies badge: poll the catalog + current-branch status,
  // compute the worst status + count of rules at that tier, color the nav badge.
  // Lightweight (in-process IPC) and resilient: any failure clears the badge.
  useEffect(() => {
    let cancelled = false;
    const refresh = async () => {
      try {
        const [rows, branchInfos] = await Promise.all([
          invoke<PolicyRow[]>('policy_list', { domain: null, group: null }),
          invoke<BranchInfo[]>('list_branches').catch(() => [] as BranchInfo[]),
        ]);
        const branches = branchInfos.filter(b => b.isCurrent).map(b => b.branch);
        const sel = branches.length ? branches : ['HEAD'];
        const status = await invoke<PolicyStatus[]>('policy_status', { branches: sel }).catch(() => [] as PolicyStatus[]);
        if (cancelled) return;
        setPolicyBadge({ status: overallWorst(rows, status, sel), count: worstCount(rows, status, sel) });
      } catch {
        if (!cancelled) setPolicyBadge(null);
      }
    };
    refresh();
    const id = setInterval(refresh, POLICY_BADGE_POLL_MS);
    return () => { cancelled = true; clearInterval(id); };
  }, []);

  // ── Pushed live agent-event stream (B4: "vox://agent-events" Tauri event) ──
  // Each AgentEvent is prepended to the dashboard activity feed as a StreamItem.
  // We keep this independent from the status subscription above; the list is
  // capped so it does not grow unbounded.
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listenAgentEvents((frame) => {
      const kindType = frame.kind?.type ?? '';
      if (kindType !== 'token_streamed') {
        const item = mapAgentEvent(frame);
        setData(prev => ({
          ...prev,
          stream: [item, ...prev.stream].slice(0, STREAM_CAP),
        }));
      }
      dispatchSessionChat({ type: 'agentEvent', event: frame });

      const sessionId = resolveSessionForEvent(chatStoreRef.current, frame);
      if (sessionId) {
        const item = mapAgentEvent(frame);
        setSessionAgentStreams(prev => ({
          ...prev,
          [sessionId]: [...(prev[sessionId] ?? []), item].slice(-STREAM_CAP),
        }));
      }
    })
      .then((fn) => {
        if (cancelled) {
          // Effect already cleaned up before subscription resolved.
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch(() => {
        // listen() unavailable (e.g. plain browser dev) — no live feed.
      });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  // ── Pending-bubble honesty watchdog: nothing server-side ever expires a
  // pending chat bubble, so sweep client-side and flip anything stuck in
  // `pending` for PENDING_TIMEOUT_MS to an honest failure.
  useEffect(() => {
    const id = setInterval(() => {
      dispatchSessionChat({ type: 'pendingTimeout', nowMs: Date.now() });
    }, PENDING_WATCHDOG_SWEEP_MS);
    return () => clearInterval(id);
  }, []);

  // T1.5: best-effort extraction of the orchestrator's numeric task_id from a
  // command's result (currently only `submit_orchestrator_task` -> SubmitTaskResult
  // carries one). Passed to `finish_gui_run` as the correlation key the Rust side
  // uses to join `agent_runs.approval_ref`/cost/tokens from durable oplog +
  // vox-telemetry data — see crates/vox-gui/src/commands/runs.rs. `run_id` (the
  // GUI-minted id) and `task_id` (the orchestrator's) are distinct id spaces;
  // this is the one place they meet.
  const extractTaskId = (result: unknown): string | null => {
    if (result && typeof result === 'object' && 'task_id' in result) {
      const tid = (result as { task_id?: string | number | null }).task_id;
      return tid == null ? null : String(tid);
    }
    return null;
  };

  const executeIpcWithRun = useCallback(async <T,>(command: string, payload: Record<string, unknown>, workflowName: string, onRun?: (runId: string) => void): Promise<T> => {
    const runId = nextGuiRunId();
    onRun?.(runId);
    await invoke('start_gui_run', {
      input: {
        run_id: runId,
        workflow_name: workflowName,
        planned_steps: 1,
      }
    });
    let finished = false;
    try {
      const result = await invoke<T>(command, payload);
      await invoke('finish_gui_run', {
        runId: runId,
        success: true,
        completedSteps: 1,
        error: null,
        taskId: extractTaskId(result),
      });
      finished = true;
      return result;
    } catch (err) {
      if (!finished) {
        await invoke('finish_gui_run', {
          runId: runId,
          success: false,
          completedSteps: 0,
          error: String(err), // gui-safe: sent to the backend as a run-completion record, never rendered to the user
        }).catch(() => {});
      }
      throw err;
    }
  }, []);

  // ── Global keybinds ───────────────────────────────────────────────────────
  // togglePauseSelectedRef is reassigned each render (after handlePause/handleResume
  // are defined) so the data-driven dispatcher sees live state via stable ref.
  const togglePauseSelectedRef = useRef<() => void>(() => {});

  // ── Navigation (hash-synced) ─────────────────────────────────────────────
  const navigateTo = useCallback((viewKey: string) => {
    seedDiscoveryPresetForLegacyKey(viewKey);
    const { child } = resolveNavigation(viewKey);
    openTab(child);
    syncViewToLocation(child);
  }, [openTab]);

  const openParentNav = useCallback((parentKey: string) => {
    const child = DEFAULT_CHILD_BY_PARENT[parentKey] ?? parentKey;
    navigateTo(child);
  }, [navigateTo]);

  const [focusedFeedbackId, setFocusedFeedbackId] = useState<string | null>(null);

  const onOpenFeedbackContext = useCallback((feedbackId: string) => {
    navigateTo('chat');
    setFocusedFeedbackId(feedbackId);
  }, [navigateTo]);

  const manageGamifyInSettings = useCallback(() => {
    try {
      localStorage.setItem('vox_settings_seed', JSON.stringify({ section: 'gamify' }));
      window.dispatchEvent(new Event('vox-settings-seed'));
    } catch {
      /* ignore storage errors */
    }
    setAchievementsOpen(false);
    navigateTo('settings');
  }, [navigateTo]);

  useEffect(() => {
    if (!achievementsOpen || !gamifySettings.enabled) return;
    let cancelled = false;
    invoke<LudusProfile>('get_ludus_profile')
      .then((p) => {
        if (!cancelled) setLudusProfile(p);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [achievementsOpen, gamifySettings.enabled]);

  useEffect(() => {
    const onHashChange = () => {
      const fromHash = parseViewFromLocation(window.location);
      // VG-2: #view=search no longer renders a surface — open the Omnibar and
      // park on a real child. Real setters only (no setActiveViewRaw, no
      // navigateTo recursion — finding #7).
      if (
        fromHash &&
        redirectSearchViewToOmnibar(fromHash, {
          openOmnibar: () => setIsCommandOpen(true),
          navigateTo: (vk) => {
            openTab(vk);
            syncViewToLocation(vk);
          },
          fallbackChild: 'memory',
        })
      ) {
        return;
      }
      if (fromHash) {
        seedDiscoveryPresetForLegacyKey(fromHash);
        const { child } = resolveNavigation(fromHash);
        openTab(child);
        syncViewToLocation(child);
      }
    };
    window.addEventListener('hashchange', onHashChange);
    return () => window.removeEventListener('hashchange', onHashChange);
  }, [openTab]);

  // ── Cross-surface deep-link: a surface may request navigation to another
  // surface (optionally seeding a value) by dispatching a `vox://navigate-surface`
  // CustomEvent. Used by the Discovery Inbox "Open review" action to jump to the
  // Discovery Review surface with the publication id pre-filled.
  useEffect(() => {
    const onNavigate = (e: Event) => {
      const detail = (e as CustomEvent<{ view?: string; publicationId?: string }>).detail;
      if (!detail?.view || !isKnownView(detail.view)) return;
      if (detail.publicationId != null) {
        try {
          window.localStorage.setItem('vox_discovery_review_seed', detail.publicationId);
        } catch { /* localStorage unavailable — surface still switches */ }
      }
      navigateTo(detail.view);
    };
    window.addEventListener('vox://navigate-surface', onNavigate as EventListener);
    return () => window.removeEventListener('vox://navigate-surface', onNavigate as EventListener);
  }, [navigateTo]);

  const hydrateChatSession = useCallback(async (sessionId: string) => {
    if (!sessionId) return;
    try {
      const rows = await invoke<
        Array<{
          id: number;
          role: string;
          content: string;
          task_id?: string;
          model_id?: string;
          latency_ms?: number;
          selection_reason?: string;
        }>
      >('chat_get_messages', { sessionId, limit: 500 });
      dispatchSessionChat({
        type: 'hydrate',
        sessionId,
        messages: rows.map(r => ({
          id: String(r.id),
          role: r.role as 'user' | 'assistant' | 'system',
          text: r.content,
          status: 'done' as const,
          runId: r.task_id ?? `persist-${r.id}`,
          taskId: r.task_id ?? undefined,
          modelId: r.model_id ?? undefined,
          latencyMs: r.latency_ms ?? undefined,
          selectionReason: r.selection_reason ?? undefined,
        })),
      });
    } catch {
      // keep live state when DB read fails
    }
  }, []);

  useEffect(() => {
    if (activeSessionId) hydrateChatSession(activeSessionId);
  }, [activeSessionId, hydrateChatSession]);

  const loadTaskDiff = useCallback(async (path?: string) => {
    setDiffOpen(true);
    setDiffLoading(true);
    try {
      const text = await invoke<string>('get_task_diff', { path: path ?? null });
      setDiffText(text);
    } catch (err) {
      setDiffText('');
      pushToast({ tone: 'warn', title: 'Diff failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setDiffLoading(false);
    }
  }, [pushToast]);

  const handleLoquelaSubmit = useCallback(async (payload: ChatPayload, skillExclusionsOverride?: string[]) => {
    const sessionId = payload.session_id ?? activeSessionId;
    if (!sessionId) {
      pushToast({ tone: 'warn', title: 'No chat session', body: 'Create or select a chat session first.', cause: 'validation' });
      return;
    }
    lastChatPayloadRef.current = payload;
    // ONE payload, ONE command (`chat_turn`) — the dispatch fork now lives in
    // Rust. The frontend still branches below, but only on STORE LIFECYCLE:
    // `submitResolved` is the sole writer of `taskToSession`, the map that
    // routes every task_*/token_streamed frame to a bubble and replays the
    // 30s pending buffer. See spec §6.
    const turn = buildChatTurn(payload, {
      sessionId,
      modelOverride: chatModelOverride,
      groundingCheckEnabled,
      activeSkillId: activeSkill?.id ?? null,
      // `skillExclusionsOverride` lets `excludeSkillAndRetry` re-dispatch with
      // the freshly-excluded id included immediately, instead of racing this
      // callback's own closed-over `skillExclusions` state (which would still
      // read the pre-exclusion value on the very next tick).
      skillExclusions: skillExclusionsOverride ?? skillExclusions,
      allowDuplicate: false,
      // The real originating chat session -- distinct from `sessionId` above
      // on the background path, where it's a synthetic throwaway id (see
      // `newBackgroundSessionId` call sites). Carries delegation lineage to
      // the backend even when the dispatch session itself is disposable.
      chatSessionId: activeSessionId,
    });

    // Checked BEFORE chat_append_message persists anything: a second send
    // while the first is still in flight must not write an orphaned user
    // message row that nothing will ever reply to (the persisted row would
    // otherwise survive the early-return below with no assistant turn).
    if (turn.execution === 'sync' && chatSendInFlightRef.current.has(sessionId)) {
      pushToast({
        tone: 'warn',
        title: 'Please wait',
        body: 'A reply is still in progress for this chat.',
        cause: 'validation',
      });
      return;
    }

    invoke('chat_append_message', {
      input: userAppendInput(sessionId, payload.description),
    }).catch((err) => pushToast({ tone: 'warn', title: 'Message not saved', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
    recordGamifyGuiEvent('chat_message_sent', { session_id: sessionId }, { enabled: gamifySettings.enabled });

    if (turn.execution === 'background') {
      // Long-lived correlated stream. `submitResolved` is the ONLY writer of
      // taskToSession, which routes every task_*/token_streamed frame to this
      // bubble and replays the 30s pending buffer. Do not collapse this into
      // chatPending/chatReplySettled: a background dispatch returns a task_id
      // and no answer text, so settling it 'done' would strand an empty bubble
      // the pending watchdog cannot rescue.
      let runId = '';
      const dispatchAttempt = (allowDuplicate: boolean) =>
        // The one place GUI run-ids and orchestrator task-ids meet — the join
        // key `runs.rs` uses for cost/token telemetry.
        executeIpcWithRun<ChatTurnDto>(
          'chat_turn',
          { input: { ...turn, allow_duplicate: allowDuplicate } },
          'gui.loquela.submit',
          // Mint the runId and create the bubbles BEFORE the invoke resolves
          // so streamed tokens correlate to a live transcript entry.
          (id) => {
            runId = id;
            dispatchSessionChat({
              type: 'submit',
              sessionId,
              runId: id,
              prompt: String(payload.description ?? ''),
            });
          },
        );

      try {
        let result = await dispatchAttempt(false);
        // Refused as a near-duplicate: retract the optimistic bubble and ask.
        if (result?.task_id == null && result?.duplicate_of) {
          if (runId) {
            dispatchSessionChat({
              type: 'failRun',
              sessionId,
              runId,
              error: `Skipped — near-duplicate of task #${result.duplicate_of}`,
            });
          }
          const proceed = window.confirm(
            `This looks like a near-duplicate of task #${result.duplicate_of}.\n\nSubmit it anyway?`,
          );
          if (!proceed) {
            pushToast({ tone: 'info', title: 'Duplicate skipped', body: `Kept existing task #${result.duplicate_of}.`, cause: 'backend-ok' });
            return;
          }
          result = await dispatchAttempt(true);
        }
        if (runId && result?.task_id != null) {
          dispatchSessionChat({
            type: 'submitResolved',
            sessionId,
            runId,
            taskId: String(result.task_id),
          });
          recordGamifyGuiEvent(
            'task_submitted',
            { session_id: sessionId, task_id: String(result.task_id) },
            { enabled: gamifySettings.enabled },
          );
          checkBudgetWarn(sessionId);
        }
      } catch (err) {
        // Covers BOTH the first attempt and the post-confirm duplicate retry
        // (`dispatchAttempt(true)` above) -- either can throw here, and
        // either would otherwise leave the optimistic pending bubble minted
        // by `dispatchAttempt`'s onStart callback stuck with no terminal
        // state until the multi-minute pendingTimeout watchdog eventually
        // flips it to a generic timeout message.
        if (runId) {
          dispatchSessionChat({
            type: 'failRun',
            sessionId,
            runId,
            error: chatTurnErrorMessage(err),
          });
        }
        pushToast(dispatchErrorToast(err, 'Dispatch Failed'));
      }
      return;
    }

    // Sync: terminal request/response, no task to correlate against.
    chatSendInFlightRef.current.add(sessionId);
    const tempId = nextGuiRunId();
    dispatchSessionChat({
      type: 'chatPending',
      sessionId,
      tempId,
      userText: String(payload.description ?? ''),
    });
    try {
      const reply = await sendChatTurn(turn);
      // `chat_turn` already persisted this exact reply server-side. Mark it as
      // already-persisted BEFORE dispatching, so the "persist assistant
      // transcript rows" effect below (which sweeps chatStore.sessions for
      // status 'done'/'failed' messages not yet in persistedAssistantIdsRef)
      // doesn't re-persist it via chat_append_message and double the row on
      // reload. `ChatMessage.id` is a string — never Number() it.
      let persisted = persistedAssistantIdsRef.current.get(sessionId);
      if (!persisted) {
        persisted = new Set<string>();
        persistedAssistantIdsRef.current.set(sessionId, persisted);
      }
      persisted.add(reply.id);
      dispatchSessionChat({
        type: 'chatReplySettled',
        sessionId,
        tempId,
        result: {
          ok: true,
          message: {
            id: reply.id,
            role: 'assistant',
            text: reply.text,
            status: 'done',
            runId: tempId,
            modelId: reply.modelId,
            latencyMs: reply.latencyMs,
            selectionReason: reply.selectionReason,
            groundingFlagged: reply.groundingFlagged,
            events: reply.events,
          },
        },
      });
      checkBudgetWarn(sessionId);
    } catch (err) {
      const errorText = chatTurnErrorMessage(err);
      dispatchSessionChat({
        type: 'chatReplySettled',
        sessionId,
        tempId,
        result: { ok: false, error: errorText },
      });
      pushToast(dispatchErrorToast(err, 'Chat reply failed'));
    } finally {
      chatSendInFlightRef.current.delete(sessionId);
    }
  }, [executeIpcWithRun, pushToast, activeSessionId, activeSkill, gamifySettings.enabled, checkBudgetWarn, chatModelOverride, groundingCheckEnabled, skillExclusions]);

  // "not this one" (ChatTurnEventRow, via ChatSurface -> ChatTranscript):
  // append the skill to session-scoped exclusions AND immediately re-dispatch
  // the last turn so the excluded skill is absent from the system prompt on
  // the retry. Both actions matter — a first draft of this feature added the
  // `skill_exclusions` field end-to-end but nothing ever appended to it or
  // re-ran the turn, so excluding a skill silently did nothing.
  const excludeSkillAndRetry = useCallback((skillId: string) => {
    const next = skillExclusionsRef.current.includes(skillId)
      ? skillExclusionsRef.current
      : [...skillExclusionsRef.current, skillId];
    skillExclusionsRef.current = next;
    setSkillExclusions(next);
    const last = lastChatPayloadRef.current;
    if (last) handleLoquelaSubmit(last, next);
  }, [handleLoquelaSubmit]);

  const handleLoquelaSlash = useCallback(async (
    cmd: string,
    ctx: { setText: (text: string) => void },
  ): Promise<boolean> => {
    const base = slashCommandBase(cmd);

    if (base === '/diff') {
      loadTaskDiff();
      return true;
    }
    if (base === '/memory') {
      navigateTo('memory');
      return true;
    }
    if (base === '/spawn') {
      // Phase D Task D3: carry the user's actual typed text (e.g. "/spawn fix
      // the login bug") through to the spawned agent's task description
      // instead of always sending the same generic placeholder — a model or
      // reviewer reading the delegated task later has no way to recover what
      // was actually asked for otherwise. Falls back to the original generic
      // description only when no text follows `/spawn`.
      const spawnGoal = cmd.slice(base.length).trim();
      void handleLoquelaSubmit({
        description: spawnGoal || 'Spawn a sub-agent on the current branch to pursue this task in parallel.',
        mode: 'act',
        // Explicit, not inferred: `execution_mode` defaults to 'chat' when
        // omitted, so /spawn must say 'task' or it would silently become a
        // synchronous reply.
        execution_mode: 'task',
        // Own session id (not activeSessionId): the background path never
        // writes to the orchestrator's chat_history context store, only
        // vox_chat_message does, so borrowing the chat session id here would
        // silently desync that store from the GUI transcript.
        session_id: newBackgroundSessionId(),
      });
      return true;
    }
    if (base === '/plan') {
      const goal = cmd.slice(base.length).trim();
      if (!goal) {
        pushToast({ tone: 'warn', title: '/plan needs a goal', body: 'Try: /plan add a health endpoint', cause: 'validation' });
        return true;
      }
      const sessionId = activeSessionId ?? newBackgroundSessionId();
      ctx.setText('');
      void (async () => {
        try {
          // Bypasses handleLoquelaSubmit deliberately: that path persists a
          // chat_append_message row and tracks sync/background dedup state,
          // neither of which applies here — `execution: 'plan'` returns no
          // assistant row (see chat_turn.rs's run_plan), just the plan DAG's
          // session id/version to point PlanPanel at.
          const dto = await sendChatTurnRaw({
            session_id: sessionId,
            content: goal,
            execution: 'plan',
            context_files: [],
            skill_exclusions: [],
          });
          if (dto.plan_session_id) {
            setOpenPlanSessionId(dto.plan_session_id);
            setOpenPlanVersion(dto.plan_version ?? null);
          }
        } catch (err) {
          pushToast({ tone: 'warn', title: '/plan failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
        }
      })();
      return true;
    }
    if (base === '/rollback') {
      void (async () => {
        try {
          const res = await invoke<McpInvokeResult>('invoke_mcp_tool', { tool: 'vox_undo', args: {} });
          const failed = !res || res.is_error;
          pushToast({
            tone: failed ? 'warn' : 'ok',
            title: failed ? 'Rollback failed' : 'Rollback complete',
            body: !res
              ? 'No response from the backend.'
              : failed
                ? sanitizeErrorForToast(typeof res.result === 'string' ? res.result : JSON.stringify(res.result))
                : 'Reverted to last durable checkpoint.',
            cause: failed ? 'backend-error' : 'backend-ok',
          });
        } catch (err) {
          pushToast({ tone: 'warn', title: 'Rollback failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
        }
      })();
      return true;
    }
    if (base === '/doubt') {
      ctx.setText('/doubt ');
      return true;
    }
    if (base === '/audit') {
      void (async () => {
        try {
          const out = await invoke<{ exit_code: number; stdout: string; stderr: string }>('execute_command', {
            path: ['check'],
            args: { __argv: [] },
          });
          if (!out) {
            pushToast({ tone: 'warn', title: 'Audit unavailable', body: 'No response from the backend.', cause: 'backend-error' });
            return;
          }
          const text = [out.stdout, out.stderr].filter(Boolean).join('\n').trim();
          pushToast({
            tone: out.exit_code === 0 ? 'ok' : 'warn',
            title: out.exit_code === 0 ? 'Audit passed' : 'Audit findings',
            body: text.slice(0, 400) || 'vox check completed',
            cause: out.exit_code === 0 ? 'backend-ok' : 'backend-error',
          });
        } catch {
          try {
            const res = await invoke<McpInvokeResult>('invoke_mcp_tool', { tool: 'vox_check', args: {} });
            const failed = !res || res.is_error;
            const rawResult = res && typeof res.result === 'string' ? res.result : JSON.stringify(res?.result);
            const body = !res
              ? 'No response from the backend.'
              : failed
                ? sanitizeErrorForToast(rawResult).slice(0, 400)
                : rawResult.slice(0, 400);
            pushToast({
              tone: failed ? 'warn' : 'ok',
              title: failed ? 'Audit failed' : 'Audit complete',
              body: body || 'vox_check finished',
              cause: failed ? 'backend-error' : 'backend-ok',
            });
          } catch (err) {
            pushToast({ tone: 'warn', title: 'Audit unavailable', body: sanitizeErrorForToast(err), cause: 'backend-error' });
          }
        }
      })();
      return true;
    }
    return false;
  }, [loadTaskDiff, navigateTo, handleLoquelaSubmit, pushToast]);

  // Persist assistant transcript rows when a run completes (user rows persist on submit).
  useEffect(() => {
    for (const [sessionId, state] of Object.entries(chatStore.sessions)) {
      let persisted = persistedAssistantIdsRef.current.get(sessionId);
      if (!persisted) {
        persisted = new Set();
        persistedAssistantIdsRef.current.set(sessionId, persisted);
      }
      const ready = assistantMessagesReadyToPersist(state.messages, persisted);
      for (const m of ready) {
        persisted.add(m.id);
        const content = assistantPersistContent(m);
        if (!content.trim()) continue;
        invoke('chat_append_message', {
          input: {
            session_id: sessionId,
            role: 'assistant',
            content,
            task_id: m.taskId ?? null,
            model_id: m.modelId ?? null,
          },
        }).catch((err) => pushToast({ tone: 'warn', title: 'Message not saved', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
      }
    }
  }, [chatStore, pushToast]);

  // Attach one or more locators to the shared Loquela context set. These chips
  // become the next task's file manifest (see handleLoquelaSubmit), so this is
  // a real "pin to context" — not a toast-only gesture. Deduped by chip id.
  const attachContext = useCallback((items: Array<{ kind: 'file' | 'url' | 'image'; label: string }>) => {
    if (items.length === 0) return;
    setChips(prev => {
      const seen = new Set(prev.map(c => c.id));
      const next = [...prev];
      for (const it of items) {
        const id = `ctx-${it.kind}-${it.label}`;
        if (seen.has(id)) continue;
        seen.add(id);
        next.push({ id, kind: it.kind, label: it.label });
      }
      return next;
    });
    pushToast({
      tone: 'ok',
      title: items.length === 1 ? 'Pinned to context' : `${items.length} pinned to context`,
      body: items.length === 1 ? items[0].label : `${items.length} citations → Loquela`,
      cmd: 'context.attach',
      cause: 'backend-ok',
    });
  }, [pushToast]);

  const handlePause = useCallback(async (a: Agent) => {
    setData(prev => ({ ...prev, agents: prev.agents.map(x => x.id === a.id ? { ...x, phase: 'Paused' } : x) }));
    const id = Number(a.id.replace('A-', ''));
    if (!Number.isFinite(id)) {
      pushToast({ tone: 'warn', title: 'Pause unavailable', body: 'Selected agent has non-numeric id; cannot route pause command.', cause: 'validation' });
      return;
    }
    await executeIpcWithRun('pause_orchestrator_agent', { agentId: id }, 'gui.agent.pause')
      .catch((err) => pushToast({ tone: 'warn', title: 'Pause failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
  }, [executeIpcWithRun, pushToast]);

  const handleResume = useCallback(async (a: Agent) => {
    setData(prev => ({ ...prev, agents: prev.agents.map(x => x.id === a.id ? { ...x, phase: 'Executing' } : x) }));
    const id = Number(a.id.replace('A-', ''));
    if (!Number.isFinite(id)) {
      pushToast({ tone: 'warn', title: 'Resume unavailable', body: 'Selected agent has non-numeric id; cannot route resume command.', cause: 'validation' });
      return;
    }
    await executeIpcWithRun('resume_orchestrator_agent', { agentId: id }, 'gui.agent.resume')
      .catch((err) => pushToast({ tone: 'warn', title: 'Resume failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
  }, [executeIpcWithRun, pushToast]);

  const handleDoubt = useCallback((item: StreamItem) => {
    if (item.taskId == null) return;
    voxTransport.doubtTask(item.taskId)
      .then(() => pushToast({ tone: 'ok', title: 'Doubt cast', body: item.title, cause: 'backend-ok' }))
      .catch((e) => pushToast({ tone: 'warn', title: 'Doubt failed', body: sanitizeErrorForToast(e), cause: 'backend-error' }));
  }, [pushToast]);

  const handleOverrule = useCallback((item: StreamItem) => {
    if (item.taskId == null) return;
    voxTransport.overruleTask(item.taskId, 'overruled from dashboard')
      .then(() => pushToast({ tone: 'ok', title: 'Overruled', body: item.title, cause: 'backend-ok' }))
      .catch((e) => pushToast({ tone: 'warn', title: 'Overrule failed', body: sanitizeErrorForToast(e), cause: 'backend-error' }));
  }, [pushToast]);

  // Wire pause/resume to the stable ref so the data-driven dispatcher sees live state.
  togglePauseSelectedRef.current = () => {
    const agent = data.agents.find(a => a.id === selectedAgentId);
    if (!agent) return;
    if (agent.phase === 'Paused') handleResume(agent);
    else handlePause(agent);
  };

  const actionHandlers = useMemo(() => ({
    'open-palette': () => setIsCommandOpen(true),
    'toggle-sidebar': () => setSidebarMode(m => m === 'rail' ? 'default' : m === 'default' ? 'wide' : 'rail'),
    'pause-resume-agent': () => togglePauseSelectedRef.current(),
  }), []);
  useKeybinds(actionHandlers, bindings);



  const handleAckAlert = useCallback(async (note: LudusAlert) => {
    setData(prev => ({ ...prev, alerts: prev.alerts.filter(x => x.id !== note.id) }));
    await invoke('ack_ludus_notification', { notificationId: note.id })
      .catch((err) => pushToast({ tone: 'warn', title: 'Alert ack failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
  }, [pushToast]);

  const focusComposer = useCallback(() => {
    const el = document.getElementById('loquela-composer') as HTMLTextAreaElement | null;
    if (el) {
      el.focus();
      el.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    }
  }, []);

  const handleCommandAction = useCallback((cmd: CommandPaletteAction) => {
    if ('type' in cmd && cmd.type === 'navigate' && cmd.viewKey) navigateTo(cmd.viewKey);
    else if ('type' in cmd && cmd.type === 'agent' && 'id' in cmd) { navigateTo('flow'); setSelectedAgentId(`${cmd.id}`); }
    else if ('type' in cmd && cmd.type === 'command') navigateTo('catalog');
    else if ('type' in cmd && cmd.type === 'hit' && cmd.locator) {
      const viewKey = cmd.viewKey ?? viewKeyForLocator(cmd.locator);
      if (cmd.locator.kind === 'file' || cmd.locator.kind === 'web') {
        voxTransport.openLocator(cmd.locator).catch(() => {});
      }
      navigateTo(viewKey);
    }
    else if ('id' in cmd && cmd.id === 'submit') handleSubmitTaskAction(navigateTo, focusComposer);
    else if ('id' in cmd && cmd.id === 'pause-all') data.agents.forEach(handlePause);
    else if ('id' in cmd && cmd.id === 'resume-all') data.agents.filter(a => a.phase === 'Paused').forEach(handleResume);
    else if ('id' in cmd && cmd.id === 'ack-all') data.alerts.forEach(handleAckAlert);
    else if ('id' in cmd && typeof cmd.id === 'string' && cmd.id.startsWith('agent:')) { navigateTo('flow'); setSelectedAgentId(cmd.id.slice(6)); }
    else if ('id' in cmd && typeof cmd.id === 'string' && cmd.id.startsWith('skill:')) {
      const skillId = cmd.id.slice(6);
      const s = installedSkillEntries.find(
        (x) => x.command === skillId || x.capability_id === skillId,
      );
      if (s) {
        const deployId = s.capability_id ?? s.command;
        setDeployedSet(prev => new Set([...prev, deployId]));
        // See /spawn handler above: own session id so this background
        // dispatch doesn't silently borrow the active chat session's
        // identity in the orchestrator's chat_history store.
        handleLoquelaSubmit({
          description: `Deploy skill: ${s.command}`,
          active_skill: deployId,
          execution_mode: 'task',
          session_id: newBackgroundSessionId(),
        });
      }
    } else if ('codename' in cmd) {
      navigateTo('flow');
      setSelectedAgentId(cmd.id);
    } else if ('command' in cmd) {
      navigateTo('catalog');
    } else if ('label' in cmd || 'type' in cmd) {
      const action = cmd as { label?: string; type?: string };
    }
  }, [data, installedSkillEntries, handlePause, handleResume, handleAckAlert, handleLoquelaSubmit, pushToast, navigateTo, focusComposer]);


  const chatExecutionKpis = useMemo(
    () => ({
      activeAgents: { value: kpis.activeAgents.value },
      queueDepth: { value: kpis.queueDepth.value },
      mesh: { peers: chatMeshPeers > 0 ? chatMeshPeers : kpis.mesh.peers },
    }),
    [kpis, chatMeshPeers],
  );

  // Derive the in-flight task from the shared chat transcript: the latest
  // assistant bubble still streaming/pending whose task_id has resolved is the
  // task the Stop button (and Enter, when running) should interrupt. No parallel
  // source of truth — this rides the same EventBus correlation as the bubbles.
  const inFlightAssistant = useMemo(() => {
    for (let i = activeChatMessages.length - 1; i >= 0; i -= 1) {
      const m = activeChatMessages[i];
      if (
        m.role === 'assistant' &&
        (m.status === 'pending' || m.status === 'streaming') &&
        m.taskId != null
      ) {
        return m;
      }
    }
    return undefined;
  }, [activeChatMessages]);
  const inFlightTaskId = inFlightAssistant ? Number(inFlightAssistant.taskId) : undefined;
  const taskInProgress = inFlightTaskId != null && Number.isFinite(inFlightTaskId);

  // "Current agent" for the composer's inline Resume button: chat messages
  // carry a task id but no agent id, so there is no direct per-session agent
  // link (unlike the Agents view, which acts on an explicitly selected
  // Agent). As a minimal, documented derivation: when exactly one agent is
  // paused fleet-wide, treat it as this session's paused agent — mirrors how
  // taskInProgress/inFlightTaskId are derived from a single unambiguous
  // signal rather than inventing new per-session agent tracking.
  // Known limitations (both from the lack of a real per-session agent id):
  //   - 2+ agents paused fleet-wide: ambiguous, Resume is hidden everywhere.
  //   - exactly 1 agent paused fleet-wide but unrelated to the session
  //     currently open: Resume still shows here and would resume the wrong
  //     agent. Low-risk today (chat is effectively single-session-at-a-time
  //     in practice), but real — fix properly once messages/tasks carry a
  //     real agent id.
  const pausedAgents = useMemo(
    () => data.agents.filter((a) => a.phase === 'Paused'),
    [data.agents],
  );
  const currentPausedAgent = pausedAgents.length === 1 ? pausedAgents[0] : undefined;

  const handleInterruptTask = useCallback(
    (taskId?: number) => {
      if (taskId == null || !Number.isFinite(taskId)) return;
      invoke('interrupt_orchestrator_task', { taskId })
        .then(() => pushToast({ tone: 'info', title: 'Interrupting task', body: `Task #${taskId}`, cause: 'backend-ok' }))
        .catch((err) => pushToast({ tone: 'warn', title: 'Interrupt failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
    },
    [pushToast],
  );

  const loquelaComposer = (
    <Loquela
      chips={chips}
      setChips={setChips}
      onSubmit={(p) => handleLoquelaSubmit({
        ...p,
        // 'chat' mode (the composer's default) stays part of the active
        // chat session, same as before. The "Background task" toggle
        // position (execution_mode: 'task') must NOT reuse activeSessionId --
        // same fix as /spawn and Deploy-skill below, for the same reason (the
        // background path never writes to the orchestrator's
        // chat_history:{session_id} context store, so folding it into the
        // active session desyncs it).
        session_id: p.execution_mode === 'task' ? newBackgroundSessionId() : activeSessionId,
        model_override: chatModelOverride,
        grounding_check_enabled: groundingCheckEnabled,
      })}
      onSlashCommand={handleLoquelaSlash}
      taskInProgress={taskInProgress}
      currentTaskId={taskInProgress ? inFlightTaskId : undefined}
      onInterrupt={handleInterruptTask}
      agentPaused={!!currentPausedAgent}
      currentAgent={currentPausedAgent ?? null}
      onResume={handleResume}
      sessionBudget={{
        spent: kpis.budgetBurn.value,
        cap: kpis.budgetBurn.cap,
        source: kpis.budgetBurn.source,
      }}
      activeSkill={activeSkill}
      setActiveSkill={setActiveSkill}
      skills={installedSkillEntries}
      toast={pushToast}
      agents={data.agents}
      trailingSlot={
        <>
          <ChatModelPicker
            activeModel={chatModelOverride ?? activeModel}
            onApplied={setChatModelOverride}
          />
          <GroundingCheckToggle
            enabled={groundingCheckEnabled}
            onToggle={setGroundingCheckEnabled}
          />
        </>
      }
    />
  );

  // Appendix D: composer only on Chat surface; no global Loquela dock.
  const chatDocked = false;

  const surfaceProps = {
    pushToast,
    data,
    dashboardLoading: orchQuery.isLoading,
    onPause: handlePause,
    onResume: handleResume,
    onDoubt: handleDoubt,
    onOverrule: handleOverrule,
    onOpenFeedbackContext: onOpenFeedbackContext,
    focusedFeedbackId: focusedFeedbackId,
    onAckLudus: handleAckAlert,
    filterKind,
    setFilterKind,
    selectedAgentId,
    setSelectedAgentId,
    skills: data.skills,
    onAttachContext: attachContext,
    onNavigate: navigateTo,
    onOpenChat: () => navigateTo('chat'),
    onOpenInConsole: (a: Agent) => {
      setSelectedAgentId(a.id);
      navigateTo('console');
    },
    attention_budget: orchQuery.data?.attention_budget,
    activeSessionId,
    onSessionChange: setActiveSessionId,
    chatMessages: activeChatMessages,
    onFocusComposer: focusComposer,
    chatTasks,
    chatIntents,
    chatExecutionKpis,
    chatActiveModel: activeModel,
    groundingCheckEnabled,
    // Set by the sidebar's task-badge click (onTaskBadgeClick below), resolved via
    // `latest_plan_session_for_chat` against the real origin_session_id link. Null renders
    // PlanPanel's honest empty state until a badge has been clicked.
    chatPlanSessionId: openPlanSessionId,
    chatPlanVersion: openPlanVersion,
    onDiscardPlan: () => {
      setOpenPlanSessionId(null);
      setOpenPlanVersion(null);
    },
    chatActiveSkillId: activeSkill?.id ?? null,
    onExcludeSkill: excludeSkillAndRetry,
    chatOpenrouterSpendUsd: openrouterSpendUsd,
    chatAgentStreamItems: activeChatAgentItems,
    onOpenAgentInFlow: (agentId: string) => {
      setSelectedAgentId(agentId);
      navigateTo('flow');
    },
    chatComposer: loquelaComposer,
    gamifyEnabled: gamifySettings.enabled,
    hudTilesConfig,
    onHudTilesChange: setHudTilesConfig,
    attention,
  };

  const mainSurface = renderSurfaceContent(activeView, surfaceProps);

  const chatDock = (
    <>
      <InlineApprovals pushToast={pushToast} onViewAll={() => navigateTo('approvals')} />
      {diffOpen && (
        <DiffReview
          diff={diffText}
          loading={diffLoading}
          onClose={() => setDiffOpen(false)}
        />
      )}
      <Transcript messages={activeChatMessages} />
      {loquelaComposer}
    </>
  );

  return (
    <>
      <div className="flex h-screen flex-col">
        <BackendBanner />
        <VersionMismatchBanner mismatch={orchQuery.versionMismatch} />
        <OnboardingWizard pushToast={pushToast} gamifyEnabled={gamifySettings.enabled} />
        <AppShell
        activeView={activeView}
        onNavigate={(v) => navigateTo(v)}
        onOpenParent={openParentNav}
        onOpenTab={(v) => navigateTo(v)}
        sidebarMode={sidebarMode}
        setSidebarMode={setSidebarMode}
        agentsCount={data.agents.filter((a) => a.phase !== 'Idle').length}
        data={data}
        pushToast={pushToast}
        appVersion={appVersion}
        policyBadge={policyBadge}
        needsYouCount={attention.totalCount}
        pendingApprovals={attention.approvals.length}
        kpis={kpis}
        onOpenCommandPalette={() => setIsCommandOpen(true)}
        lastOrchEventAt={lastOrchEventAt}
        orchUsesPolling={orchUsesPolling}
        liveFreshMs={LIVE_EVENT_FRESH_MS}
        surfaceKey={activeView}
        surfaceLabel={labelForNavKey(activeView)}
        chatDocked={chatDocked}
        chatDock={chatDock}
        activeModel={activeModel}
        openrouterSpendUsd={openrouterSpendUsd}
        gamifyEnabled={gamifySettings.enabled}
        onOpenAchievements={openAchievements}
        hudTilesConfig={hudTilesConfig}
        onHudTilesChange={setHudTilesConfig}
        meshNodes={meshNodes}
        chatSessions={chatSessionsApi.sessions}
        activeSessionId={activeSessionId}
        chatTaskCounts={chatTaskCounts}
        archivedChatSessions={archivedSessions}
        showArchivedChatSessions={showArchivedSessions}
        pendingHarnessIssueSessionIds={pendingHarnessIssueSessionIds}
        onSessionChange={setActiveSessionId}
        onCreateSession={() => {
          if (creatingSessionRef.current) return;
          creatingSessionRef.current = true;
          chatSessionsApi.createSession()
            .then(s => setActiveSessionId(s.session_id))
            .catch(err => pushToast({ tone: 'warn', title: 'New session failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }))
            .finally(() => { creatingSessionRef.current = false; });
        }}
        onRenameSession={(sessionId, title) => {
          chatSessionsApi.renameSession(sessionId, title)
            .catch(err => pushToast({ tone: 'warn', title: 'Rename failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
        }}
        onArchiveSession={(sessionId) => {
          chatSessionsApi.archiveSession(sessionId, {
            wasActive: sessionId === activeSessionId,
            onReassign: setActiveSessionId,
          }).catch(err => pushToast({ tone: 'warn', title: 'Archive failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
        }}
        onUnarchiveSession={(sessionId) => {
          // Fetch the active list fresh alongside the full list rather than filtering against
          // chatSessionsApi.sessions -- that closure is a snapshot from render time and can be
          // stale by the time this .then runs, which would leave the just-unarchived session
          // still shown as archived until an unrelated re-render caught it up.
          chatSessionsApi.unarchiveSession(sessionId)
            .then(() => Promise.all([
              invoke<ChatSession[]>('chat_list_sessions', { limit: 200, includeArchived: false }),
              invoke<ChatSession[]>('chat_list_sessions', { limit: 200, includeArchived: true }),
            ]))
            .then(([active, all]) => setArchivedSessions(all.filter(s => !active.some(a => a.session_id === s.session_id))))
            .catch(err => pushToast({ tone: 'warn', title: 'Unarchive failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
        }}
        onToggleArchivedSessions={() => {
          const next = !showArchivedSessions;
          setShowArchivedSessions(next);
          if (next) {
            Promise.all([
              invoke<ChatSession[]>('chat_list_sessions', { limit: 200, includeArchived: false }),
              invoke<ChatSession[]>('chat_list_sessions', { limit: 200, includeArchived: true }),
            ])
              .then(([active, all]) => setArchivedSessions(all.filter(s => !active.some(a => a.session_id === s.session_id))))
              .catch(err => pushToast({ tone: 'warn', title: 'Load archived sessions failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
          }
        }}
        onTaskBadgeClick={(sessionId: string) => {
          invoke<string | null>('latest_plan_session_for_chat', { sessionId })
            .then(planSessionId => {
              // A null result (badge showed a stale nonzero count, or the session's plan
              // was archived/retracted between render and click) is a silent no-op by
              // design -- there is nothing to open, and it isn't an error worth a toast.
              if (planSessionId) {
                setOpenPlanSessionId(planSessionId);
                setActiveSessionId(sessionId);
              }
            })
            .catch(err => pushToast({ tone: 'warn', title: 'Open tasks failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
        }}
      >
        {mainSurface}
        </AppShell>
      </div>

      <AchievementsDrawer
        open={achievementsOpen}
        onClose={closeAchievements}
        profile={ludusProfile}
        onManageInSettings={manageGamifyInSettings}
      />

      <Omnibar
        open={isCommandOpen}
        onClose={() => setIsCommandOpen(false)}
        onNavigate={(vk, anchorId) => {
          navigateTo(vk);
          if (anchorId) {
            requestAnimationFrame(() => {
              scrollAndFocusAnchor(anchorId);
            });
          }
        }}
        onRunCommand={(command) => {
          // 'skill:<id>' commands (Omnibar's skill rows, see Omnibar.tsx's
          // `onRunCommand(`skill:${row.activate.skillId}`)`) must reach
          // handleCommandAction's `cmd.id.startsWith('skill:')` branch below —
          // wrapping them in the generic 'hit' shape like every other command
          // string would hardcode id/type to 'hit' and short-circuit past that
          // branch in the if/else-if chain, silently no-op'ing skill deploys.
          if (command.startsWith('skill:')) {
            handleCommandAction({ id: command });
            return;
          }
          handleCommandAction({
            id: 'hit',
            type: 'hit',
            locator: { kind: 'command', value: command },
            viewKey: 'console',
          });
        }}
        onSubmitTask={() => handleSubmitTaskAction(navigateTo, focusComposer)}
        onSendToChat={(query) => {
          navigateTo('chat');
          handleLoquelaSubmit({ description: query, session_id: activeSessionId, execution_mode: 'chat' });
        }}
        onOpenDoc={(path) => openDocTab(path)}
        agents={data.agents}
        skills={installedSkillEntries}
        gamifyEnabled={gamifySettings.enabled}
      />

      <DocViewerDrawer doc={activeDoc} onClose={closeDocViewer} />

      <Toasts
        items={toasts}
        onClose={id => setToasts(curr => curr.filter(x => x.id !== id))}
      />

      {achievementToasts.toasts.length > 0 && (
        <div
          // z-[80]: must render above every modal/overlay in the app, including
          // OnboardingWizard's z-[70] backdrop — otherwise an error toast fired
          // while the wizard is open (e.g. BudgetSetupScreen.save() failing)
          // renders invisibly underneath it. Toasts are terminal user feedback
          // and should never be hidden behind any overlay.
          className="pointer-events-none fixed bottom-4 right-4 z-[80] flex max-w-sm flex-col gap-2"
          aria-live="polite"
          aria-label="Achievement notifications"
        >
          {achievementToasts.toasts.map((toast) => (
            <AchievementToast
              key={toast.id}
              title={toast.title}
              body={toast.body}
              autoDismissMs={4000}
              onDismiss={() => achievementToasts.dismissToast(toast.id)}
            />
          ))}
        </div>
      )}
    </>
  );
}
