import React, { useCallback, useEffect, useState } from 'react';
import {
  getArchiveStatus,
  listPublicationManifests,
  type ArchiveStatus,
} from './archiveApi';

interface ArchiveRollup {
  publication_id: string;
  state: string;
  status: ArchiveStatus;
}

function depositedCount(rows: ArchiveRollup[]): { zenodo: number; swh: number; pending: number } {
  let zenodo = 0;
  let swh = 0;
  let pending = 0;
  for (const r of rows) {
    const hasZenodo = Boolean(r.status.zenodo_doi);
    const hasSwh = Boolean(r.status.swhid);
    if (hasZenodo) zenodo += 1;
    if (hasSwh) swh += 1;
    if (!hasZenodo && !hasSwh) pending += 1;
  }
  return { zenodo, swh, pending };
}

/**
 * Compact archive deposit rollup for the Scientia dashboard: samples recent
 * scientia manifests and shows Zenodo / SWHID coverage (read-only).
 */
interface ArchiveStatusSummaryProps {
  onFetchError?: (message: string) => void;
}

export function ArchiveStatusSummary({ onFetchError }: ArchiveStatusSummaryProps) {
  const [rows, setRows] = useState<ArchiveRollup[] | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const manifests = await listPublicationManifests(8);
      const sample = manifests.slice(0, 5);
      const rollups = await Promise.all(
        sample.map(async (m) => ({
          publication_id: m.publication_id,
          state: m.state,
          status: await getArchiveStatus(m.publication_id),
        })),
      );
      setRows(rollups);
    } catch (err) {
      setRows([]);
      onFetchError?.(err instanceof Error ? err.message : 'Archive rollup failed');
    } finally {
      setLoading(false);
    }
  }, [onFetchError]);

  useEffect(() => {
    void load();
  }, [load]);

  const counts = rows ? depositedCount(rows) : null;

  return (
    <div className="rounded-xl border border-border-subtle bg-overlay-subtle p-3">
      <div className="mb-2 flex items-center justify-between">
        <span className="font-mono text-[10px] uppercase tracking-wider text-text-muted">
          Archive deposit status
        </span>
        <button
          type="button"
          onClick={load}
          disabled={loading}
          className="rounded-sm border border-border-subtle px-2 py-0.5 font-mono text-[10px] text-text-muted hover:bg-overlay-subtle disabled:opacity-40"
        >
          {loading ? '…' : 'Refresh'}
        </button>
      </div>

      {!rows && loading && (
        <div className="font-mono text-[11px] text-text-muted">Loading archive rollup…</div>
      )}

      {rows && counts && (
        <>
          <div className="mb-3 grid grid-cols-3 gap-2">
            <div className="rounded-lg border border-border-subtle bg-overlay-subtle px-2 py-1.5 text-center">
              <div className="font-mono text-lg text-emerald-300">{counts.zenodo}</div>
              <div className="font-mono text-[9px] uppercase tracking-wider text-text-muted">Zenodo DOI</div>
            </div>
            <div className="rounded-lg border border-border-subtle bg-overlay-subtle px-2 py-1.5 text-center">
              <div className="font-mono text-lg text-cyan">{counts.swh}</div>
              <div className="font-mono text-[9px] uppercase tracking-wider text-text-muted">SWHID</div>
            </div>
            <div className="rounded-lg border border-border-subtle bg-overlay-subtle px-2 py-1.5 text-center">
              <div className="font-mono text-lg text-amber-300">{counts.pending}</div>
              <div
                className="font-mono text-[9px] uppercase tracking-wider text-text-muted"
                title="Sample of recent publications without Zenodo DOI or SWHID"
              >
                Pending deposit (sample)
              </div>
            </div>
          </div>

          {rows.length === 0 ? (
            <div className="font-mono text-[11px] text-text-muted">No scientia publications to sample.</div>
          ) : (
            <ul className="space-y-1">
              {rows.map((r) => (
                <li key={r.publication_id} className="flex items-center gap-2 font-mono text-[11px]">
                  <span className="truncate text-text-secondary">{r.publication_id}</span>
                  <span className="ml-auto shrink-0 text-text-muted">{r.state}</span>
                  <span className="shrink-0 text-emerald-300/80">
                    {r.status.zenodo_doi ? 'DOI' : '—'}
                  </span>
                  <span className="shrink-0 text-cyan/80">{r.status.swhid ? 'SWH' : '—'}</span>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </div>
  );
}
