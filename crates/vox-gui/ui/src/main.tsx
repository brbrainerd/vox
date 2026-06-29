import React from 'react'
import ReactDOM from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import './index.css'
import App from './App'
import { installConsoleBridge } from './lib/consoleBridge'
import { LanguageProvider } from './hooks/useLanguage'
import { applyTheme } from './lib/theme'
import { voxTransport } from './transport'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 30_000,
    },
  },
})

// Bare-browser detection: Tauri injects window.__TAURI_INTERNALS__. Outside it
// (vite preview, headless screenshot capture) tag <html> so index.css can
// neutralize backdrop-filter — software compositing cannot rasterize the glass
// blur and hangs screenshot capture. The real Tauri app is unaffected.
if (typeof (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ === 'undefined') {
  document.documentElement.classList.add('no-tauri')
}

// Mirror webview console errors/warnings into the backend log stream (no-op outside Tauri).
installConsoleBridge()

// Apply the persisted accent palette at bootstrap. The default `:root` is
// already 'arcane' (the historical look), so there is no flash for that case;
// non-default themes apply as soon as the preference resolves. Degrades to the
// default theme if the preference DB / Tauri bridge is unavailable.
voxTransport
  .getGuiPreference('gui.theme')
  .then((theme) => applyTheme(theme))
  .catch(() => applyTheme('arcane'))

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <LanguageProvider>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </LanguageProvider>
  </React.StrictMode>,
)
