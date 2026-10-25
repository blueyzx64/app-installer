import { expect, test } from '@playwright/test';
import {
  APP,
  dragIconTo,
  icon,
  openScenario,
  progressbar,
  UI_PENDING,
  uiWired,
  waitReady
} from './helpers';

test.describe('drag outside and keyboard', () => {
  test.beforeEach(async ({ page }) => {
    test.fixme(!uiWired, UI_PENDING);
    await openScenario(page, 'appimage');
    await waitReady(page);
  });

  test('dropping outside the folder springs back, nothing installs (F-20) @smoke', async ({
    page
  }) => {
    const before = await icon(page).boundingBox();
    expect(before).not.toBeNull();
    // Far from the folder: bottom-left corner of the window.
    await dragIconTo(page, { x: 20, y: 500 });

    await expect
      .poll(async () => (await icon(page).boundingBox())?.x ?? -1, { timeout: 4_000 })
      .toBeCloseTo(before!.x, 0);
    await expect
      .poll(async () => (await icon(page).boundingBox())?.y ?? -1)
      .toBeCloseTo(before!.y, 0);
    await expect(progressbar(page)).toHaveCount(0);
    await expect(page.getByText(`Drag ${APP} to Applications`)).toBeVisible();
  });

  test('Enter on the focused icon installs; hint is announced via aria-describedby (F-21) @smoke', async ({
    page
  }) => {
    await icon(page).focus();
    const id = await icon(page).getAttribute('aria-describedby');
    expect(id).toBeTruthy();
    await expect(page.locator(`[id="${id}"]`)).toHaveText('Press Enter to install');

    await page.keyboard.press('Enter');
    await expect(progressbar(page)).toBeVisible();
    await expect(page.getByText(`${APP} is installed`)).toBeVisible({ timeout: 10_000 });
  });

  test('Space also installs', async ({ page }) => {
    await icon(page).focus();
    await page.keyboard.press('Space');
    await expect(progressbar(page)).toBeVisible();
  });
});

test('reduced motion: spring back is instant (F-20)', async ({ page }) => {
  test.fixme(!uiWired, UI_PENDING);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await openScenario(page, 'appimage');
  await waitReady(page);
  const before = await icon(page).boundingBox();
  await dragIconTo(page, { x: 20, y: 500 });
  const after = await icon(page).boundingBox();
  expect(Math.abs(after!.x - before!.x)).toBeLessThan(2);
  expect(Math.abs(after!.y - before!.y)).toBeLessThan(2);
});

// Pointer events with pointerType touch need CDP touch emulation; component test covers it.
test.fixme('touch drag installs (F-20)', async () => {
  // Needs a hasTouch context + CDP Input.dispatchTouchEvent. Do in W2 if component test is not enough.
});
