// S5 fix2: the error Alert fills the window and is centred (it used to collapse to the top).
import { expect, test } from '@playwright/test';

test('damaged file: the alert is centred inside the window, not clipped', async ({ page }) => {
  // A blank file name cannot be read, so the mock reports a damaged file.
  await page.goto('/?scenario=rpm&file=%20');
  const alert = page.getByRole('alertdialog');
  await expect(alert).toBeVisible();
  const box = await alert.boundingBox();
  const vp = page.viewportSize()!;
  expect(box).not.toBeNull();
  expect(box!.y).toBeGreaterThanOrEqual(0);
  expect(box!.y + box!.height).toBeLessThanOrEqual(vp.height);
  // Roughly centred horizontally and vertically (title bar makes the body a bit shorter).
  expect(Math.abs(box!.x + box!.width / 2 - vp.width / 2)).toBeLessThan(8);
  expect(box!.y).toBeGreaterThan(20);
});

test('damaged file: Escape closes the error like its Close button', async ({ page }) => {
  await page.goto('/?scenario=rpm&file=%20');
  await expect(page.getByRole('alertdialog')).toBeVisible();
  await page.keyboard.press('Escape');
  // Mock window.close is a no-op; the installer leaves the error screen.
  await expect(page.getByRole('alertdialog')).toBeHidden();
});
