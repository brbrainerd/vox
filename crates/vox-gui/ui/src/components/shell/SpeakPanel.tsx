import React, { useRef, useEffect } from 'react';
import { useVoxChat, type ChatMessage } from '../../hooks/useVoxChat';
import { voxTransport } from '../../transport';
import type { ConnectionStatusPayload } from '../../types';

function MessageBubble({ msg }: { msg: ChatMessage }) {
  const isUser = msg.role === 'user';
  return (
    <div className={isUser ? 'justify-end flex px-4 py-2' : 'justify-start flex px-4 py-2'}>
      <div className={isUser
        ? 'max-w-xl bg-blue-600/20 border border-blue-500/30 rounded-2xl rounded-br-sm px-4 py-3'
        : 'max-w-2xl bg-white/5 border border-white/10 rounded-2xl rounded-bl-sm px-4 py-3'}>
        <div className="text-xs font-bold text-zinc-400 uppercase tracking-widest mb-2">
          {msg.role}
        </div>
        <div className="text-sm text-white/80 leading-relaxed whitespace-pre-wrap">
          {msg.content}
        </div>
      </div>
    </div>
  );
}

function ConnectionDot({ status }: { status: string }) {
  const colour = status === 'connected' ? 'bg-emerald-400'
    : status === 'error' || status === 'failed_permanently' ? 'bg-rose-500'
    : 'bg-zinc-600';
  return <div className={`w-2 h-2 rounded-full ${colour}`} />;
}

export function SpeakPanel() {
  const { messages, isLoading, error, send, reset } = useVoxChat();
  const [connStatus, setConnStatus] = React.useState('disconnected');
  const [draft, setDraft] = React.useState('');
  const scrollRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    voxTransport.connect();
    const off = voxTransport.on('connection_status', (data: ConnectionStatusPayload) => {
      setConnStatus(data.status);
    });
    return off;
  }, []);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const handleSend = async () => {
    const text = draft.trim();
    if (!text || isLoading) return;
    setDraft('');
    await send(text);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void handleSend();
    }
  };

  const statusLabel = connStatus === 'connected' ? 'CONNECTED'
    : connStatus === 'failed_permanently' ? 'OFFLINE'
    : connStatus.toUpperCase();

  return (
    <div className="flex flex-col flex-1 overflow-hidden bg-zinc-950">
      {/* Header */}
      <div className="h-12 border-b border-zinc-800 px-6 flex items-center justify-between shrink-0">
        <div className="flex flex-col gap-0">
          <span className="text-sm font-black tracking-tighter text-white">LOQUELA</span>
          <span className="text-xs text-zinc-500 tracking-widest">VOICE INTERFACE</span>
        </div>
        <div className="flex gap-2 items-center">
          <ConnectionDot status={connStatus} />
          <span className="text-xs text-zinc-500">{statusLabel}</span>
          {messages.length > 0 && (
            <button
              onClick={reset}
              className="ml-3 text-xs text-zinc-600 hover:text-zinc-400 px-2 py-0.5 rounded border border-white/5"
            >
              CLEAR
            </button>
          )}
        </div>
      </div>

      {/* Message list */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto py-4">
        {messages.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-4 text-zinc-500">
            <span className="text-sm font-bold uppercase tracking-widest">START A CONVERSATION</span>
            <span className="text-xs">Messages will appear here once a session is active.</span>
          </div>
        ) : (
          <div className="flex flex-col">
            {messages.map((msg, i) => (
              <MessageBubble key={i} msg={msg} />
            ))}
            {isLoading && (
              <div className="justify-start flex px-4 py-2">
                <div className="bg-white/5 border border-white/10 rounded-2xl rounded-bl-sm px-4 py-3">
                  <span className="text-xs text-zinc-500 animate-pulse">Thinking…</span>
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Composer */}
      <div className="border-t border-white/10 p-4 gap-3 flex flex-col bg-zinc-950/80">
        <div className="flex gap-3 items-end">
          <textarea
            ref={textareaRef}
            aria-label="Message"
            className="flex-1 bg-zinc-900 border border-white/10 rounded-2xl px-4 py-3 min-h-16 resize-none text-sm text-white/80 placeholder-white/30 focus:outline-none focus:ring-1 focus:ring-blue-500/50"
            placeholder="Type a message…"
            rows={2}
            value={draft}
            onChange={e => setDraft(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={isLoading}
          />
          <button
            aria-label="Send message"
            onClick={() => void handleSend()}
            disabled={isLoading || !draft.trim()}
            className={isLoading || !draft.trim()
              ? 'w-12 h-12 rounded-xl bg-blue-600/50 text-white/50 flex items-center justify-center shrink-0'
              : 'w-12 h-12 rounded-xl bg-blue-600 text-white flex items-center justify-center shrink-0 hover:bg-blue-500'}
          >
            {isLoading ? '…' : '→'}
          </button>
        </div>
        <div className="flex justify-between items-center px-1">
          <span className="text-xs text-zinc-500">Shift+Enter for new line · Enter to send</span>
          {error && <span className="text-xs text-rose-400">{error}</span>}
        </div>
      </div>
    </div>
  );
}
