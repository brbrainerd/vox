import React from 'react'
import ReactDOM from 'react-dom/client'
import './index.css'
import App from './App'
import { installConsoleBridge } from './lib/consoleBridge'
import { invoke } from '@tauri-apps/api/core'
import { applyTheme } from './lib/theme'

// Mirror webview console errors/warnings into the backend log stream (no-op outside Tauri).
installConsoleBridge()

// Apply the persisted accent palette at bootstrap. The default `:root` is
// already 'arcane' (the historical look), so there is no flash for that case;
// non-default themes apply as soon as the preference resolves. Degrades to the
// default theme if the preference DB / Tauri bridge is unavailable.
invoke<string | null>('get_gui_preference', { key: 'gui.theme' })
  .then((theme) => applyTheme(theme))
  .catch(() => applyTheme('arcane'))

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
