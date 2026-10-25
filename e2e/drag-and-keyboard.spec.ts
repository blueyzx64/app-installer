import { expect, test } from '@playwright/test';
import { APP, dragIconTo, folder, icon, openScenario, progressbar, waitReady } from './helpers';

test.describe('drag outside and keyboard', () => {
  test.beforeEach(async ({ page }) => {
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
  await openScenario(page, 'appimage');
  await waitReady(page);
  // The mock appearance does not follow the OS query (UIBUG), so set it the way GNOME would.
  await page.evaluate(() =>
    (
      globalThis as unknown as { __mock: { emitAppearance(a: unknown): void } }
    ).__mock.emitAppearance({
      colorScheme: 'light',
      accent: null,
      highContrast: false,
      reducedMotion: true,
      controls: { side: 'right', buttons: ['close'] }
    })
  );
  await expect(page.locator('html')).toHaveAttribute('data-motion', 'reduce');
  const before = await icon(page).boundingBox();
  await dragIconTo(page, { x: 20, y: 500 });
  const after = await icon(page).boundingBox();
  expect(Math.abs(after!.x - before!.x)).toBeLessThan(2);
  expect(Math.abs(after!.y - before!.y)).toBeLessThan(2);
});

test.describe('touch', () => {
  test.use({ hasTouch: true });
  test('touch drag installs (F-20)', async ({ page }) => {
    await openScenario(page, 'appimage');
    await waitReady(page);
    const a = (await icon(page).boundingBox())!;
    const b = (await folder(page).boundingBox())!;
    const from = { x: a.x + a.width / 2, y: a.y + a.height / 2 };
    const to = { x: b.x + b.width / 2, y: b.y + b.height / 2 };
    const cdp = await page.context().newCDPSession(page);
    const touch = (type: string, p?: { x: number; y: number }) =>
      cdp.send('Input.dispatchTouchEvent', { type, touchPoints: p ? [p] : [] });
    await touch('touchStart', from);
    for (let i = 1; i <= 10; i++) {
      await touch('touchMove', {
        x: from.x + ((to.x - from.x) * i) / 10,
        y: from.y + ((to.y - from.y) * i) / 10
      });
    }
    await touch('touchEnd');
    await expect(progressbar(page)).toBeVisible();
  });
});
