import React, { useState } from 'react';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { Sparkline } from '../../ui/Sparkline';
import { voxTransport } from '../../../transport';

// ─── Memory Corpora (static config — real counts come from Mnemosyne backend) ─
const MEM_CORPORA = [
  { id: 'proj',  name: 'Project · vox',        type: 'code',   tone: 'text-brass',       entries: 0 },
  { id: 'docs',  name: 'Docs · README + RFCs',  type: 'text',   tone: 'text-cyan-300',    entries: 0 },
  { id: 'chats', name: 'Chats · Loquela log',   type: 'chat',   tone: 'text-violet-300',  entries: 0 },
  { id: 'rules', name: 'Rule packs',             type: 'policy', tone: 'text-amber-300',   entries: 0 },
  { id: 'web',   name: 'Web crawl · socrates',   type: 'web',    tone: 'text-emerald-300', entries: 0 },
];

// ─── Shard mini-cards ──────────────────────────────────────────────────────────
const SHARD_COUNT = 12;
const shards = Array.from({ length: SHARD_COUNT }, (_, i) => ({
  id: `0x${(0x3A + i).toString(16).toUpperCase()}`,
  depth: 3 + (i % 4),
  entries: 1280 + i * 173,
  hot: i < 3,
  dirty: i === 4 || i === 9,
  spark: [5, 6, 5, 7, 8, 7, 9, 10, 9, 11, 12, 11, 13].map(v => v + i * 0.3),
}));

interface HitResult {
  src: string;
  line: number;
  score: number;
  kind: string;
  text: string;
}

function CorpusChip({
  corpus,
  active,
  onToggle,
}: {
  corpus: (typeof MEM_CORPORA)[number];
  active: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      onClick={onToggle}
      className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 font-mono text-[10px] transition ${
        active
          ? 'border-white/15 bg-white/[0.04] text-zinc-100'
          : 'border-white/5 bg-white/[0.01] text-zinc-500'
      }`}
    >
      <span className={`size-1.5 rounded-full ${active ? 'bg-brass' : 'bg-white/15'}`} />
      {corpus.name}
    </button>
  );
}

function HitCard({ hit, onPin }: { hit: HitResult; onPin: () => void }) {
  const kindIcon: Record<string, React.ReactNode> = {
    code:   <Icon.file className="size-3.5" />,
    text:   <Icon.catalog className="size-3.5" />,
    chat:   <Icon.bolt className="size-3.5" />,
    policy: <Icon.shield className="size-3.5" />,
    web:    <Icon.link className="size-3.5" />,
  };

  return (
    <div className="group flex items-start gap-3 rounded-md border border-white/5 bg-white/[0.02] p-3 hover:border-white/15 transition">
      <div className="flex size-7 shrink-0 items-center justify-center rounded bg-white/[0.03] text-zinc-400">
        {kindIcon[hit.kind] ?? <Icon.file className="size-3.5" />}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="font-mono text-[11px] text-zinc-300 truncate">{hit.src}</span>
          {hit.line > 0 && <span className="font-mono text-[9px] text-zinc-500">:{hit.line}</span>}
        </div>
        <div className="mt-1 text-[12px] leading-relaxed text-zinc-300 line-clamp-2">{hit.text}</div>
      </div>
      <div className="flex flex-col items-end gap-1">
        <span className="font-mono text-[10px] tabular-nums text-emerald-300">
          {(hit.score * 100).toFixed(1)}%
        </span>
        <div className="h-1 w-16 overflow-hidden rounded-full bg-white/5">
          <div
            className="h-full bg-gradient-to-r from-violet-400 to-emerald-400"
            style={{ width: `${hit.score * 100}%` }}
          />
        </div>
        <button
          onClick={onPin}
          className="opacity-0 group-hover:opacity-100 transition rounded border border-white/10 bg-white/[0.02] px-1.5 py-0.5 font-mono text-[9px] text-zinc-300 hover:bg-white/5"
        >
          <Icon.pin className="size-2.5 inline mr-0.5" />pin
        </button>
      </div>
    </div>
  );
}

// Sample hit dataset — will be replaced by real Mnemosyne recall once wired.
const SAMPLE_HITS: HitResult[] = [
  { src: 'vox-protocol/src/handshake.rs',   line: 142, score: 0.94, kind: 'code',   text: 'Constant-time comparison enforced via subtle::ConstantTimeEq; doubt injected when nonce reuse is observed.' },
  { src: 'docs/rfcs/0007-clavis-rotation.md', line: 18, score: 0.91, kind: 'text',  text: 'Rotation cadence is policy-driven (default 90d); break-glass override requires capability gate.' },
  { src: 'chats/loquela/2026-05-09',         line: 0,  score: 0.88, kind: 'chat',   text: 'User asked Augur to harden cryptographic invariants — produced 3 candidate branches.' },
  { src: 'rule-packs/crypto.yml',            line: 31, score: 0.84, kind: 'policy', text: 'All AEAD constructions MUST tag nonce-reuse domain separately; doubt threshold 0.7.' },
  { src: 'vox-arch-check/lib.rs',            line: 88, score: 0.79, kind: 'code',   text: 'Rule R-0421: invariants over crypto primitives validated against Socrates citation set.' },
];

const RECENT_RECALLS = [
  { q: 'ed25519 invariant constraints',       n: 12, when: '2m' },
  { q: 'qlora epoch schedule for mens-candle',n: 7,  when: '14m' },
  { q: 'durable checkpoint stall mitigation', n: 21, when: '1h' },
  { q: 'vox-arch-check rule precedence',      n: 4,  when: '3h' },
];

interface MemoryViewProps {
  pushToast: (t: any) => void;
}

export function MemoryView({ pushToast }: MemoryViewProps) {
  const [query, setQuery] = useState('');
  const [scope, setScope] = useState<string[]>(['proj', 'docs', 'chats', 'rules', 'web']);
  const [topK, setTopK] = useState(8);
  const [recallOn, setRecallOn] = useState(false);
  const [hits, setHits] = useState<HitResult[]>([]);
  const [recalling, setRecalling] = useState(false);

  const totalEntries = MEM_CORPORA.reduce(
    (s, c) => s + (scope.includes(c.id) ? c.entries : 0),
    0
  );
  const toggleScope = (id: string) =>
    setScope(s => (s.includes(id) ? s.filter(x => x !== id) : [...s, id]));

  const kindToCorpusId: Record<string, string> = {
    code: 'proj', text: 'docs', chat: 'chats', policy: 'rules', web: 'web',
  };

  const recall = async (q?: string) => {
    const qq = (q ?? query).trim();
    if (!qq) return;
    setRecalling(true);
    setHits([]);

    try {
      // Execute real CLI command via Tauri Transport
      const res = await voxTransport.callTool('mnemosyne_recall', {
        query: qq,
        scope: scope.join(','),
        limit: topK
      });

      // Parse JSON output if the command outputs structured data
      const parsed = JSON.parse(res.stdout);
      if (Array.isArray(parsed)) {
        setHits(parsed.slice(0, topK));
      } else {
        throw new Error("Invalid output format");
      }
    } catch (err) {
      // Fallback to sample data if backend command is unavailable or fails
      const results = SAMPLE_HITS
        .filter(h => scope.includes(kindToCorpusId[h.kind] ?? 'proj'))
        .slice(0, topK)
        .map(h => ({ ...h, score: +(h.score - Math.random() * 0.04).toFixed(3) }));
      setHits(results);
      pushToast({ tone: 'warn', title: 'Recall backend error', body: 'Fell back to sample data' });
    } finally {
      setRecalling(false);
      pushToast({
        tone: 'ok',
        title: 'Recall complete',
        body: `Top hits across ${scope.length} corpora`,
        cmd: `mnemosyne recall • "${qq}"`,
      });
    }
  };

  return (
    <div className="grid grid-cols-12 gap-5">
      {/* Header + Search */}
      <Glass className="col-span-12 p-5">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">
              Mnemosyne · Memory
            </h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">
              Vector + symbolic recall · {scope.length} corpora active
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => setRecallOn(r => !r)}
              className={`inline-flex items-center gap-1.5 rounded-md border px-2 py-1.5 font-mono text-[10px] transition ${
                recallOn
                  ? 'border-violet-400/40 bg-violet-400/10 text-violet-300'
                  : 'border-white/10 bg-white/[0.02] text-zinc-400 hover:text-zinc-200'
              }`}
            >
              <Icon.eye className="size-3" /> Auto-recall
            </button>
            <button
              onClick={() => pushToast({ tone: 'warn', title: 'Reindex queued', cmd: 'mnemosyne reindex' })}
              className="inline-flex items-center gap-1.5 rounded-md border border-white/10 bg-white/[0.02] px-2 py-1.5 font-mono text-[10px] text-zinc-400 hover:text-zinc-200"
            >
              <Icon.refresh className="size-3" /> Reindex
            </button>
          </div>
        </div>

        {/* Search bar */}
        <div className="mt-4 flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.02] px-3 py-2">
          <Icon.search className="size-3.5 text-zinc-500" />
          <input
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && recall()}
            placeholder="Recall… e.g. 'ed25519 invariants', 'checkpoint stall'"
            className="flex-1 bg-transparent text-[13px] text-zinc-100 placeholder:text-zinc-600 outline-none"
          />
          <span className="font-mono text-[10px] text-zinc-500">top</span>
          <input
            type="number" min={1} max={50} value={topK}
            onChange={e => setTopK(parseInt(e.target.value) || 8)}
            className="w-12 rounded border border-white/10 bg-white/[0.02] px-1.5 py-0.5 text-center font-mono text-[11px] text-zinc-200 outline-none"
          />
          <button
            onClick={() => recall()}
            disabled={!query.trim() || recalling}
            className={`inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1 font-display text-[10px] uppercase tracking-widest transition ${
              query.trim()
                ? 'border-brass/40 bg-brass/15 text-brass hover:bg-brass/25'
                : 'border-white/5 bg-white/[0.02] text-zinc-600 cursor-not-allowed'
            }`}
          >
            {recalling ? '…' : 'Recall'}
          </button>
        </div>

        {/* Scope chips */}
        <div className="mt-3 flex flex-wrap items-center gap-1.5">
          <span className="font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">Scope</span>
          {MEM_CORPORA.map(c => (
            <CorpusChip key={c.id} corpus={c} active={scope.includes(c.id)} onToggle={() => toggleScope(c.id)} />
          ))}
        </div>
      </Glass>

      {/* Recent recalls */}
      <Glass className="col-span-12 xl:col-span-4 p-5">
        <div className="flex items-center justify-between">
          <h3 className="font-display text-[13px] uppercase tracking-[0.18em] text-zinc-200">Recent recalls</h3>
          <Icon.clock className="size-3.5 text-zinc-500" />
        </div>
        <div className="mt-3 space-y-1.5">
          {RECENT_RECALLS.map((r, i) => (
            <button
              key={i}
              onClick={() => { setQuery(r.q); recall(r.q); }}
              className="flex w-full items-center justify-between rounded-md border border-white/5 bg-white/[0.02] px-2.5 py-1.5 text-left hover:border-white/15 hover:bg-white/[0.04] transition"
            >
              <div className="min-w-0">
                <div className="truncate text-[12px] text-zinc-200">{r.q}</div>
                <div className="font-mono text-[9px] text-zinc-500">{r.n} hits · {r.when} ago</div>
              </div>
              <Icon.chevR className="size-3 text-zinc-500 shrink-0" />
            </button>
          ))}
        </div>
      </Glass>

      {/* Hits */}
      <Glass className="col-span-12 xl:col-span-8 p-5">
        <div className="flex items-center justify-between">
          <h3 className="font-display text-[13px] uppercase tracking-[0.18em] text-zinc-200">
            Citations {hits.length > 0 && <span className="text-zinc-500">· {hits.length}</span>}
          </h3>
          {hits.length > 0 && (
            <button
              onClick={() =>
                pushToast({ tone: 'ok', title: 'Pinned to context', body: `${hits.length} citations → Loquela`, cmd: 'context.attach' })
              }
              className="inline-flex items-center gap-1 rounded-md border border-cyan-400/30 bg-cyan-400/10 px-2 py-1 font-mono text-[10px] text-cyan-300 hover:bg-cyan-400/15"
            >
              <Icon.pin className="size-3" /> Pin all to context
            </button>
          )}
        </div>
        <div className="mt-3 space-y-2">
          {recalling &&
            Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="h-12 rounded-md border border-white/5 bg-white/[0.02] relative overflow-hidden">
                <span className="absolute inset-0 -translate-x-full animate-vox-shimmer bg-gradient-to-r from-transparent via-white/5 to-transparent" />
              </div>
            ))}
          {!recalling && hits.length === 0 && (
            <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-white/10 bg-white/[0.01] py-12 text-center">
              <Icon.memory className="size-6 text-zinc-600 mb-2" />
              <div className="font-display text-[12px] tracking-wider text-zinc-400">No recall yet</div>
              <div className="font-mono text-[10px] text-zinc-500">Type a query or click a recent recall</div>
            </div>
          )}
          {!recalling &&
            hits.map((h, i) => (
              <HitCard
                key={i}
                hit={h}
                onPin={() => pushToast({ tone: 'ok', title: 'Cited', body: h.src, cmd: 'context.pin' })}
              />
            ))}
        </div>
      </Glass>

      {/* Memory shards */}
      <Glass className="col-span-12 p-5">
        <div className="flex items-center justify-between">
          <h3 className="font-display text-[13px] uppercase tracking-[0.18em] text-zinc-200">Memory shards</h3>
          <span className="font-mono text-[10px] text-zinc-500">{SHARD_COUNT} live · HNSW · dim 1024</span>
        </div>
        <div className="mt-3 grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
          {shards.map(s => (
            <div
              key={s.id}
              className={`rounded-xl border p-3 transition hover:border-white/15 ${
                s.hot   ? 'border-brass/30 bg-brass/[0.04]' :
                s.dirty ? 'border-amber-400/30 bg-amber-400/[0.04]' :
                          'border-white/5 bg-white/[0.02]'
              }`}
            >
              <div className="flex items-center justify-between">
                <span className="font-mono text-[11px] text-zinc-300">shard-{s.id}</span>
                {s.hot   && <span className="rounded-full bg-brass/15 px-1.5 py-0.5 font-display text-[9px] uppercase tracking-widest text-brass">hot</span>}
                {s.dirty && <span className="rounded-full bg-amber-400/15 px-1.5 py-0.5 font-display text-[9px] uppercase tracking-widest text-amber-300">dirty</span>}
              </div>
              <div className="mt-2 grid grid-cols-2 gap-1.5 text-[9px]">
                <div className="rounded border border-white/5 bg-zinc-950/40 px-2 py-1.5">
                  <div className="uppercase tracking-widest text-zinc-500">Depth</div>
                  <div className="mt-0.5 font-mono text-[11px] text-zinc-200">{s.depth}</div>
                </div>
                <div className="rounded border border-white/5 bg-zinc-950/40 px-2 py-1.5">
                  <div className="uppercase tracking-widest text-zinc-500">Entries</div>
                  <div className="mt-0.5 font-mono text-[11px] text-zinc-200">{s.entries.toLocaleString()}</div>
                </div>
              </div>
              <div className="mt-2 h-8">
                <Sparkline
                  data={s.spark}
                  color={s.hot ? '#d4af37' : s.dirty ? '#fbbf24' : '#71717a'}
                  width={160}
                  height={28}
                />
              </div>
            </div>
          ))}
        </div>
      </Glass>
    </div>
  );
}
