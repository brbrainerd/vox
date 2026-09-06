import React, { useCallback, useEffect, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { mapDiscoveredSkills, type DiscoveredSkill } from './discovery';
import { SkillDetailPanel, type SkillDetail } from './SkillDetailPanel';

// ── Wire types (mirror the MCP tool JSON envelopes) ────────────────────────
interface SkillInfo {
  id: string;
  name: string;
  version: string;
  category: string;
  description: string;
  tools: string[];
  source: string;
  permissions: string[];
  tags: string[];
}

interface PluginRow {
  id: string;
  version: string;
  payload_kind: string;
  install_dir: string;
}

interface CatalogPlugin {
  id: string;
  'payload-kind': string;
  description: string;
  status?: string;
  'default-source'?: string;
}

interface SkillsPluginsViewProps {
  pushToast: (t: any) => void;
}

type Tab = 'installed' | 'marketplace' | 'discovered';

// Every tool call routes through the single persistent daemon via invoke_mcp_tool —
// no dedicated Tauri command. Mutating calls (install/remove/uninstall) flow through
// the daemon's HITL approval gate automatically.
async function callTool<T = any>(tool: string, args: Record<string, any> = {}): Promise<{ is_error: boolean; result: any }> {
  return invoke<{ tool: string; is_error: boolean; result: T }>('invoke_mcp_tool', { tool, args });
}

// MCP envelopes wrap payloads as `{ success, data }`. Pull `.data`, tolerating
// the bare-value shape too.
function unwrap(result: any): any {
  if (result && typeof result === 'object' && 'data' in result) return result.data;
  return result;
}

export function SkillsPluginsView({ pushToast }: SkillsPluginsViewProps) {
  const [tab, setTab] = useState<Tab>('installed');
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);

  const [detail, setDetail] = useState<SkillDetail | null>(null);

  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [plugins, setPlugins] = useState<PluginRow[]>([]);
  const [catalogPlugins, setCatalogPlugins] = useState<CatalogPlugin[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchHits, setSearchHits] = useState<SkillInfo[]>([]);
  const [discovered, setDiscovered] = useState<DiscoveredSkill[]>([]);
  const [addSource, setAddSource] = useState('');

  const refreshInstalled = useCallback(async () => {
    setLoading(true);
    try {
      const [sk, pl] = await Promise.all([
        callTool('vox_skill_list'),
        callTool('vox_plugin_list'),
      ]);
      const skillList = unwrap(sk?.result);
      const pluginList = unwrap(pl?.result);
      setSkills(Array.isArray(skillList) ? skillList : []);
      setPlugins(Array.isArray(pluginList) ? pluginList : []);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Installed load failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  const refreshMarketplace = useCallback(async () => {
    setLoading(true);
    try {
      const cat = await callTool('vox_plugin_catalog');
      const data = unwrap(cat?.result);
      setCatalogPlugins(Array.isArray(data?.plugins) ? data.plugins : []);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Marketplace load failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  const refreshDiscovered = useCallback(async () => {
    setLoading(true);
    try {
      const res = await callTool('vox_skill_discover');
      setDiscovered(mapDiscoveredSkills(unwrap(res?.result)));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Discovery failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  const addSkill = useCallback(async () => {
    if (busy) return; // ignore Enter/click while an add or remove is in flight
    const src = addSource.trim();
    if (!src) return;
    setBusy(src);
    try {
      const res = await callTool('vox_skill_add', { source: src });
      if (res?.is_error) {
        pushToast({ tone: 'warn', title: 'Add failed', body: JSON.stringify(unwrap(res.result)), cause: 'backend-error' });
      } else {
        pushToast({ tone: 'ok', title: 'Skill added', body: src, cause: 'backend-ok' });
        setAddSource('');
        await refreshDiscovered();
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Add failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(null);
    }
  }, [addSource, busy, pushToast, refreshDiscovered]);

  const removeSkill = useCallback(
    async (id: string) => {
      setBusy(id);
      try {
        const res = await callTool('vox_skill_remove', { id });
        if (res?.is_error) {
          pushToast({ tone: 'warn', title: 'Remove failed', body: JSON.stringify(unwrap(res.result)), cause: 'backend-error' });
        } else {
          pushToast({ tone: 'ok', title: 'Skill removed', body: id, cause: 'backend-ok' });
          await refreshDiscovered();
        }
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Remove failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      } finally {
        setBusy(null);
      }
    },
    [pushToast, refreshDiscovered],
  );

  useEffect(() => {
    if (tab === 'installed') refreshInstalled();
    else if (tab === 'marketplace') refreshMarketplace();
    else refreshDiscovered();
  }, [tab, refreshInstalled, refreshMarketplace, refreshDiscovered]);

  const runSearch = useCallback(async () => {
    const q = searchQuery.trim();
    if (!q) {
      setSearchHits([]);
      return;
    }
    try {
      const res = await callTool('vox_skill_search', { query: q });
      const hits = unwrap(res?.result);
      setSearchHits(Array.isArray(hits) ? hits : []);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Skill search failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  }, [searchQuery, pushToast]);

  const doAction = useCallback(
    async (key: string, tool: string, args: Record<string, any>, okTitle: string) => {
      setBusy(key);
      try {
        const res = await callTool(tool, args);
        if (res?.is_error) {
          pushToast({ tone: 'warn', title: `${okTitle} failed`, body: JSON.stringify(unwrap(res.result)), cause: 'backend-error' });
        } else {
          pushToast({ tone: 'ok', title: okTitle, body: key, cause: 'backend-ok' });
        }
        await refreshInstalled();
      } catch (err) {
        pushToast({ tone: 'warn', title: `${okTitle} failed`, body: sanitizeErrorForToast(err), cause: 'backend-error' });
      } finally {
        setBusy(null);
      }
    },
    [pushToast, refreshInstalled],
  );

  const showSkillInfo = useCallback(
    async (id: string) => {
      try {
        const res = await callTool('vox_skill_info', { id });
        const data = unwrap(res?.result);
        if (data && typeof data === 'object') {
          setDetail({ kind: 'skill-info', id, ...data } as SkillDetail);
        }
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Info failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      }
    },
    [pushToast],
  );

  const showPluginInfo = useCallback(
    async (id: string) => {
      try {
        const res = await callTool('vox_plugin_info', { id });
        const data = unwrap(res?.result);
        if (data && typeof data === 'object') {
          setDetail({ kind: 'plugin-info', name: id, description: '', ...data } as SkillDetail);
        }
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Info failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      }
    },
    [pushToast],
  );

  const showSkillUse = useCallback(
    async (id: string) => {
      try {
        const res = await callTool('vox_skill_use', { id });
        const data = unwrap(res?.result);
        setDetail({
          kind: 'skill-use',
          name: id,
          description: '',
          body: typeof data === 'string' ? data : JSON.stringify(data, null, 2),
        });
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Skill use failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      }
    },
    [pushToast],
  );

  const TabButton = ({ id, label }: { id: Tab; label: string }) => (
    <button
      type="button"
      role="tab"
      aria-selected={tab === id}
      onClick={() => setTab(id)}
      className={`rounded-md px-3 py-1.5 font-display text-[11px] tracking-wider uppercase transition ${
        tab === id ? 'bg-brass/10 text-brass ring-1 ring-brass/30' : 'text-text-muted hover:text-text-secondary'
      }`}
    >
      {label}
    </button>
  );

  return (
    <div className="grid grid-cols-12 gap-5">
      <Glass className={`${detail ? 'col-span-8' : 'col-span-12'} p-4 overflow-auto`}>
        <div className="mb-3 flex items-center gap-2">
          <span className="flex size-7 items-center justify-center rounded-lg bg-brass/10 text-brass ring-1 ring-brass/30">
            <Icon.catalog className="size-4" aria-hidden="true" />
          </span>
          <div className="font-display text-sm tracking-widest uppercase text-text-secondary">Skills &amp; Plugins</div>
          <div className="ml-4 flex items-center gap-1" role="tablist" aria-label="Skills and plugins views">
            <TabButton id="installed" label="Installed" />
            <TabButton id="marketplace" label="Marketplace" />
            <TabButton id="discovered" label="Discovered" />
          </div>
          <button
            type="button"
            aria-label="Refresh list"
            onClick={() =>
              tab === 'installed'
                ? refreshInstalled()
                : tab === 'marketplace'
                  ? refreshMarketplace()
                  : refreshDiscovered()
            }
            className="ml-auto flex items-center gap-1.5 rounded-md border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle"
          >
            <Icon.refresh className="size-3" aria-hidden="true" /> refresh
          </button>
        </div>

        {loading ? (
          <div className="text-sm text-text-muted">Loading…</div>
        ) : tab === 'discovered' ? (
          <DiscoveredTab
            discovered={discovered}
            busy={busy}
            addSource={addSource}
            setAddSource={setAddSource}
            onAdd={addSkill}
            onRemove={removeSkill}
            onSkillUse={(id) => showSkillUse(id)}
          />
        ) : tab === 'installed' ? (
          <InstalledTab
            skills={skills}
            plugins={plugins}
            busy={busy}
            onUninstallSkill={(id) =>
              doAction(id, 'vox_skill_uninstall', { id }, 'Skill uninstalled')
            }
            onRemovePlugin={(id) => doAction(id, 'vox_plugin_remove', { id }, 'Plugin removed')}
            onSkillInfo={(id) => showSkillInfo(id)}
            onPluginInfo={(id) => showPluginInfo(id)}
          />
        ) : (
          <MarketplaceTab
            catalogPlugins={catalogPlugins}
            searchQuery={searchQuery}
            setSearchQuery={setSearchQuery}
            runSearch={runSearch}
            searchHits={searchHits}
            busy={busy}
            onInstallPlugin={(id) =>
              doAction(id, 'vox_plugin_install', { id }, 'Plugin installed')
            }
            onPluginInfo={(id) => showPluginInfo(id)}
            onInstallSkill={(id) =>
              doAction(id, 'vox_skill_install', { id }, 'Skill installed')
            }
            onSkillInfo={(id) => showSkillInfo(id)}
          />
        )}
      </Glass>
      {detail && (
        <Glass className="col-span-4 p-4 overflow-auto">
          <div className="mb-3 flex items-center justify-between">
            <span className="font-display text-[11px] tracking-widest uppercase text-text-muted">Detail</span>
            <button
              type="button"
              aria-label="Close detail panel"
              onClick={() => setDetail(null)}
              className="flex items-center rounded-md border border-border-subtle bg-overlay-subtle px-2 py-1 font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle"
            >
              ✕
            </button>
          </div>
          <SkillDetailPanel detail={detail} />
        </Glass>
      )}
    </div>
  );
}

// ── Installed tab ───────────────────────────────────────────────────────────
function InstalledTab(props: {
  skills: SkillInfo[];
  plugins: PluginRow[];
  busy: string | null;
  onUninstallSkill: (id: string) => void;
  onRemovePlugin: (id: string) => void;
  onSkillInfo: (id: string) => void;
  onPluginInfo: (id: string) => void;
}) {
  const { skills, plugins, busy, onUninstallSkill, onRemovePlugin, onSkillInfo, onPluginInfo } = props;
  return (
    <div className="flex flex-col gap-5">
      <Section title="Skills" count={skills.length}>
        {skills.length === 0 ? (
          <Empty text="No skills installed." />
        ) : (
          skills.map((s) => (
            <Row
              key={s.id}
              id={s.id}
              title={s.name || s.id}
              subtitle={s.description}
              version={s.version}
              tags={[s.source, ...(s.tags || []), ...(s.permissions || []).map((p) => `perm:${p}`)]}
              busy={busy === s.id}
              actions={[
                { label: 'Info', onClick: () => onSkillInfo(s.id), tone: 'neutral' },
                { label: 'Uninstall', onClick: () => onUninstallSkill(s.id), tone: 'danger' },
              ]}
            />
          ))
        )}
      </Section>

      <Section title="Plugins" count={plugins.length}>
        {plugins.length === 0 ? (
          <Empty text="No plugins installed." />
        ) : (
          plugins.map((p) => (
            <Row
              key={p.id}
              id={p.id}
              title={p.id}
              subtitle={p.install_dir}
              version={p.version}
              tags={[p.payload_kind]}
              busy={busy === p.id}
              actions={[
                { label: 'Info', onClick: () => onPluginInfo(p.id), tone: 'neutral' },
                { label: 'Remove', onClick: () => onRemovePlugin(p.id), tone: 'danger' },
              ]}
            />
          ))
        )}
      </Section>
    </div>
  );
}

// ── Marketplace tab ──────────────────────────────────────────────────────────
function MarketplaceTab(props: {
  catalogPlugins: CatalogPlugin[];
  searchQuery: string;
  setSearchQuery: (s: string) => void;
  runSearch: () => void;
  searchHits: SkillInfo[];
  busy: string | null;
  onInstallPlugin: (id: string) => void;
  onPluginInfo: (id: string) => void;
  onInstallSkill: (id: string) => void;
  onSkillInfo: (id: string) => void;
}) {
  const { catalogPlugins, searchQuery, setSearchQuery, runSearch, searchHits, busy, onInstallPlugin, onPluginInfo, onInstallSkill, onSkillInfo } =
    props;
  return (
    <div className="flex flex-col gap-5">
      <Section title="Skill search" count={searchHits.length}>
        <div className="mb-2 flex items-center gap-2">
          <input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && runSearch()}
            placeholder="Search for skills…"
            aria-label="Search for skills"
            className="flex-1 rounded-md border border-border-subtle bg-overlay-subtle px-3 py-1.5 font-mono text-xs text-text-secondary placeholder:text-text-muted focus:outline-hidden focus:ring-1 focus:ring-brass/40"
          />
          <button
            type="button"
            onClick={runSearch}
            className="flex items-center gap-1.5 rounded-md border border-border-subtle bg-overlay-subtle px-3 py-1.5 font-display text-[11px] tracking-wider uppercase text-text-secondary hover:bg-overlay-subtle"
          >
            <Icon.search className="size-3.5" aria-hidden="true" /> Search
          </button>
        </div>
        {searchHits.map((s) => (
          <Row
            key={s.id}
            id={s.id}
            title={s.name || s.id}
            subtitle={s.description}
            version={s.version}
            tags={[s.source, ...(s.tags || [])]}
            busy={busy === s.id}
            actions={[
              { label: 'Info', onClick: () => onSkillInfo(s.id), tone: 'neutral' },
              { label: 'Install', onClick: () => onInstallSkill(s.id), tone: 'ok' },
            ]}
          />
        ))}
      </Section>

      <Section title="Plugin catalog" count={catalogPlugins.length}>
        {catalogPlugins.length === 0 ? (
          <Empty text="Catalog is empty." />
        ) : (
          catalogPlugins.map((p) => (
            <Row
              key={p.id}
              id={p.id}
              title={p.id}
              subtitle={p.description}
              version={p.status ?? ''}
              tags={[p['payload-kind'], p['default-source'] ?? ''].filter(Boolean)}
              busy={busy === p.id}
              actions={[
                { label: 'Info', onClick: () => onPluginInfo(p.id), tone: 'neutral' },
                { label: 'Install', onClick: () => onInstallPlugin(p.id), tone: 'ok' },
              ]}
            />
          ))
        )}
      </Section>
    </div>
  );
}

// ── Discovered tab ───────────────────────────────────────────────────────────
// Bare SKILL.md skills found under the standard interop roots
// (.vox/.agents/.claude × workspace+home). Skills under these roots auto-load
// into the registry at daemon start; this tab surfaces what is discoverable
// on disk and whether each is currently active.
function DiscoveredTab(props: {
  discovered: DiscoveredSkill[];
  busy: string | null;
  addSource: string;
  setAddSource: (s: string) => void;
  onAdd: () => void;
  onRemove: (id: string) => void;
  onSkillUse: (id: string) => void;
}) {
  const { discovered, busy, addSource, setAddSource, onAdd, onRemove, onSkillUse } = props;
  // Two-click confirm: Remove deletes the skill directory on disk, so require a
  // deliberate second click rather than firing on the first.
  const [confirmId, setConfirmId] = useState<string | null>(null);
  return (
    <div className="flex flex-col gap-5">
      <div>
        <div className="mb-2 font-display text-[11px] tracking-[0.2em] uppercase text-text-muted">Add from URL or path</div>
        <div className="mb-2 flex items-center gap-2">
          <input
            value={addSource}
            onChange={(e) => setAddSource(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && onAdd()}
            placeholder="https://github.com/owner/repo  or  C:/path/to/skill"
            aria-label="Skill git URL or path"
            className="flex-1 rounded-md border border-border-subtle bg-overlay-subtle px-3 py-1.5 font-mono text-xs text-text-secondary placeholder:text-text-muted focus:outline-hidden focus:ring-1 focus:ring-brass/40"
          />
          <button
            type="button"
            onClick={onAdd}
            disabled={!!busy}
            className="flex items-center gap-1.5 rounded-md border border-emerald-400/30 bg-emerald-400/10 px-3 py-1.5 font-display text-[11px] tracking-wider uppercase text-emerald-300 hover:bg-emerald-400/20 disabled:opacity-40"
          >
            Add
          </button>
        </div>
      </div>

      <Section title="Discovered skills" count={discovered.length}>
        {discovered.length === 0 ? (
          <Empty text="No SKILL.md skills found under .vox/.agents/.claude roots." />
        ) : (
          discovered.map((s) => (
            <Row
              key={s.path || s.id}
              id={s.id}
              title={s.name || s.id}
              subtitle={s.description}
              tags={[
                s.installed ? 'active' : 'available',
                s.source_root,
                s.license ? 'licensed' : '',
              ].filter(Boolean)}
              busy={busy === s.id}
              actions={
                s.removable
                  ? [
                      { label: 'View', onClick: () => onSkillUse(s.id), tone: 'neutral' },
                      confirmId === s.id
                        ? {
                            label: 'Confirm?',
                            onClick: () => {
                              setConfirmId(null);
                              onRemove(s.id);
                            },
                            tone: 'danger',
                          }
                        : { label: 'Remove', onClick: () => setConfirmId(s.id), tone: 'danger' },
                    ]
                  : [{ label: 'View', onClick: () => onSkillUse(s.id), tone: 'neutral' }]
              }
            />
          ))
        )}
      </Section>
    </div>
  );
}

// ── Shared bits ──────────────────────────────────────────────────────────────
function Section({ title, count, children }: { title: string; count: number; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-2 flex items-center gap-2">
        <div className="font-display text-[11px] tracking-[0.2em] uppercase text-text-muted">{title}</div>
        <span className="rounded-full bg-overlay-subtle px-2 py-0.5 font-mono text-[10px] text-text-muted">{count}</span>
      </div>
      <div className="flex flex-col gap-2">{children}</div>
    </div>
  );
}

function Empty({ text }: { text: string }) {
  return <div className="rounded-lg border border-border-subtle bg-overlay-subtle p-3 text-xs text-text-muted">{text}</div>;
}

interface Action {
  label: string;
  onClick: () => void;
  tone: 'ok' | 'danger' | 'neutral';
}

function Row(props: {
  id: string;
  title: string;
  subtitle?: string;
  version?: string;
  tags?: string[];
  busy: boolean;
  actions: Action[];
}) {
  const { id, title, subtitle, version, tags, busy, actions } = props;
  const toneCls = (t: Action['tone']) =>
    t === 'ok'
      ? 'border-emerald-400/30 bg-emerald-400/10 text-emerald-300 hover:bg-emerald-400/20'
      : t === 'danger'
        ? 'border-rose-400/30 bg-rose-400/10 text-rose-300 hover:bg-rose-400/20'
        : 'border-border-subtle bg-overlay-subtle text-text-secondary hover:bg-overlay-subtle';
  return (
    <div className="flex flex-col gap-3 rounded-lg border border-border-subtle bg-overlay-subtle p-3 sm:flex-row sm:items-center sm:justify-between">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs text-brass">{title}</span>
          {version ? <span className="font-mono text-[10px] text-text-muted">{version}</span> : null}
        </div>
        {subtitle ? <div className="mt-1 wrap-break-word text-xs text-text-secondary">{subtitle}</div> : null}
        <div className="mt-1 font-mono text-[10px] text-text-muted break-all">{id}</div>
        {tags && tags.length > 0 ? (
          <div className="mt-1.5 flex flex-wrap gap-1">
            {tags.filter(Boolean).map((t, i) => (
              <span key={`${t}-${i}`} className="rounded-sm bg-overlay-subtle px-1.5 py-0.5 font-mono text-[9px] text-text-muted">
                {t}
              </span>
            ))}
          </div>
        ) : null}
      </div>
      {actions.length > 0 ? (
        <div className="flex shrink-0 items-center gap-2">
          {actions.map((a) => (
            <button
              key={a.label}
              type="button"
              onClick={a.onClick}
              disabled={busy}
              className={`rounded-md border px-3 py-1.5 font-display text-[11px] tracking-wider uppercase transition disabled:cursor-not-allowed disabled:opacity-40 ${toneCls(
                a.tone,
              )}`}
            >
              {a.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
