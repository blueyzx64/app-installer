// Library must not scroll sideways at the smallest window (600x400), also at 1.25x text.
import { expect, test } from '@playwright/test';

for (const scale of [1, 1.25]) {
  for (const scenario of ['rpm', 'empty-library']) {
    test(`library has no horizontal scroll at 600x400, text ${scale}x, ${scenario}`, async ({
      page
    }) => {
      await page.setViewportSize({ width: 600, height: 400 });
      await page.goto(`/library?scenario=${scenario}&launch=none`);
      await expect(page.getByRole('button', { name: 'Open a Package…' }).first()).toBeVisible();
      await page.addStyleTag({ content: `:root{--text-scale:${scale} !important}` });
      const sizes = await page.evaluate(() => {
        const main = document.querySelector('main')!;
        const btn = document.querySelector('header button')!.getBoundingClientRect();
        return {
          doc: [document.documentElement.scrollWidth, document.documentElement.clientWidth],
          main: [main.scrollWidth, main.clientWidth],
          buttonRight: btn.right,
          width: innerWidth
        };
      });
      expect(sizes.doc[0]).toBeLessThanOrEqual(sizes.doc[1]!);
      expect(sizes.main[0]).toBeLessThanOrEqual(sizes.main[1]!);
      expect(sizes.buttonRight).toBeLessThanOrEqual(sizes.width);
    });
  }
}
