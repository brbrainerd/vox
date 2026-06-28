import React from 'react';

interface PartRow {
  part: string;
  role: string;
  sources: string;
  lastPrice: string;
  status: string;
}

const PLACEHOLDER_ROWS: PartRow[] = [
  { part: 'RTX 4090', role: 'GPU (primary)', sources: 'Newegg, B&H, Amazon', lastPrice: '—', status: 'unwired' },
  { part: 'Qwen3-30B-A3B Q4_K_M', role: 'LLM weight', sources: 'HuggingFace, Ollama', lastPrice: '—', status: 'unwired' },
  { part: 'NVMe 4TB PCIe 5', role: 'Storage tier X', sources: 'Amazon, MicroCenter', lastPrice: '—', status: 'unwired' },
  { part: 'DDR5-6400 64 GB', role: 'System RAM', sources: 'Newegg, Crucial', lastPrice: '—', status: 'unwired' },
];

const STATUS_CLASS: Record<string, string> = {
  live: 'text-accent-secondary',
  stale: 'text-brass',
  unwired: 'text-text-muted',
  error: 'text-red-400',
};

export function Mercatus() {
  return (
    <section className="space-y-4">
      <div className="flex items-baseline gap-3">
        <h2 className="font-display text-lg text-text-primary tracking-wider uppercase">
          Mercatus — Price Watch
        </h2>
        <span className="rounded-full border border-border-subtle bg-overlay-subtle px-2 py-0.5 font-mono text-[10px] text-text-muted">
          IPC wiring pending C1
        </span>
      </div>

      <div className="rounded-lg border border-dashed border-brass/20 bg-brass/5 px-4 py-2 text-[11px] text-brass">
        Live price data will arrive via IPC from the Mercatus price-watch process once C1 is wired.
        The table below shows the static part catalogue; prices update to real values automatically.
      </div>

      <div className="overflow-x-auto rounded-lg border border-border-subtle">
        <table className="w-full border-collapse text-[12px]">
          <thead>
            <tr className="border-b border-border-subtle bg-overlay-subtle">
              {(['Part', 'Role', 'Sources', 'Last Price', 'Status'] as const).map((h) => (
                <th
                  key={h}
                  className="px-3 py-2 text-left font-display text-[10px] uppercase tracking-widest text-text-muted"
                >
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {PLACEHOLDER_ROWS.map((row, i) => (
              <tr
                key={row.part}
                className={`border-b border-border-subtle last:border-0 ${i % 2 === 0 ? '' : 'bg-overlay-subtle/40'}`}
              >
                <td className="px-3 py-2 font-mono text-text-primary">{row.part}</td>
                <td className="px-3 py-2 text-text-secondary">{row.role}</td>
                <td className="px-3 py-2 text-text-muted">{row.sources}</td>
                <td className="px-3 py-2 font-mono text-text-secondary">{row.lastPrice}</td>
                <td className={`px-3 py-2 font-mono ${STATUS_CLASS[row.status] ?? 'text-text-muted'}`}>
                  {row.status}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="text-[11px] text-text-muted">
        Part catalogue is static. Connect via IPC in C1 to enable live polling, alerts, and history.
      </div>
    </section>
  );
}
