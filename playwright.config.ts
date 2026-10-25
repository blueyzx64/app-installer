import { defineConfig, devices } from '@playwright/test';
import { existsSync } from 'node:fs';

// E2E on `vite dev` with the mock gateways (plain browser = mock, see routes/+layout.svelte).
// Chromium comes from PLAYWRIGHT_BROWSERS_PATH. Set E2E_CHROMIUM=/path/to/chrome to override.
const executablePath =
  process.env.E2E_CHROMIUM && existsSync(process.env.E2E_CHROMIUM)
    ? process.env.E2E_CHROMIUM
    : undefined;

const viewport = { width: 760, height: 520 };
const base = {
  ...devices['Desktop Chrome'],
  viewport,
  launchOptions: { executablePath }
};

export default defineConfig({
  testDir: './e2e',
  outputDir: './test-results',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : 'list',
  expect: { timeout: 7_000 },
  use: { baseURL: 'http://localhost:1420', trace: 'retain-on-failure' },
  webServer: {
    command: 'pnpm dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000
  },
  projects: [
    { name: 'light', use: { ...base, colorScheme: 'light' } },
    // dark and reduced-motion re-run only the specs tagged @smoke.
    { name: 'dark', grep: /@smoke/, use: { ...base, colorScheme: 'dark' } },
    {
      name: 'reduced-motion',
      grep: /@smoke/,
      use: { ...base, colorScheme: 'light', reducedMotion: 'reduce' }
    }
  ]
});
