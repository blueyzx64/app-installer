// Lane Q2 (S9b). Compatibility screens: F-10 F-11 F-12 F-13 F-14 (+ missing runtime). Mock via ?scenario=.
import { expect, test, type Page } from '@playwright/test';
import en from '../src/lib/i18n/en.json' with { type: 'json' };
import { closeButton } from './helpers';

function tx(key: keyof typeof en, vars: Record<string, string | number> = {}): string {
  return String(en[key]).replace(/\{(\w+)\}/g, (_, k: string) => String(vars[k] ?? `{${k}}`));
}
const APP = 'Lumen Notes';
test.describe.configure({ timeout: 30_000 });

function installTarget(page: Page) {
  return page
    .getByRole('button', { name: /Drag into Applications to install/ })
    .or(page.getByRole('button', { name: tx('dmg.folder'), exact: true }));
}

/** No Applications folder and no draggable icon: nothing to install. */
async function expectNoInstallUi(page: Page) {
  await expect(installTarget(page)).toHaveCount(0);
  await expect(page.getByText(tx('dmg.folder'), { exact: true })).toHaveCount(0);
  await expect(page.getByText(tx('dmg.folderUpdate'), { exact: true })).toHaveCount(0);
}

test('F-10 wrong family (.deb on Fedora): plain sentence, no Applications folder', async ({
  page
}) => {
  await page.goto('/?scenario=wrong-family');
  await expect(page.getByText(tx('compat.wrongFamily.debBased.title', { app: APP }))).toBeVisible();
  await expect(page.getByText(tx('compat.wrongFamily.debBased.body'))).toBeVisible();
  await expectNoInstallUi(page);
});

test('F-11 wrong architecture: plain explanation, Close only', async ({ page }) => {
  await page.goto('/?scenario=incompatible-arch');
  await expect(page.getByText(tx('compat.wrongArch.title', { app: APP }))).toBeVisible();
  await expect(page.getByText(tx('compat.wrongArch.body'))).toBeVisible();
  await expect(closeButton(page)).toBeVisible();
  await expect(page.getByRole('button', { name: tx('action.open'), exact: true })).toHaveCount(0);
  await expectNoInstallUi(page);
  // No technical words on screen.
  const body = await page.locator('body').innerText();
  expect(body).not.toMatch(/aarch64|x86_64|arm64|amd64/i);
});

test('F-12 already installed (same version) offers Open', async ({ page }) => {
  await page.goto('/?scenario=already-installed');
  await expect(
    page.getByText(tx('compat.alreadyInstalled.same.title', { app: APP }))
  ).toBeVisible();
  await expect(page.getByRole('button', { name: tx('action.open'), exact: true })).toBeVisible();
  await expect(closeButton(page)).toBeVisible();
  await expectNoInstallUi(page);
});

test('F-12 pressing Open launches the app without an error', async ({ page }) => {
  await page.goto('/?scenario=already-installed');
  await page.getByRole('button', { name: tx('action.open'), exact: true }).click();
  await expect(page.getByText(/went wrong|error|failed/i)).toHaveCount(0);
});

test('F-13 newer version on disk: folder says Update, not Applications', async ({ page }) => {
  await page.goto('/?scenario=update');
  await expect(installTarget(page)).toBeVisible();
  await expect(page.getByText(tx('dmg.folderUpdate'), { exact: true })).toBeVisible();
  await expect(page.getByText(tx('dmg.folder'), { exact: true })).toHaveCount(0);
});

test('F-13 Update runs through permission and done like a new install', async ({ page }) => {
  await page.goto('/?scenario=update');
  await expect(installTarget(page)).toBeVisible();
  await installTarget(page).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText(tx('perm.waiting'))).toBeVisible();
  await expect(page.getByText(tx('done.title', { app: APP }))).toBeVisible({ timeout: 15_000 });
});

test('F-14 older version than installed: "A newer version is already installed"', async ({
  page
}) => {
  await page.goto('/?scenario=older-installed');
  await expect(
    page.getByText(tx('compat.alreadyInstalled.older.title', { app: APP }))
  ).toBeVisible();
  await expect(page.getByRole('button', { name: tx('action.open'), exact: true })).toHaveCount(0);
  await expectNoInstallUi(page);
});

test('missing FUSE 2 support: plain message, nothing to drag', async ({ page }) => {
  await page.goto('/?scenario=missing-fuse2');
  await expect(page.getByText(tx('compat.missingRuntime.fuse2.title', { app: APP }))).toBeVisible();
  await expectNoInstallUi(page);
});

test('Close on an incompatible package ends the flow without an error', async ({ page }) => {
  await page.goto('/?scenario=incompatible-arch');
  await closeButton(page).click();
  await expect(page.getByText(/went wrong|error|failed/i)).toHaveCount(0);
});
