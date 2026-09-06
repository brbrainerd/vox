import React from 'react';
import { useVoxQuery } from '../../../hooks/useVoxQuery';
import { voxTransport } from '../../../transport';
import { EmptyState } from '../../ui/EmptyState';
import { Icon } from '../../ui/Icons';

export function HarnessHealthView() {
  const { data: runs, isLoading } = useVoxQuery(
    ['harnessEvalHistory'],
    () => voxTransport.harnessEvalHistory(50),
  );
  const { data: regressions } = useVoxQuery(
    ['harnessEvalRegressions'],
    () => voxTransport.harnessEvalRegressions(),
  );

  if (isLoading) {
    return <p className="text-[12px] text-text-muted">Loading harness eval history…</p>;
  }

  if (!runs || runs.length === 0) {
    return (
      <EmptyState
        icon={<Icon.bolt className="size-8" />}
        title="No harness eval runs yet"
        description="Run `vox harness eval --live` locally, or wait for the nightly scheduled workflow, to see chat harness quality and model-selection trends here."
      />
    );
  }

  return (
    <section className="space-y-4" aria-labelledby="harness-health-title">
      <h2 id="harness-health-title" className="font-display text-lg text-text-primary tracking-wider uppercase">
        Harness Health
      </h2>
      {regressions && regressions.length > 0 && (
        <div className="space-y-2" role="alert">
          {regressions.map((r) => (
            <div
              key={`${r.kind}-${r.previous_run_id}-${r.current_run_id}`}
              className="rounded-lg border border-red-400/30 bg-red-400/6 p-3 text-[12px]"
            >
              <p className="font-medium text-red-300">
                Regression detected ({r.kind}): {r.detail}
              </p>
              <p className="mt-1 font-mono text-[10px] text-text-muted">
                {r.previous_git_sha}..{r.current_git_sha}
              </p>
              {r.flipped_task_ids.length > 0 && (
                <p className="mt-1 text-[10px] text-red-300/80">
                  Flipped tasks: {r.flipped_task_ids.join(', ')}
                </p>
              )}
              {r.changed_files.length > 0 && (
                <ul className="mt-1 space-y-0.5 font-mono text-[10px] text-text-muted">
                  {r.changed_files.map((f) => (
                    <li key={f}>{f}</li>
                  ))}
                </ul>
              )}
            </div>
          ))}
        </div>
      )}
      <div className="overflow-auto rounded-lg border border-border-subtle">
        <table className="w-full text-left text-[12px]">
          <caption className="sr-only">Recent chat harness eval runs</caption>
          <thead className="text-text-muted">
            <tr>
              <th scope="col" className="p-2">Run</th>
              <th scope="col" className="p-2">Git SHA</th>
              <th scope="col" className="p-2">Triggered by</th>
              <th scope="col" className="p-2">Pass</th>
              <th scope="col" className="p-2">Fail</th>
              <th scope="col" className="p-2">Skip</th>
              <th scope="col" className="p-2">Cost</th>
              <th scope="col" className="p-2">By category</th>
            </tr>
          </thead>
          <tbody>
            {runs.map((r) => (
              <tr key={r.run_id} className="border-t border-border-subtle">
                <td className="p-2 font-mono text-text-secondary">{r.run_id}</td>
                <td className="p-2 font-mono text-text-muted">{r.git_sha}</td>
                <td className="p-2 text-text-muted">{r.triggered_by}</td>
                <td className="p-2 text-emerald-300">{r.pass_count}</td>
                <td className="p-2 text-red-300">{r.fail_count}</td>
                <td className="p-2 text-text-muted">{r.skip_count}</td>
                <td className="p-2">${r.total_cost_usd.toFixed(4)}</td>
                <td className="p-2">
                  <div className="flex flex-wrap gap-1">
                    {r.category_breakdown.map((c) => (
                      <span
                        key={c.category}
                        className={`rounded px-1 py-0.5 font-mono text-[11px] ${
                          c.fail_count > 0
                            ? "bg-red-950 text-red-300"
                            : "bg-emerald-950 text-emerald-300"
                        }`}
                        title={`${c.category}: ${c.pass_count} pass, ${c.fail_count} fail`}
                      >
                        {c.category} {c.pass_count}/{c.pass_count + c.fail_count}
                      </span>
                    ))}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
