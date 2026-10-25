import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig(({ mode }) => ({
  plugins: [sveltekit()],
  // Svelte must resolve to its browser build for component tests.
  resolve: mode === 'test' ? { conditions: ['browser'] } : undefined,
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_'],
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] }
  },
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
    setupFiles: ['src/lib/ui/test-setup.ts']
  }
}));
