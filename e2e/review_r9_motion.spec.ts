import { expect, test } from '@playwright/test';

// Browser reports prefers-reduced-motion: reduce. Mock appearance must not override it to "full".
test('review_r9_motion OS reduce-motion is honoured in dev/mock @smoke', async ({ page }) => {
  await page.goto('/library?scenario=rpm&launch=none');
  await page.getByRole('heading', { name: 'Applications' }).waitFor();
  const r = await page.evaluate(() => ({
    mq: matchMedia('(prefers-reduced-motion: reduce)').matches,
    attr: document.documentElement.dataset.motion ?? null,
    anim: getComputedStyle(document.documentElement).getPropertyValue('--anim-sheet').trim()
  }));
  if (r.mq) {
    expect(r.attr).not.toBe('full');
    expect(r.anim).toMatch(/fade-in/);
  }
});
