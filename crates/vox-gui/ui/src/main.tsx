import React from 'react'
import ReactDOM from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import './index.css'
import App from './App'
import { installConsoleBridge } from './lib/consoleBridge'
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
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
)
