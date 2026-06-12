import React, { useState } from 'react';
import { Icon } from '../ui/Icons';
import { ChatSession } from '../../lib/sessions';

interface SessionTabsProps {
  sessions: ChatSession[];
  activeId: string;
  onSelect: (id: string) => void;
  onNew: () => void;
  onClose: (id: string) => void;
  onRename: (id: string, title: string) => void;
  /** queued-task count per session id (from TasksView data), for badges */
  queuedBySession?: Record<string, number>;
}

export function SessionTabs({
  sessions,
  activeId,
  onSelect,
  onNew,
  onClose,
  onRename,
  queuedBySession,
}: SessionTabsProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState('');

  return (
    <div className="flex items-center gap-1 overflow-x-auto custom-scrollbar px-1 pb-1">
      {sessions.map(s => {
        const active = s.id === activeId;
        const queued = queuedBySession?.[s.id] ?? 0;
        return (
          <div
            key={s.id}
            className={`group flex shrink-0 items-center gap-1.5 rounded-t-lg border-x border-t px-2.5 py-1 text-[12px] transition ${
              active
                ? 'border-white/10 bg-white/[0.04] text-zinc-100'
                : 'border-transparent text-zinc-500 hover:bg-white/[0.02] hover:text-zinc-300'
            }`}
          >
            {editingId === s.id ? (
              <input
                autoFocus
                value={draft}
                onChange={e => setDraft(e.target.value)}
                onKeyDown={e => {
                  if (e.key === 'Enter') {
                    onRename(s.id, draft);
                    setEditingId(null);
                  }
                  if (e.key === 'Escape') setEditingId(null);
                }}
                onBlur={() => {
                  onRename(s.id, draft);
                  setEditingId(null);
                }}
                className="w-24 bg-transparent outline-none border-b border-brass/40"
              />
            ) : (
              <button
                onClick={() => onSelect(s.id)}
                onDoubleClick={() => {
                  setEditingId(s.id);
                  setDraft(s.title);
                }}
                title={`${s.title} — double-click to rename`}
                className="focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40 rounded"
              >
                {s.title}
              </button>
            )}
            {queued > 0 && (
              <span className="rounded-full bg-brass/15 px-1.5 font-mono text-[9px] text-brass">
                {queued}
              </span>
            )}
            {sessions.length > 1 && (
              <button
                onClick={() => onClose(s.id)}
                title="Close tab (queued tasks keep running)"
                className="rounded p-0.5 text-zinc-600 opacity-0 transition group-hover:opacity-100 hover:text-zinc-200 focus:outline-none focus-visible:opacity-100"
              >
                <Icon.x className="size-3" />
              </button>
            )}
          </div>
        );
      })}
      <button
        onClick={onNew}
        title="New chat session"
        className="shrink-0 rounded p-1 text-zinc-500 hover:bg-white/[0.04] hover:text-zinc-200 focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40"
      >
        <Icon.plus className="size-3.5" />
      </button>
    </div>
  );
}
