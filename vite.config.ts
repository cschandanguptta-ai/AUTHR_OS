import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { authrIpcProxy } from './scripts/ipc-proxy.mjs';

// AUTHR_OS — Vite 6 + React 19. The `authrIpcProxy` plugin wires the browser dev
// bridge (POST /api/ipc) to the real Rust CLI core (`authr` binary), so the
// cockpit is fully functional in plain-browser dev mode without Tauri.
export default defineConfig({
  plugins: [react(), authrIpcProxy()],
  server: {
    port: 5173,
    strictPort: false,
    // The Rust target tree exhausts inotify watches (ENOSPC) — nothing in the
    // dev bridge needs it; the middleware spawns the release binary directly.
    watch: {
      ignored: ['**/src-tauri/target/**'],
    },
  },
  build: {
    target: 'es2022',
    sourcemap: true,
  },
});
