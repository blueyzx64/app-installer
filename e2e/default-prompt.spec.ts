// Lane Q2 (S9b). First-launch default-app prompt: F-50. Mock status starts as not default, not asked.
import { expect, test, type Page } from '@playwright/test';
import en from '../src/lib/i18n/en.json' with { type: 'json' };

function tx(key: keyof typeof en, vars: Record<string, string | number> = {}): string {
  return String(en[key]).replace(/\{(\w+)\}/g, (_, k: string) => String(vars[k] ?? `{${k}}`));
}
test.describe.configure({ timeout: 30_000 });

const WAITING = 'waiting S5/S6: DefaultHandlerController + Sheet not wired into the install route';

function sheet(page: Page) {
  return page.getByRole('dialog').filter({ hasText: tx('default.title') });
}

test('F-50 sheet appears after the package is inspected', async ({ page }) => {
  test.fixme(true, WAITING);
  await page.goto('/?scenario=rpm');
  await expect(sheet(page)).toBeVisible();
  await expect(sheet(page).getByRole('button', { name: tx('default.make') })).toBeVisible();
  await expect(sheet(page).getByRole('button', { name: tx('default.notNow') })).toBeVisible();
});

test('F-50 Make Default closes the sheet and shows the confirming toast', async ({ page }) => {
  test.fixme(true, WAITING);
  await page.goto('/?scenario=rpm');
  await sheet(page)
    .getByRole('button', { name: tx('default.make') })
    .click();
  await expect(sheet(page)).toBeHidden();
  await expect(page.getByRole('status').filter({ hasText: tx('default.toast') })).toBeVisible();
  // The toast goes away by itself.
  await expect(page.getByText(tx('default.toast'))).toBeHidden({ timeout: 8_000 });
});

test('F-50 Not Now closes the sheet and shows no toast', async ({ page }) => {
  test.fixme(true, WAITING);
  await page.goto('/?scenario=rpm');
  await sheet(page)
    .getByRole('button', { name: tx('default.notNow') })
    .click();
  await expect(sheet(page)).toBeHidden();
  await expect(page.getByText(tx('default.toast'))).toHaveCount(0);
});

test('F-50 the sheet is never shown on top of the permission shade', async ({ page }) => {
  test.fixme(true, WAITING);
  await page.goto('/?scenario=rpm');
  await sheet(page)
    .getByRole('button', { name: tx('default.notNow') })
    .click();
  await page.getByRole('button', { name: /Drag into Applications to install/ }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText(tx('perm.waiting'))).toBeVisible();
  await expect(sheet(page)).toHaveCount(0);
});

test('F-50 the question is asked once', async ({ page }) => {
  test.fixme(
    true,
    'waiting S5: needs a persisted askedBefore; mock keeps it in memory per page load'
  );
  await page.goto('/?scenario=rpm');
  await sheet(page)
    .getByRole('button', { name: tx('default.notNow') })
    .click();
  await page.reload();
  await expect(sheet(page)).toHaveCount(0);
});

test('F-50 sheet uses plain words only', async ({ page }) => {
  test.fixme(true, WAITING);
  await page.goto('/?scenario=rpm');
  await expect(sheet(page)).toBeVisible();
  const text = (await sheet(page).innerText()).toLowerCase();
  expect(text).not.toMatch(/mime|handler|xdg|desktop file|association/);
});
