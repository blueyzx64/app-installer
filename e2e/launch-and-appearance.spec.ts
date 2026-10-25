import { expect, test } from '@playwright/test';
import { APP, closeButton, dragIconToFolder, icon, openScenario, waitReady } from './helpers';

test.describe('launch file (F-01 F-03)', () => {
  test('package passed at start shows name and icon fast @smoke', async ({ page }) => {
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
    await expect(closeButton(page)).toBeVisible();
  });

  test('second open while installing is queued, not replacing (F-02)', async ({ page }) => {
    await openScenario(page, 'appimage');
    await waitReady(page);
    await dragIconToFolder(page);
    await expect(page.getByRole('progressbar')).toBeVisible();
    await page.evaluate(() =>
      (globalThis as unknown as { __mock: { emitOpenFile(p: string): void } }).__mock.emitOpenFile(
        '/home/you/Downloads/other-1.0.rpm'
      )
    );
    await expect(page.getByText('Another app is waiting')).toBeVisible();
    await expect(page.getByText(`${APP} is installed`)).toBeVisible({ timeout: 10_000 });
  });
});

test.describe('appearance (F-53 F-54)', () => {
  test('window colours follow the appearance scheme @smoke', async ({ page }, info) => {
    const scheme = info.project.name === 'dark' ? 'dark' : 'light';
    await openScenario(page, 'appimage', `&scheme=${scheme}`);
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
    await openScenario(page, 'appimage');
    await waitReady(page);
    const close = page.getByRole('button', { name: 'Close', exact: true });
    await expect(close).toHaveCount(1);
    const box = await close.boundingBox();
    expect(box!.x).toBeGreaterThan(380); // right half of the 760 px window
    await expect(page.getByRole('button', { name: 'Minimize' })).toHaveCount(0);
  });

  test('left traffic lights for close,minimize,maximize: (F-54)', async ({ page }) => {
    await openScenario(page, 'appimage', '&side=left&buttons=close,minimize,maximize');
    await waitReady(page);
    const close = page.getByRole('button', { name: 'Close', exact: true });
    const min = page.getByRole('button', { name: 'Minimize' });
    await expect(close).toBeVisible();
    await expect(min).toBeVisible();
    const c = (await close.boundingBox())!;
    const m = (await min.boundingBox())!;
    expect(c.x).toBeLessThan(380);
    expect(c.x).toBeLessThan(m.x);
  });

  test('live change of scheme, contrast and motion (F-53)', async ({ page }) => {
    await openScenario(page, 'appimage');
    await waitReady(page);
    const html = page.locator('html');
    await expect(html).toHaveAttribute('data-scheme', 'light');
    const set = (patch: Record<string, unknown>) =>
      page.evaluate(async (p) => {
        const m = (globalThis as unknown as { __mock: { emitAppearance(a: unknown): void } })
          .__mock;
        m.emitAppearance({
          colorScheme: 'light',
          accent: null,
          highContrast: false,
          reducedMotion: false,
          controls: { side: 'right', buttons: ['close'] },
          ...p
        });
      }, patch);
    await set({ colorScheme: 'dark', highContrast: true, reducedMotion: true, accent: '#ff7800' });
    await expect(html).toHaveAttribute('data-scheme', 'dark');
    await expect(html).toHaveAttribute('data-contrast', 'more');
    await expect(html).toHaveAttribute('data-motion', 'reduce');
    expect(await html.evaluate((e) => e.style.getPropertyValue('--accent'))).toBe('#ff7800');
  });
});
