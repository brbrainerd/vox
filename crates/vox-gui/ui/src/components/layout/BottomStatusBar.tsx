import React, { useEffect, useRef, useState } from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { formatBudgetCap } from '../../config/budget';
import { useFreshness } from '../../hooks/useFreshness';
import {
  resolveVisibleHudTiles,
  toggleHudTile,
  setHudOption,
  HUD_TILE_LABELS,
  HUD_DENSITIES,
  defaultHudOptions,
  type HudTilesConfig,
  type HudTileKind,
  type HudDensity,
} from '../../hooks/useHudTiles';
import type { LlmSpendState } from '../../hooks/useLlmSpend';
import { INITIAL_KPIS } from '../../data/initialState';
import { WORKBENCH_TABBAR_TRAILING_SLOT_ID } from '../../lib/domIds';
import type { MeshNode } from '../surfaces/Mesh/MeshView';

type KpiState = typeof INITIAL_KPIS;

export interface BottomStatusBarProps {
  kpis: KpiState;
  hudTilesConfig: HudTilesConfig;
  onHudTilesChange: (config: HudTilesConfig) => void;
  onNavigate: (view: string) => void;
  lastOrchEventAt: number | null;
  orchUsesPolling: boolean;
  liveFreshMs: number;
  activeModel?: string | null;
  /** Full spend + caps payload; see useLlmSpend. */
  llmSpend?: LlmSpendState | null;
  buildDisplay?: string | null;
  pendingApprovals?: number | null;
  meshNodes?: MeshNode[];
  gamifyEnabled?: boolean;
  onOpenAchievements?: () => void;
}

function freshnessClasses(tone: 'live' | 'poll' | 'stale') {
  if (tone === 'live') {
    return {
      pill: 'border-status-pass/20 bg-status-pass/[0.06] text-status-pass',
      dot: 'bg-status-pass',
      label: 'Live',
    };
  }
  if (tone === 'poll') {
    return {
      pill: 'border-status-warn/20 bg-status-warn/[0.06] text-status-warn',
      dot: 'bg-status-warn',
      label: 'Poll',
    };
  }
  return {
    pill: 'border-border-subtle bg-overlay-subtle text-text-muted',
    dot: 'bg-text-muted',
    label: 'Offline',
  };
}

/** Severity of a value relative to the cap that governs it. */
export type SegmentTone = 'ok' | 'warn' | 'over' | 'error';

/**
 * Tone -> theme token class. Every entry resolves through `--color-status-*`,
 * which Style Dictionary emits into all three theme scopes, so the bar follows
 * [data-theme]. Hardcoded Tailwind literals (emerald-400/amber-400) do not:
 * they stay put under high-contrast, which is what the visual review kept
 * flagging as a contrast failure on this bar.
 */
const TONE_CLASS: Record<SegmentTone, string> = {
  ok: 'text-text-secondary',
  warn: 'text-status-warn',
  over: 'text-status-fail',
  error: 'text-status-fail',
};

/**
 * Classify `spent` against `cap`. `null` cap means "no cap configured" — not a
 * pass; the value is simply uncapped, so it stays neutral rather than being
 * scored against a number nobody set.
 */
export function toneForSpend(
  spent: number | null,
  cap: number | null,
  warnThresholdPct: number | null,
): SegmentTone {
  if (spent == null || cap == null || cap <= 0) return 'ok';
  if (spent >= cap) return 'over';
  const threshold = warnThresholdPct ?? 1;
  // Mirrors budget_guard's proportional tolerance: warnThresholdPct round-trips
  // through f32 on the Rust side, so an exact-cent spend can land a few ULPs
  // under the "true" threshold and miss the warning.
  if (spent >= cap * threshold - Math.abs(cap) * 1e-6) return 'warn';
  return 'ok';
}

function Segment({
  testId,
  label,
  value,
  onClick,
  density,
  tone = 'ok',
  title,
}: {
  testId: string;
  label: string;
  value: string;
  onClick: () => void;
  density: HudDensity;
  tone?: SegmentTone;
  title?: string;
}) {
  return (
    <button
      type="button"
      data-testid={testId}
      data-tone={tone}
      title={title}
      aria-label={density === 'labeled' ? undefined : `${label}: ${value}`}
      onClick={onClick}
      className={`inline-flex items-center gap-1.5 rounded px-2 py-0.5 text-[10px] text-text-muted hover:bg-overlay-subtle hover:text-text-secondary transition ${TONE_CLASS[tone]}`}
    >
      {density === 'labeled' && (
        <span className="uppercase tracking-[0.14em] text-text-muted">{label}</span>
      )}
      <span className={`font-mono tabular-nums ${tone === 'ok' ? 'text-text-secondary' : ''}`}>
        {value}
      </span>
    </button>
  );
}

export function BottomStatusBar({
  kpis,
  hudTilesConfig,
  onHudTilesChange,
  onNavigate,
  lastOrchEventAt,
  orchUsesPolling,
  liveFreshMs,
  activeModel = null,
  llmSpend = null,
  buildDisplay = null,
  pendingApprovals = null,
  meshNodes,
  gamifyEnabled = false,
  onOpenAchievements,
}: BottomStatusBarProps) {
  const tone = useFreshness(lastOrchEventAt, {
    freshMs: liveFreshMs,
    usesPolling: orchUsesPolling,
  });
  const fresh = freshnessClasses(tone);
  const visible = resolveVisibleHudTiles(hudTilesConfig);
  // v1 configs migrate on read, but a hand-built config object in a test or a
  // caller that predates v2 can still arrive without options.
  const options = hudTilesConfig.options ?? defaultHudOptions();
  const density = options.density;

  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const onOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (menuRef.current?.contains(target)) return;
      if (triggerRef.current?.contains(target)) return;
      setMenuOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setMenuOpen(false);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener('mousedown', onOutside);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onOutside);
      document.removeEventListener('keydown', onKey);
    };
  }, [menuOpen]);

  const budgetSource = kpis.budgetBurn?.source ?? 'fallback';
  const capDisplay = formatBudgetCap(
    budgetSource === 'daemon' ? kpis.budgetBurn.cap : null,
    budgetSource,
  );
  const budgetValue = `$${kpis.budgetBurn.value.toFixed(2)}/${capDisplay}`;

  const money = (v: number | null) => (v == null ? '—' : `$${v.toFixed(2)}`);
  const warnPct = llmSpend?.warnThresholdPct ?? null;
  const spendTone: SegmentTone = llmSpend?.error
    ? 'error'
    : toneForSpend(llmSpend?.dayUsd ?? null, llmSpend?.dailyBudgetUsd ?? null, warnPct);
  const sessionTone: SegmentTone = llmSpend?.error
    ? 'error'
    : toneForSpend(llmSpend?.sessionUsd ?? null, llmSpend?.perSessionBudgetUsd ?? null, warnPct);

  const renderSegment = (kind: HudTileKind): React.ReactNode => {
    switch (kind) {
      case 'active_agents':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-agents"
            density={density}
            label="Agents"
            value={String(kpis.activeAgents.value)}
            onClick={() => onNavigate('agents')}
          />
        );
      case 'queue_depth':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-queue"
            density={density}
            label="Queue"
            value={String(kpis.queueDepth.value)}
            onClick={() => onNavigate('runs')}
          />
        );
      case 'budget_burn':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-budget"
            label="Run Cap"
            value={budgetValue}
            density={density}
            title="Orchestrator per-run financial cost cap (financial_cost_budget_micros). Distinct from LLM Spend, which is the daily cross-provider spend cap."
            onClick={() => onNavigate('settings')}
          />
        );
      case 'mesh_peers': {
        // Keep the trailing "online" wording stable across both states so the
        // segment doesn't visibly change shape once richer node data loads —
        // before meshNodes arrives we don't know the online/offline split,
        // so show the peer count as a single figure rather than switching
        // from "N peers" to "X/Y online" (two different phrasings for what
        // reads as the same kind of number).
        const onlineCount = meshNodes?.filter((n) => n.status === 'online').length ?? 0;
        const totalCount = meshNodes?.length ?? 0;
        const meshValue =
          meshNodes == null ? `${kpis.mesh.peers} online` : `${onlineCount}/${totalCount} online`;
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-mesh"
            density={density}
            label="Mesh"
            value={meshValue}
            onClick={() => onNavigate('mesh')}
          />
        );
      }
      case 'active_model':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-model"
            density={density}
            label="Model"
            value={activeModel ?? 'auto-route'}
            onClick={() => onNavigate('models')}
          />
        );
      case 'openrouter_spend':
        // Daily spend against daily_budget_usd — the pair the budget guard
        // actually blocks dispatch on. This tile previously showed *lifetime*
        // spend against no cap, which said nothing about whether the next
        // request would be refused. Named "LLM Spend" because
        // `llm_spend_summary` sums every provider, not just OpenRouter.
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-openrouter"
            label="LLM Spend"
            value={
              llmSpend?.error
                ? '!'
                : `${money(llmSpend?.dayUsd ?? null)}/${money(llmSpend?.dailyBudgetUsd ?? null)}`
            }
            tone={spendTone}
            density={density}
            title={
              llmSpend?.error
                ? `LLM spend unavailable: ${llmSpend.error}`
                : `Today's spend across all providers vs daily_budget_usd. Lifetime: ${money(llmSpend?.totalUsd ?? null)}.`
            }
            onClick={() => onNavigate('settings')}
          />
        );
      case 'session_spend':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-session-spend"
            label="Session"
            value={
              llmSpend?.error
                ? '!'
                : `${money(llmSpend?.sessionUsd ?? null)}/${money(llmSpend?.perSessionBudgetUsd ?? null)}`
            }
            tone={sessionTone}
            density={density}
            title="This session's LLM spend vs per_session_budget_usd."
            onClick={() => onNavigate('settings')}
          />
        );
      case 'vram_total':
        // total_vram_gb has always been fetched from the orchestrator status
        // and plumbed into kpis.mesh.vramGb; nothing rendered it until now.
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-vram"
            label="VRAM"
            value={`${kpis.mesh.vramGb ?? 0} GB`}
            density={density}
            title="Total VRAM reported across mesh peers."
            onClick={() => onNavigate('mesh')}
          />
        );
      case 'build_version':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-build"
            label="Build"
            value={buildDisplay ?? '—'}
            density={density}
            onClick={() => onNavigate('settings')}
          />
        );
      case 'pending_approvals':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-approvals"
            density={density}
            label="Approvals"
            value={String(pendingApprovals ?? 0)}
            onClick={() => onNavigate('approvals')}
          />
        );
      default:
        return null;
    }
  };

  return (
    <Glass
      data-testid="bottom-status-bar"
      role="status"
      aria-label="Operator status"
      className="flex h-7 w-full items-center gap-1 p-0 px-3 rounded-none border-x-0 border-b-0 shadow-none text-[10px] text-text-muted"
    >
      <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
        {visible.map((kind) => renderSegment(kind))}
      </div>
      {gamifyEnabled && onOpenAchievements && (
        <button
          type="button"
          data-testid="achievements-trigger"
          aria-label="Open achievements"
          onClick={onOpenAchievements}
          className="inline-flex shrink-0 items-center justify-center rounded px-1.5 py-0.5 text-amber-300/80 hover:bg-overlay-subtle hover:text-amber-200 transition"
        >
          <Icon.trophy className="size-3.5" aria-hidden="true" />
        </button>
      )}
      <div className="relative shrink-0">
        <button
          ref={triggerRef}
          type="button"
          onClick={() => setMenuOpen((o) => !o)}
          aria-expanded={menuOpen}
          aria-label="Configure status bar"
          className="rounded px-1.5 py-0.5 text-[10px] text-text-muted hover:bg-overlay-subtle hover:text-text-secondary transition"
        >
          Configure ▾
        </button>
        {menuOpen ? (
          <div
            ref={menuRef}
            className="absolute bottom-full right-0 z-50 mb-1 w-56 rounded-lg border border-border-subtle bg-bg-base p-2 shadow-2xl"
          >
            {hudTilesConfig.tiles.map((tile) => (
              <label
                key={tile.id}
                className="flex items-center gap-2 rounded px-2 py-1 text-[11px] text-text-secondary hover:bg-overlay-subtle"
              >
                <input
                  type="checkbox"
                  checked={tile.enabled}
                  onChange={(e) =>
                    onHudTilesChange(toggleHudTile(hudTilesConfig, tile.id, e.target.checked))
                  }
                  className="rounded border-border-subtle bg-bg-base text-brass focus:ring-brass/40 focus:ring-offset-bg-base size-3.5"
                />
                {HUD_TILE_LABELS[tile.kind]}
              </label>
            ))}

            <div className="my-1 h-px bg-border-subtle" />

            <label className="flex items-center justify-between gap-2 rounded px-2 py-1 text-[11px] text-text-secondary hover:bg-overlay-subtle">
              Density
              <select
                aria-label="Status bar density"
                value={options.density}
                onChange={(e) =>
                  onHudTilesChange(setHudOption(hudTilesConfig, 'density', e.target.value as HudDensity))
                }
                className="rounded border border-border-subtle bg-bg-base px-1 py-0.5 font-mono text-[10px] text-text-secondary"
              >
                {HUD_DENSITIES.map((d) => (
                  <option key={d} value={d}>{d.replace('_', ' ')}</option>
                ))}
              </select>
            </label>

            <label className="flex items-center gap-2 rounded px-2 py-1 text-[11px] text-text-secondary hover:bg-overlay-subtle">
              <input
                type="checkbox"
                checked={options.showFreshness}
                onChange={(e) =>
                  onHudTilesChange(setHudOption(hudTilesConfig, 'showFreshness', e.target.checked))
                }
                className="rounded border-border-subtle bg-bg-base text-brass focus:ring-brass/40 focus:ring-offset-bg-base size-3.5"
              />
              Freshness pill
            </label>

            <p className="px-2 pt-1 text-[10px] leading-snug text-text-muted">
              Budget caps live in Settings → Runtime (they govern dispatch, not just display).
            </p>
          </div>
        ) : null}
      </div>
      {options.showFreshness && (
        <div
          data-testid="bottom-status-bar-freshness"
          className={`ml-auto inline-flex shrink-0 items-center gap-1.5 rounded border px-2 py-0.5 ${fresh.pill}`}
        >
          <span className={`size-1.5 rounded-full ${fresh.dot}`} />
          <span className="uppercase tracking-[0.14em]">{fresh.label}</span>
        </div>
      )}

      {/* Fixed home for surface-level chrome that needs to sit inline with
          persistent app chrome rather than in the per-surface content area
          (e.g. Chat's "Panels ▾" dock-visibility menu, portaled in here from
          ChatSurface). BottomStatusBar is a single, non-wrapping row rendered
          once in the app shell's footer — unlike WorkbenchTabBar's tablist,
          it never grows to multiple lines as more tabs open, so anything
          docked here stays put and reachable regardless of how many
          top-level tabs are open or how the tab bar wraps. It's also
          independent of the tab bar's own lifecycle: the tab bar is slated
          for eventual removal, this slot is not. `shrink-0` (and the KPI
          segments' own scroll region above) keeps it pinned at the right
          edge even when the window itself is too narrow to fit everything. */}
      <div
        id={WORKBENCH_TABBAR_TRAILING_SLOT_ID}
        data-testid={WORKBENCH_TABBAR_TRAILING_SLOT_ID}
        className="ml-2 flex shrink-0 items-center"
      />
    </Glass>
  );
}
