// R9 reverify (library case): the More button shows a focus ring on keyboard focus.
import { expect, test } from '@playwright/test';

test('review_r9_rv_more_focus_ring visible on keyboard focus', async ({ page }) => {
  await page.goto('/library?scenario=rpm&launch=none');
  await page.getByRole('heading', { name: 'Applications' }).waitFor();
  const more = page.getByRole('button', { name: /^More options for/ }).first();
  // Reach it by keyboard so :focus-visible applies.
  for (let i = 0; i < 20; i++) {
    await page.keyboard.press('Tab');
    if (await more.evaluate((e) => e === document.activeElement)) break;
  }
  const s = await more.evaluate((e) => {
    const c = getComputedStyle(e);
    return {
      os: c.outlineStyle,
      ow: c.outlineWidth,
      sh: c.boxShadow,
      focused: e === document.activeElement
    };
  });
  expect(s.focused).toBe(true);
  expect((s.os !== 'none' && s.ow !== '0px') || s.sh !== 'none', JSON.stringify(s)).toBe(true);
});
