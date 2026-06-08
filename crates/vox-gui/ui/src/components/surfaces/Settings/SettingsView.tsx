import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Glass } from '../../ui/Glass';
import { invoke } from '@tauri-apps/api/core';
import { Icon } from '../../ui/Icons';
import { voxTransport } from '../../../transport';
import { PriorityChainEditor } from './PriorityChainEditor';
import { applyTheme } from '../../../lib/theme';
import { useLocalStorage } from '../../../hooks/useLocalStorage';

const SECTIONS = [
  { id: 'orchestrator', icon: 'cpu',     label: 'Orchestrator' },
  { id: 'routing',      icon: 'matrix',  label: 'Model routing' },
  { id: 'runtime',      icon: 'flow',    label: 'Runtime' },
  { id: 'mesh',         icon: 'flow',    label: 'Mesh & peers' },
  { id: 'signing',      icon: 'shield',  label: 'Signing keys' },
  { id: 'secrets',      icon: 'shield',  label: 'Keys & Secrets' },
  { id: 'telemetry',    icon: 'scale',   label: 'Telemetry' },
  { id: 'keybinds',     icon: 'command', label: 'Keybinds' },
  { id: 'theme',        icon: 'spark',   label: 'Theme' },
  { id: 'gamify',       icon: 'bolt',    label: 'Gamification' },
];

const KEYBINDS = [
  ['⌘K',   'Open command palette'],
  ['⌘↵',  'Dispatch intent'],
  ['⇧↵',  'Newline in composer'],
  ['/',     'Slash command'],
  ['@',     'Mention agent'],
  ['↑/↓', 'History recall'],
  ['⌘B',   'Toggle sidebar'],
  ['⌘.',   'Pause/resume selected agent'],
];

interface SettingsState {
  doubt: boolean;
  autobudget: boolean;
  theme: string;
  concurrency: number;
  capUsd: number;
  doubtThresh: number;
  sign: boolean;
  telemetry: string;
  isolation: string;
  checkpointMins: number;
}

function Row({ label, hint, children }: { label: string; hint: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-xl border border-white/5 bg-white/[0.02] p-3">
      <div>
        <div className="font-display text-[12px] text-zinc-200">{label}</div>
        <div className="text-[11px] text-zinc-500">{hint}</div>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Toggle({ on, onClick }: { on: boolean; onClick: () => void }) {
  return (
    <button onClick={onClick} className={`relative h-5 w-9 rounded-full transition ${on ? 'bg-brass/40' : 'bg-white/10'}`}>
      <span className={`absolute top-0.5 size-4 rounded-full bg-zinc-50 transition ${on ? 'left-[18px]' : 'left-0.5'}`} />
    </button>
  );
}

function RangeInline({
  value, min, max, step = 1, suffix = '', onChange,
}: {
  value: number; min: number; max: number; step?: number; suffix?: string; onChange: (v: number) => void;
}) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div className="flex w-52 items-center gap-3">
      <input
        type="range" min={min} max={max} step={step} value={value}
        onChange={e => onChange(Number(e.target.value))}
        className="vox-range flex-1 h-1 appearance-none rounded-full overflow-hidden"
        style={{ background: `linear-gradient(to right, rgb(var(--brass)) ${pct}%, rgba(255,255,255,0.08) ${pct}%)` } as any}
      />
      <span className="w-14 text-right font-mono text-[11px] text-zinc-200">{value}{suffix}</span>
    </div>
  );
}

/** One node row as summarized by the `vox_mesh_nodes` MCP tool (mirrors MeshView). */
interface MeshNode {
  id: string;
  status: string;
  host_triple?: string | null;
  gpu_summary?: string | null;
  trust_tier?: string | null;
  ed25519_pub_key_b64?: string | null;
  advertised_models?: string[];
}

interface MeshNodesResult {
  source?: string;
  control_plane_error?: string;
  nodes?: MeshNode[];
}

interface McpEnvelope<T> {
  tool: string;
  is_error: boolean;
  result: T;
}

/** Locally-trusted peer, mirrors Rust `TrustedNodeDto`. */
interface TrustedNodeDto {
  nodeId: string;
  pubkeyHex: string;
  label: string | null;
  addedAt: string;
}

/** Live signing identity, mirrors Rust `SigningKeyDto`. */
interface SigningKeyDto {
  nodeId: string;
  algorithm: string;
  fingerprint: string;
  pubkeyHex: string;
  present: boolean;
}

function MeshPeersSection({ pushToast }: { pushToast: (t: any) => void }) {
  const [nodes, setNodes] = useState<MeshNode[]>([]);
  const [meta, setMeta] = useState<MeshNodesResult>({});
  const [trusted, setTrusted] = useState<Record<string, TrustedNodeDto>>({});
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      const [nodesRes, trustedList] = await Promise.all([
        invoke<McpEnvelope<MeshNodesResult>>('invoke_mcp_tool', { tool: 'vox_mesh_nodes', args: {} }),
        invoke<TrustedNodeDto[]>('list_trusted_nodes'),
      ]);
      const m = nodesRes?.result ?? {};
      setMeta(m);
      setNodes(Array.isArray(m.nodes) ? m.nodes : []);
      const map: Record<string, TrustedNodeDto> = {};
      for (const t of trustedList) map[t.nodeId] = t;
      setTrusted(map);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Mesh load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => { reload(); }, [reload]);

  const toggleTrust = async (n: MeshNode) => {
    const isTrusted = !!trusted[n.id];
    setBusy(n.id);
    try {
      if (isTrusted) {
        await invoke<boolean>('untrust_mesh_node', { nodeId: n.id });
        pushToast({ tone: 'ok', title: 'Peer untrusted', body: n.id });
      } else {
        await invoke<boolean>('trust_mesh_node', {
          nodeId: n.id,
          pubkeyHex: n.ed25519_pub_key_b64 ?? '',
          label: n.host_triple ?? null,
        });
        pushToast({ tone: 'ok', title: 'Peer trusted', body: n.id });
      }
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Trust update failed', body: String(err) });
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Mesh &amp; peers</h2>
      <p className="mt-0.5 text-[11px] text-zinc-500">Discover and authorise peer compute on the local mesh (source: {meta.source ?? '—'})</p>
      {meta.control_plane_error && (
        <div className="mt-3 rounded-md border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-300">
          Control plane unreachable — showing local registry. <span className="font-mono">{meta.control_plane_error}</span>
        </div>
      )}
      {loading ? (
        <div className="mt-4 text-[12px] text-zinc-500">Loading peers…</div>
      ) : nodes.length === 0 ? (
        <div className="mt-4 rounded-md border border-white/5 bg-white/[0.02] p-4 text-[11px] leading-relaxed text-zinc-500">
          No mesh peers. Join one with <code className="font-mono text-zinc-400">vox populi join</code>, or configure a control plane via{' '}
          <code className="font-mono text-zinc-400">VOX_ORCHESTRATOR_MESH_CONTROL_URL</code>.
        </div>
      ) : (
        <div className="mt-4 space-y-2">
          {nodes.map(p => {
            const isTrusted = !!trusted[p.id];
            const online = p.status === 'online';
            return (
              <div key={p.id} className="flex items-center justify-between rounded-md border border-white/5 bg-white/[0.02] p-3">
                <div className="flex items-center gap-3">
                  <span className={`size-2 rounded-full ${online ? 'bg-emerald-400' : 'bg-zinc-600'}`} />
                  <div className="leading-tight">
                    <div className="font-mono text-[12px] text-zinc-100 break-all">{p.id}</div>
                    <div className="font-mono text-[10px] text-zinc-500">{(p.host_triple ?? '—')} · {(p.gpu_summary ?? '—')}</div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <span className={`rounded-full px-2 py-0.5 font-display text-[9px] uppercase tracking-widest ${
                    isTrusted ? 'bg-emerald-400/15 text-emerald-300' : 'bg-zinc-700/40 text-zinc-400'
                  }`}>{isTrusted ? 'trusted' : (p.trust_tier ?? 'untrusted')}</span>
                  <button
                    disabled={busy === p.id}
                    onClick={() => toggleTrust(p)}
                    className="rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5 disabled:opacity-40"
                  >{isTrusted ? 'untrust' : 'trust'}</button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </>
  );
}

function SigningKeysSection({ vals, update, pushToast }: {
  vals: SettingsState; update: (patch: Partial<SettingsState>) => void; pushToast: (t: any) => void;
}) {
  const [key, setKey] = useState<SigningKeyDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [rotating, setRotating] = useState(false);

  const reload = useCallback(async () => {
    try {
      setKey(await invoke<SigningKeyDto>('signing_key_status'));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Could not load signing key', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => { reload(); }, [reload]);

  const rotate = async () => {
    const present = key?.present;
    const verb = present ? 'rotate' : 'create';
    const prompt = present
      ? 'Rotate signing key? Enter master password to confirm. This generates a NEW ed25519 keypair; peers trusting the old key must re-trust.'
      : 'Create node identity? Choose a master password to encrypt the new ed25519 key.';
    // eslint-disable-next-line no-alert
    const password = window.prompt(prompt) ?? '';
    if (!password) return;
    setRotating(true);
    try {
      const next = await invoke<SigningKeyDto>('rotate_signing_key', { password });
      setKey(next);
      pushToast({ tone: 'ok', title: `Key ${verb}d`, body: next.nodeId || next.fingerprint });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: `Key ${verb} failed`, body: String(err) });
    } finally {
      setRotating(false);
    }
  };

  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Signing keys</h2>
      <p className="mt-0.5 text-[11px] text-zinc-500">ed25519 capability gate for high-risk dispatch (local node identity)</p>
      <div className="mt-4 space-y-2">
        {loading ? (
          <div className="text-[12px] text-zinc-500">Loading…</div>
        ) : !key?.present ? (
          <div className="rounded-md border border-white/5 bg-white/[0.02] p-4">
            <div className="text-[11px] text-zinc-500">No local node identity yet. Create one to enable signed dispatch.</div>
            <button
              disabled={rotating}
              onClick={rotate}
              className="mt-3 rounded border border-white/10 bg-white/[0.02] px-3 py-1.5 font-mono text-[10px] text-zinc-300 hover:bg-white/5 disabled:opacity-40"
            >{rotating ? 'working…' : 'create identity'}</button>
          </div>
        ) : (
          <div className="flex items-center justify-between rounded-md border border-white/5 bg-white/[0.02] p-3">
            <div className="flex items-center gap-3">
              <Icon.shield className="size-4 text-amber-300" />
              <div className="leading-tight">
                <div className="font-mono text-[12px] text-zinc-100">{key.nodeId || '(locked)'}</div>
                <div className="font-mono text-[10px] text-zinc-500">{key.fingerprint}</div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <span className="rounded-full bg-white/[0.04] px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-zinc-300">{key.algorithm}</span>
              <button
                disabled={rotating}
                onClick={rotate}
                className="rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5 disabled:opacity-40"
              >{rotating ? 'rotating…' : 'rotate'}</button>
            </div>
          </div>
        )}
        <Row label="Require signature on Native isolation" hint="Hard gate; refuses dispatch without ed25519">
          <Toggle on={vals.sign} onClick={() => update({ sign: !vals.sign })} />
        </Row>
      </div>
    </>
  );
}

// Redaction-safe DTO mirroring the Rust `SecretStatusDto`. NOTE: there is no
// field carrying the raw secret value — the backend never returns it.
interface SecretStatusDto {
  id: string;
  canonicalEnv: string;
  scopeDescription: string;
  taxonomySlug: string;
  authRegistry: string | null;
  required: boolean;
  isPresent: boolean;
  status: string;
  redacted: string;
  source: string | null;
  remediation: string;
}

// Backend/profile status header DTO, mirrors Rust `SecretsBackendStatusDto`.
interface SecretsBackendStatusDto {
  backendMode: string;
  profile: string;
  strict: boolean;
  available: boolean;
  detail: string | null;
}

// One recognised key from an `.env` import preview. NAMES + redacted only — no values.
interface ImportEnvEntryDto {
  sourceKey: string;
  canonicalEnv: string;
  redacted: string;
}

interface ImportEnvResultDto {
  applied: boolean;
  count: number;
  entries: ImportEnvEntryDto[];
}

function KeysSecretsSection({ pushToast }: { pushToast: (t: any) => void }) {
  const [rows, setRows] = useState<SecretStatusDto[]>([]);
  const [loading, setLoading] = useState(true);
  // Holds ONLY the in-flight input value per key. Cleared immediately on save.
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [status, setStatus] = useState<SecretsBackendStatusDto | null>(null);

  // Per-taxonomy collapse state, persisted (mirrors the Sidebar pattern).
  const [collapsed, setCollapsed] = useLocalStorage<Record<string, boolean>>('vox_secrets_groups', {});
  // Tracks which slugs we've already applied the auto-expand default to, so a
  // user's later manual collapse of a required group is never re-overridden.
  const seededSlugs = useRef<Set<string>>(new Set());

  // Inline Import .env flow state.
  const [envPath, setEnvPath] = useState('');
  const [preview, setPreview] = useState<ImportEnvResultDto | null>(null);
  const [importBusy, setImportBusy] = useState(false);

  const reload = async () => {
    try {
      const [next, st] = await Promise.all([
        invoke<SecretStatusDto[]>('list_secret_status'),
        invoke<SecretsBackendStatusDto>('secrets_backend_status').catch(() => null),
      ]);
      setRows(next);
      if (st) setStatus(st);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Could not load secrets', body: String(err) });
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { reload(); }, []);

  // Group rows by taxonomy slug (stable insertion order from the backend list).
  const groups = React.useMemo(() => {
    const map = new Map<string, SecretStatusDto[]>();
    for (const r of rows) {
      const slug = r.taxonomySlug || 'other';
      (map.get(slug) ?? map.set(slug, []).get(slug)!).push(r);
    }
    return Array.from(map.entries()).map(([slug, items]) => {
      const set = items.filter(i => i.isPresent).length;
      const missing = items.length - set;
      const needsAttention = items.some(i => i.required && !i.isPresent);
      return { slug, items, set, missing, needsAttention };
    });
  }, [rows]);

  // Default: groups with an unmet required secret start expanded; others stay as
  // stored (or collapsed on first sight). Only seed each slug once.
  useEffect(() => {
    if (groups.length === 0) return;
    setCollapsed(prev => {
      const next = { ...prev };
      let changed = false;
      for (const g of groups) {
        if (seededSlugs.current.has(g.slug)) continue;
        seededSlugs.current.add(g.slug);
        if (!(g.slug in next)) {
          next[g.slug] = !g.needsAttention;
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, [groups, setCollapsed]);

  const toggleGroup = (slug: string) =>
    setCollapsed(prev => ({ ...prev, [slug]: !prev[slug] }));

  const migrate = async () => {
    setImportBusy(true);
    try {
      const moved = await invoke<number>('migrate_auth_store');
      pushToast({ tone: 'ok', title: 'Auth store migrated', body: `${moved} entr${moved === 1 ? 'y' : 'ies'} moved to vault` });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Migrate failed', body: String(err) });
    } finally {
      setImportBusy(false);
    }
  };

  const runPreview = async () => {
    setImportBusy(true);
    try {
      const res = await invoke<ImportEnvResultDto>('import_env', { path: envPath || null, apply: false });
      setPreview(res);
      if (res.count === 0) {
        pushToast({ tone: 'warn', title: 'No managed secrets found', body: envPath || '.env' });
      }
    } catch (err) {
      setPreview(null);
      pushToast({ tone: 'warn', title: 'Preview failed', body: String(err) });
    } finally {
      setImportBusy(false);
    }
  };

  const runImport = async () => {
    setImportBusy(true);
    try {
      const res = await invoke<ImportEnvResultDto>('import_env', { path: envPath || null, apply: true });
      setPreview(null);
      pushToast({ tone: 'ok', title: 'Secrets imported', body: `${res.count} value${res.count === 1 ? '' : 's'} stored in vault` });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Import failed', body: String(err) });
    } finally {
      setImportBusy(false);
    }
  };

  const save = async (key: string) => {
    const value = drafts[key];
    if (!value) return;
    setBusy(key);
    try {
      await invoke<boolean>('set_secret', { key, value });
      // Clear the field immediately — the value never lives in UI state beyond this.
      setDrafts(d => { const n = { ...d }; delete n[key]; return n; });
      pushToast({ tone: 'ok', title: 'Secret saved', body: key });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Save failed', body: String(err) });
    } finally {
      setBusy(null);
    }
  };

  const remove = async (key: string) => {
    setBusy(key);
    try {
      await invoke<boolean>('remove_secret', { key });
      pushToast({ tone: 'ok', title: 'Secret removed', body: key });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Remove failed', body: String(err) });
    } finally {
      setBusy(null);
    }
  };

  const renderSecretRow = (r: SecretStatusDto) => (
    <div key={r.id} className="rounded-md border border-white/5 bg-white/[0.02] p-3">
      <div className="flex items-center justify-between gap-3">
        <div className="leading-tight">
          <div className="flex items-center gap-2">
            <span className="font-mono text-[12px] text-zinc-100">{r.canonicalEnv}</span>
            {r.required && (
              <span className="rounded-full bg-amber-400/15 px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-amber-300">required</span>
            )}
          </div>
          <div className="mt-0.5 text-[10px] text-zinc-500">{r.scopeDescription}</div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <span className={`rounded-full px-2 py-0.5 font-display text-[9px] uppercase tracking-widest ${
            r.isPresent ? 'bg-emerald-400/15 text-emerald-300' : 'bg-zinc-700/40 text-zinc-400'
          }`}>{r.isPresent ? 'set' : 'missing'}</span>
          {r.isPresent && (
            <span className="font-mono text-[10px] text-zinc-500">{r.redacted}</span>
          )}
        </div>
      </div>
      <div className="mt-2 flex items-center gap-2">
        <input
          type="password"
          autoComplete="new-password"
          placeholder="Paste new value…"
          value={drafts[r.canonicalEnv] ?? ''}
          onChange={e => setDrafts(d => ({ ...d, [r.canonicalEnv]: e.target.value }))}
          className="flex-1 rounded border border-white/10 bg-black/30 px-2 py-1 font-mono text-[11px] text-zinc-100 placeholder:text-zinc-600 focus:border-brass/40 focus:outline-none"
        />
        <button
          disabled={!drafts[r.canonicalEnv] || busy === r.canonicalEnv}
          onClick={() => save(r.canonicalEnv)}
          className="rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5 disabled:opacity-40"
        >save</button>
        <button
          disabled={!r.isPresent || busy === r.canonicalEnv}
          onClick={() => remove(r.canonicalEnv)}
          className="rounded border border-rose-500/20 bg-rose-500/[0.04] px-2 py-1 font-mono text-[10px] text-rose-300 hover:bg-rose-500/10 disabled:opacity-40"
        >remove</button>
      </div>
    </div>
  );

  return (
    <>
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Keys &amp; Secrets</h2>
        {status && (
          <>
            <span className="rounded-full bg-white/[0.04] px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-zinc-300" title="Active secrets backend mode">
              backend: {status.backendMode}
            </span>
            <span className={`rounded-full px-2 py-0.5 font-display text-[9px] uppercase tracking-widest ${
              status.strict ? 'bg-amber-400/15 text-amber-300' : 'bg-white/[0.04] text-zinc-300'
            }`} title="Active resolution profile">
              profile: {status.profile}
            </span>
            <span className={`rounded-full px-2 py-0.5 font-display text-[9px] uppercase tracking-widest ${
              status.available ? 'bg-emerald-400/15 text-emerald-300' : 'bg-rose-500/15 text-rose-300'
            }`} title={status.detail ?? undefined}>
              {status.available ? 'available' : 'unavailable'}
            </span>
          </>
        )}
      </div>
      <p className="mt-0.5 text-[11px] text-zinc-500">
        Managed API keys and tokens (Vox Secrets / Clavis). Values are write-only — once saved they are never shown again, only a redacted preview.
      </p>

      {/* Actions: migrate auth.json + import .env */}
      <div className="mt-4 rounded-md border border-white/5 bg-white/[0.02] p-3">
        <div className="flex flex-wrap items-center gap-2">
          <button
            disabled={importBusy}
            onClick={migrate}
            className="rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5 disabled:opacity-40"
          >Migrate auth.json → vault</button>
          <input
            type="text"
            value={envPath}
            placeholder="default .env (optional path)"
            onChange={e => { setEnvPath(e.target.value); setPreview(null); }}
            className="min-w-[180px] flex-1 rounded border border-white/10 bg-black/30 px-2 py-1 font-mono text-[11px] text-zinc-100 placeholder:text-zinc-600 focus:border-brass/40 focus:outline-none"
          />
          <button
            disabled={importBusy}
            onClick={runPreview}
            className="rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5 disabled:opacity-40"
          >Preview</button>
          {preview && preview.count > 0 && (
            <button
              disabled={importBusy}
              onClick={runImport}
              className="rounded border border-emerald-400/20 bg-emerald-400/[0.06] px-2 py-1 font-mono text-[10px] text-emerald-300 hover:bg-emerald-400/10 disabled:opacity-40"
            >Import {preview.count}</button>
          )}
        </div>
        {preview && (
          <div className="mt-2 rounded border border-white/5 bg-black/20 p-2">
            <div className="font-display text-[10px] uppercase tracking-widest text-zinc-400">
              {preview.count} managed secret{preview.count === 1 ? '' : 's'} would import (names only — no values shown)
            </div>
            {preview.entries.length > 0 && (
              <div className="mt-1 flex flex-col gap-0.5">
                {preview.entries.map(e => (
                  <div key={e.sourceKey} className="flex items-center gap-2 font-mono text-[10px] text-zinc-400">
                    <span className="text-zinc-200">{e.sourceKey}</span>
                    <span className="text-zinc-600">→</span>
                    <span className="text-zinc-300">{e.canonicalEnv}</span>
                    <span className="text-zinc-600">{e.redacted}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>

      {loading ? (
        <div className="mt-4 text-[12px] text-zinc-500">Loading…</div>
      ) : (
        <div className="mt-4 space-y-2">
          {groups.map(g => {
            const isCollapsed = !!collapsed[g.slug];
            return (
              <div key={g.slug} className="rounded-md border border-white/5 bg-white/[0.01]">
                <button
                  onClick={() => toggleGroup(g.slug)}
                  className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left hover:bg-white/[0.02]"
                >
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-[10px] text-zinc-500">{isCollapsed ? '▸' : '▾'}</span>
                    <span className="rounded-full bg-white/[0.04] px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-zinc-300">{g.slug}</span>
                    {g.needsAttention && (
                      <span className="rounded-full bg-amber-400/15 px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-amber-300">action needed</span>
                    )}
                  </div>
                  <span className="font-mono text-[10px] text-zinc-500">
                    {g.set} set / {g.missing} missing
                  </span>
                </button>
                {!isCollapsed && (
                  <div className="space-y-2 px-2 pb-2">
                    {g.items.map(renderSecretRow)}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </>
  );
}

/** Editable runtime config field, mirrors Rust `UserConfigFieldDto`. */
interface UserConfigFieldDto {
  key: string;
  label: string;
  hint: string;
  group: string;
  kind: 'string' | 'float' | 'int' | 'path' | 'enum';
  options: string[];
  default: string;
  currentValue: string;
}

const RUNTIME_GROUP_ORDER = ['General', 'Models & endpoints', 'Tuning', 'Training'];

function RuntimeConfigSection({ pushToast }: { pushToast: (t: any) => void }) {
  const [fields, setFields] = useState<UserConfigFieldDto[]>([]);
  const [loading, setLoading] = useState(true);
  // In-flight edits keyed by config key; cleared on reload.
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const savedToast = useRef<ReturnType<typeof setTimeout> | null>(null);

  const reload = useCallback(async () => {
    try {
      const next = await invoke<UserConfigFieldDto[]>('get_user_config');
      setFields(next);
      setDrafts({});
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Could not load runtime config', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => { reload(); }, [reload]);
  useEffect(() => () => { if (savedToast.current) clearTimeout(savedToast.current); }, []);

  const save = async (f: UserConfigFieldDto, value: string) => {
    setBusy(f.key);
    try {
      await invoke('set_user_config', { key: f.key, value });
      if (savedToast.current) clearTimeout(savedToast.current);
      savedToast.current = setTimeout(() => {
        pushToast({ tone: 'ok', title: 'Setting saved', body: f.label });
      }, 600);
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Save failed', body: String(err) });
    } finally {
      setBusy(null);
    }
  };

  const reset = async (f: UserConfigFieldDto) => {
    setBusy(f.key);
    try {
      await invoke('reset_user_config', { key: f.key });
      pushToast({ tone: 'ok', title: 'Reset to default', body: f.label });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Reset failed', body: String(err) });
    } finally {
      setBusy(null);
    }
  };

  const draftFor = (f: UserConfigFieldDto) => drafts[f.key] ?? f.currentValue;

  const control = (f: UserConfigFieldDto) => {
    if (f.kind === 'enum') {
      return (
        <div className="inline-flex flex-wrap items-center rounded-md border border-white/10 bg-black/30 p-0.5">
          {f.options.map(opt => (
            <button
              key={opt}
              disabled={busy === f.key}
              onClick={() => save(f, opt)}
              className={`rounded-[5px] px-2 py-1 font-display text-[10px] uppercase tracking-[0.12em] transition disabled:opacity-40 ${
                f.currentValue === opt ? 'bg-white/10 text-zinc-50' : 'text-zinc-500 hover:text-zinc-300'
              }`}
            >{opt}</button>
          ))}
        </div>
      );
    }
    return (
      <div className="flex items-center gap-2">
        <input
          type="text"
          inputMode={f.kind === 'float' || f.kind === 'int' ? 'numeric' : 'text'}
          value={draftFor(f)}
          placeholder={f.default || '—'}
          onChange={e => setDrafts(d => ({ ...d, [f.key]: e.target.value }))}
          className="w-56 rounded border border-white/10 bg-black/30 px-2 py-1 font-mono text-[11px] text-zinc-100 placeholder:text-zinc-600 focus:border-brass/40 focus:outline-none"
        />
        <button
          disabled={busy === f.key || draftFor(f) === f.currentValue}
          onClick={() => save(f, draftFor(f))}
          className="rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5 disabled:opacity-40"
        >save</button>
      </div>
    );
  };

  const groups = RUNTIME_GROUP_ORDER
    .map(g => ({ group: g, items: fields.filter(f => f.group === g) }))
    .filter(g => g.items.length > 0);

  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Runtime</h2>
      <p className="mt-0.5 text-[11px] text-zinc-500">
        Core user config persisted to your Vox user config (effective values: ENV &gt; Vox.toml &gt; global &gt; defaults)
      </p>
      {loading ? (
        <div className="mt-4 text-[12px] text-zinc-500">Loading…</div>
      ) : (
        <div className="mt-4 space-y-5">
          {groups.map(({ group, items }) => (
            <div key={group}>
              <div className="font-display text-[11px] uppercase tracking-[0.15em] text-zinc-400">{group}</div>
              <div className="mt-2 space-y-2">
                {items.map(f => (
                  <Row key={f.key} label={f.label} hint={f.hint}>
                    <div className="flex items-center gap-2">
                      {control(f)}
                      <button
                        disabled={busy === f.key}
                        onClick={() => reset(f)}
                        title="Reset to default"
                        className="rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-400 hover:bg-white/5 disabled:opacity-40"
                      >reset</button>
                    </div>
                  </Row>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </>
  );
}

interface SettingsViewProps {
  pushToast: (t: any) => void;
}

export function SettingsView({ pushToast }: SettingsViewProps) {
  const [section, setSection] = useState('orchestrator');
  const [routing, setRouting] = useState({
    efficiency: 25,
    precision: 30,
    latency: 20,
    availability: 20,
    balance: 5,
    mobile: 0,
  });
  const [vals, setVals] = useState<SettingsState>({
    doubt: true, autobudget: true, theme: 'arcane', concurrency: 7,
    capUsd: 5, doubtThresh: 0.6, sign: false, telemetry: 'local',
    isolation: 'wasm', checkpointMins: 5,
  });

  const update = async (patch: Partial<SettingsState>) => {
    const next = { ...vals, ...patch };
    setVals(next);

    // Apply the accent palette immediately on theme change (before/independent
    // of persistence), so the swatch selection takes visible effect at once.
    if (patch.theme !== undefined) applyTheme(next.theme);

    // Attempt to push to Rust (fails gracefully if command not registered)
    try {
      await invoke('set_orchestrator_config', { config: next });
      for (const [k, v] of Object.entries(patch)) {
        if (['theme', 'telemetry', 'sign', 'checkpointMins'].includes(k)) {
          await invoke('set_gui_preference', { key: `gui.${k}`, value: String(v) });
        }
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Save failed', body: String(err) });
    }
  };

  useEffect(() => {
    voxTransport.getRoutingSummaryLive().then((s: any) => {
      if (s?.routing_priority) setRouting(s.routing_priority);
    }).catch(() => {});
    const hydrate = async () => {
      try {
        const [theme, telemetry, sign, checkpoint] = await Promise.all([
          invoke<string | null>('get_gui_preference', { key: 'gui.theme' }),
          invoke<string | null>('get_gui_preference', { key: 'gui.telemetry' }),
          invoke<string | null>('get_gui_preference', { key: 'gui.sign' }),
          invoke<string | null>('get_gui_preference', { key: 'gui.checkpointMins' }),
        ]);
        setVals((prev) => ({
          ...prev,
          theme: theme || prev.theme,
          telemetry: telemetry || prev.telemetry,
          sign: sign != null ? sign === 'true' : prev.sign,
          // checkpointMins is a UI-only preference (no OrchestratorConfig field).
          checkpointMins: checkpoint != null ? Number(checkpoint) || prev.checkpointMins : prev.checkpointMins,
        }));
      } catch {
        // keep default local state when preference DB isn't available
      }
      // Hydrate the orchestrator sliders from the real Vox.toml [orchestrator]
      // table (falls back to real defaults if no manifest/daemon). This replaces
      // the previously hardcoded literals so the panel reflects actual state.
      try {
        const orch = await invoke<{
          concurrency: number; capUsd: number; doubtThresh: number;
          isolation: string; autobudget: boolean; doubt: boolean;
        }>('get_orchestrator_config');
        setVals((prev) => ({
          ...prev,
          concurrency: orch.concurrency ?? prev.concurrency,
          capUsd: orch.capUsd ?? prev.capUsd,
          doubtThresh: orch.doubtThresh ?? prev.doubtThresh,
          isolation: orch.isolation ?? prev.isolation,
          autobudget: orch.autobudget ?? prev.autobudget,
          doubt: orch.doubt ?? prev.doubt,
        }));
      } catch {
        // Missing daemon/manifest: keep current defaults.
      }
    };
    hydrate();
    invoke<{ enabled: boolean; mode: string }>('get_gamify_settings').then(setGamify).catch(() => {});
  }, []);

  const [gamify, setGamify] = useState<{ enabled: boolean; mode: string }>({ enabled: true, mode: 'balanced' });

  const updateGamify = async (patch: Partial<{ enabled: boolean; mode: string }>) => {
    const next = { ...gamify, ...patch };
    setGamify(next);
    try {
      await invoke('set_gamify_settings', { enabled: next.enabled, mode: next.mode });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Gamify save failed', body: String(err) });
    }
  };

  const [advanced, setAdvanced] = useState(false);

  // Trailing-debounce the success toast so a dragging RangeInline slider (which
  // fires onChange on every tick) only surfaces one "saved" toast once the value
  // settles. Persisting still happens on every change.
  const savedToastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => {
    if (savedToastTimer.current) clearTimeout(savedToastTimer.current);
  }, []);

  const updateRouting = useCallback(async (patch: Partial<typeof routing>) => {
    const next = { ...routing, ...patch };
    setRouting(next);
    try {
      await voxTransport.setRoutingPriority(next);
      if (savedToastTimer.current) clearTimeout(savedToastTimer.current);
      savedToastTimer.current = setTimeout(() => {
        pushToast({ tone: 'ok', title: 'Routing weights saved', body: 'VOX_AUTO_ROUTING_PRIORITY persisted' });
      }, 600);
    } catch (err) {
      if (savedToastTimer.current) {
        clearTimeout(savedToastTimer.current);
        savedToastTimer.current = null;
      }
      pushToast({ tone: 'warn', title: 'Routing save failed', body: String(err) });
    }
  }, [routing, pushToast]);

  // The three user-facing characteristics map onto the 6-axis priority:
  //   intelligence -> precision, efficiency -> efficiency, responsiveness -> latency.
  // availability / balance / mobile are system-derived and preserved as-is.
  const applyEmphasis = (e: { intelligence: number; efficiency: number; responsiveness: number }) =>
    updateRouting({ precision: e.intelligence, efficiency: e.efficiency, latency: e.responsiveness });

  const EMPHASIS_PRESETS: [string, { intelligence: number; efficiency: number; responsiveness: number }][] = [
    ['Balanced',       { intelligence: 33, efficiency: 33, responsiveness: 34 }],
    ['Intelligence',   { intelligence: 70, efficiency: 15, responsiveness: 15 }],
    ['Efficiency',     { intelligence: 15, efficiency: 70, responsiveness: 15 }],
    ['Responsiveness', { intelligence: 15, efficiency: 15, responsiveness: 70 }],
  ];

  // Reverse-map current 6-axis priority into the 3 user-facing characteristics.
  const emphasis = {
    intelligence: routing.precision,
    efficiency: routing.efficiency,
    responsiveness: routing.latency,
  };
  const activePreset = EMPHASIS_PRESETS.find(([, p]) =>
    p.intelligence === emphasis.intelligence &&
    p.efficiency === emphasis.efficiency &&
    p.responsiveness === emphasis.responsiveness,
  )?.[0] ?? null;

  return (
    <div className="grid grid-cols-12 gap-5">
      {/* Nav */}
      <Glass className="col-span-12 md:col-span-3 p-3">
        <nav className="flex flex-col gap-1">
          {SECTIONS.map(s => {
            const IcoCmp = (Icon as any)[s.icon] ?? Icon.bolt;
            const on = section === s.id;
            return (
              <button
                key={s.id}
                onClick={() => setSection(s.id)}
                className={`flex items-center gap-2.5 rounded-lg px-3 py-2 text-left transition ${
                  on ? 'bg-white/[0.05] text-zinc-100' : 'text-zinc-400 hover:bg-white/[0.025] hover:text-zinc-200'
                }`}
              >
                <IcoCmp className={`size-4 ${on ? 'text-brass' : 'text-zinc-500'}`} />
                <span className="font-display text-[12px] tracking-[0.12em] uppercase">{s.label}</span>
              </button>
            );
          })}
        </nav>
      </Glass>

      {/* Content */}
      <Glass className="col-span-12 md:col-span-9 p-5">
        {section === 'orchestrator' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Orchestrator</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Global scheduling, budget, and verification policy</p>
            <div className="mt-4 space-y-3">
              <Row label="Max concurrent agents" hint="Hard cap before scheduler back-pressure">
                <RangeInline value={vals.concurrency} min={1} max={16} onChange={v => update({ concurrency: v })} />
              </Row>
              <Row label="Global budget cap (USD)" hint="Soft + hard cap. Throttles when reached.">
                <RangeInline value={vals.capUsd} min={1} max={50} step={1} suffix="$" onChange={v => update({ capUsd: v })} />
              </Row>
              <Row label="Auto-doubt threshold" hint="Confidence floor below which Augur intervenes">
                <RangeInline value={Math.round(vals.doubtThresh * 100)} min={0} max={100} step={5} suffix="%" onChange={v => update({ doubtThresh: v / 100 })} />
              </Row>
              <Row label="Durable checkpoint cadence" hint="Snapshot interval for resumable runs (UI preference)">
                <RangeInline value={vals.checkpointMins} min={1} max={30} step={1} suffix=" min" onChange={v => update({ checkpointMins: v })} />
              </Row>
              <Row label="Default isolation tier" hint="Runtime sandbox for new agents">
                <div className="inline-flex items-center rounded-md border border-white/10 bg-black/30 p-0.5">
                  {([['wasm', 'WASM'], ['ctr', 'Container'], ['native', 'Native']] as [string, string][]).map(([id, l]) => (
                    <button
                      key={id}
                      onClick={() => update({ isolation: id })}
                      className={`rounded-[5px] px-2 py-1 font-display text-[10px] uppercase tracking-[0.15em] transition ${
                        vals.isolation === id ? 'bg-white/10 text-zinc-50' : 'text-zinc-500 hover:text-zinc-300'
                      }`}
                    >
                      {l}
                    </button>
                  ))}
                </div>
              </Row>
              <Row label="Auto-doubt unverified outputs" hint="Inject Augur into the verify lane">
                <Toggle on={vals.doubt} onClick={() => update({ doubt: !vals.doubt })} />
              </Row>
              <Row label="Auto-budget per agent" hint="Derived from skill + recent burn">
                <Toggle on={vals.autobudget} onClick={() => update({ autobudget: !vals.autobudget })} />
              </Row>
            </div>
          </>
        )}

        {section === 'routing' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Model routing</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Emphasis tunes how the scorer trades off intelligence, efficiency, and responsiveness (persisted to VOX_AUTO_ROUTING_PRIORITY)</p>

            {/* Emphasis: presets + three labeled characteristic sliders */}
            <div className="mt-4 rounded-xl border border-white/5 bg-white/[0.02] p-3">
              <div className="font-display text-[12px] tracking-[0.12em] uppercase text-zinc-300">Emphasis</div>
              <div className="mt-3 grid grid-cols-2 gap-2 md:grid-cols-4">
                {EMPHASIS_PRESETS.map(([name, preset]) => (
                  <button
                    key={name}
                    onClick={() => applyEmphasis(preset)}
                    className={`rounded-lg border p-2 text-center transition ${
                      activePreset === name ? 'border-brass/40 bg-brass/[0.05] text-zinc-50' : 'border-white/5 bg-white/[0.02] text-zinc-400 hover:border-white/15 hover:text-zinc-200'
                    }`}
                  >
                    <span className="font-display text-[11px] tracking-wide">{name}</span>
                  </button>
                ))}
              </div>
              <div className="mt-3 space-y-3">
                <Row label="Intelligence" hint="Prefer higher-capability models">
                  <RangeInline value={emphasis.intelligence} min={0} max={100} onChange={v => applyEmphasis({ ...emphasis, intelligence: v })} />
                </Row>
                <Row label="Efficiency" hint="Prefer cheaper models when viable">
                  <RangeInline value={emphasis.efficiency} min={0} max={100} onChange={v => applyEmphasis({ ...emphasis, efficiency: v })} />
                </Row>
                <Row label="Responsiveness" hint="Prefer faster p50 models">
                  <RangeInline value={emphasis.responsiveness} min={0} max={100} onChange={v => applyEmphasis({ ...emphasis, responsiveness: v })} />
                </Row>
              </div>
            </div>

            <button
              onClick={() => setAdvanced(a => !a)}
              className="mt-4 font-display text-[11px] uppercase tracking-[0.15em] text-zinc-500 hover:text-zinc-300"
            >
              {advanced ? '▾ Hide advanced axes' : '▸ Advanced (all 6 axes)'}
            </button>

            {advanced && (
            <div className="mt-3 space-y-3">
              <Row label="Efficiency (cost)" hint="Prefer cheaper models when viable">
                <RangeInline value={routing.efficiency} min={0} max={100} onChange={v => updateRouting({ efficiency: v })} />
              </Row>
              <Row label="Precision (intelligence)" hint="Prefer higher-capability models">
                <RangeInline value={routing.precision} min={0} max={100} onChange={v => updateRouting({ precision: v })} />
              </Row>
              <Row label="Latency (responsiveness)" hint="Prefer faster p50 models">
                <RangeInline value={routing.latency} min={0} max={100} onChange={v => updateRouting({ latency: v })} />
              </Row>
              <Row label="Availability" hint="Weight provider quota / uptime signals">
                <RangeInline value={routing.availability} min={0} max={100} onChange={v => updateRouting({ availability: v })} />
              </Row>
              <Row label="Balance (context fill)" hint="Prefer models sized to prompt context">
                <RangeInline value={routing.balance} min={0} max={100} onChange={v => updateRouting({ balance: v })} />
              </Row>
              <Row label="Mobile / local bias" hint="Prefer Ollama / mesh when set high">
                <RangeInline value={routing.mobile} min={0} max={100} onChange={v => updateRouting({ mobile: v })} />
              </Row>
            </div>
            )}

            {/* Ordered priority chain — the additive, orderable form of emphasis.
                An EmphasizeAxis step is the ordered version of the sliders above. */}
            <PriorityChainEditor pushToast={pushToast} />
          </>
        )}

        {section === 'runtime' && <RuntimeConfigSection pushToast={pushToast} />}

        {section === 'mesh' && <MeshPeersSection pushToast={pushToast} />}

        {section === 'signing' && <SigningKeysSection vals={vals} update={update} pushToast={pushToast} />}

        {section === 'secrets' && <KeysSecretsSection pushToast={pushToast} />}

        {section === 'telemetry' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Telemetry</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Where Vox sends spans, metrics, and traces</p>
            <div className="mt-4 grid grid-cols-3 gap-2">
              {([['off', 'Off', 'Nothing leaves the device'], ['local', 'Local', 'OTLP → localhost:4317'], ['cloud', 'Cloud', 'Encrypted → vendor']] as [string, string, string][]).map(([id, l, h]) => (
                <button
                  key={id}
                  onClick={() => update({ telemetry: id })}
                  className={`rounded-xl border p-3 text-left transition ${
                    vals.telemetry === id ? 'border-brass/40 bg-brass/[0.05]' : 'border-white/5 hover:border-white/15 bg-white/[0.02]'
                  }`}
                >
                  <div className="font-display text-[12px] tracking-wider text-zinc-100">{l}</div>
                  <div className="mt-1 font-mono text-[10px] text-zinc-500">{h}</div>
                </button>
              ))}
            </div>
          </>
        )}

        {section === 'keybinds' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Keybinds</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Global shortcuts</p>
            <div className="mt-4 grid grid-cols-1 gap-1.5 md:grid-cols-2">
              {KEYBINDS.map(([k, d]) => (
                <div key={k} className="flex items-center justify-between rounded-md border border-white/5 bg-white/[0.02] px-3 py-2">
                  <span className="text-[12px] text-zinc-200">{d}</span>
                  <kbd className="rounded border border-white/10 bg-white/5 px-2 py-0.5 font-mono text-[10px] text-zinc-300">{k}</kbd>
                </div>
              ))}
            </div>
          </>
        )}

        {section === 'gamify' && (
          <div className="space-y-3">
            <label className="flex items-center justify-between rounded-lg border border-white/10 bg-white/[0.02] p-3 text-sm">
              <span>Gamification enabled</span>
              <input type="checkbox" checked={gamify.enabled} onChange={e => updateGamify({ enabled: e.target.checked })} />
            </label>
            <label className="flex items-center justify-between rounded-lg border border-white/10 bg-white/[0.02] p-3 text-sm">
              <span>Mode</span>
              <select value={gamify.mode} disabled={!gamify.enabled}
                onChange={e => updateGamify({ mode: e.target.value })}
                className="rounded bg-black/40 px-2 py-1 text-zinc-200">
                <option value="balanced">Balanced</option>
                <option value="serious">Serious (silent)</option>
                <option value="learning">Learning</option>
              </select>
            </label>
            <p className="text-[11px] text-zinc-500">Serious mode keeps rewards active but hides overlays and hints.</p>
          </div>
        )}

        {section === 'theme' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Theme</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Aesthetic mode for the GUI layer</p>
            <div className="mt-4 grid grid-cols-2 gap-3 md:grid-cols-3">
              {[
                { id: 'arcane',  name: 'Arcane',  swatch: 'from-brass via-amber-600 to-zinc-900' },
                { id: 'void',    name: 'Void',    swatch: 'from-violet-500 via-zinc-800 to-zinc-950' },
                { id: 'glacier', name: 'Glacier', swatch: 'from-cyan-400 via-slate-700 to-zinc-950' },
              ].map(t => (
                <button
                  key={t.id}
                  onClick={() => update({ theme: t.id })}
                  className={`rounded-xl border p-3 text-left transition ${
                    vals.theme === t.id ? 'border-brass/40 bg-brass/[0.05]' : 'border-white/5 hover:border-white/15 bg-white/[0.02]'
                  }`}
                >
                  <div className={`h-16 w-full rounded-lg bg-gradient-to-br ${t.swatch}`} />
                  <div className="mt-2 font-display text-[12px] tracking-wider text-zinc-200">{t.name}</div>
                </button>
              ))}
            </div>
          </>
        )}
      </Glass>
    </div>
  );
}
