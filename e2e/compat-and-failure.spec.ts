import { expect, test } from '@playwright/test';
import {
  APP,
  UI_PENDING,
  alertDialog,
  dragIconToFolder,
  icon,
  openScenario,
  uiWired,
  waitReady
} from './helpers';

test.describe('incompatible packages (F-03 compat, state machine Incompatible)', () => {
  test('wrong processor: plain message, Close only @smoke', async ({ page }) => {
    await openScenario(page, 'incompatible-arch');
    await expect(page.getByText(`${APP} can’t be installed here`)).toBeVisible();
    await expect(page.getByText('different kind of processor')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Close' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Open' })).toHaveCount(0);
    await expect(icon(page)).toHaveCount(0);
  });

  test('already installed: shows Open', async ({ page }) => {
    await openScenario(page, 'already-installed');
    await expect(page.getByText(`${APP} is already installed`)).toBeVisible();
    await expect(page.getByRole('button', { name: 'Open' })).toBeVisible();
    await page.getByRole('button', { name: 'Open' }).click();
  });

  test('update: stays installable, folder says Update', async ({ page }) => {
    test.fixme(!uiWired, UI_PENDING);
    await openScenario(page, 'update');
    await waitReady(page);
    await expect(page.getByText('Update', { exact: true })).toBeVisible();
  });

  test('missing FUSE 2: asks for help in plain words', async ({ page }) => {
    await openScenario(page, 'missing-fuse2');
    await expect(page.getByText(`${APP} needs something your computer doesn’t have`)).toBeVisible();
  });
});

test.describe('failures and cancel (system package path on mock)', () => {
  test.beforeEach(() => test.fixme(!uiWired, UI_PENDING));

  test('fail: Details hidden by default, Try Again goes back to Ready', async ({ page }) => {
    await openScenario(page, 'fail-offline');
    await waitReady(page);
    await dragIconToFolder(page);

    const alert = alertDialog(page);
    await expect(alert).toBeVisible({ timeout: 15_000 });
    await expect(alert).toContainText(`${APP} needs an internet connection`);

    const details = page.getByRole('button', { name: 'Details' });
    await expect(details).toHaveAttribute('aria-expanded', 'false');
    await expect(page.getByText('mock: could not reach the network')).toBeHidden();
    await details.click();
    await expect(page.getByText('mock: could not reach the network')).toBeVisible();

    await page.getByRole('button', { name: 'Try Again' }).click();
    await expect(alert).toBeHidden();
    await expect(icon(page)).toBeVisible();
  });

  test('cancel at the password step: icon springs back, no error text', async ({ page }) => {
    await openScenario(page, 'cancel-permission');
    await waitReady(page);
    await dragIconToFolder(page);

    await expect(page.getByText('Waiting for permission…')).toBeVisible();
    // The mock reports "cancelled" by itself after ~1.6 s, as a dismissed polkit dialog would.
    await expect(icon(page)).toBeVisible({ timeout: 8_000 });
    await expect(page.getByText('Waiting for permission…')).toBeHidden();
    await expect(alertDialog(page)).toHaveCount(0);
    await expect(page.locator('body')).not.toContainText(/went wrong|error|failed|denied/i);
  });

  test('Cancel button on the permission shade also springs back', async ({ page }) => {
    await openScenario(page, 'cancel-permission');
    await waitReady(page);
    await dragIconToFolder(page);
    await page.getByRole('button', { name: 'Cancel' }).click();
    await expect(icon(page)).toBeVisible({ timeout: 8_000 });
    await expect(page.locator('body')).not.toContainText(/went wrong|error|failed|denied/i);
  });
});
