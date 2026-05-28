import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Vox codegen output lives in dist/; treat as additional source root
  resolve: {
    alias: {
      '@vox': '/dist',
    },
  },
  build: {
    outDir: 'build',
    sourcemap: false,
    // Inline small assets to reduce request count
    assetsInlineLimit: 4096,
  },
  server: {
    port: 3000,
    // Proxy API calls to the Vox Rust runtime in dev
    proxy: {
      '/api': {
        target: 'http://localhost:3001',
        changeOrigin: true,
      },
    },
  },
})
