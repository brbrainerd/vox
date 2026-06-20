import React, { useEffect, useState, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { filterEntries } from '../../../lib/historyFilter';
import { SurfaceDecoratorProps } from '../decoratorRegistry';
import { voxTransport, type HistoryEntry } from '../../../transport';

export function HistoryPanel({ pushToast }: SurfaceDecoratorProps) {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [kindFilter, setKindFilter] = useState<'all' | 'clip' | 'command' | 'chat'>('all');

  const fetchEntries = useCallback(async () => {
    try {
      const list = await voxTransport.historyList(
        kindFilter === 'all' ? null : kindFilter,
        100
      );
      setEntries(list);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Failed to load history', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [kindFilter, pushToast]);

  useEffect(() => {
    fetchEntries();
  }, [fetchEntries]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const setupListener = async () => {
      const u = await listen('vox://history-changed', () => {
        fetchEntries();
      });
      unlisten = u;
    };
    setupListener();
    return () => {
      if (unlisten) unlisten();
    };
  }, [fetchEntries]);

  const handleSearchAll = async () => {
    if (!searchQuery.trim()) {
      fetchEntries();
      return;
    }
    setLoading(true);
    try {
      const list = await voxTransport.historySearch(searchQuery, 100);
      const filtered = kindFilter === 'all' ? list : list.filter(e => e.kind === kindFilter);
      setEntries(filtered);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Search failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  };

  const handleCopy = async (entry: HistoryEntry) => {
    try {
      await navigator.clipboard.writeText(entry.text);
      pushToast({ tone: 'ok', title: 'Copied to clipboard', body: entry.redacted_text.slice(0, 60) });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Failed to copy', body: String(err) });
    }
  };

  const handleTogglePin = async (entry: HistoryEntry) => {
    try {
      await voxTransport.historyPin(entry.id, !entry.pinned);
      pushToast({
        tone: 'ok',
        title: entry.pinned ? 'Entry unpinned' : 'Entry pinned',
      });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Failed to toggle pin', body: String(err) });
    }
  };

  const handleDelete = async (entry: HistoryEntry) => {
    try {
      await voxTransport.historyDelete(entry.id);
      pushToast({ tone: 'ok', title: 'Entry deleted' });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Failed to delete', body: String(err) });
    }
  };

  const handleRerun = async (entry: HistoryEntry) => {
    try {
      await navigator.clipboard.writeText(entry.text);
      pushToast({ tone: 'ok', title: 'Command ready', body: 'Paste it in your terminal to re-run' });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Failed to copy command', body: String(err) });
    }
  };

  const handleReinsert = async (entry: HistoryEntry) => {
    try {
      await navigator.clipboard.writeText(entry.text);
      pushToast({ tone: 'ok', title: 'Chat turn copied', body: 'Paste it into the chat input to re-send' });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Failed to copy chat turn', body: String(err) });
    }
  };

  const filtered = filterEntries(searchQuery, entries);

  return (
    <div className="flex flex-col gap-5 p-4 animate-fadeIn">
      {/* Header & Controls */}
      <Glass className="p-4 flex flex-col gap-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <div className="font-display text-sm tracking-widest text-zinc-200 uppercase">History & Clips</div>
            <div className="text-xs text-zinc-500 mt-1">Project clipboard history, commands, and chat turns</div>
          </div>
          {/* Kind filter tabs */}
          <div className="flex items-center gap-1 bg-white/[0.02] border border-white/5 rounded-lg p-1">
            {(['all', 'clip', 'command', 'chat'] as const).map(k => (
              <button
                key={k}
                onClick={() => setKindFilter(k)}
                className={`px-3 py-1.5 text-xs rounded-md capitalize transition-all duration-200 ${
                  kindFilter === k
                    ? 'bg-brass/20 text-brass font-semibold border border-brass/30'
                    : 'text-zinc-400 hover:text-zinc-200 border border-transparent'
                }`}
              >
                {k === 'all' ? 'All' : `${k}s`}
              </button>
            ))}
          </div>
        </div>

        {/* Search Input */}
        <div className="flex gap-2">
          <div className="relative flex-1 flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.04] px-3 py-2.5 focus-within:border-brass/40 transition-all duration-200">
            <Icon.search className="size-4 shrink-0 text-brass" />
            <input
              type="text"
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && handleSearchAll()}
              placeholder="Fuzzy filter local entries, or press enter to search all history..."
              className="flex-1 bg-transparent text-[14px] text-zinc-100 placeholder:text-zinc-600 outline-none"
            />
            {searchQuery && (
              <button
                onClick={() => setSearchQuery('')}
                className="text-zinc-500 hover:text-zinc-300 transition"
              >
                <Icon.x className="size-4" />
              </button>
            )}
          </div>
          <button
            onClick={handleSearchAll}
            className="px-4 py-2 bg-brass/10 hover:bg-brass/25 border border-brass/35 text-brass rounded-xl text-xs font-semibold transition-all duration-200"
          >
            Search DB
          </button>
        </div>
      </Glass>

      {/* Entries List */}
      <Glass className="p-4 flex flex-col gap-2 min-h-[300px]">
        {loading ? (
          <div className="flex items-center justify-center py-20 text-zinc-500 text-xs">
            <div className="size-5 border-2 border-brass/20 border-t-brass/80 animate-spin rounded-full mr-2" />
            Loading history...
          </div>
        ) : filtered.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20 text-zinc-600 text-xs">
            <Icon.clock className="size-8 mb-2 text-zinc-700 animate-pulse" />
            No history entries found.
          </div>
        ) : (
          <div className="flex flex-col gap-3 max-h-[600px] overflow-y-auto pr-1">
            {filtered.map(entry => (
              <div
                key={entry.id}
                data-testid="history-row"
                className="flex items-start justify-between gap-3 p-3 rounded-lg bg-white/[0.02] border border-white/5 hover:border-white/10 hover:bg-white/[0.03] transition-all duration-200"
              >
                <div className="flex-1 flex flex-col gap-2 min-w-0">
                  <div className="flex items-center gap-2">
                    <span
                      className={`text-[9px] font-mono uppercase tracking-widest px-1.5 py-0.5 rounded ${
                        entry.kind === 'clip'
                          ? 'bg-blue-500/10 text-blue-400'
                          : entry.kind === 'command'
                          ? 'bg-amber-500/10 text-amber-400'
                          : 'bg-purple-500/10 text-purple-400'
                      }`}
                    >
                      {entry.kind}
                    </span>
                    {entry.source && (
                      <span className="text-[10px] text-zinc-600 font-mono">
                        via {entry.source}
                      </span>
                    )}
                    <span className="text-[10px] text-zinc-600">
                      {new Date(entry.created_at).toLocaleTimeString()}
                    </span>
                  </div>
                  <pre className="font-mono text-xs text-zinc-300 whitespace-pre-wrap break-all bg-black/25 p-3 rounded-xl border border-white/5 leading-relaxed">
                    {entry.redacted_text}
                  </pre>
                </div>

                {/* Actions */}
                <div className="flex items-center gap-1.5 shrink-0 self-center">
                  <button
                    onClick={() => handleCopy(entry)}
                    title="Copy to clipboard"
                    className="p-2 hover:bg-white/5 rounded-lg text-zinc-400 hover:text-zinc-200 transition-colors"
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="size-4">
                      <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                    </svg>
                  </button>
                  {entry.kind === 'command' && (
                    <button
                      data-testid="rerun-btn"
                      onClick={() => handleRerun(entry)}
                      title="Re-run command"
                      className="p-2 hover:bg-white/5 rounded-lg text-zinc-400 hover:text-amber-300 transition-colors"
                    >
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="size-4">
                        <polyline points="23 4 23 10 17 10" />
                        <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
                      </svg>
                    </button>
                  )}
                  {entry.kind === 'chat' && (
                    <button
                      data-testid="reinsert-btn"
                      onClick={() => handleReinsert(entry)}
                      title="Re-insert into chat"
                      className="p-2 hover:bg-white/5 rounded-lg text-zinc-400 hover:text-purple-300 transition-colors"
                    >
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="size-4">
                        <line x1="22" y1="2" x2="11" y2="13" />
                        <polygon points="22 2 15 22 11 13 2 9 22 2" />
                      </svg>
                    </button>
                  )}
                  <button
                    onClick={() => handleTogglePin(entry)}
                    title={entry.pinned ? 'Unpin' : 'Pin'}
                    className={`p-2 hover:bg-white/5 rounded-lg transition-colors ${
                      entry.pinned ? 'text-brass hover:text-brass-light' : 'text-zinc-400 hover:text-zinc-200'
                    }`}
                  >
                    <Icon.pin className="size-4" />
                  </button>
                  <button
                    onClick={() => handleDelete(entry)}
                    title="Delete"
                    className="p-2 hover:bg-red-500/10 hover:text-red-400 rounded-lg text-zinc-400 transition-colors"
                  >
                    <Icon.trash className="size-4" />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </Glass>
    </div>
  );
}
