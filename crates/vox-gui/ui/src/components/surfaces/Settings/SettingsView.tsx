import React, { useCallback, useEffect, useRef, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { scrollAndFocusAnchor } from '../../../lib/anchorFocus';
import { Glass } from '../../ui/Glass';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Icon } from '../../ui/Icons';
import { voxTransport } from '../../../transport';
import type { OrchestratorStatus, RoutingSummary, Toast } from '../../../types/tauri';
import { DEFAULT_BUDGET_CAP_USD } from '../../../config/budget';
import { PriorityChainEditor } from './PriorityChainEditor';
import { TaskPolicySection } from './TaskPolicySection';
import { HudTilesEditor } from './HudTilesEditor';
import { applyTheme } from '../../../lib/theme';
import { useLocalStorage } from '../../../hooks/useLocalStorage';
import { useLang, useLabel } from '../../../hooks/useLanguage';
import { labelFor } from '../../../lib/lexicon';
import { useVoxMutation } from '../../../hooks/useVoxQuery';
import { searchSettings } from './settingsIndex';
import type { HudTilesConfig } from '../../../hooks/useHudTiles';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';
import { ACTION_REGISTRY, DEFAULT_BINDINGS, type Bindings, parseBindings, serializeBindings, chordFromEvent } from '../../../lib/keybinds';
import { useOnboardingGate } from '../Onboarding/useOnboardingGate';

const GUI_PREF_KEYS = ['theme', 'telemetry', 'sign', 'checkpointMins'] as const;

const SECTIONS = [
  { id: 'orchestrator', icon: 'cpu',     label: 'Orchestrator' },
  { id: 'scaling',      icon: 'cpu',     label: 'Scaling' },
  { id: 'llm',          icon: 'bolt',    label: 'LLM & providers' },
  { id: 'routing',      icon: 'matrix',  label: 'Model routing' },
  { id: 'runtime',      icon: 'flow',    label: 'Runtime' },
  { id: 'voice',        icon: 'bolt',    label: 'Voice & dictation' },
  { id: 'mesh',         icon: 'flow',    label: 'Mesh & peers' },
  { id: 'signing',      icon: 'shield',  label: 'Signing keys' },
  { id: 'secrets',      icon: 'shield',  label: 'Keys & Secrets' },
  { id: 'telemetry',    icon: 'scale',   label: 'Telemetry' },
  { id: 'onboarding',   icon: 'refresh', label: 'Onboarding' },
  { id: 'keybinds',     icon: 'command', label: 'Keybinds' },
  { id: 'theme',        icon: 'spark',   label: 'Theme' },
  { id: 'display',      icon: 'monitor', label: 'Display' },
  { id: 'gamify',       icon: 'bolt',    label: 'Gamification' },
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
  scalingEnabled: boolean;
  harnessIssueDetectionEnabled: boolean;
  minAgents: number;
  scalingThreshold: number;
  scaleCpuCeilingPct: number;
  scaleMemFloorMb: number;
}

function Row({ label, hint, children }: { label: string; hint: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-xl border border-border-subtle bg-overlay-subtle p-3">
      <div>
        <div className="font-display text-[12px] text-text-secondary">{label}</div>
        <div className="text-[11px] text-text-muted">{hint}</div>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Toggle({ on, onClick }: { on: boolean; onClick: () => void }) {
  return (
    <button type="button" aria-label={on ? 'Toggle on' : 'Toggle off'} aria-pressed={on} onClick={onClick} className={`relative h-5 w-9 rounded-full transition ${on ? 'bg-brass/40' : 'bg-overlay-subtle'}`}>
      <span className={`absolute top-0.5 size-4 rounded-full bg-[#fafafa] transition ${on ? 'left-[18px]' : 'left-0.5'}`} />
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
      <span className="w-14 text-right font-mono text-[11px] text-text-secondary">{value}{suffix}</span>
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
  cpu_usage_pct?: number | null;
  memory_free_bytes?: number | null;
  gpu_total_count?: number | null;
  gpu_allocatable_count?: number | null;
}

/** Aggregate the node list into the at-a-glance resource summary (client-side
 * mirror of the populi /v1/populi/resources/summary endpoint). */
function aggregateMeshResources(nodes: MeshNode[]) {
  const ready = nodes.filter(n => n.status === 'online');
  const gpusFree = ready.reduce((s, n) => s + (n.gpu_allocatable_count ?? 0), 0);
  const ramFreeGib = ready.reduce((s, n) => s + (n.memory_free_bytes ?? 0), 0) / 2 ** 30;
  const cpuVals = ready.map(n => n.cpu_usage_pct).filter((c): c is number => typeof c === 'number');
  const cpuAvg = cpuVals.length ? cpuVals.reduce((a, b) => a + b, 0) / cpuVals.length : 0;
  return { readyCount: ready.length, total: nodes.length, gpusFree, ramFreeGib, cpuAvg, hasMetrics: cpuVals.length > 0 || gpusFree > 0 || ramFreeGib > 0 };
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

function MeshPeersSection({ pushToast }: { pushToast: (t: Toast) => void }) {
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
      pushToast({ tone: 'warn', title: 'Mesh load failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
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
        pushToast({ tone: 'ok', title: 'Peer untrusted', body: n.id, cause: 'backend-ok' });
      } else {
        await invoke<boolean>('trust_mesh_node', {
          nodeId: n.id,
          pubkeyHex: n.ed25519_pub_key_b64 ?? '',
          label: n.host_triple ?? null,
        });
        pushToast({ tone: 'ok', title: 'Peer trusted', body: n.id, cause: 'backend-ok' });
      }
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Trust update failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-mesh-peers')}</h2>
      <p className="mt-0.5 text-[11px] text-text-muted">Discover and authorise peer compute on the local mesh (source: {meta.source ?? '—'})</p>
      {meta.control_plane_error && (
        <div className="mt-3 rounded-md border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-300">
          Control plane unreachable — showing local registry. <span className="font-mono">{meta.control_plane_error}</span>
        </div>
      )}
      {loading ? (
        <div className="mt-4 text-[12px] text-text-muted">Loading peers…</div>
      ) : nodes.length === 0 ? (
        <div className="mt-4 rounded-md border border-border-subtle bg-overlay-subtle p-4 text-[11px] leading-relaxed text-text-muted">
          No mesh peers. Join one with <code className="font-mono text-text-muted">vox populi join</code>, or configure a control plane via{' '}
          <code className="font-mono text-text-muted">VOX_ORCHESTRATOR_MESH_CONTROL_URL</code>.
        </div>
      ) : (
        <div className="mt-4 space-y-2">
          {(() => {
            const r = aggregateMeshResources(nodes);
            if (!r.hasMetrics) return null;
            return (
              <div className="grid grid-cols-4 gap-2 rounded-lg border border-border-subtle bg-overlay-subtle p-3 text-center">
                <div><div className="text-[18px] text-text-primary">{r.readyCount}/{r.total}</div><div className="text-[9px] uppercase tracking-widest text-text-muted">nodes ready</div></div>
                <div><div className="text-[18px] text-text-primary">{r.gpusFree}</div><div className="text-[9px] uppercase tracking-widest text-text-muted">GPUs free</div></div>
                <div><div className="text-[18px] text-text-primary">{r.ramFreeGib.toFixed(0)} GiB</div><div className="text-[9px] uppercase tracking-widest text-text-muted">RAM free</div></div>
                <div><div className="text-[18px] text-text-primary">{r.cpuAvg.toFixed(0)}%</div><div className="text-[9px] uppercase tracking-widest text-text-muted">avg CPU</div></div>
              </div>
            );
          })()}
          {nodes.map(p => {
            const isTrusted = !!trusted[p.id];
            const online = p.status === 'online';
            return (
              <div key={p.id} className="flex items-center justify-between rounded-md border border-border-subtle bg-overlay-subtle p-3">
                <div className="flex items-center gap-3">
                  <span className={`size-2 rounded-full ${online ? 'bg-emerald-400' : 'bg-text-muted'}`} />
                  <div className="leading-tight">
                    <div className="font-mono text-[12px] text-text-primary break-all">{p.id}</div>
                    <div className="font-mono text-[10px] text-text-muted">{(p.host_triple ?? '—')} · {(p.gpu_summary ?? '—')}</div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <span className={`rounded-full px-2 py-0.5 font-display text-[9px] uppercase tracking-widest ${
                    isTrusted ? 'bg-emerald-400/15 text-emerald-300' : 'bg-bg-elevated/40 text-text-muted'
                  }`}>{isTrusted ? 'trusted' : (p.trust_tier ?? 'untrusted')}</span>
                  <button
                    type="button"
                    disabled={busy === p.id}
                    onClick={() => toggleTrust(p)}
                    className="rounded-sm border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle disabled:opacity-40"
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

function SigningKeysSection({ vals, update, pushToast, gamifyEnabled }: {
  vals: SettingsState; update: (patch: Partial<SettingsState>) => void; pushToast: (t: Toast) => void;
  gamifyEnabled?: boolean;
}) {
  const [key, setKey] = useState<SigningKeyDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [rotating, setRotating] = useState(false);

  const reload = useCallback(async () => {
    try {
      setKey(await invoke<SigningKeyDto>('signing_key_status'));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Could not load signing key', body: sanitizeErrorForToast(err), cause: 'backend-error' });
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
      pushToast({ tone: 'ok', title: `Key ${verb}d`, body: next.nodeId || next.fingerprint, cause: 'backend-ok' });
      if (present) {
        void recordGamifyGuiEvent(
          'signing_key_rotated',
          { node_id: next.nodeId, fingerprint: next.fingerprint },
          { enabled: gamifyEnabled },
        );
      }
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: `Key ${verb} failed`, body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setRotating(false);
    }
  };

  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-signing')}</h2>
      <p className="mt-0.5 text-[11px] text-text-muted">ed25519 capability gate for high-risk dispatch (local node identity)</p>
      <div className="mt-4 space-y-2">
        {loading ? (
          <div className="text-[12px] text-text-muted">Loading…</div>
        ) : !key?.present ? (
          <div className="rounded-md border border-border-subtle bg-overlay-subtle p-4">
            <div className="text-[11px] text-text-muted">No local node identity yet. Create one to enable signed dispatch.</div>
            <button
              type="button"
              disabled={rotating}
              onClick={rotate}
              className="mt-3 rounded-sm border border-border-subtle bg-overlay-subtle px-3 py-1.5 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle disabled:opacity-40"
            >{rotating ? 'working…' : 'create identity'}</button>
          </div>
        ) : (
          <div className="flex items-center justify-between rounded-md border border-border-subtle bg-overlay-subtle p-3">
            <div className="flex items-center gap-3">
              <Icon.shield className="size-4 text-amber-300" />
              <div className="leading-tight">
                <div className="font-mono text-[12px] text-text-primary">{key.nodeId || '(locked)'}</div>
                <div className="font-mono text-[10px] text-text-muted">{key.fingerprint}</div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <span className="rounded-full bg-overlay-subtle px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-text-secondary">{key.algorithm}</span>
              <button
                type="button"
                disabled={rotating}
                onClick={rotate}
                className="rounded-sm border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle disabled:opacity-40"
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

export function KeysSecretsSection({ pushToast, gamifyEnabled }: { pushToast: (t: Toast) => void; gamifyEnabled?: boolean }) {
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
      pushToast({ tone: 'warn', title: 'Could not load secrets', body: sanitizeErrorForToast(err), cause: 'backend-error' });
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
      pushToast({ tone: 'ok', title: 'Auth store migrated', body: `${moved} entr${moved === 1 ? 'y' : 'ies'} moved to vault`, cause: 'backend-ok' });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Migrate failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
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
        pushToast({ tone: 'warn', title: 'No managed secrets found', body: envPath || '.env', cause: 'validation' });
      }
    } catch (err) {
      setPreview(null);
      pushToast({ tone: 'warn', title: 'Preview failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setImportBusy(false);
    }
  };

  const runImport = async () => {
    setImportBusy(true);
    try {
      const res = await invoke<ImportEnvResultDto>('import_env', { path: envPath || null, apply: true });
      setPreview(null);
      pushToast({ tone: 'ok', title: 'Secrets imported', body: `${res.count} value${res.count === 1 ? '' : 's'} stored in vault`, cause: 'backend-ok' });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Import failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
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
      pushToast({ tone: 'ok', title: 'Secret saved', body: key, cause: 'backend-ok' });
      void recordGamifyGuiEvent('secret_rotated', { key }, { enabled: gamifyEnabled });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Save failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(null);
    }
  };

  const remove = async (key: string) => {
    setBusy(key);
    try {
      await invoke<boolean>('remove_secret', { key });
      pushToast({ tone: 'ok', title: 'Secret removed', body: key, cause: 'backend-ok' });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Remove failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(null);
    }
  };

  const renderSecretRow = (r: SecretStatusDto) => (
    <div key={r.id} className="rounded-md border border-border-subtle bg-overlay-subtle p-3">
      <div className="flex items-center justify-between gap-3">
        <div className="leading-tight">
          <div className="flex items-center gap-2">
            <span className="font-mono text-[12px] text-text-primary">{r.canonicalEnv}</span>
            {r.required && (
              <span className="rounded-full bg-amber-400/15 px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-amber-300">required</span>
            )}
          </div>
          <div className="mt-0.5 text-[10px] text-text-muted">{r.scopeDescription}</div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <span className={`rounded-full px-2 py-0.5 font-display text-[9px] uppercase tracking-widest ${
            r.isPresent ? 'bg-emerald-400/15 text-emerald-300' : 'bg-bg-elevated/40 text-text-muted'
          }`}>{r.isPresent ? 'set' : 'missing'}</span>
          {r.isPresent && (
            <span className="font-mono text-[10px] text-text-muted">{r.redacted}</span>
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
          className="flex-1 rounded-sm border border-border-subtle bg-black/30 px-2 py-1 font-mono text-[11px] text-text-primary placeholder:text-text-muted focus:border-brass/40 focus:outline-hidden"
        />
        <button
          type="button"
          disabled={!drafts[r.canonicalEnv] || busy === r.canonicalEnv}
          onClick={() => save(r.canonicalEnv)}
          className="rounded-sm border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle disabled:opacity-40"
        >save</button>
        <button
          type="button"
          disabled={!r.isPresent || busy === r.canonicalEnv}
          onClick={() => remove(r.canonicalEnv)}
          className="rounded-sm border border-rose-500/20 bg-rose-500/4 px-2 py-1 font-mono text-[10px] text-rose-300 hover:bg-rose-500/10 disabled:opacity-40"
        >remove</button>
      </div>
    </div>
  );

  return (
    <>
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-secrets')}</h2>
        {status && (
          <>
            <span className="rounded-full bg-overlay-subtle px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-text-secondary" title="Active secrets backend mode">
              backend: {status.backendMode}
            </span>
            <span className={`rounded-full px-2 py-0.5 font-display text-[9px] uppercase tracking-widest ${
              status.strict ? 'bg-amber-400/15 text-amber-300' : 'bg-overlay-subtle text-text-secondary'
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
      <p className="mt-0.5 text-[11px] text-text-muted">
        Managed API keys and tokens (Vox Secrets / Clavis). Values are write-only — once saved they are never shown again, only a redacted preview.
      </p>

      {/* Actions: migrate auth.json + import .env */}
      <div className="mt-4 rounded-md border border-border-subtle bg-overlay-subtle p-3">
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            disabled={importBusy}
            onClick={migrate}
            className="rounded-sm border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle disabled:opacity-40"
          >Migrate auth.json → vault</button>
          <input
            type="text"
            value={envPath}
            placeholder="default .env (optional path)"
            onChange={e => { setEnvPath(e.target.value); setPreview(null); }}
            className="min-w-[180px] flex-1 rounded-sm border border-border-subtle bg-black/30 px-2 py-1 font-mono text-[11px] text-text-primary placeholder:text-text-muted focus:border-brass/40 focus:outline-hidden"
          />
          <button
            type="button"
            disabled={importBusy}
            onClick={runPreview}
            className="rounded-sm border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle disabled:opacity-40"
          >Preview</button>
          {preview && preview.count > 0 && (
            <button
              type="button"
              disabled={importBusy}
              onClick={runImport}
              className="rounded-sm border border-emerald-400/20 bg-emerald-400/6 px-2 py-1 font-mono text-[10px] text-emerald-300 hover:bg-emerald-400/10 disabled:opacity-40"
            >Import {preview.count}</button>
          )}
        </div>
        {preview && (
          <div className="mt-2 rounded-sm border border-border-subtle bg-black/20 p-2">
            <div className="font-display text-[10px] uppercase tracking-widest text-text-muted">
              {preview.count} managed secret{preview.count === 1 ? '' : 's'} would import (names only — no values shown)
            </div>
            {preview.entries.length > 0 && (
              <div className="mt-1 flex flex-col gap-0.5">
                {preview.entries.map(e => (
                  <div key={e.sourceKey} className="flex items-center gap-2 font-mono text-[10px] text-text-muted">
                    <span className="text-text-secondary">{e.sourceKey}</span>
                    <span className="text-text-muted">→</span>
                    <span className="text-text-secondary">{e.canonicalEnv}</span>
                    <span className="text-text-muted">{e.redacted}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>

      {loading ? (
        <div className="mt-4 text-[12px] text-text-muted">Loading…</div>
      ) : (
        <div className="mt-4 space-y-2">
          {groups.map(g => {
            const isCollapsed = !!collapsed[g.slug];
            return (
              <div key={g.slug} className="rounded-md border border-border-subtle bg-overlay-subtle">
                <button
                  type="button"
                  onClick={() => toggleGroup(g.slug)}
                  className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left hover:bg-overlay-subtle"
                >
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-[10px] text-text-muted">{isCollapsed ? '▸' : '▾'}</span>
                    <span className="rounded-full bg-overlay-subtle px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-text-secondary">{g.slug}</span>
                    {g.needsAttention && (
                      <span className="rounded-full bg-amber-400/15 px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-amber-300">action needed</span>
                    )}
                  </div>
                  <span className="font-mono text-[10px] text-text-muted">
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

/** Recorded LLM spend vs budget caps, mirrors Rust `LlmSpendDto`. */
interface LlmSpendDto {
  sessionUsd: number;
  dayUsd: number;
  totalUsd: number;
  dailyBudgetUsd: number;
  perSessionBudgetUsd: number;
}

function RuntimeConfigSection({ pushToast }: { pushToast: (t: any) => void }) {
  const [fields, setFields] = useState<UserConfigFieldDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [spend, setSpend] = useState<LlmSpendDto | null>(null);
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
      pushToast({ tone: 'warn', title: 'Could not load runtime config', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setLoading(false);
    }
    // Recorded spend vs budget (single SSOT aggregate); best-effort, never blocks config.
    try {
      setSpend(await invoke<LlmSpendDto>('get_llm_spend', {}));
    } catch {
      /* store may be unavailable on a fresh install — leave spend hidden */
    }
  }, [pushToast]);

  useEffect(() => { reload(); }, [reload]);
  // Reactive refresh: the backend emits "vox://llm-config-changed" whenever config
  // changes (this GUI, env reload, or mesh sync). Re-pull the catalog on each event.
  useEffect(() => {
    // Guarded: listen() rejects outside Tauri (bare browser/tests).
    const un = listen('vox://llm-config-changed', () => { reload(); }).catch(() => undefined);
    return () => { un.then((f) => f?.()); };
  }, [reload]);
  useEffect(() => () => { if (savedToast.current) clearTimeout(savedToast.current); }, []);

  const save = async (f: UserConfigFieldDto, value: string) => {
    setBusy(f.key);
    try {
      await invoke('set_user_config', { key: f.key, value });
      if (savedToast.current) clearTimeout(savedToast.current);
      savedToast.current = setTimeout(() => {
        pushToast({ tone: 'ok', title: 'Setting saved', body: f.label, cause: 'backend-ok' });
      }, 600);
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Save failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(null);
    }
  };

  const reset = async (f: UserConfigFieldDto) => {
    setBusy(f.key);
    try {
      await invoke('reset_user_config', { key: f.key });
      pushToast({ tone: 'ok', title: 'Reset to default', body: f.label, cause: 'backend-ok' });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Reset failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(null);
    }
  };

  const draftFor = (f: UserConfigFieldDto) => drafts[f.key] ?? f.currentValue;

  const control = (f: UserConfigFieldDto) => {
    if (f.kind === 'enum') {
      return (
        <div className="inline-flex flex-wrap items-center rounded-md border border-border-subtle bg-black/30 p-0.5">
          {f.options.map(opt => (
            <button
              key={opt}
              type="button"
              disabled={busy === f.key}
              onClick={() => save(f, opt)}
              className={`rounded-[5px] px-2 py-1 font-display text-[10px] uppercase tracking-[0.12em] transition disabled:opacity-40 ${
                f.currentValue === opt ? 'bg-overlay-subtle text-text-primary' : 'text-text-muted hover:text-text-secondary'
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
          className="w-56 rounded-sm border border-border-subtle bg-black/30 px-2 py-1 font-mono text-[11px] text-text-primary placeholder:text-text-muted focus:border-brass/40 focus:outline-hidden"
        />
        <button
          type="button"
          disabled={busy === f.key || draftFor(f) === f.currentValue}
          onClick={() => save(f, draftFor(f))}
          className="rounded-sm border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle disabled:opacity-40"
        >save</button>
      </div>
    );
  };

  const groups = RUNTIME_GROUP_ORDER
    .map(g => ({ group: g, items: fields.filter(f => f.group === g) }))
    .filter(g => g.items.length > 0);

  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-runtime')}</h2>
      <p className="mt-0.5 text-[11px] text-text-muted">
        Core user config persisted to your Vox user config (effective values: ENV &gt; Vox.toml &gt; global &gt; defaults)
      </p>
      {spend && (
        <div className="mt-3 rounded-sm border border-border-subtle bg-black/20 p-3" data-testid="llm-spend">
          <div className="font-mono text-[10px] uppercase tracking-wide text-text-muted">LLM spend (recorded actuals)</div>
          <div className="mt-1 flex flex-wrap gap-x-6 gap-y-1 font-mono text-[11px] text-text-secondary">
            <span>session ${spend.sessionUsd.toFixed(4)} / ${spend.perSessionBudgetUsd.toFixed(2)}</span>
            <span>today ${spend.dayUsd.toFixed(4)} / ${spend.dailyBudgetUsd.toFixed(2)}</span>
            <span>total ${spend.totalUsd.toFixed(4)}</span>
          </div>
        </div>
      )}
      {loading ? (
        <div className="mt-4 text-[12px] text-text-muted">Loading…</div>
      ) : (
        <div className="mt-4 space-y-5">
          {groups.map(({ group, items }) => (
            <div key={group}>
              <div className="font-display text-[11px] uppercase tracking-[0.15em] text-text-muted">{group}</div>
              <div className="mt-2 space-y-2">
                {items.map(f => (
                  <Row key={f.key} label={f.label} hint={f.hint}>
                    <div className="flex items-center gap-2">
                      {control(f)}
                      <button
                        type="button"
                        disabled={busy === f.key}
                        onClick={() => reset(f)}
                        title="Reset to default"
                        className="rounded-sm border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[10px] text-text-muted hover:bg-overlay-subtle disabled:opacity-40"
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

function LlmSettingsSection({ pushToast, onJumpToKeysSecrets }: { pushToast: (t: any) => void; onJumpToKeysSecrets: () => void }) {
  const [cfg, setCfg] = useState({
    maxConcurrentRequests: 8,
    openrouterMaxConcurrent: null as number | null,
    retryMaxAttempts: 4,
  });
  const [keyConfigured, setKeyConfigured] = useState<boolean | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const c = await invoke<Record<string, unknown>>('get_llm_config');
        setCfg({
          maxConcurrentRequests: Number(c.max_concurrent_requests ?? 8),
          openrouterMaxConcurrent:
            c.openrouter_max_concurrent == null ? null : Number(c.openrouter_max_concurrent),
          retryMaxAttempts: Number(c.retry_max_attempts ?? 4),
        });
      } catch { /* defaults */ }
      try {
        const s = await invoke<{ configured: boolean }>('openrouter_key_status');
        setKeyConfigured(s.configured);
      } catch { /* probe best-effort */ }
    })();
  }, []);

  const save = (next: typeof cfg) => {
    setCfg(next);
    invoke('set_llm_config', {
      config: {
        maxConcurrentRequests: next.maxConcurrentRequests,
        openrouterMaxConcurrent: next.openrouterMaxConcurrent,
        retryMaxAttempts: next.retryMaxAttempts,
      },
    }).catch((err) => pushToast({ tone: 'warn', title: 'LLM save failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
  };

  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-llm')}</h2>
      <p className="mt-0.5 text-[11px] text-text-muted">Concurrency throttle for LLM egress. OpenRouter paid tier has no platform request cap, so parallelism is the real dial.</p>
      <div className="mt-4 space-y-3">
        <Row label="Max parallel LLM requests" hint="Global ceiling across all providers (AIMD throttle)">
          <RangeInline value={cfg.maxConcurrentRequests} min={1} max={64} onChange={v => save({ ...cfg, maxConcurrentRequests: v })} />
        </Row>
        <Row label="OpenRouter override" hint="Provider-specific cap (0 = use global)">
          <RangeInline value={cfg.openrouterMaxConcurrent ?? 0} min={0} max={64} onChange={v => save({ ...cfg, openrouterMaxConcurrent: v === 0 ? null : v })} />
        </Row>
        <Row label="429 retry attempts" hint="Backoff retries before surfacing a rate-limit error">
          <RangeInline value={cfg.retryMaxAttempts} min={0} max={10} onChange={v => save({ ...cfg, retryMaxAttempts: v })} />
        </Row>
      </div>
      {keyConfigured !== null && (
        <div className="mt-4 rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-2 text-[11px] text-text-muted">
          {keyConfigured ? (
            'OpenRouter API key is configured.'
          ) : (
            <>
              No OpenRouter key configured —{' '}
              <button
                type="button"
                onClick={onJumpToKeysSecrets}
                className="text-brass underline hover:no-underline"
              >
                add one under Keys & Secrets
              </button>
              .
            </>
          )}
        </div>
      )}
    </>
  );
}

function OnboardingSection() {
  // This section only ever calls `.replay()`, a pure `setDismissed(false)` — the
  // dummy input values deliberately produce `shouldShow: false` from this call site.
  const gate = useOnboardingGate({ secretCount: 1, localModelCount: 0 });
  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">Onboarding</h2>
      <p className="mt-0.5 text-[11px] text-text-muted">Replay the first-run setup wizard.</p>
      <button
        type="button"
        onClick={gate.replay}
        className="mt-3 rounded-lg border border-border-subtle px-3 py-1.5 text-[11px] hover:bg-overlay-subtle"
      >
        Replay setup wizard
      </button>
    </>
  );
}

interface SttConfigFieldDto {
  key: string;
  label: string;
  hint: string;
  options: string[];
  currentValue: string;
}

function SttSettingsSection({ pushToast }: { pushToast: (t: Toast) => void }) {
  const [fields, setFields] = useState<SttConfigFieldDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setFields(await invoke<SttConfigFieldDto[]>('get_stt_config'));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Could not load voice settings', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => { reload(); }, [reload]);

  const save = async (key: string, value: string) => {
    setBusy(key);
    try {
      await invoke('set_stt_config', { key, value });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Save failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">Voice &amp; dictation</h2>
      <p className="mt-0.5 text-[11px] text-text-muted">ASR engine and dictation domain for the mic button in chat.</p>
      {loading ? (
        <div className="mt-4 text-[12px] text-text-muted">Loading…</div>
      ) : (
        <div className="mt-4 space-y-2">
          {fields.map(f => (
            <Row key={f.key} label={f.label} hint={f.hint}>
              <div className="inline-flex flex-wrap items-center rounded-md border border-border-subtle bg-black/30 p-0.5">
                {f.options.map(opt => (
                  <button
                    key={opt}
                    type="button"
                    disabled={busy === f.key}
                    onClick={() => save(f.key, opt)}
                    className={`rounded-[5px] px-2 py-1 font-display text-[10px] uppercase tracking-[0.12em] transition disabled:opacity-40 ${
                      f.currentValue === opt ? 'bg-overlay-subtle text-text-primary' : 'text-text-muted hover:text-text-secondary'
                    }`}
                  >{opt}</button>
                ))}
              </div>
            </Row>
          ))}
        </div>
      )}
    </>
  );
}

interface SettingsViewProps {
  pushToast: (t: Toast) => void;
  gamifyEnabled?: boolean;
  hudTilesConfig?: HudTilesConfig;
  onHudTilesChange?: (config: HudTilesConfig) => void;
}

export function SettingsView({ pushToast, gamifyEnabled, hudTilesConfig, onHudTilesChange }: SettingsViewProps) {
  const { lang, setLang } = useLang();
  const [section, setSection] = useState('orchestrator');
  const [filter, setFilter] = useState('');
  const [keybindings, setKeybindings] = useState<Bindings>(DEFAULT_BINDINGS);
  const [capturingId, setCapturingId] = useState<string | null>(null);
  useEffect(() => {
    voxTransport.getGuiPreference('gui.keybinds')
      .then(json => setKeybindings(parseBindings(json)))
      .catch(() => setKeybindings(DEFAULT_BINDINGS));
  }, []);
  useEffect(() => {
    if (!capturingId) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const chord = chordFromEvent(e);
      const next = { ...keybindings, [capturingId]: chord };
      setKeybindings(next);
      setCapturingId(null);
      voxTransport.setGuiPreference('gui.keybinds', serializeBindings(next)).catch(() => {});
    };
    window.addEventListener('keydown', onKey, { capture: true });
    return () => window.removeEventListener('keydown', onKey, { capture: true });
  }, [capturingId, keybindings]);

  // Deep link from omni-search: { section } seed in localStorage. Read on mount
  // AND on the 'vox-settings-seed' event, so deep-linking works even when the
  // Settings surface is already active (no remount fires in that case).
  useEffect(() => {
    const consume = () => {
      try {
        const raw = localStorage.getItem('vox_settings_seed');
        if (raw) {
          localStorage.removeItem('vox_settings_seed');
          const seed = JSON.parse(raw) as { section?: string };
          if (seed.section) setSection(seed.section);
        }
      } catch { /* ignore malformed seed */ }
    };
    consume();
    window.addEventListener('vox-settings-seed', consume);
    return () => window.removeEventListener('vox-settings-seed', consume);
  }, []);
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
    capUsd: DEFAULT_BUDGET_CAP_USD, doubtThresh: 0.6, sign: false, telemetry: 'local',
    isolation: 'wasm', checkpointMins: 5,
    scalingEnabled: false, minAgents: 1, scalingThreshold: 5,
    harnessIssueDetectionEnabled: true,
    scaleCpuCeilingPct: 85, scaleMemFloorMb: 1024,
  });

  // Mirror `vals` into a ref so `update` reads the freshest state, not a stale
  // closure capture. Without this, a rapid second edit builds `next` from the
  // pre-first-edit `vals` and persists the first field's old value to Vox.toml.
  // The ref is written synchronously in `update` (not just via this effect) so
  // two `update` calls in the same tick — before React re-renders — still
  // compose instead of the second dropping the first's patch.
  const valsRef = useRef(vals);
  useEffect(() => { valsRef.current = vals; }, [vals]);

  // Becomes true once mount hydration finishes. Until then `update` must not push
  // hardcoded default OrchestratorConfig values to disk (an early click before
  // get_orchestrator_config resolves would otherwise clobber real config).
  const [hydrated, setHydrated] = useState(false);
  const [prefAnnounce, setPrefAnnounce] = useState('');

  const guiPrefMutation = useVoxMutation(
    async (patch: Partial<SettingsState>) => {
      for (const [k, v] of Object.entries(patch)) {
        if ((GUI_PREF_KEYS as readonly string[]).includes(k)) {
          await voxTransport.setGuiPreference(`gui.${k}`, String(v));
        }
      }
    },
    {
      onSuccess: () => {
        setPrefAnnounce('Preferences saved');
        window.setTimeout(() => setPrefAnnounce(''), 3000);
      },
    },
  );

  const update = async (patch: Partial<SettingsState>) => {
    const next = { ...valsRef.current, ...patch };
    valsRef.current = next;
    setVals(next);

    // Apply the accent palette immediately on theme change (before/independent
    // of persistence), so the swatch selection takes visible effect at once.
    // `applyTheme` (lib/theme) drives the [data-theme] attribute the CSS hooks
    // key off — the picker was previously inert.
    if (patch.theme !== undefined) applyTheme(next.theme);

    // GUI-only preferences (theme/telemetry/sign/checkpointMins) always persist —
    // they don't depend on orchestrator hydration.
    try {
      const guiPatch = Object.fromEntries(
        Object.entries(patch).filter(([k]) => (GUI_PREF_KEYS as readonly string[]).includes(k)),
      ) as Partial<SettingsState>;
      if (Object.keys(guiPatch).length > 0) {
        await guiPrefMutation.mutateAsync(guiPatch);
      }
      // Defer orchestrator persistence until real values are loaded, so we never
      // write defaults over a user's on-disk config before hydration completes.
      if (hydrated) {
        await invoke('set_orchestrator_config', { config: next });
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Save failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  };

  useEffect(() => {
    voxTransport.getRoutingSummaryLive().then((s: RoutingSummary) => {
      if (s?.routing_priority) setRouting(s.routing_priority);
    }).catch(() => {});
    const hydrate = async () => {
      try {
        const [theme, telemetry, sign, checkpoint, statusRaw] = await Promise.all([
          voxTransport.getGuiPreference('gui.theme'),
          voxTransport.getGuiPreference('gui.telemetry'),
          voxTransport.getGuiPreference('gui.sign'),
          voxTransport.getGuiPreference('gui.checkpointMins'),
          invoke<Uint8Array>('get_orchestrator_status_bin').catch(() => null),
        ]);
        let orchPatch: Partial<SettingsState> = {};
        if (statusRaw) {
          try {
            const { decode } = await import('@msgpack/msgpack');
            const status = decode(statusRaw) as OrchestratorStatus;
            if (typeof status.budget_cap === 'number') orchPatch.capUsd = status.budget_cap;
            if (typeof status.agent_count === 'number') orchPatch.concurrency = status.agent_count;
          } catch {
            // ignore decode errors
          }
        }
        setVals((prev) => ({
          ...prev,
          ...orchPatch,
          theme: theme || prev.theme,
          telemetry: telemetry || prev.telemetry,
          sign: sign != null ? sign === 'true' : prev.sign,
          // checkpointMins is a UI-only preference (no OrchestratorConfig field).
          checkpointMins: checkpoint != null
            ? (Number.isFinite(Number(checkpoint)) ? Number(checkpoint) : prev.checkpointMins)
            : prev.checkpointMins,
        }));
      } catch {
        // keep default local state when preference DB isn't available
      }
      // Hydrate orchestrator + scaling controls from the persisted Vox.toml
      // [orchestrator] table (fixes the inert-sliders bug). Scaling fields are
      // #273's resource-aware scaling; `setHydrated(true)` gates #229's persist
      // path (see `if (hydrated)` in `update`).
      try {
        const cfg = await invoke<Record<string, unknown>>('get_orchestrator_config');
        const num = (k: string) => (cfg[k] == null ? undefined : Number(cfg[k]));
        const bool = (k: string) => (cfg[k] == null ? undefined : Boolean(cfg[k]));
        setVals((prev) => ({
          ...prev,
          ...(num('concurrency') != null ? { concurrency: num('concurrency')! } : {}),
          ...(num('capUsd') != null ? { capUsd: num('capUsd')! } : {}),
          ...(num('doubtThresh') != null ? { doubtThresh: num('doubtThresh')! } : {}),
          ...(typeof cfg.isolation === 'string' ? { isolation: cfg.isolation } : {}),
          ...(bool('autobudget') != null ? { autobudget: bool('autobudget')! } : {}),
          ...(bool('doubt') != null ? { doubt: bool('doubt')! } : {}),
          ...(bool('scalingEnabled') != null ? { scalingEnabled: bool('scalingEnabled')! } : {}),
          ...(bool('harnessIssueDetectionEnabled') != null ? { harnessIssueDetectionEnabled: bool('harnessIssueDetectionEnabled')! } : {}),
          ...(num('minAgents') != null ? { minAgents: num('minAgents')! } : {}),
          ...(num('scalingThreshold') != null ? { scalingThreshold: num('scalingThreshold')! } : {}),
          ...(num('scaleCpuCeilingPct') != null ? { scaleCpuCeilingPct: num('scaleCpuCeilingPct')! } : {}),
          ...(num('scaleMemFloorMb') != null ? { scaleMemFloorMb: num('scaleMemFloorMb')! } : {}),
        }));
      } catch {
        // daemon-less dev: keep defaults
      } finally {
        setHydrated(true);
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
      pushToast({ tone: 'warn', title: 'Gamify save failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
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
        pushToast({ tone: 'ok', title: 'Routing weights saved', body: 'VOX_AUTO_ROUTING_PRIORITY persisted', cause: 'backend-ok' });
      }, 600);
    } catch (err) {
      if (savedToastTimer.current) {
        clearTimeout(savedToastTimer.current);
        savedToastTimer.current = null;
      }
      pushToast({ tone: 'warn', title: 'Routing save failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
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
      <div role="status" aria-live="polite" className="sr-only">
        {prefAnnounce}
      </div>
      <h1 className="col-span-12 font-display text-lg tracking-[0.14em] uppercase text-text-primary">
        {useLabel('settings')}
      </h1>
      {/* Nav */}
      <Glass className="col-span-12 md:col-span-3 p-3">
        <input
          value={filter}
          onChange={e => setFilter(e.target.value)}
          placeholder="Search settings…"
          aria-label="Search settings"
          className="mb-2 w-full rounded-lg border border-border-subtle bg-overlay-subtle px-2.5 py-1.5 text-[12px] text-text-secondary placeholder:text-text-muted outline-hidden focus:border-brass/30"
        />
        {filter.trim() ? (
          <nav className="flex flex-col gap-1">
            {searchSettings(filter).map(entry => (
              <button
                key={entry.id}
                type="button"
                onClick={() => { setSection(entry.section); setFilter(''); }}
                className="flex flex-col rounded-lg px-3 py-2 text-left text-text-secondary hover:bg-overlay-subtle hover:text-text-primary focus:outline-hidden focus-visible:ring-1 focus-visible:ring-brass/40"
              >
                <span className="text-[12px]">{entry.label}</span>
                <span className="text-[10px] text-text-muted">{entry.hint}</span>
              </button>
            ))}
            {searchSettings(filter).length === 0 && (
              <p className="px-3 py-2 text-[11px] text-text-muted">No settings match.</p>
            )}
          </nav>
        ) : (
        <nav className="flex flex-col gap-1">
          {SECTIONS.map(s => {
            const IcoCmp = (Icon as any)[s.icon] ?? Icon.bolt;
            const on = section === s.id;
            return (
              <button
                key={s.id}
                type="button"
                onClick={() => setSection(s.id)}
                className={`flex items-center gap-2.5 rounded-lg px-3 py-2 text-left transition ${
                  on ? 'bg-overlay-subtle text-text-primary' : 'text-text-muted hover:bg-overlay-subtle hover:text-text-secondary'
                }`}
              >
                <IcoCmp className={`size-4 ${on ? 'text-brass' : 'text-text-muted'}`} />
                <span className="font-display text-[12px] tracking-[0.12em] uppercase">{s.label}</span>
              </button>
            );
          })}
        </nav>
        )}
      </Glass>

      {/* Content */}
      <Glass className="col-span-12 md:col-span-9 p-5">
        {section === 'orchestrator' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-orchestrator')}</h2>
            <p className="mt-0.5 text-[11px] text-text-muted">Global scheduling, budget, and verification policy</p>
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
                <div className="inline-flex items-center rounded-md border border-border-subtle bg-black/30 p-0.5">
                  {([['wasm', 'WASM'], ['ctr', 'Container'], ['native', 'Native']] as [string, string][]).map(([id, l]) => (
                    <button
                      key={id}
                      type="button"
                      onClick={() => update({ isolation: id })}
                      className={`rounded-[5px] px-2 py-1 font-display text-[10px] uppercase tracking-[0.15em] transition ${
                        vals.isolation === id ? 'bg-overlay-subtle text-text-primary' : 'text-text-muted hover:text-text-secondary'
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

        {section === 'scaling' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-scaling')}</h2>
            <p className="mt-0.5 text-[11px] text-text-muted">Spawn and retire agents automatically based on queue load and local host resources</p>
            <div className="mt-4 space-y-3">
              <Row label="Auto-scaling" hint="Let the orchestrator add/remove agents dynamically">
                <Toggle on={vals.scalingEnabled} onClick={() => update({ scalingEnabled: !vals.scalingEnabled })} />
              </Row>
              <Row label="Harness issue detection" hint="Watch chat turns for repeated mistakes and surface a review queue">
                <Toggle
                  on={vals.harnessIssueDetectionEnabled}
                  onClick={() => update({ harnessIssueDetectionEnabled: !vals.harnessIssueDetectionEnabled })}
                />
              </Row>
              <Row label="Min agents" hint="Never retire below this fleet size">
                <RangeInline value={vals.minAgents} min={0} max={8} onChange={v => update({ minAgents: v })} />
              </Row>
              <Row label="Max agents" hint="Hard ceiling on concurrent agents">
                <RangeInline value={vals.concurrency} min={1} max={16} onChange={v => update({ concurrency: v })} />
              </Row>
              <Row label="Queue threshold" hint="Per-agent queued tasks that trigger a scale-up">
                <RangeInline value={vals.scalingThreshold} min={1} max={20} onChange={v => update({ scalingThreshold: v })} />
              </Row>
              <Row label="CPU ceiling" hint="Don't spawn agents while local CPU is above this (0 = off)">
                <RangeInline value={vals.scaleCpuCeilingPct} min={0} max={100} step={5} suffix="%" onChange={v => update({ scaleCpuCeilingPct: v })} />
              </Row>
              <Row label="Memory floor" hint="Don't spawn agents below this free RAM (0 = off)">
                <RangeInline value={vals.scaleMemFloorMb} min={0} max={16384} step={256} suffix=" MiB" onChange={v => update({ scaleMemFloorMb: v })} />
              </Row>
            </div>
          </>
        )}

        {section === 'llm' && (
          <LlmSettingsSection
            pushToast={pushToast}
            onJumpToKeysSecrets={() => {
              setSection('secrets');
              requestAnimationFrame(() => {
                scrollAndFocusAnchor('keys-secrets-section');
              });
            }}
          />
        )}

        {section === 'routing' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-routing')}</h2>
            <p className="mt-0.5 text-[11px] text-text-muted">Emphasis tunes how the scorer trades off intelligence, efficiency, and responsiveness (persisted to VOX_AUTO_ROUTING_PRIORITY)</p>

            {/* Emphasis: presets + three labeled characteristic sliders */}
            <div className="mt-4 rounded-xl border border-border-subtle bg-overlay-subtle p-3">
              <div className="font-display text-[12px] tracking-[0.12em] uppercase text-text-secondary">Emphasis</div>
              <div className="mt-3 grid grid-cols-2 gap-2 md:grid-cols-4">
                {EMPHASIS_PRESETS.map(([name, preset]) => (
                  <button
                    key={name}
                    type="button"
                    onClick={() => applyEmphasis(preset)}
                    className={`rounded-lg border p-2 text-center transition ${
                      activePreset === name ? 'border-brass/40 bg-brass/5 text-text-primary' : 'border-border-subtle bg-overlay-subtle text-text-muted hover:border-white/15 hover:text-text-secondary'
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
              type="button"
              onClick={() => setAdvanced(a => !a)}
              className="mt-4 font-display text-[11px] uppercase tracking-[0.15em] text-text-muted hover:text-text-secondary"
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

            {/* Per-task-category / per-trigger-source overrides on top of the
                global emphasis/priority chain above. */}
            <TaskPolicySection />
          </>
        )}

        {section === 'runtime' && <RuntimeConfigSection pushToast={pushToast} />}
        {section === 'voice' && <SttSettingsSection pushToast={pushToast} />}

        {section === 'mesh' && <MeshPeersSection pushToast={pushToast} />}

        {section === 'signing' && (
          <SigningKeysSection vals={vals} update={update} pushToast={pushToast} gamifyEnabled={gamifyEnabled} />
        )}

        {section === 'secrets' && (
          <div id="keys-secrets-section" tabIndex={-1}>
            <KeysSecretsSection pushToast={pushToast} gamifyEnabled={gamifyEnabled} />
          </div>
        )}

        {section === 'telemetry' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-telemetry')}</h2>
            <p className="mt-0.5 text-[11px] text-text-muted">Where Vox sends spans, metrics, and traces</p>
            <div className="mt-4 grid grid-cols-3 gap-2">
              {([['off', 'Off', 'Nothing leaves the device'], ['local', 'Local', 'OTLP → localhost:4317'], ['cloud', 'Cloud', 'Encrypted → vendor']] as [string, string, string][]).map(([id, l, h]) => (
                <button
                  key={id}
                  type="button"
                  onClick={() => update({ telemetry: id })}
                  className={`rounded-xl border p-3 text-left transition ${
                    vals.telemetry === id ? 'border-brass/40 bg-brass/5' : 'border-border-subtle hover:border-white/15 bg-overlay-subtle'
                  }`}
                >
                  <div className="font-display text-[12px] tracking-wider text-text-primary">{l}</div>
                  <div className="mt-1 font-mono text-[10px] text-text-muted">{h}</div>
                </button>
              ))}
            </div>
          </>
        )}

        {section === 'onboarding' && <OnboardingSection />}

        {section === 'keybinds' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-keybinds')}</h2>
            <p className="mt-0.5 text-[11px] text-text-muted">Click a shortcut to rebind it. Press any key combination to set the new chord.</p>
            <div className="mt-4 grid grid-cols-1 gap-1.5 md:grid-cols-2">
              {ACTION_REGISTRY.map(a => (
                <div key={a.id} className="flex items-center justify-between rounded-md border border-border-subtle bg-overlay-subtle px-3 py-2">
                  <span className="text-[12px] text-text-secondary">{a.label}</span>
                  <button
                    type="button"
                    data-testid={`keybind-btn-${a.id}`}
                    onClick={() => setCapturingId(capturingId === a.id ? null : a.id)}
                    className={`rounded border px-2 py-0.5 font-mono text-[10px] transition ${
                      capturingId === a.id
                        ? 'border-brass/60 bg-brass/10 text-brass animate-pulse'
                        : 'border-border-subtle bg-overlay-subtle text-text-secondary hover:border-white/20'
                    }`}
                  >
                    {capturingId === a.id ? 'press keys…' : (keybindings[a.id] ?? DEFAULT_BINDINGS[a.id])}
                  </button>
                </div>
              ))}
            </div>
            <button
              type="button"
              onClick={() => {
                setKeybindings(DEFAULT_BINDINGS);
                setCapturingId(null);
                voxTransport.setGuiPreference('gui.keybinds', serializeBindings(DEFAULT_BINDINGS)).catch(() => {});
              }}
              className="mt-3 rounded-md border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-[11px] text-text-muted hover:text-text-secondary transition"
            >
              Reset to defaults
            </button>
          </>
        )}

        {section === 'gamify' && (
          <div className="space-y-3">
            <label className="flex items-center justify-between rounded-lg border border-border-subtle bg-overlay-subtle p-3 text-sm">
              <span>Gamification enabled</span>
              <input type="checkbox" checked={gamify.enabled} onChange={e => updateGamify({ enabled: e.target.checked })} />
            </label>
            <label className="flex items-center justify-between rounded-lg border border-border-subtle bg-overlay-subtle p-3 text-sm">
              <span>Mode</span>
              <select value={gamify.mode} disabled={!gamify.enabled}
                onChange={e => updateGamify({ mode: e.target.value })}
                className="rounded-sm bg-black/40 px-2 py-1 text-text-secondary">
                <option value="balanced">Balanced</option>
                <option value="serious">Serious (silent)</option>
                <option value="learning">Learning</option>
              </select>
            </label>
            <p className="text-[11px] text-text-muted">Serious mode keeps rewards active but hides overlays and hints.</p>
          </div>
        )}

        {section === 'theme' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('set-theme')}</h2>
            <p className="mt-0.5 text-[11px] text-text-muted">Aesthetic mode for the GUI layer</p>
            <div className="mt-4 grid grid-cols-2 gap-3 md:grid-cols-3">
              {[
                { id: 'arcane',  name: 'Arcane',  swatch: 'from-brass via-amber-600 to-zinc-900' },
                { id: 'void',    name: 'Void',    swatch: 'from-violet-500 via-zinc-800 to-zinc-950' },
                { id: 'glacier', name: 'Glacier', swatch: 'from-cyan-400 via-slate-700 to-zinc-950' },
              ].map(t => (
                <button
                  key={t.id}
                  type="button"
                  onClick={() => update({ theme: t.id })}
                  className={`rounded-xl border p-3 text-left transition ${
                    vals.theme === t.id ? 'border-brass/40 bg-brass/5' : 'border-border-subtle hover:border-white/15 bg-overlay-subtle'
                  }`}
                >
                  <div className={`h-16 w-full rounded-lg bg-linear-to-br ${t.swatch}`} />
                  <div className="mt-2 font-display text-[12px] tracking-wider text-text-secondary">{t.name}</div>
                </button>
              ))}
            </div>
          </>
        )}

        {section === 'display' && (
          <>
            <Row label="Latin labels" hint="Show nav and section names in Latin">
              <Toggle on={lang === 'la'} onClick={() => setLang(lang === 'la' ? 'en' : 'la')} />
            </Row>
            {hudTilesConfig && onHudTilesChange && (
              <HudTilesEditor config={hudTilesConfig} onChange={onHudTilesChange} />
            )}
          </>
        )}
      </Glass>
    </div>
  );
}
