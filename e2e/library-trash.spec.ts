// Lane Q2 (S9b). Library: F-40 empty state, F-41 trash AppImage, F-42 remove system package, F-43 reconcile.
import { expect, test, type Page } from '@playwright/test';
import { dismissDefaultPrompt } from './helpers';
import en from '../src/lib/i18n/en.json' with { type: 'json' };

function tx(key: keyof typeof en, vars: Record<string, string | number> = {}): string {
  return String(en[key]).replace(/\{(\w+)\}/g, (_, k: string) => String(vars[k] ?? `{${k}}`));
}
test.describe.configure({ timeout: 30_000 });

// Mock seed: Orbit Player (system package), Pixel Draw + Tidy Tasks (AppImage).
const SYSTEM_APP = 'Orbit Player';
const USER_APP = 'Pixel Draw';

async function openLibrary(page: Page, scenario = 'rpm') {
  await page.goto(`/library?scenario=${scenario}&launch=none`);
  await expect(page.getByRole('heading', { name: tx('library.title') })).toBeVisible();
}

function tile(page: Page, name: string) {
  return page.getByRole('listitem').filter({ hasText: name });
}

/** Opens the tile's More menu and picks "Move to Trash" (AppImage) or "Remove" (system package). */
async function startTrash(page: Page, name: string) {
  await tile(page, name)
    .getByRole('button', { name: tx('library.more', { app: name }) })
    .click();
  await page.getByRole('menuitem', { name: /Move to Trash|Remove/ }).click();
}

test('F-40 library lists the installed apps', async ({ page }) => {
  await openLibrary(page);
  for (const n of [SYSTEM_APP, USER_APP, 'Tidy Tasks']) {
    await expect(tile(page, n)).toBeVisible();
  }
});

test('F-40 empty library shows the empty state', async ({ page }) => {
  await openLibrary(page, 'empty-library');
  await expect(page.getByText(tx('library.empty'))).toBeVisible();
  await expect(page.getByRole('listitem')).toHaveCount(0);
});

test('F-41 trash confirm: Cancel keeps the app', async ({ page }) => {
  await openLibrary(page);
  await startTrash(page, USER_APP);
  const dialog = page.getByRole('alertdialog');
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText(tx('trash.title.user', { app: USER_APP }))).toBeVisible();
  await expect(dialog.getByText(tx('trash.body.user'))).toBeVisible();
  await dialog.getByRole('button', { name: tx('trash.cancel') }).click();
  await expect(dialog).toBeHidden();
  await expect(tile(page, USER_APP)).toBeVisible();
});

test('F-41 trash confirm: Move to Trash removes the AppImage tile', async ({ page }) => {
  await openLibrary(page);
  await startTrash(page, USER_APP);
  const dialog = page.getByRole('alertdialog');
  await expect(dialog).toBeVisible();
  // Real view titles it "Move to Trash"; the placeholder shows a generic confirm button.
  await dialog.getByRole('button', { name: /Move to Trash|Remove/ }).click();
  await expect(tile(page, USER_APP)).toHaveCount(0, { timeout: 8_000 });
  await expect(tile(page, 'Tidy Tasks')).toBeVisible();
});

test('F-41 trashing an AppImage never asks for a password', async ({ page }) => {
  await openLibrary(page);
  await startTrash(page, USER_APP);
  const dialog = page.getByRole('alertdialog');
  await expect(dialog.getByText(tx('trash.body.system'))).toHaveCount(0);
});

test('F-42 system package: password note, then removed', async ({ page }) => {
  await openLibrary(page);
  await startTrash(page, SYSTEM_APP);
  const dialog = page.getByRole('alertdialog');
  await expect(dialog.getByText(tx('trash.title.system', { app: SYSTEM_APP }))).toBeVisible();
  await expect(dialog.getByText(tx('trash.body.system'))).toBeVisible();
  await dialog.getByRole('button', { name: tx('trash.confirm.system') }).click();
  await expect(tile(page, SYSTEM_APP)).toHaveCount(0, { timeout: 10_000 });
});

test('F-42 removing shows "Removing …" progress text', async ({ page }) => {
  await openLibrary(page);
  await startTrash(page, SYSTEM_APP);
  await page
    .getByRole('alertdialog')
    .getByRole('button', { name: tx('trash.confirm.system') })
    .click();
  await expect(page.getByText(tx('library.removing', { app: SYSTEM_APP }))).toBeVisible();
});

test('F-42 extra packages: "This also removes 2 other items"', async ({ page }) => {
  await openLibrary(page);
  await startTrash(page, SYSTEM_APP);
  await expect(
    page.getByRole('alertdialog').getByText(tx('trash.body.extras', { n: 2, app: SYSTEM_APP }))
  ).toBeVisible();
});

test('F-42 dismissing the password dialog keeps the app and shows no error', async ({ page }) => {
  await openLibrary(page, 'cancel-permission');
  await startTrash(page, SYSTEM_APP);
  await page
    .getByRole('alertdialog')
    .getByRole('button', { name: tx('trash.confirm.system') })
    .click();
  await expect(page.getByText(tx('library.removing', { app: SYSTEM_APP }))).toBeHidden({
    timeout: 8_000
  });
  await expect(tile(page, SYSTEM_APP)).toBeVisible();
  await expect(page.getByText(/went wrong|error|failed|denied/i)).toHaveCount(0);
});

test('F-42 a failed removal keeps the app and says so in plain words', async ({ page }) => {
  await openLibrary(page, 'fail-offline');
  await startTrash(page, SYSTEM_APP);
  await page
    .getByRole('alertdialog')
    .getByRole('button', { name: tx('trash.confirm.system') })
    .click();
  await expect(page.getByText(tx('error.removeUnknown', { app: SYSTEM_APP }))).toBeVisible({
    timeout: 8_000
  });
  await expect(tile(page, SYSTEM_APP)).toBeVisible();
});

test('F-43 apps removed outside the installer disappear on next open', async ({ page }) => {
  await openLibrary(page);
  await expect(tile(page, USER_APP)).toBeVisible();
  // Remove behind the ledger's back, leave the library and come back (the "next open").
  await page.evaluate(() => {
    const m = (
      globalThis as unknown as {
        __mock: { removeApp(id: string): void; emitOpenFile(p: string): void };
      }
    ).__mock;
    m.removeApp('pixel-draw');
    m.emitOpenFile('/home/you/Downloads/Lumen_Notes-2.4.1.AppImage');
  });
  await expect(page).toHaveURL(/\/install/);
  await dismissDefaultPrompt(page, page.getByText('Drag Lumen Notes to Applications'));
  await page.goBack();
  await expect(page).toHaveURL(/\/library/);
  await expect(page.getByRole('heading', { name: tx('library.title') })).toBeVisible();
  await expect(tile(page, 'Tidy Tasks')).toBeVisible();
  await expect(tile(page, USER_APP)).toHaveCount(0);
});
