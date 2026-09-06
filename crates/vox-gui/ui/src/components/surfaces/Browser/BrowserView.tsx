import React, { useCallback, useEffect, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { invoke } from '@tauri-apps/api/core';
import { useLabel } from '../../../hooks/useLanguage';
import {
  listenAgentEvents,
  listenBrowserFrames,
  listenPreviewAvailable,
  type BrowserPageInfo,
  type BrowserPageSummary,
  type BrowserFramePayload,
  type PreviewAvailablePayload,
} from '../../../transport';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';
import type { Toast } from '../../../types/tauri';
import { useIsEmbeddedSurface } from '../../dashboard/EmbeddedSurfaceContext';

interface BrowserViewProps {
  pushToast: (item: Toast) => void;
  gamifyEnabled?: boolean;
}

interface PreviewStatus {
  active: boolean;
  url: string | null;
  app_dir: string | null;
  source: string;
}

interface PlaywrightValidateResult {
  exit_code: number;
  stdout: string;
  stderr: string;
  preview_url: string | null;
}

type BrowserTab = 'preview' | 'agent';
type ControlMode = 'you' | 'agent';
const DEFAULT_VIEWPORT_WIDTH = 1280;
const DEFAULT_VIEWPORT_HEIGHT = 800;

const MAX_ACTION_LOG = 50;

export function mapClickToViewport(
  clientX: number,
  clientY: number,
  rect: { left: number; top: number; width: number; height: number },
  viewWidth: number,
  viewHeight: number,
): { x: number; y: number } | null {
  if (rect.width <= 0 || rect.height <= 0 || viewWidth <= 0 || viewHeight <= 0) return null;
  const scale = Math.min(rect.width / viewWidth, rect.height / viewHeight);
  const shownW = viewWidth * scale;
  const shownH = viewHeight * scale;
  const padX = (rect.width - shownW) / 2;
  const padY = (rect.height - shownH) / 2;
  const localX = clientX - rect.left - padX;
  const localY = clientY - rect.top - padY;
  if (localX < 0 || localY < 0 || localX > shownW || localY > shownH) return null;
  return {
    x: (localX / shownW) * viewWidth,
    y: (localY / shownH) * viewHeight,
  };
}

export function BrowserView({ pushToast, gamifyEnabled }: BrowserViewProps) {
  const embedded = useIsEmbeddedSurface();
  const [tab, setTab] = useState<BrowserTab>('preview');
  const [previewUrl, setPreviewUrl] = useState('http://127.0.0.1:3000');
  const [appDir, setAppDir] = useState('');
  const [preview, setPreview] = useState<PreviewStatus | null>(null);
  const [previewReloadNonce, setPreviewReloadNonce] = useState(0);
  const [busy, setBusy] = useState(false);

  const [agentUrl, setAgentUrl] = useState('https://example.com');
  const [headless, setHeadless] = useState(true);
  const [pageId, setPageId] = useState<string | null>(null);
  const [pages, setPages] = useState<BrowserPageSummary[]>([]);
  const [pageInfo, setPageInfo] = useState<BrowserPageInfo | null>(null);
  const [agentNavUrl, setAgentNavUrl] = useState('');
  const [controlMode, setControlMode] = useState<ControlMode>('you');
  const [previewFrameBlocked, setPreviewFrameBlocked] = useState(false);
  const [frame, setFrame] = useState<BrowserFramePayload | null>(null);
  const [actionLog, setActionLog] = useState<string[]>([]);
  const [validateOut, setValidateOut] = useState('');

  const refreshPreviewStatus = useCallback(async () => {
    try {
      const status = await invoke<PreviewStatus>('preview_status');
      setPreview(status);
      if (status.url) setPreviewUrl(status.url);
      if (status.app_dir) setAppDir(status.app_dir);
    } catch {
      setPreview(null);
    }
  }, []);

  const refreshSessionStatus = useCallback(async () => {
    try {
      const status = await invoke<{
        page_id: string | null;
        headless: boolean;
        control_mode?: string;
        action_log: string[];
      }>(
        'browser_session_status',
      );
      setPageId(status.page_id);
      setHeadless(status.headless);
      if (status.control_mode === 'agent' || status.control_mode === 'you') {
        setControlMode(status.control_mode);
      }
      setActionLog(status.action_log ?? []);
      if (!status.page_id) setPageInfo(null);
    } catch {
      setPageId(null);
    }
  }, []);

  const refreshPages = useCallback(async () => {
    try {
      const list = await invoke<BrowserPageSummary[]>('browser_list_pages');
      setPages(list);
      if (list.length === 0) {
        setPageInfo(null);
        setAgentNavUrl('');
      }
    } catch {}
  }, []);

  const refreshPageInfo = useCallback(async (selectedPageId?: string | null) => {
    const currentPage = selectedPageId ?? pageId;
    if (!currentPage) {
      setPageInfo(null);
      return;
    }
    try {
      const info = await invoke<BrowserPageInfo>('browser_page_info', {
        input: { page_id: currentPage },
      });
      setPageInfo(info);
      if (info.url) setAgentNavUrl(info.url);
    } catch {
      setPageInfo(null);
    }
  }, [pageId]);

  useEffect(() => {
    refreshPreviewStatus();
    refreshSessionStatus();
    refreshPages();
  }, [refreshPreviewStatus, refreshSessionStatus, refreshPages]);

  useEffect(() => {
    // Embedded mini-render: the initial fetch (effect above) already populated
    // the thumbnail; skip the 2s poll.
    if (embedded) return;
    const id = window.setInterval(() => {
      refreshPages();
      refreshSessionStatus();
    }, 2000);
    return () => window.clearInterval(id);
  }, [refreshPages, refreshSessionStatus, embedded]);

  useEffect(() => {
    refreshPageInfo(pageId);
  }, [pageId, refreshPageInfo]);

  // Surface genuine agent-driven browser activity from the orchestrator event
  // stream. NOTE: AgentEventKind has no general "tool invoked" variant, so the
  // only browser-identifying signal available here is `tool_timed_out` (carries
  // `tool_key`). GUI-initiated session actions are mirrored separately via the
  // browser-frame stream's `action_log`. Fully mirroring an agent-opened page
  // (one the GUI never opened) would need a `vox_browser_list_pages` tool to
  // share the page_id; that is intentionally out of scope here.
  useEffect(() => {
    let unlistenAgent: (() => void) | undefined;
    listenAgentEvents((frame) => {
      const kind = frame.kind ?? { type: '' };
      const toolKey: string | undefined =
        typeof kind.tool_key === 'string' ? kind.tool_key : undefined;
      if (kind.type === 'tool_timed_out' && toolKey?.startsWith('vox_browser')) {
        const when = new Date(frame.timestamp_ms ?? Date.now()).toLocaleTimeString();
        const line = `${when} agent: ${toolKey} timed out`;
        setActionLog((prev) => [...prev.slice(-(MAX_ACTION_LOG - 1)), line]);
      }
    })
      .then((fn) => {
        unlistenAgent = fn;
      })
      .catch(() => {});
    return () => {
      if (unlistenAgent) unlistenAgent();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenBrowserFrames((payload) => {
      setFrame(payload);
      if (payload.action_log?.length) setActionLog(payload.action_log);
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenPreviewAvailable((payload: PreviewAvailablePayload) => {
      setPreviewFrameBlocked(false);
      setTab('preview');
      setPreview({
        active: true,
        url: payload.url,
        app_dir: payload.app_dir,
        source: payload.source,
      });
      setPreviewUrl(payload.url);
      void recordGamifyGuiEvent(
        'browser_preview_loaded',
        { url: payload.url, source: payload.source },
        { enabled: gamifyEnabled },
      );
      // Treat subsequent preview-available events as rebuild/restart signals.
      setPreviewReloadNonce((n) => n + 1);
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
  }, [gamifyEnabled]);

  const startPreview = async () => {
    setBusy(true);
    try {
      const status = await invoke<PreviewStatus>('preview_start', {
        input: {
          url: previewUrl.trim() || null,
          app_dir: appDir.trim() || null,
        },
      });
      setPreview(status);
      pushToast({ tone: 'ok', title: 'Preview started', body: status.url ?? undefined, cause: 'backend-ok' });
      void recordGamifyGuiEvent(
        'browser_preview_loaded',
        { url: status.url, source: status.source },
        { enabled: gamifyEnabled },
      );
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Preview failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  };

  const stopPreview = async () => {
    setBusy(true);
    try {
      const status = await invoke<PreviewStatus>('preview_stop');
      setPreview(status);
      pushToast({ tone: 'info', title: 'Preview stopped', cause: 'backend-ok' });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Stop failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  };

  const openAgentSession = async () => {
    setBusy(true);
    try {
      const result = await invoke<{ page_id: string | null }>('browser_open_session', {
        input: { url: agentUrl.trim(), headless },
      });
      setPageId(result.page_id ?? null);
      setTab('agent');
      pushToast({ tone: 'ok', title: 'Browser session opened', body: result.page_id ?? undefined, cause: 'backend-ok' });
      await refreshSessionStatus();
      await refreshPages();
      await refreshPageInfo(result.page_id ?? null);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Open failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  };

  const closeAgentSession = async () => {
    setBusy(true);
    try {
      await invoke('browser_close_session');
      setPageId(null);
      setFrame(null);
      setPageInfo(null);
      pushToast({ tone: 'info', title: 'Browser session closed', cause: 'backend-ok' });
      await refreshPages();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Close failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  };

  const captureFrame = async () => {
    try {
      const payload = await invoke<BrowserFramePayload>('browser_screenshot_frame');
      setFrame(payload);
      if (payload.action_log?.length) setActionLog(payload.action_log);
      if (payload.page_id) {
        setPageId(payload.page_id);
        await refreshPageInfo(payload.page_id);
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Screenshot failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  };

  const attachPage = async (selectedPageId: string) => {
    setBusy(true);
    try {
      await invoke('browser_attach_session', { input: { page_id: selectedPageId } });
      setPageId(selectedPageId);
      await refreshSessionStatus();
      await refreshPageInfo(selectedPageId);
      pushToast({ tone: 'ok', title: 'Attached session', body: selectedPageId, cause: 'backend-ok' });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Attach failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  };

  const closePage = async (selectedPageId: string) => {
    setBusy(true);
    try {
      await invoke('browser_close_page', { input: { page_id: selectedPageId } });
      if (pageId === selectedPageId) {
        setPageId(null);
        setPageInfo(null);
      }
      await refreshPages();
      pushToast({ tone: 'info', title: 'Page closed', body: selectedPageId, cause: 'backend-ok' });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Close page failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  };

  const setControlModeRemote = async (nextMode: ControlMode) => {
    setControlMode(nextMode);
    try {
      await invoke('browser_set_control_mode', { input: { mode: nextMode } });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Failed to set control mode', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      await refreshSessionStatus();
    }
  };

  const navigate = async (action: 'back' | 'forward' | 'reload' | 'stop') => {
    if (!pageId) return;
    setBusy(true);
    try {
      await invoke('browser_navigate', { input: { action } });
      await refreshPageInfo(pageId);
    } catch (err) {
      pushToast({ tone: 'warn', title: `Navigation ${action} failed`, body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  };

  const gotoUrl = async () => {
    if (!pageId || !agentNavUrl.trim()) return;
    setBusy(true);
    try {
      await invoke('browser_goto_url', { input: { url: agentNavUrl.trim() } });
      await refreshPageInfo(pageId);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Goto failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  };

  const onFrameClick = async (event: React.MouseEvent<HTMLDivElement>) => {
    if (!pageId || controlMode !== 'you') return;
    const viewWidth = frame?.viewport_width ?? DEFAULT_VIEWPORT_WIDTH;
    const viewHeight = frame?.viewport_height ?? DEFAULT_VIEWPORT_HEIGHT;
    const rect = event.currentTarget.getBoundingClientRect();
    const mapped = mapClickToViewport(
      event.clientX,
      event.clientY,
      { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
      viewWidth,
      viewHeight,
    );
    if (!mapped) return;
    try {
      await invoke('browser_click_xy', { input: mapped });
      await captureFrame();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Click failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  };

  const onFrameWheel = async (event: React.WheelEvent<HTMLDivElement>) => {
    if (!pageId || controlMode !== 'you') return;
    event.preventDefault();
    try {
      await invoke('browser_scroll', {
        input: { dx: Math.round(event.deltaX), dy: Math.round(event.deltaY) },
      });
      await captureFrame();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Scroll failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  };

  const onFrameKeyDown = async (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (!pageId || controlMode !== 'you') return;
    if (event.key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey) {
      event.preventDefault();
      await invoke('browser_type_text', { input: { text: event.key } });
      await captureFrame();
      return;
    }
    const handledKeys = new Set([
      'Enter',
      'Backspace',
      'Tab',
      'Escape',
      'ArrowUp',
      'ArrowDown',
      'ArrowLeft',
      'ArrowRight',
      'Home',
      'End',
      'PageUp',
      'PageDown',
    ]);
    if (!handledKeys.has(event.key)) return;
    event.preventDefault();
    await invoke('browser_input_key', { input: { key: event.key } });
    await captureFrame();
  };

  const runPlaywrightValidate = async () => {
    setBusy(true);
    setValidateOut('');
    try {
      const result = await invoke<PlaywrightValidateResult>('browser_validate_playwright', {
        input: { preview_url: (preview?.url ?? previewUrl.trim()) || null },
      });
      const text = [result.stdout, result.stderr].filter(Boolean).join('\n').trim();
      setValidateOut(text || `(exit ${result.exit_code})`);
      pushToast({
        tone: result.exit_code === 0 ? 'ok' : 'warn',
        title: result.exit_code === 0 ? 'Playwright passed' : 'Playwright failed',
        body: result.preview_url ?? undefined,
        cause: result.exit_code === 0 ? 'backend-ok' : 'backend-error',
      });
    } catch (err) {
      setValidateOut(sanitizeErrorForToast(err));
      pushToast({ tone: 'warn', title: 'Validate failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  };

  const activePreviewUrl = preview?.url ?? (previewUrl.trim() || null);

  return (
    <section className="space-y-4">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="font-display text-lg tracking-[0.14em] uppercase text-text-primary">{useLabel('browser')}</h1>
          <p className="text-[12px] text-text-muted mt-1">
            Preview Vox web apps and mirror agent-driven CDP browser sessions.
          </p>
        </div>
        <div className="flex gap-2" role="tablist" aria-label="Browser view">
          <button
            type="button"
            role="tab"
            aria-selected={tab === 'preview'}
            onClick={() => setTab('preview')}
            className={`px-3 py-1.5 rounded-lg text-[11px] uppercase tracking-wider ${tab === 'preview' ? 'bg-brass/15 text-brass ring-1 ring-brass/30' : 'bg-overlay-subtle text-text-muted'}`}
          >
            Preview
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === 'agent'}
            onClick={() => setTab('agent')}
            className={`px-3 py-1.5 rounded-lg text-[11px] uppercase tracking-wider ${tab === 'agent' ? 'bg-brass/15 text-brass ring-1 ring-brass/30' : 'bg-overlay-subtle text-text-muted'}`}
          >
            Agent live view
          </button>
        </div>
      </header>

      {tab === 'preview' && (
        <div className="space-y-3">
          <div className="grid gap-3 md:grid-cols-2">
            <label className="block space-y-1">
              <span className="text-[10px] uppercase tracking-wider text-text-muted">Preview URL</span>
              <input
                value={previewUrl}
                onChange={(e) => setPreviewUrl(e.target.value)}
                className="w-full rounded-lg bg-overlay-subtle border border-border-subtle px-3 py-2 text-sm text-text-secondary"
                placeholder="http://127.0.0.1:3000"
              />
            </label>
            <label className="block space-y-1">
              <span className="text-[10px] uppercase tracking-wider text-text-muted">
                App dir (Preview spawn — needs dev:ssr-upstream or dev script)
              </span>
              <input
                value={appDir}
                onChange={(e) => setAppDir(e.target.value)}
                className="w-full rounded-lg bg-overlay-subtle border border-border-subtle px-3 py-2 text-sm text-text-secondary"
                placeholder="path to a vox web app (leave blank to use the URL above)"
              />
            </label>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={startPreview}
              className="rounded-lg bg-brass/20 text-brass px-4 py-2 text-[11px] uppercase tracking-wider disabled:opacity-50"
            >
              Start preview
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={stopPreview}
              className="rounded-lg bg-overlay-subtle text-text-secondary px-4 py-2 text-[11px] uppercase tracking-wider disabled:opacity-50"
            >
              Stop
            </button>
            <button
              type="button"
              disabled={busy || !activePreviewUrl}
              onClick={runPlaywrightValidate}
              className="rounded-lg bg-overlay-subtle text-text-secondary px-4 py-2 text-[11px] uppercase tracking-wider disabled:opacity-50"
            >
              Validate (Playwright)
            </button>
          </div>
          {preview && (
            <p className="text-[11px] text-text-muted font-mono">
              source={preview.source} active={String(preview.active)} url={preview.url ?? '—'}
            </p>
          )}
          {validateOut && (
            <pre className="max-h-40 overflow-auto rounded-lg bg-black/40 p-3 text-[11px] text-text-muted font-mono whitespace-pre-wrap">
              {validateOut}
            </pre>
          )}
          {activePreviewUrl && !previewFrameBlocked ? (
            <iframe
              key={`${activePreviewUrl}-${previewReloadNonce}`}
              title="Vox app preview"
              src={activePreviewUrl}
              className="w-full min-h-[480px] rounded-xl border border-border-subtle bg-white"
              sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
              onError={() => setPreviewFrameBlocked(true)}
            />
          ) : (
            <div className="rounded-xl border border-dashed border-border-subtle p-8 text-center text-text-muted text-sm space-y-3">
              <div>
                {activePreviewUrl
                  ? 'Preview frame blocked by page framing policy (X-Frame-Options/CSP).'
                  : 'Enter a URL or app directory, then start preview.'}
              </div>
              {activePreviewUrl && (
                <button
                  type="button"
                  onClick={async () => {
                    setTab('agent');
                    setAgentUrl(activePreviewUrl);
                    await openAgentSession();
                  }}
                  className="rounded-lg bg-brass/20 text-brass px-4 py-2 text-[11px] uppercase tracking-wider"
                >
                  Open in agent live view
                </button>
              )}
            </div>
          )}
        </div>
      )}

      {tab === 'agent' && (
        <div className="space-y-3">
          <div className="grid gap-3 md:grid-cols-[1fr_auto]">
            <label className="block space-y-1">
              <span className="text-[10px] uppercase tracking-wider text-text-muted">Agent browser URL</span>
              <input
                value={agentUrl}
                onChange={(e) => setAgentUrl(e.target.value)}
                className="w-full rounded-lg bg-overlay-subtle border border-border-subtle px-3 py-2 text-sm text-text-secondary"
              />
            </label>
            <label className="flex items-end gap-2 pb-1">
              <input
                type="checkbox"
                checked={headless}
                onChange={(e) => setHeadless(e.target.checked)}
                className="rounded-sm"
              />
              <span className="text-[11px] text-text-muted">Headless</span>
            </label>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={openAgentSession}
              className="rounded-lg bg-brass/20 text-brass px-4 py-2 text-[11px] uppercase tracking-wider disabled:opacity-50"
            >
              Open session
            </button>
            <button
              type="button"
              disabled={busy || !pageId}
              onClick={closeAgentSession}
              className="rounded-lg bg-overlay-subtle text-text-secondary px-4 py-2 text-[11px] uppercase tracking-wider disabled:opacity-50"
            >
              Close
            </button>
            <button
              type="button"
              disabled={!pageId || busy}
              onClick={captureFrame}
              className="rounded-lg bg-overlay-subtle text-text-secondary px-4 py-2 text-[11px] uppercase tracking-wider disabled:opacity-50"
            >
              Capture frame
            </button>
            <button
              type="button"
              disabled={!pageId || busy || !(pageInfo?.can_go_back ?? false)}
              onClick={() => navigate('back')}
              className="rounded-lg bg-overlay-subtle text-text-secondary px-4 py-2 text-[11px] uppercase tracking-wider disabled:opacity-50"
            >
              Back
            </button>
            <button
              type="button"
              disabled={!pageId || busy || !(pageInfo?.can_go_forward ?? false)}
              onClick={() => navigate('forward')}
              className="rounded-lg bg-overlay-subtle text-text-secondary px-4 py-2 text-[11px] uppercase tracking-wider disabled:opacity-50"
            >
              Forward
            </button>
            <button
              type="button"
              disabled={!pageId || busy}
              onClick={() => navigate('reload')}
              className="rounded-lg bg-overlay-subtle text-text-secondary px-4 py-2 text-[11px] uppercase tracking-wider disabled:opacity-50"
            >
              Reload
            </button>
            <button
              type="button"
              disabled={!pageId || busy}
              onClick={() => navigate('stop')}
              className="rounded-lg bg-overlay-subtle text-text-secondary px-4 py-2 text-[11px] uppercase tracking-wider disabled:opacity-50"
            >
              Stop
            </button>
            <button
              type="button"
              aria-pressed={controlMode === 'agent'}
              onClick={() => setControlModeRemote(controlMode === 'you' ? 'agent' : 'you')}
              className={`rounded-lg px-4 py-2 text-[11px] uppercase tracking-wider ${
                controlMode === 'you'
                  ? 'bg-brass/20 text-brass'
                  : 'bg-overlay-subtle text-text-secondary'
              }`}
            >
              Mode: {controlMode === 'you' ? 'You' : 'Agent'}
            </button>
          </div>
          <div className="grid gap-2 md:grid-cols-[1fr_auto]">
            <input
              value={agentNavUrl}
              onChange={(e) => setAgentNavUrl(e.target.value)}
              className="w-full rounded-lg bg-overlay-subtle border border-border-subtle px-3 py-2 text-sm text-text-secondary"
              placeholder="https://example.com"
            />
            <button
              type="button"
              disabled={!pageId || busy || !agentNavUrl.trim()}
              onClick={gotoUrl}
              className="rounded-lg bg-overlay-subtle text-text-secondary px-4 py-2 text-[11px] uppercase tracking-wider disabled:opacity-50"
            >
              Go
            </button>
          </div>
          <div className="flex flex-wrap gap-2">
            {pages.map((p) => {
              const active = p.page_id === pageId;
              return (
                <div
                  key={p.page_id}
                  className={`rounded-lg px-3 py-1.5 text-[11px] max-w-[280px] truncate ${
                    active
                      ? 'bg-brass/20 text-brass border border-brass/40'
                      : 'bg-overlay-subtle text-text-secondary border border-border-subtle'
                  }`}
                >
                  <button
                    type="button"
                    onClick={() => attachPage(p.page_id)}
                    className="mr-2 max-w-[220px] truncate align-middle"
                    title={`${p.title || '(untitled)'} — ${p.url}`}
                  >
                    {(p.title || '(untitled)').slice(0, 42)}
                  </button>
                  <button
                    type="button"
                    onClick={() => closePage(p.page_id)}
                    className="align-middle text-text-muted hover:text-text-primary"
                    aria-label={`Close ${p.title || p.page_id}`}
                    title="Close tab"
                  >
                    ×
                  </button>
                </div>
              );
            })}
          </div>
          <p className="text-[11px] text-text-muted font-mono">
            page_id={pageId ?? '—'} · can_go_back={String(pageInfo?.can_go_back ?? false)} · can_go_forward={String(pageInfo?.can_go_forward ?? false)}
          </p>
          <div className="grid gap-3 lg:grid-cols-[2fr_1fr]">
            <div
              tabIndex={0}
              onClick={onFrameClick}
              onWheel={onFrameWheel}
              onKeyDown={onFrameKeyDown}
              className="rounded-xl border border-border-subtle bg-black/30 min-h-[360px] flex items-center justify-center overflow-hidden focus:outline-hidden focus:ring-2 focus:ring-brass/30"
            >
              {frame?.image_base64 ? (
                <img
                  src={`data:image/png;base64,${frame.image_base64}`}
                  alt="Agent browser frame"
                  className="w-full h-full max-h-[480px] object-contain pointer-events-none"
                />
              ) : (
                <span className="text-text-muted text-sm">
                  {frame?.error ?? 'No frame yet — open a session or wait for the live stream (~3s).'}
                </span>
              )}
            </div>
            <div className="rounded-xl border border-border-subtle bg-black/20 p-3 max-h-[360px] overflow-auto">
              <h3 className="text-[10px] uppercase tracking-wider text-text-muted mb-2">Action log</h3>
              <ul role="log" aria-live="polite" aria-label="Browser action log" className="space-y-1 text-[11px] font-mono text-text-muted">
                {(actionLog.length ? actionLog : ['(empty)']).map((line, i) => (
                  <li key={`${line}-${i}`}>{line}</li>
                ))}
              </ul>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
