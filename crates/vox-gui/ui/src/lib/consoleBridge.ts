import { invoke } from '@tauri-apps/api/core';

/**
 * Forward webview console errors/warnings and uncaught errors to the Rust backend (the
 * `log_frontend` command) so they appear in the single `cargo run -p vox-gui` log stream
 * alongside backend output — one place to watch for both frontend and backend problems.
 *
 * No-op outside the Tauri webview, and skipped under automation (Playwright sets
 * `navigator.webdriver`) so the visual-audit sweep is unaffected.
 */
export function installConsoleBridge(): void {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
  if (typeof w.__TAURI_INTERNALS__ === 'undefined') return;
  if ((navigator as Navigator & { webdriver?: boolean }).webdriver) return;

  const send = (level: 'error' | 'warn', parts: unknown[]) => {
    try {
      const message = parts
        .map((p) =>
          p instanceof Error ? (p.stack ?? p.message) : typeof p === 'string' ? p : safeJson(p),
        )
        .join(' ');
      void invoke('log_frontend', { level, message });
    } catch {
      /* never let logging break the app */
    }
  };

  (['error', 'warn'] as const).forEach((level) => {
    const original = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      send(level, args);
      original(...args);
    };
  });

  window.addEventListener('error', (e) =>
    send('error', [`${e.message} (${e.filename}:${e.lineno}:${e.colno})`]),
  );
  window.addEventListener('unhandledrejection', (e) =>
    send('error', ['unhandledrejection', (e as PromiseRejectionEvent).reason]),
  );
}

function safeJson(v: unknown): string {
  try {
    return JSON.stringify(v);
  } catch {
    return String(v);
  }
}
