import React, { useMemo, useState } from 'react';
import {
  BrainCircuit,
  Database,
  Globe2,
  Library,
  RefreshCcw,
  Search,
  FolderKanban,
  Compass,
  Rocket,
} from 'lucide-react';
import { voxTransport } from '../transport';
import type { WorkspaceInspectorState } from '../types';

function pretty(value: unknown): string {
  if (value == null) return 'None';
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function manifestSummary(manifest: unknown): string {
  if (!manifest || typeof manifest !== 'object') return 'No manifest snapshot';
  const obj = manifest as Record<string, unknown>;
  const counts = [
    typeof obj.capabilities === 'object' && obj.capabilities ? `capabilities` : null,
    typeof obj.mcp_tools === 'object' && obj.mcp_tools ? `mcp tools` : null,
    typeof obj.cli_paths === 'object' && obj.cli_paths ? `cli paths` : null,
  ].filter(Boolean);
  return counts.length > 0 ? `Live manifest includes ${counts.join(', ')}.` : 'Live manifest loaded.';
}

export function ContextExplorer({
  inspector,
}: {
  inspector: WorkspaceInspectorState | null;
}) {
  const [contextKey, setContextKey] = useState('');
  const [contextValue, setContextValue] = useState('');
  const [repoQuery, setRepoQuery] = useState('');
  const [planGoal, setPlanGoal] = useState('');
  const [browserUrl, setBrowserUrl] = useState('https://example.com');
  const [browserInstruction, setBrowserInstruction] = useState('Summarize the page for a developer.');
  const [screenshotPath, setScreenshotPath] = useState('.vox/tmp/browser-shot.png');
  const [projectName, setProjectName] = useState('');
  const [targetSubdir, setTargetSubdir] = useState('');
  const [memorySearchQuery, setMemorySearchQuery] = useState('');
  const [memorySearchRaw, setMemorySearchRaw] = useState<string | null>(null);
  const [memorySearchErr, setMemorySearchErr] = useState<string | null>(null);
  const [kgQuery, setKgQuery] = useState('');
  const [kgLimit, setKgLimit] = useState(8);
  const [kgRaw, setKgRaw] = useState<string | null>(null);
  const [kgErr, setKgErr] = useState<string | null>(null);

  const socrates = inspector?.lastChatMeta?.socrates;
  const retrieval = inspector?.lastChatMeta?.retrieval;
  const repoIndexSummary = useMemo(() => pretty(inspector?.repoIndexStatus), [inspector?.repoIndexStatus]);

  return (
    <div className="h-full overflow-y-auto p-4 flex flex-col gap-4">
      <section className="glass rounded-2xl border border-white/10 p-4">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Compass size={16} className="text-blue-400" />
            <h3 className="text-sm font-bold">Workspace Context</h3>
          </div>
          <button
            type="button"
            className="p-2 rounded-lg border border-white/10 text-zinc-300"
            aria-label="Refresh orchestrator status"
            title="Refresh orchestrator status"
            onClick={() => voxTransport.callTool('vox_orchestrator_status', {})}
          >
            <RefreshCcw size={14} />
          </button>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="text-xs font-bold uppercase tracking-widest text-zinc-500 mb-2">Active editor</div>
            <div className="text-sm">{inspector?.activeEditor?.filePath || 'No active editor'}</div>
            <div className="text-[11px] text-zinc-500 mt-1">
              {inspector?.activeEditor?.languageId || 'n/a'} · line {inspector?.activeEditor?.line || 0}
            </div>
            {inspector?.activeEditor?.selectedText ? (
              <pre className="mt-2 m-0 p-2 rounded-lg bg-black/30 text-[11px] whitespace-pre-wrap break-words text-zinc-400">
                {inspector.activeEditor.selectedText}
              </pre>
            ) : null}
          </div>
          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="text-xs font-bold uppercase tracking-widest text-zinc-500 mb-2">Open files</div>
            {(inspector?.openFiles?.length ?? 0) === 0 ? (
              <div className="text-xs text-zinc-500">No visible editors.</div>
            ) : (
              <div className="flex flex-wrap gap-2">
                {inspector?.openFiles?.map((file) => (
                  <span key={file} className="px-2 py-1 rounded-full bg-blue-500/10 border border-blue-500/20 text-[11px] text-blue-200">
                    {file}
                  </span>
                ))}
              </div>
            )}
            {(inspector?.activeEditor?.diagnostics?.length ?? 0) > 0 ? (
              <div className="mt-3">
                <div className="text-xs font-bold uppercase tracking-widest text-zinc-500 mb-1">Diagnostics</div>
                <div className="flex flex-col gap-1">
                  {inspector?.activeEditor?.diagnostics?.slice(0, 4).map((diag) => (
                    <div key={`${diag.line}-${diag.message}`} className="text-[11px] text-zinc-400">
                      {diag.severity} · L{diag.line} · {diag.message}
                    </div>
                  ))}
                </div>
              </div>
            ) : null}
          </div>
        </div>
      </section>

      <section className="glass rounded-2xl border border-white/10 p-4">
        <div className="flex items-center gap-2 mb-3">
          <BrainCircuit size={16} className="text-violet-400" />
          <h3 className="text-sm font-bold">Evidence And Planning</h3>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="text-xs font-bold uppercase tracking-widest text-zinc-500 mb-2">Last chat gate</div>
            <div className="text-sm text-zinc-100">{socrates?.risk_decision ?? 'No Socrates turn yet'}</div>
            <div className="text-[11px] text-zinc-400 mt-2">
              confidence {typeof socrates?.confidence_estimate === 'number' ? socrates.confidence_estimate.toFixed(2) : '--'}
              {' · '}
              contradiction {typeof socrates?.contradiction_ratio === 'number' ? socrates.contradiction_ratio.toFixed(2) : '--'}
            </div>
            <div className="text-[11px] text-zinc-500 mt-2">
              retrieval {retrieval?.retrieval_tier ?? '--'} · evidence {retrieval?.evidence_count ?? '--'}
            </div>
          </div>
          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="text-xs font-bold uppercase tracking-widest text-zinc-500 mb-2">Plan adequacy</div>
            <textarea
              className="w-full min-h-[72px] rounded-lg border border-white/10 bg-black/30 p-2 text-sm resize-y outline-none"
              placeholder="Goal to test for planning depth and adequacy..."
              value={planGoal}
              onChange={(event) => setPlanGoal(event.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <button
                type="button"
                className="px-3 py-2 rounded-xl bg-violet-600 text-white text-xs font-semibold"
                onClick={() => voxTransport.callTool('vox_plan', { goal: planGoal, depth: 'deep' })}
              >
                Preview Deep Plan
              </button>
            </div>
            {inspector?.lastPlan ? (
              <div className="mt-3 text-[11px] text-zinc-400 leading-relaxed">
                adequacy {typeof inspector.lastPlan.plan_adequacy_score === 'number' ? inspector.lastPlan.plan_adequacy_score.toFixed(2) : '--'}
                {' · '}
                {inspector.lastPlan.plan_too_thin ? 'too thin' : 'acceptable'}
                {' · '}
                {inspector.lastPlan.tasks?.length ?? 0} tasks
                {(inspector.lastPlan.adequacy_reason_codes?.length ?? 0) > 0 ? (
                  <div className="mt-1 text-zinc-500">{inspector.lastPlan.adequacy_reason_codes?.join(', ')}</div>
                ) : null}
              </div>
            ) : null}
          </div>
        </div>
      </section>

      <section className="glass rounded-2xl border border-white/10 p-4">
        <div className="flex items-center gap-2 mb-3">
          <Library size={16} className="text-sky-400" />
          <h3 className="text-sm font-bold">Agent Retrieval (MCP)</h3>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="text-[11px] text-zinc-400 mb-2 leading-relaxed">
              Calls <code className="text-zinc-500">vox_memory_search</code> (same bundle as chat retrieval: memory,
              chunks, knowledge, repo paths per server plan).
            </div>
            <input
              className="w-full rounded-lg border border-white/10 bg-black/30 p-2 text-sm outline-none mb-2"
              placeholder="Retrieval query..."
              value={memorySearchQuery}
              onChange={(event) => setMemorySearchQuery(event.target.value)}
            />
            <button
              type="button"
              className="px-3 py-2 rounded-xl bg-sky-600 text-white text-xs font-semibold"
              onClick={async () => {
                const q = memorySearchQuery.trim();
                if (!q) {
                  setMemorySearchErr('Enter a query.');
                  setMemorySearchRaw(null);
                  return;
                }
                setMemorySearchErr(null);
                setMemorySearchRaw(null);
                try {
                  const raw = await voxTransport.callTool('vox_memory_search', { query: q });
                  setMemorySearchRaw(pretty(raw));
                } catch (err) {
                  setMemorySearchErr(err instanceof Error ? err.message : String(err));
                }
              }}
            >
              Search memory
            </button>
            {memorySearchErr ? (
              <div className="mt-2 text-[11px] text-red-400 whitespace-pre-wrap">{memorySearchErr}</div>
            ) : null}
            {memorySearchRaw ? (
              <pre className="mt-3 m-0 text-[11px] text-zinc-400 whitespace-pre-wrap break-words max-h-56 overflow-auto">
                {memorySearchRaw}
              </pre>
            ) : null}
          </div>
          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="text-[11px] text-zinc-400 mb-2 leading-relaxed">
              Calls <code className="text-zinc-500">vox_knowledge_query</code> against Turso knowledge nodes.
            </div>
            <input
              className="w-full rounded-lg border border-white/10 bg-black/30 p-2 text-sm outline-none mb-2"
              placeholder="Knowledge graph query..."
              value={kgQuery}
              onChange={(event) => setKgQuery(event.target.value)}
            />
            <div className="flex gap-2 mb-2 items-center">
              <label className="text-[11px] text-zinc-500 shrink-0" htmlFor="kg-limit">
                Limit
              </label>
              <input
                id="kg-limit"
                aria-label="Knowledge graph result limit"
                type="number"
                min={1}
                max={100}
                className="flex-1 rounded-lg border border-white/10 bg-black/30 p-2 text-sm outline-none"
                value={kgLimit}
                onChange={(event) => {
                  const n = Number.parseInt(event.target.value, 10);
                  setKgLimit(Number.isFinite(n) ? Math.min(100, Math.max(1, n)) : 8);
                }}
              />
            </div>
            <button
              type="button"
              className="px-3 py-2 rounded-xl bg-indigo-600 text-white text-xs font-semibold"
              onClick={async () => {
                const q = kgQuery.trim();
                if (!q) {
                  setKgErr('Enter a query.');
                  setKgRaw(null);
                  return;
                }
                setKgErr(null);
                setKgRaw(null);
                try {
                  const raw = await voxTransport.callTool('vox_knowledge_query', {
                    query: q,
                    limit: kgLimit,
                  });
                  setKgRaw(pretty(raw));
                } catch (err) {
                  setKgErr(err instanceof Error ? err.message : String(err));
                }
              }}
            >
              Query knowledge graph
            </button>
            {kgErr ? <div className="mt-2 text-[11px] text-red-400 whitespace-pre-wrap">{kgErr}</div> : null}
            {kgRaw ? (
              <pre className="mt-3 m-0 text-[11px] text-zinc-400 whitespace-pre-wrap break-words max-h-56 overflow-auto">
                {kgRaw}
              </pre>
            ) : null}
          </div>
        </div>
      </section>

      <section className="glass rounded-2xl border border-white/10 p-4">
        <div className="flex items-center gap-2 mb-3">
          <FolderKanban size={16} className="text-emerald-400" />
          <h3 className="text-sm font-bold">Repo And Capability Inspector</h3>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="text-xs font-bold uppercase tracking-widest text-zinc-500 mb-2">Repo index</div>
            <pre className="m-0 text-[11px] text-zinc-400 whitespace-pre-wrap break-words max-h-56 overflow-auto">{repoIndexSummary}</pre>
          </div>
          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="text-xs font-bold uppercase tracking-widest text-zinc-500 mb-2">Capability manifest</div>
            <div className="text-[11px] text-zinc-400 mb-2">{manifestSummary(inspector?.capabilityManifest)}</div>
            <pre className="m-0 text-[11px] text-zinc-500 whitespace-pre-wrap break-words max-h-56 overflow-auto">
              {pretty(inspector?.capabilityManifest)}
            </pre>
          </div>
        </div>
        <div className="mt-3 rounded-xl border border-white/10 bg-black/20 p-3">
          <div className="flex items-center gap-2 mb-2">
            <Search size={14} className="text-blue-300" />
            <span className="text-xs font-bold uppercase tracking-widest text-zinc-500">Cross-repo query</span>
          </div>
          <div className="flex gap-2">
            <input
              className="flex-1 rounded-lg border border-white/10 bg-black/30 p-2 text-sm outline-none"
              placeholder="Search repo catalog text..."
              value={repoQuery}
              onChange={(event) => setRepoQuery(event.target.value)}
            />
            <button
              type="button"
              className="px-3 py-2 rounded-xl bg-blue-600 text-white text-xs font-semibold"
              onClick={() => voxTransport.callTool('vox_repo_query_text', { query: repoQuery, limit: 8 })}
            >
              Query
            </button>
          </div>
          {inspector?.repoQueryResult ? (
            <pre className="mt-3 m-0 text-[11px] text-zinc-400 whitespace-pre-wrap break-words max-h-56 overflow-auto">
              {pretty(inspector.repoQueryResult)}
            </pre>
          ) : null}
        </div>
      </section>

      <section className="glass rounded-2xl border border-white/10 p-4">
        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="flex items-center gap-2 mb-2">
              <Database size={14} className="text-amber-300" />
              <span className="text-xs font-bold uppercase tracking-widest text-zinc-500">Context store</span>
            </div>
            <div className="flex gap-2 mb-2">
              <input
                className="flex-1 rounded-lg border border-white/10 bg-black/30 p-2 text-sm outline-none"
                placeholder="key"
                value={contextKey}
                onChange={(event) => setContextKey(event.target.value)}
              />
              <button
                type="button"
                className="px-3 py-2 rounded-xl border border-white/10 text-xs font-semibold text-zinc-300"
                onClick={() => voxTransport.callTool('vox_db_explain_query', { query: contextKey })}
              >
                Get
              </button>
            </div>
            <textarea
              className="w-full min-h-[72px] rounded-lg border border-white/10 bg-black/30 p-2 text-sm resize-y outline-none"
              placeholder="value"
              value={contextValue}
              onChange={(event) => setContextValue(event.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <button
                type="button"
                className="px-3 py-2 rounded-xl bg-amber-600 text-white text-xs font-semibold"
                onClick={() => voxTransport.callTool('vox_db_sample_data', { agentId: 0, key: contextKey, value: contextValue })}
              >
                Set Key
              </button>
            </div>
            {(inspector?.contextKeys?.length ?? 0) > 0 ? (
              <div className="mt-3 flex flex-wrap gap-2">
                {inspector?.contextKeys?.slice(0, 12).map((key: string) => (
                  <button
                    key={key}
                    type="button"
                    className="px-2 py-1 rounded-full bg-amber-500/10 border border-amber-500/20 text-[11px] text-amber-100"
                    onClick={() => {
                      setContextKey(key);
                      voxTransport.callTool('vox_db_explain_query', { query: key });
                    }}
                  >
                    {key}
                  </button>
                ))}
              </div>
            ) : null}
            {inspector?.contextValue != null ? (
              <pre className="mt-3 m-0 text-[11px] text-zinc-400 whitespace-pre-wrap break-words max-h-40 overflow-auto">
                {pretty(inspector.contextValue)}
              </pre>
            ) : null}
          </div>

          <div className="rounded-xl border border-white/10 bg-black/20 p-3 flex flex-col gap-3">
            <div>
              <div className="flex items-center gap-2 mb-2">
                <Globe2 size={14} className="text-cyan-300" />
                <span className="text-xs font-bold uppercase tracking-widest text-zinc-500">Browser lab</span>
              </div>
              <input
                className="w-full rounded-lg border border-white/10 bg-black/30 p-2 text-sm outline-none mb-2"
                value={browserUrl}
                onChange={(event) => setBrowserUrl(event.target.value)}
              />
              <div className="flex gap-2 mb-2">
                <button
                  type="button"
                  className="px-3 py-2 rounded-xl bg-cyan-600 text-white text-xs font-semibold"
                  onClick={() => voxTransport.callTool('vox_browser_open', { url: browserUrl })}
                >
                  Open
                </button>
                <button
                  type="button"
                  className="px-3 py-2 rounded-xl border border-white/10 text-xs font-semibold text-zinc-300"
                  onClick={() => voxTransport.callTool('vox_browser_goto', { url: browserUrl })}
                >
                  Goto
                </button>
              </div>
              <textarea
                className="w-full min-h-[72px] rounded-lg border border-white/10 bg-black/30 p-2 text-sm resize-y outline-none"
                value={browserInstruction}
                onChange={(event) => setBrowserInstruction(event.target.value)}
              />
              <div className="flex gap-2 mt-2">
                <button
                  type="button"
                  className="px-3 py-2 rounded-xl bg-cyan-700 text-white text-xs font-semibold"
                  onClick={() => voxTransport.callTool('vox_browser_extract', { instruction: browserInstruction })}
                >
                  Extract
                </button>
              </div>
              <input
                className="w-full rounded-lg border border-white/10 bg-black/30 p-2 text-sm outline-none mt-2"
                value={screenshotPath}
                onChange={(event) => setScreenshotPath(event.target.value)}
              />
              <button
                type="button"
                className="mt-2 px-3 py-2 rounded-xl border border-white/10 text-xs font-semibold text-zinc-300"
                onClick={() => voxTransport.callTool('vox_browser_screenshot', { path: screenshotPath })}
              >
                Save Screenshot
              </button>
              {inspector?.browserState ? (
                <pre className="mt-3 m-0 text-[11px] text-zinc-400 whitespace-pre-wrap break-words max-h-40 overflow-auto">
                  {pretty(inspector.browserState)}
                </pre>
              ) : null}
            </div>

            <div className="pt-3 border-t border-white/10">
              <div className="flex items-center gap-2 mb-2">
                <Rocket size={14} className="text-emerald-300" />
                <span className="text-xs font-bold uppercase tracking-widest text-zinc-500">Project scaffold</span>
              </div>
              <input
                className="w-full rounded-lg border border-white/10 bg-black/30 p-2 text-sm outline-none mb-2"
                placeholder="project name"
                value={projectName}
                onChange={(event) => setProjectName(event.target.value)}
              />
              <input
                className="w-full rounded-lg border border-white/10 bg-black/30 p-2 text-sm outline-none mb-2"
                placeholder="target subdir (optional)"
                value={targetSubdir}
                onChange={(event) => setTargetSubdir(event.target.value)}
              />
              <button
                type="button"
                className="px-3 py-2 rounded-xl bg-emerald-700 text-white text-xs font-semibold"
                onClick={() =>
                  voxTransport.callTool('vox_project_init', {
                    projectName,
                    packageKind: 'application',
                    template: 'dashboard',
                    targetSubdir,
                  })
                }
              >
                Scaffold Dashboard App
              </button>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
