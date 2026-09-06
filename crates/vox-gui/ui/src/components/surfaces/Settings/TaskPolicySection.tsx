import { useCallback, useEffect, useState } from 'react';
import { voxTransport, type TaskPolicyEntry, type TaskPolicyOverrides } from '../../../transport';

const CLUTCH_OPTIONS = ['free', 'efficiency', 'balanced', 'genius'];
const RISK_OPTIONS = ['high', 'moderate', 'low'];

// Mirrors contracts/orchestration/model-routing.v1.yaml::task_categories + the
// codegen'd #[default] General variant. Hand-maintained, like driveConsole.ts's
// CLUTCH_DETENTS/RISK_POSTURES — update if that YAML's category list changes.
const ALL_CATEGORIES = [
  'General', 'CodeGen', 'Testing', 'Debugging', 'TypeChecking', 'Research',
  'Parsing', 'Review', 'Ars', 'Planning', 'InterAgent', 'ToolOrchestration',
  'Visus', 'CodeEffortJudge', 'Chat',
];
// Mirrors crate::mode::TriggerSource's four variants exactly (Debug-style names).
const ALL_SOURCES = ['Interactive', 'Automated', 'Subagent', 'Mesh'];

const BTN =
  'rounded-sm border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle disabled:opacity-40';
const SEL =
  'rounded-sm border border-border-subtle bg-black/30 px-2 py-1 font-mono text-[11px] text-text-primary focus:border-brass/40 focus:outline-hidden';

export function TaskPolicySection() {
  const [overrides, setOverrides] = useState<TaskPolicyOverrides>({ category: {}, source: {} });
  const [addScope, setAddScope] = useState('');

  const refresh = useCallback(() => {
    voxTransport.getTaskPolicyOverrides().then(setOverrides);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const rows: Array<{ scopeKind: 'category' | 'source'; scopeKey: string; entry: TaskPolicyEntry }> = [
    ...Object.entries(overrides.category).map(([scopeKey, entry]) => ({
      scopeKind: 'category' as const,
      scopeKey,
      entry,
    })),
    ...Object.entries(overrides.source).map(([scopeKey, entry]) => ({
      scopeKind: 'source' as const,
      scopeKey,
      entry,
    })),
  ];

  const setOverride = (scopeKind: 'category' | 'source', scopeKey: string, clutch?: string, risk?: string) => {
    voxTransport.setTaskPolicyOverride(scopeKind, scopeKey, clutch, risk).then(refresh);
  };

  const clearOverride = (scopeKind: 'category' | 'source', scopeKey: string) => {
    voxTransport.clearTaskPolicyOverride(scopeKind, scopeKey).then(refresh);
  };

  const addableCategories = ALL_CATEGORIES.filter((c) => !(c in overrides.category));
  const addableSources = ALL_SOURCES.filter((s) => !(s in overrides.source));

  const handleAdd = () => {
    if (!addScope) return;
    const [scopeKind, scopeKey] = addScope.split(':') as ['category' | 'source', string];
    setOverride(scopeKind, scopeKey, undefined, undefined);
    setAddScope('');
  };

  return (
    <div className="mt-5 rounded-xl border border-border-subtle bg-overlay-subtle p-3">
      <div className="font-display text-[12px] tracking-[0.12em] uppercase text-text-secondary">
        Task-type cost/model policy
      </div>
      <p className="mt-1 text-[11px] text-text-muted">
        Per-category or per-trigger-source overrides for clutch (cost tier) and risk posture. Falls back to the global
        emphasis/priority chain above when unset.
      </p>

      {rows.length === 0 ? (
        <div className="mt-3 rounded-md border border-dashed border-border-subtle p-3 text-center text-[11px] text-text-muted">
          No overrides — every task category/source uses the default policy.
        </div>
      ) : (
        <ul className="mt-3 space-y-1.5">
          {rows.map(({ scopeKind, scopeKey, entry }) => (
            <li
              key={`${scopeKind}:${scopeKey}`}
              className="flex flex-wrap items-center gap-2 rounded-md border border-border-subtle bg-overlay-subtle p-2"
            >
              <span className="flex-1 text-[12px] text-text-secondary">
                {scopeKind === 'category' ? `Category: ${scopeKey}` : `Source: ${scopeKey}`}
              </span>
              <select
                className={SEL}
                aria-label={`Clutch for ${scopeKey}`}
                value={entry.clutch ?? ''}
                onChange={(e) => setOverride(scopeKind, scopeKey, e.target.value || undefined, entry.risk)}
              >
                <option value="">(inherit)</option>
                {CLUTCH_OPTIONS.map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
              <select
                className={SEL}
                aria-label={`Risk for ${scopeKey}`}
                value={entry.risk ?? ''}
                onChange={(e) => setOverride(scopeKind, scopeKey, entry.clutch, e.target.value || undefined)}
              >
                <option value="">(inherit)</option>
                {RISK_OPTIONS.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className="rounded-sm border border-rose-500/20 bg-rose-500/4 px-2 py-1 font-mono text-[10px] text-rose-300 hover:bg-rose-500/10"
                onClick={() => clearOverride(scopeKind, scopeKey)}
                aria-label={`Remove override for ${scopeKey}`}
              >
                remove
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="mt-3 flex flex-wrap items-center gap-2">
        <label htmlFor="task-policy-add-scope" className="font-display text-[10px] uppercase tracking-widest text-text-muted">
          Add override for
        </label>
        <select
          id="task-policy-add-scope"
          className={SEL}
          value={addScope}
          onChange={(e) => setAddScope(e.target.value)}
        >
          <option value="">(choose a category or source)</option>
          {addableCategories.map((c) => (
            <option key={`category:${c}`} value={`category:${c}`}>
              Category: {c}
            </option>
          ))}
          {addableSources.map((s) => (
            <option key={`source:${s}`} value={`source:${s}`}>
              Source: {s}
            </option>
          ))}
        </select>
        <button type="button" className={BTN} onClick={handleAdd} disabled={!addScope}>
          Add
        </button>
      </div>
    </div>
  );
}
