import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const TAURI_DEV_PORT = 1420; // required by Tauri devUrl; do not change

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // esbuild 0.28+ cannot downlevel destructuring for legacy browser targets during dep prebundle.
  build: {
    target: 'es2022',
  },
  optimizeDeps: {
    esbuildOptions: {
      target: 'es2022',
    },
  },
  server: {
    port: TAURI_DEV_PORT,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
})
