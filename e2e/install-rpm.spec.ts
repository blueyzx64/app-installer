// Lane Q2 (S9b). rpm install flow: F-23 F-24 F-25 F-26 F-27 F-28. Runs against the mock (pnpm dev).
import { expect, test, type Page } from '@playwright/test';
import en from '../src/lib/i18n/en.json' with { type: 'json' };

/** i18n text with {param} filled in, so specs assert on the strings the UI shows. */
function tx(key: keyof typeof en, vars: Record<string, string | number> = {}): string {
  return String(en[key]).replace(/\{(\w+)\}/g, (_, k: string) => String(vars[k] ?? `{${k}}`));
}
const APP = 'Lumen Notes';
test.describe.configure({ timeout: 30_000 });

/** The draggable icon (real view) or the Applications button (placeholder route). */
function installTarget(page: Page) {
  return page
    .getByRole('button', { name: /Drag into Applications to install/ })
    .or(page.getByRole('button', { name: tx('dmg.folder'), exact: true }));
}

async function openScenario(page: Page, scenario: string) {
  await page.goto(`/?scenario=${scenario}`);
  await expect(installTarget(page)).toBeVisible();
}

/** Keyboard install: focus the icon and press Enter (same event as a drop). */
async function keyInstall(page: Page) {
  await installTarget(page).focus();
  await page.keyboard.press('Enter');
}

test('F-23 rpm: permission shade, then progress, then done with Open', async ({ page }) => {
  await openScenario(page, 'rpm');
  await keyInstall(page);

  // Shade first: the system password dialog is open.
  await expect(page.getByText(tx('perm.waiting'))).toBeVisible();
  // Then real progress.
  await expect(page.getByRole('progressbar')).toBeVisible({ timeout: 10_000 });
  await expect(page.getByText(tx('phase.installing', { app: APP }))).toBeVisible({
    timeout: 15_000
  });
  // Done.
  await expect(page.getByText(tx('done.title', { app: APP }))).toBeVisible({ timeout: 15_000 });
  await expect(page.getByRole('button', { name: tx('done.open'), exact: true })).toBeVisible();
});

test('F-23 rpm: dragging the icon onto Applications starts the install', async ({ page }) => {
  test.fixme(true, 'waiting S6: route wiring of DmgView (placeholder route has no draggable icon)');
  await openScenario(page, 'rpm');
  const icon = installTarget(page);
  const folder = page.getByText(tx('dmg.folder'), { exact: true });
  const a = (await icon.boundingBox())!;
  const b = (await folder.boundingBox())!;
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 12 });
  await page.mouse.up();
  await expect(page.getByText(tx('perm.waiting'))).toBeVisible();
});

test('F-23 the app never asks for a password itself', async ({ page }) => {
  await openScenario(page, 'rpm');
  await keyInstall(page);
  await expect(page.getByText(tx('perm.waiting'))).toBeVisible();
  await expect(page.locator('input[type="password"]')).toHaveCount(0);
  await expect(page.getByLabel(/password/i)).toHaveCount(0);
});

test('F-24 dismissing the permission dialog springs back with no error text', async ({ page }) => {
  await openScenario(page, 'cancel-permission');
  await keyInstall(page);
  await expect(page.getByText(tx('perm.waiting'))).toBeVisible();
  // The shade goes away and the icon is back and usable.
  await expect(page.getByText(tx('perm.waiting'))).toBeHidden({ timeout: 8_000 });
  await expect(installTarget(page)).toBeVisible();
  await expect(installTarget(page)).toBeEnabled();
  // Nothing that looks like an error.
  await expect(page.getByRole('alert')).toHaveCount(0);
  await expect(page.getByText(/went wrong|error|failed|denied/i)).toHaveCount(0);
  await expect(page.getByRole('button', { name: tx('action.tryAgain') })).toHaveCount(0);
});

test('F-25 progress uses plain phrases and never names dependency packages', async ({ page }) => {
  await openScenario(page, 'rpm');
  await keyInstall(page);
  await expect(page.getByText(tx('phase.downloading'), { exact: false })).toBeVisible({
    timeout: 10_000
  });
  const body = (await page.locator('body').innerText()).toLowerCase();
  expect(body).not.toMatch(/\blib[a-z0-9_+-]+|\.rpm\b|packagekit|dnf/);
});

test('F-25 progress bar is indeterminate when the percentage is unknown', async ({ page }) => {
  test.fixme(true, 'waiting mock: no scripted step with percent=null after the shade');
  await openScenario(page, 'rpm');
  await keyInstall(page);
  await expect(page.getByRole('progressbar')).not.toHaveAttribute('aria-valuenow', /.+/);
});

test('F-27 more than 50 MB of extra downloads says so in the progress view', async ({ page }) => {
  await openScenario(page, 'rpm'); // mock plan: 2 extra packages, 62 MB
  await keyInstall(page);
  await expect(page.getByText(/Getting a few things it needs \(\d+(\.\d+)? MB\)/)).toBeVisible({
    timeout: 10_000
  });
  // No confirmation step.
  await expect(page.getByRole('button', { name: /confirm|continue/i })).toHaveCount(0);
});

test('F-28 Cancel shows only while the backend allows it', async ({ page }) => {
  test.fixme(true, 'waiting mock: InstallEvent.progress.allowCancel (contracts v2) not scripted');
  await openScenario(page, 'rpm');
  await keyInstall(page);
  const cancel = page.getByRole('button', { name: tx('action.cancel'), exact: true });
  await expect(page.getByText(tx('phase.downloading'), { exact: false })).toBeVisible({
    timeout: 10_000
  });
  await expect(cancel).toBeVisible(); // allowCancel true while downloading
  await expect(page.getByText(tx('phase.installing', { app: APP }))).toBeVisible({
    timeout: 10_000
  });
  await expect(cancel).toHaveCount(0); // allowCancel false while installing
});

test('F-28 Cancel is hidden once installing (allowCancel false)', async ({ page }) => {
  await openScenario(page, 'rpm');
  await keyInstall(page);
  await expect(page.getByText(tx('phase.installing', { app: APP }))).toBeVisible({
    timeout: 15_000
  });
  await expect(page.getByRole('button', { name: tx('action.cancel'), exact: true })).toHaveCount(0);
});

test('F-26 failure shows a sentence with Try Again and Close', async ({ page }) => {
  await openScenario(page, 'fail-offline');
  await keyInstall(page);
  await expect(page.getByText(tx('error.offline', { app: APP }))).toBeVisible({ timeout: 15_000 });
  await expect(page.getByRole('button', { name: tx('action.tryAgain') })).toBeVisible();
  await expect(page.getByRole('button', { name: tx('action.close'), exact: true })).toBeVisible();
  // Technical text is not shown by default.
  await expect(page.getByText('mock: could not reach the network')).toBeHidden();
});

test('F-26 Details is a disclosure that reveals the technical text', async ({ page }) => {
  test.fixme(true, 'waiting S6: ErrorView with Disclosure not wired into the route yet');
  await openScenario(page, 'fail-offline');
  await keyInstall(page);
  const details = page.getByRole('button', { name: tx('details.label') });
  await expect(details).toBeVisible({ timeout: 15_000 });
  await expect(details).toHaveAttribute('aria-expanded', 'false');
  await expect(page.getByText('mock: could not reach the network')).toBeHidden();
  await details.click();
  await expect(details).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByText('mock: could not reach the network')).toBeVisible();
});

test('F-26 Try Again returns to the drag screen', async ({ page }) => {
  await openScenario(page, 'fail-offline');
  await keyInstall(page);
  await page.getByRole('button', { name: tx('action.tryAgain') }).click({ timeout: 15_000 });
  await expect(installTarget(page)).toBeVisible();
  await expect(page.getByText(tx('error.offline', { app: APP }))).toBeHidden();
});
