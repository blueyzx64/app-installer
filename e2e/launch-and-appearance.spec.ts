import { expect, test } from '@playwright/test';
import { APP, UI_PENDING, icon, openScenario, uiWired, waitReady } from './helpers';

test.describe('launch file (F-01 F-03)', () => {
  test('package passed at start shows name and icon fast @smoke', async ({ page }) => {
    test.fixme(!uiWired, UI_PENDING);
    await openScenario(page, 'appimage');
    await waitReady(page);
    await expect(page.getByText(APP, { exact: true }).first()).toBeVisible();
    await expect(icon(page).locator('img')).toBeVisible();
  });

  test('no launch file opens the library', async ({ page }) => {
    await page.goto('/?scenario=appimage&launch=none');
    await expect(page).toHaveURL(/\/library/);
    await expect(icon(page)).toHaveCount(0);
  });

  test('damaged file shows the plain damaged message (F-03)', async ({ page }) => {
    // Mock rejects a blank path with corruptFile.
    await page.goto('/?launch=x&file=%20');
    await expect(page.getByText(/damaged or isn’t an app/)).toBeVisible();
    await expect(page.getByRole('button', { name: 'Close' })).toBeVisible();
  });

  test.fixme('second open while installing is queued (F-02)', async () => {
    // Needs a way to emit open-file from the page. Mock has MockControls.emitOpenFile but
    // nothing exposes it to the page. ASK O: S5 sets window.__mock = gateways.controls in dev.
  });
});

test.describe('appearance (F-53 F-54)', () => {
  test('follows prefers-color-scheme @smoke', async ({ page }, info) => {
    test.fixme(!uiWired, UI_PENDING);
    await openScenario(page, 'appimage');
    await waitReady(page);
    const lum = await page.evaluate(() => {
      const m = getComputedStyle(document.body).backgroundColor.match(/\d+(\.\d+)?/g) ?? [];
      return m.length >= 3 && Number(m[3] ?? 1) === 0 ? null : Number(m[0]);
    });
    // Transparent body means the shell paints the window; accept and skip the check.
    test.skip(lum === null, 'body background is transparent; window surface checked visually');
    if (info.project.name === 'dark') expect(lum).toBeLessThan(100);
    else expect(lum).toBeGreaterThan(150);
  });

  test('default controls: one close button on the right (F-54)', async ({ page }) => {
    test.fixme(!uiWired, UI_PENDING);
    await openScenario(page, 'appimage');
    await waitReady(page);
    const close = page.getByRole('button', { name: 'Close', exact: true });
    await expect(close).toHaveCount(1);
    const box = await close.boundingBox();
    expect(box!.x).toBeGreaterThan(380); // right half of the 760 px window
    await expect(page.getByRole('button', { name: 'Minimize' })).toHaveCount(0);
  });

  test.fixme('left traffic lights for close,minimize,maximize: (F-54)', async () => {
    // Mock appearance is fixed {side:right, buttons:[close]}. ASK O: add ?buttons=close,minimize,maximize&side=left
    // to the mock (S5) so this can run. Rust mapping is covered by appearance.rs unit tests (S7).
  });

  test.fixme('live change of scheme/accent/contrast/motion (F-53)', async () => {
    // Needs MockControls.emitAppearance on window (same hook as F-02). Real portal: HUMAN.
  });
});
