// Shared vitest setup. jest-dom matchers + DOM cleanup after each component test.
// Component test files opt into jsdom with `// @vitest-environment jsdom` on line 1.
import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/svelte';
import { afterEach } from 'vitest';

afterEach(() => {
  if (typeof document !== 'undefined') cleanup();
});

// jsdom has no matchMedia; svelte/motion and motion.ts query it.
if (typeof window !== 'undefined' && typeof window.matchMedia !== 'function') {
  window.matchMedia = (query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false
    }) as MediaQueryList;
}
