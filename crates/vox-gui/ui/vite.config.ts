import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const TAURI_DEV_PORT = 1420; // required by Tauri devUrl; do not change

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: TAURI_DEV_PORT,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
})
