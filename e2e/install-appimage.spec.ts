import { expect, test } from '@playwright/test';
import {
  APP,
  alertDialog,
  dragIconToFolder,
  folder,
  icon,
  openScenario,
  progressbar,
  waitReady
} from './helpers';

// F-01 F-20 F-22 F-25 F-29 on the mock AppImage flow.
test.describe('install AppImage by dragging', () => {
  test.beforeEach(async ({ page }) => {
    await openScenario(page, 'appimage');
    await waitReady(page);
  });

  test('drag, drop, progress, done, Open @smoke', async ({ page }) => {
    await expect(page.getByText(`Drag ${APP} to Applications`)).toBeVisible();
    // No password for a user install (F-22).
    await expect(page.getByText('Needs your password')).toHaveCount(0);

    await dragIconToFolder(page);

    await expect(page.getByText(`Installing ${APP}`).first()).toBeVisible();
    await expect(progressbar(page)).toBeVisible();
    await expect(page.getByText('Waiting for permission…')).toHaveCount(0);

    await expect(page.getByText(`${APP} is installed`)).toBeVisible({ timeout: 10_000 });
    await expect(page.getByRole('button', { name: 'Done' })).toBeVisible();
    await page.getByRole('button', { name: 'Open' }).click();
    await expect(alertDialog(page)).toHaveCount(0);
  });

  test('Done button closes the success screen', async ({ page }) => {
    await dragIconToFolder(page);
    await expect(page.getByText(`${APP} is installed`)).toBeVisible({ timeout: 10_000 });
    await page.getByRole('button', { name: 'Done' }).click();
    await expect(page.getByText(`${APP} is installed`)).toBeHidden();
  });

  test('folder highlights while the icon hovers it', async ({ page }) => {
    await dragIconToFolder(page, false);
    await expect(folder(page)).toHaveClass(/hover/);
    await page.mouse.up();
  });

  test('progress uses plain words and never leaks package names (F-25)', async ({ page }) => {
    await dragIconToFolder(page);
    const body = page.locator('body');
    await expect(page.getByText(/Getting ready|Installing|Finishing up/).first()).toBeVisible();
    await expect(body).not.toContainText(/\.rpm|\.deb|dnf|libfuse|glibc|dependenc/i);
    await expect(icon(page)).toBeHidden();
  });
});
