import React from 'react'
import ReactDOM from 'react-dom/client'
import './index.css'
import App from './App'
import { installConsoleBridge } from './lib/consoleBridge'

// Mirror webview console errors/warnings into the backend log stream (no-op outside Tauri).
installConsoleBridge()

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
