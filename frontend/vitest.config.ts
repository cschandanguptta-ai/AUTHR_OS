import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'node',
    globals: true,
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    // jsdom is opted into per-file via the `@vitest-environment jsdom`
    // docblock (component-level tests).
    setupFiles: [],
  },
  // The production build (vite.config.ts) is separate: Tauri dev server
  // port rules must not affect the unit suite.
});
