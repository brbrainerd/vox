import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// vox build emits to ./dist (codegen output: routes manifest + per-page TSX +
// the bootstrap: entry.tsx / vox-app.tsx / runtime-install.ts).
// Vite serves from this dir as the project root, with the Vox-emitted
// dist/entry.tsx as the entry (see index.html).
// Final web bundle goes to ./web-dist so the codegen output and the bundled
// site never collide; Capacitor's webDir points at web-dist.
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "web-dist",
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    host: "127.0.0.1",
    strictPort: true,
  },
  preview: {
    port: 5173,
    host: "127.0.0.1",
    strictPort: true,
  },
});
