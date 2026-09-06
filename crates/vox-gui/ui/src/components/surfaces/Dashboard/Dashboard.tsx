import React, { useState, useEffect, useRef } from 'react';
import { LudusSandbox } from '../../gamify/LudusSandbox';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { StreamCard } from './StreamCard';
import { AgentRow } from './AgentRow';
import { LudusBanner } from './LudusBanner';
import { DashboardData, Agent, StreamItem, LudusAlert } from '../../../types/dashboard';
import { Skeleton } from '../../ui/Skeleton';
import { defaultDashboardLayout, type DashboardWidget, addWidgetToLayout, resetDashboardLayout } from '../../../lib/dashboardLayout';
import { DashboardGrid, persistDashboardLayout } from '../../dashboard/DashboardGrid';
import { WidgetPickerDrawer } from '../../dashboard/WidgetPickerDrawer';
import { useLocalStorage } from '../../../hooks/useLocalStorage';
import { labelFor } from '../../../lib/lexicon';
import { useLang } from '../../../hooks/useLanguage';
import { SHELL_PREFERENCE_KEYS } from '../../../lib/shellPersistence';
import { loadDashboardLayout } from '../../dashboard/DashboardGrid';
import {
  useMetricSeries,
  metricSeriesFromSpark,
  shouldAppendMetricFromKpi,
} from '../../../hooks/useMetricSeries';
import { AreaChartWidget } from '../../dashboard/widgets/AreaChartWidget';
import { LineChartWidget } from '../../dashboard/widgets/LineChartWidget';
import { BarChartWidget } from '../../dashboard/widgets/BarChartWidget';
import { Kpi } from '../../ui/Kpi';
import { AttentionBudgetMeter } from '../AttentionBudgetMeter';
import type { AttentionBudgetSnapshot } from '../../../types/tauri';
import { ResourcesWidget } from './ResourcesWidget';
import { useAgentApprovals } from '../../../hooks/useAgentApprovals';
import { surfaceKeyOf } from '../../../lib/dashboardLayout';
import { resolveWidget } from '../../dashboard/dashboardWidgetRegistry';
import { SurfaceMiniRender } from '../../dashboard/SurfaceMiniRender';
import { WidgetErrorBoundary } from '../../dashboard/WidgetErrorBoundary';
import { childRenderer, type SurfaceProps } from '../../layout/surfaceComponents';
import type { Toast } from '../../../types/tauri';

/** Consistent empty-state hint for a panel with no data yet. */
function EmptyHint({ icon, title, hint }: { icon?: React.ReactNode; title: string; hint?: string }) {
  return (
    <div role="status" className="flex flex-col items-center justify-center gap-1.5 rounded-lg border border-dashed border-border-subtle py-8 text-center">
      {icon && <div className="text-text-muted">{icon}</div>}
      <div className="text-[12px] text-text-muted">{title}</div>
      {hint && <div className="text-[11px] text-text-muted">{hint}</div>}
    </div>
  );
}

interface DashboardProps {
  data: DashboardData;
  loading?: boolean;
  onPause: (a: Agent) => void;
  onResume: (a: Agent) => void;
  onDoubt?: (item: StreamItem) => void;
  onOverrule?: (item: StreamItem) => void;
  onAckLudus: (note: LudusAlert) => void;
  filterKind: string;
  setFilterKind: (k: string) => void;
  onOpenInConsole?: (a: Agent) => void;
  onOpenChat?: () => void;
  onNavigate?: (viewKey: string) => void;
  attention_budget?: AttentionBudgetSnapshot | null;
  /** Real toast handler, threaded into mini-rendered surfaces (which call pushToast). */
  pushToast?: (toast: Toast) => void;
}

export function Dashboard({
  data,
  loading = false,
  onPause,
  onResume,
  onDoubt,
  onOverrule,
  onAckLudus,
  filterKind,
  setFilterKind,
  onOpenInConsole,
  onOpenChat,
  onNavigate,
  attention_budget,
  pushToast,
}: DashboardProps) {
  const { lang } = useLang();
  const filters = ["all", "validated", "in-progress", "doubted", "speculative"];
  const stream = data.stream.filter(s => filterKind === "all" ? true : s.kind === filterKind);
  const [customizeMode, setCustomizeMode] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [sandboxCollapsed, setSandboxCollapsed] = useState(false);
  const [layout, setLayout] = useLocalStorage(
    SHELL_PREFERENCE_KEYS.dashboardLayout,
    loadDashboardLayout(defaultDashboardLayout()),
  );
  const approvals = useAgentApprovals(data.agents.map((a) => a.codename));
  const { series: budgetSeries, setSeries: setBudgetSeries, append: appendBudget } =
    useMetricSeries('budget_burn', []);
  const { series: queueSeries, setSeries: setQueueSeries, append: appendQueue } =
    useMetricSeries('queue_depth', []);
  const prevBudgetValue = useRef<number | undefined>(undefined);
  const prevQueueValue = useRef<number | undefined>(undefined);

  const budgetSpark = data.kpis.budgetBurn.spark;
  const budgetValue =
    typeof data.kpis.budgetBurn.value === 'number' ? data.kpis.budgetBurn.value : 0;
  const queueSpark = data.kpis.queueDepth.spark;
  const queueValue = data.kpis.queueDepth.value;

  useEffect(() => {
    if (budgetSpark.length > 0) {
      setBudgetSeries(metricSeriesFromSpark(budgetSpark));
      prevBudgetValue.current = budgetValue;
    }
  }, [budgetSpark, budgetValue, setBudgetSeries]);

  useEffect(() => {
    if (shouldAppendMetricFromKpi(budgetSpark, budgetValue, prevBudgetValue.current)) {
      prevBudgetValue.current = budgetValue;
      appendBudget(budgetValue);
    }
  }, [budgetSpark, budgetValue, appendBudget]);

  useEffect(() => {
    if (queueSpark.length > 0) {
      setQueueSeries(metricSeriesFromSpark(queueSpark));
      prevQueueValue.current = queueValue;
    }
  }, [queueSpark, queueValue, setQueueSeries]);

  useEffect(() => {
    if (shouldAppendMetricFromKpi(queueSpark, queueValue, prevQueueValue.current)) {
      prevQueueValue.current = queueValue;
      appendQueue(queueValue);
    }
  }, [queueSpark, queueValue, appendQueue]);

  function updateLayout(next: ReturnType<typeof defaultDashboardLayout>) {
    setLayout(next);
    persistDashboardLayout(next);
  }

  function handleAddWidget(kind: Parameters<typeof addWidgetToLayout>[1]) {
    updateLayout(addWidgetToLayout(layout, kind));
    setPickerOpen(false);
  }

  function handleAddSurface(surfaceKey: string) {
    const next = addWidgetToLayout(layout, 'surface_widget');
    const placed = {
      ...next,
      widgets: next.widgets.map((w, i) =>
        i === next.widgets.length - 1 ? { ...w, config: { ...(w.config ?? {}), surfaceKey } } : w,
      ),
    };
    updateLayout(placed);
    setPickerOpen(false);
  }

  function handleResetLayout() {
    updateLayout(resetDashboardLayout());
    setPickerOpen(false);
  }

  function chartTitle(widget: DashboardWidget, fallback: string): string {
    const configured = widget.config?.title;
    return typeof configured === 'string' && configured.trim() !== '' ? configured : fallback;
  }

  function surfaceLabel(surfaceKey: string): string {
    // synthetic 'cost' has no registry row → give it a real label, not the raw key
    if (surfaceKey === 'cost') return 'OpenRouter Spend';
    return labelFor(surfaceKey, lang);
  }

  // Build the SurfaceProps bag the mini-render mounts real surfaces with. Closes
  // over the Dashboard's destructured props; pushToast must be a real function
  // (mini-rendered surfaces call it during render/effects), so fall back to a
  // no-op handler when the Dashboard was not given one (honest: no fabricated data).
  // Props for an embedded mini-render. The action callbacks are deliberately
  // INERT no-ops: a thumbnail must never pause/resume an agent, doubt/overrule a
  // task, ack a Ludus note, or push a toast. (The mini still issues read-only
  // mount polls — see SurfaceMiniRender's docstring.) Click-through to the live
  // surface, where the real callbacks apply, is the parent's job (onOpen).
  const noop = () => {};
  function miniPropsFor(): SurfaceProps {
    return {
      data,
      pushToast: noop,
      onPause: noop,
      onResume: noop,
      onDoubt: noop,
      onOverrule: noop,
      onAckLudus: noop,
      filterKind,
      setFilterKind,
      onOpenInConsole: noop,
      onNavigate,
      attention_budget,
    } as SurfaceProps;
  }

  function renderSurfaceWidget(surfaceKey: string): React.ReactNode {
    const resolved = resolveWidget(surfaceKey);
    if (resolved.kind === 'purpose-built') {
      return <resolved.Component data={data} />;
    }
    return (
      <SurfaceMiniRender surfaceKey={surfaceKey} label={surfaceLabel(surfaceKey)}>
        {childRenderer(miniPropsFor(), surfaceKey)}
      </SurfaceMiniRender>
    );
  }

  function renderWidget(widget: DashboardWidget) {
    switch (widget.kind) {
      case 'stream':
        return (
          <Glass className="h-full p-5">
            <div className="flex items-center justify-between">
              <div>
                <div className="flex items-center gap-3">
                  <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">The Stream</h2>
                  <span className="rounded-full border border-border-subtle bg-overlay-subtle px-2 py-0.5 font-mono text-[10px] text-text-muted">{stream.length} events</span>
                </div>
                <p className="mt-0.5 text-[11px] text-text-muted">Mission-control feed · live agent telemetry</p>
              </div>
              <div className="flex gap-1 rounded-lg border border-border-subtle bg-overlay-subtle p-1">
                {filters.map(f => (
                  <button
                    key={f}
                    type="button"
                    aria-pressed={filterKind === f}
                    onClick={() => setFilterKind(f)}
                    className={`rounded-md px-2.5 py-1 text-[10px] font-display uppercase tracking-wider transition ${
                      filterKind === f ? "bg-overlay-subtle text-text-primary" : "text-text-muted hover:text-text-secondary"
                    }`}
                  >
                    {f === "in-progress" ? "in-prog" : f}
                  </button>
                ))}
              </div>
            </div>
            <div className="mt-4 flex flex-col gap-2.5">
              {stream.length === 0 ? (
                <EmptyHint
                  icon={<Icon.flow className="size-5" />}
                  title={filterKind === 'all' ? 'No events yet' : `No ${filterKind} events`}
                  hint="Live agent telemetry streams here once tasks run. Open Chat to submit a task."
                />
              ) : (
                stream.map(s => <StreamCard key={s.id} item={s} onDoubt={onDoubt} onOverrule={onOverrule} />)
              )}
            </div>
          </Glass>
        );
      case 'alerts':
        return (
          <Glass className="h-full p-5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Icon.alert className="size-4 text-amber-300" />
                <h3 className="font-display text-[14px] font-semibold tracking-wide text-text-primary">System · Telemetry & Alerts</h3>
              </div>
              <span className="font-mono text-[10px] text-text-muted">{data.alerts.length} open</span>
            </div>
            <div className="mt-3 flex flex-col gap-2">
              {data.alerts.length === 0 ? (
                <div className="rounded-lg border border-dashed border-border-subtle py-4 text-center text-[11px] text-text-muted">
                  All clear — no open alerts.
                </div>
              ) : (
                data.alerts.map(n => <LudusBanner key={n.id} note={n} onAck={onAckLudus} />)
              )}
            </div>
          </Glass>
        );
      case 'agents':
        return (
          <Glass className="h-full p-5">
            <div className="flex items-center justify-between">
              <h3 className="font-display text-[14px] font-semibold tracking-wide text-text-primary">Active Agents</h3>
              <span className="font-mono text-[10px] text-text-muted">{data.agents.length} shards</span>
            </div>
            <div className="mt-3 flex flex-col gap-2">
              {data.agents.length === 0 ? (
                <div className="rounded-lg border border-dashed border-border-subtle py-4 text-center text-[11px] text-text-muted">
                  No active agents — open Chat to submit a task.
                </div>
              ) : (
                data.agents.map((a) => {
                  const ap = approvals.approvalFor(a.codename);
                  return (
                    <AgentRow key={a.id} a={a} onPause={onPause} onResume={onResume} onOpenInConsole={onOpenInConsole}
                      pendingApprovalId={ap?.approval_id ?? null}
                      onApprove={(id) => approvals.resolve(id, 'approved')}
                      onReject={(id) => approvals.resolve(id, 'rejected')} />
                  );
                })
              )}
            </div>
          </Glass>
        );
      case 'budget_burn':
        return (
          <AreaChartWidget
            title="Budget Burn"
            series={budgetSeries}
          />
        );
      case 'queue_depth':
        return (
          <BarChartWidget
            title="Queue Depth"
            series={queueSeries}
          />
        );
      case 'line_chart':
        return (
          <LineChartWidget
            title={chartTitle(widget, 'Metric')}
            series={budgetSeries}
          />
        );
      case 'bar_chart':
        return (
          <BarChartWidget
            title={chartTitle(widget, 'Metric')}
            series={queueSeries}
          />
        );
      case 'area_chart':
        return (
          <AreaChartWidget
            title={chartTitle(widget, 'Metric')}
            series={budgetSeries}
          />
        );
      case 'resources':
        return <ResourcesWidget data={data} />;
      case 'surface_widget': {
        const key = surfaceKeyOf(widget);
        if (!key) return null;
        return (
          <WidgetErrorBoundary label={surfaceLabel(key)}>
            {renderSurfaceWidget(key)}
          </WidgetErrorBoundary>
        );
      }
      default:
        return null;
    }
  }

  if (loading) {
    return (
      <div className="grid grid-cols-12 gap-5 p-5" role="status" aria-label="Loading dashboard">
        {/* Axe page-has-heading-one: keep the accessible root heading present
            in the loading state too, matching the loaded-state h1 below. */}
        <h1 className="sr-only">Dashboard</h1>
        <Glass className="col-span-12 xl:col-span-8 p-5">
          <Skeleton className="h-6 w-40 mb-4" />
          <Skeleton className="h-24 w-full mb-2" />
          <Skeleton className="h-24 w-full" />
        </Glass>
        <Glass className="col-span-12 xl:col-span-4 p-5">
          <Skeleton className="h-6 w-32 mb-4" />
          <Skeleton className="h-16 w-full mb-2" />
          <Skeleton className="h-16 w-full" />
        </Glass>
      </div>
    );
  }

  return (
    <div className="relative">
      {/* Axe page-has-heading-one: surface renders inside a heading-less shell.
          "The Stream" below is a section heading, not the surface root title. */}
      <h1 className="sr-only">Dashboard</h1>
      {onOpenChat && (
        <div className="mx-5 mb-4 mt-2 flex items-center justify-between gap-4 rounded-xl border border-indigo-500/20 bg-indigo-500/6 px-4 py-3">
          <div>
            <p className="font-display text-[13px] font-semibold text-text-primary">Submit tasks in Chat</p>
            <p className="mt-0.5 text-[11px] text-text-muted">The Loquela composer lives on the Chat surface — open it to describe work and spin up agents.</p>
          </div>
          <button
            type="button"
            data-testid="open-chat-cta"
            onClick={onOpenChat}
            className="shrink-0 rounded-lg border border-indigo-400/30 bg-indigo-500/20 px-4 py-2 font-display text-[12px] font-semibold tracking-wide text-indigo-100 transition hover:bg-indigo-500/30"
          >
            Open Chat
          </button>
        </div>
      )}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-4 px-5">
        <Kpi label="Active Agents" value={data.agents.length} accent="cyan" />
        <Kpi label="Queue Depth" value={data.kpis.queueDepth.value} accent="amber" />
        <Kpi label="Budget Spent" value={typeof data.kpis.budgetBurn.value === 'number' ? `$${data.kpis.budgetBurn.value.toFixed(2)}` : data.kpis.budgetBurn.value} accent="brass" />
        <Kpi label="Mesh Peers" value={data.peers.filter((p) => p.online).length} accent="emerald" />
      </div>
      {attention_budget && (
        <div className="mb-4 px-5">
          <AttentionBudgetMeter budget={attention_budget} />
        </div>
      )}
      <div className="absolute right-5 top-2 z-20 flex items-center gap-2">
        {customizeMode && (
          <>
            <button
              type="button"
              aria-expanded={pickerOpen}
              onClick={() => setPickerOpen((v) => !v)}
              className="rounded-md border border-border-subtle bg-bg-base/80 px-2.5 py-1 text-[11px] text-text-secondary hover:bg-overlay-subtle"
            >
              Add widget
            </button>
            <button
              type="button"
              onClick={handleResetLayout}
              className="rounded-md border border-border-subtle bg-bg-base/80 px-2.5 py-1 text-[11px] text-text-secondary hover:bg-overlay-subtle"
            >
              Reset to default
            </button>
          </>
        )}
        <button
          type="button"
          aria-expanded={customizeMode}
          aria-pressed={customizeMode}
          onClick={() => {
            setCustomizeMode((v) => {
              const next = !v;
              if (!next) {
                setPickerOpen(false);
              }
              return next;
            });
          }}
          className="rounded-md border border-border-subtle bg-bg-base/80 px-2.5 py-1 text-[11px] text-text-secondary hover:bg-overlay-subtle"
        >
          {customizeMode ? 'Done customizing' : 'Customize dashboard'}
        </button>
      </div>

      {/* Workspace Simulation Mini-Map */}
      <div className="mx-5 mb-4 border border-border-subtle bg-[#09090b]/80 rounded-xl overflow-hidden">
        <div className="flex items-center justify-between bg-bg-base/60 px-4 py-2 text-xs border-b border-border-subtle">
          <span className="font-semibold text-text-primary uppercase tracking-wide">⬤ Workspace Simulation Mini-Map</span>
          <div className="flex gap-2">
            <button type="button" onClick={() => onNavigate?.('gamify')} className="text-cyan hover:underline">Immersive View</button>
            <button type="button" onClick={() => setSandboxCollapsed(!sandboxCollapsed)} className="text-text-muted hover:text-text-secondary">
              {sandboxCollapsed ? 'Expand' : 'Collapse'}
            </button>
          </div>
        </div>
        {!sandboxCollapsed && (
          <div className="h-[250px] relative">
            <LudusSandbox />
          </div>
        )}
      </div>

      <WidgetPickerDrawer
        layout={layout}
        open={customizeMode && pickerOpen}
        onClose={() => setPickerOpen(false)}
        onAdd={handleAddWidget}
        onAddSurface={handleAddSurface}
      />
      <DashboardGrid
        layout={layout}
        customizeMode={customizeMode}
        onLayoutChange={updateLayout}
        renderWidget={renderWidget}
      />
    </div>
  );
}
