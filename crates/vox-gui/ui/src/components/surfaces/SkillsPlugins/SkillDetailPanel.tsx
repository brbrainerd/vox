import React from 'react';

// ── SkillDetail type union ──────────────────────────────────────────────────

export type SkillDetail =
  | SkillInfoDetail
  | SkillUseDetail
  | PluginInfoDetail;

export interface SkillInfoDetail {
  kind: 'skill-info';
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

export interface SkillUseDetail {
  kind: 'skill-use';
  name: string;
  description: string;
  body: string;
}

export interface PluginInfoDetail {
  kind: 'plugin-info';
  name: string;
  description: string;
  version?: string;
  author?: string;
  homepage?: string;
  tools?: string[];
}

// ── SkillDetailPanel component ──────────────────────────────────────────────

export function SkillDetailPanel({ detail }: { detail: SkillDetail }) {
  return (
    <div className="flex flex-col gap-3 rounded-lg border border-border-subtle bg-overlay-subtle p-4">
      {detail.kind === 'skill-info' && <SkillInfoView d={detail} />}
      {detail.kind === 'skill-use' && <SkillUseView d={detail} />}
      {detail.kind === 'plugin-info' && <PluginInfoView d={detail} />}
    </div>
  );
}

function Field({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="font-display text-[10px] tracking-[0.15em] uppercase text-text-muted">{label}</span>
      <span className="font-mono text-xs text-text-secondary">{value}</span>
    </div>
  );
}

function PillList({ items }: { items: string[] }) {
  if (!items || items.length === 0) return <span className="font-mono text-[10px] text-text-muted">—</span>;
  return (
    <div className="flex flex-wrap gap-1">
      {items.map((item, i) => (
        <span
          key={`${item}-${i}`}
          className="rounded-sm bg-brass/10 px-1.5 py-0.5 font-mono text-[9px] text-brass ring-1 ring-brass/20"
        >
          {item}
        </span>
      ))}
    </div>
  );
}

function SkillInfoView({ d }: { d: SkillInfoDetail }) {
  return (
    <>
      <div className="flex items-center gap-2">
        <span className="font-mono text-sm text-brass">{d.name}</span>
        <span className="font-mono text-[10px] text-text-muted">{d.version}</span>
        <span className="rounded-sm bg-overlay-subtle px-1.5 py-0.5 font-mono text-[9px] text-text-muted ring-1 ring-border-subtle">
          {d.category}
        </span>
      </div>
      <div className="text-xs text-text-secondary">{d.description}</div>
      <Field label="Source" value={d.source} />
      <Field label="ID" value={d.id} />
      {d.tools && d.tools.length > 0 && (
        <div className="flex flex-col gap-0.5">
          <span className="font-display text-[10px] tracking-[0.15em] uppercase text-text-muted">Tools</span>
          <PillList items={d.tools} />
        </div>
      )}
      {d.permissions && d.permissions.length > 0 && (
        <div className="flex flex-col gap-0.5">
          <span className="font-display text-[10px] tracking-[0.15em] uppercase text-text-muted">Permissions</span>
          <PillList items={d.permissions} />
        </div>
      )}
      {d.tags && d.tags.length > 0 && (
        <div className="flex flex-col gap-0.5">
          <span className="font-display text-[10px] tracking-[0.15em] uppercase text-text-muted">Tags</span>
          <PillList items={d.tags} />
        </div>
      )}
    </>
  );
}

function SkillUseView({ d }: { d: SkillUseDetail }) {
  return (
    <>
      <div className="font-mono text-sm text-brass">{d.name}</div>
      <div className="text-xs text-text-secondary">{d.description}</div>
      <pre className="overflow-auto rounded-sm bg-overlay-subtle p-3 font-mono text-[11px] text-text-secondary whitespace-pre-wrap">
        {d.body}
      </pre>
    </>
  );
}

function PluginInfoView({ d }: { d: PluginInfoDetail }) {
  return (
    <>
      <div className="flex items-center gap-2">
        <span className="font-mono text-sm text-brass">{d.name}</span>
        {d.version && <span className="font-mono text-[10px] text-text-muted">{d.version}</span>}
      </div>
      <div className="text-xs text-text-secondary">{d.description}</div>
      {d.author && <Field label="Author" value={d.author} />}
      {d.homepage && <Field label="Homepage" value={d.homepage} />}
      {d.tools && d.tools.length > 0 && (
        <div className="flex flex-col gap-0.5">
          <span className="font-display text-[10px] tracking-[0.15em] uppercase text-text-muted">Tools</span>
          <PillList items={d.tools} />
        </div>
      )}
    </>
  );
}
