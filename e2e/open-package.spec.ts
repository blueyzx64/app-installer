// Lane Q2 (S9b). Open a Package…: F-07 button and chooser, F-06 drop. Mock: ?pick=cancel dismisses the chooser.
import { expect, test, type Page } from '@playwright/test';
import en from '../src/lib/i18n/en.json' with { type: 'json' };

function tx(key: keyof typeof en, vars: Record<string, string | number> = {}): string {
  return String(en[key]).replace(/\{(\w+)\}/g, (_, k: string) => String(vars[k] ?? `{${k}}`));
}
const APP = 'Lumen Notes';
test.describe.configure({ timeout: 30_000 });

function openButton(page: Page) {
  // The header button and the empty-state button share a name: either one is fine.
  return page.getByRole('button', { name: tx('library.open') }).first();
}

test('F-07 empty library shows the Open a Package… button', async ({ page }) => {
  await page.goto('/library?scenario=empty-library');
  await expect(page.getByText(tx('library.empty'))).toBeVisible();
  await expect(openButton(page)).toBeVisible();
  await expect(openButton(page)).toBeEnabled();
});

test('F-07 the button is also there when the library has apps', async ({ page }) => {
  await page.goto('/library?scenario=rpm&launch=none');
  await expect(openButton(page)).toBeVisible();
});

test('F-07 choosing a package opens the install screen', async ({ page }) => {
  await page.goto('/library?scenario=rpm&launch=none');
  await openButton(page).click();
  await expect(page).toHaveURL(/\/install/);
  await expect(page.getByText(tx('dmg.hint', { app: APP }))).toBeVisible();
});

test('F-07 dismissing the chooser changes nothing and shows no error', async ({ page }) => {
  await page.goto('/library?scenario=rpm&launch=none&pick=cancel');
  await openButton(page).click();
  await expect(page).toHaveURL(/\/library/);
  await expect(page.getByText(/went wrong|error|failed/i)).toHaveCount(0);
});

test('F-07 no text field is used to type a path', async ({ page }) => {
  await page.goto('/library?scenario=empty-library');
  await expect(page.getByRole('heading', { name: tx('library.title') })).toBeVisible();
  await expect(page.getByRole('textbox')).toHaveCount(0);
});

test('F-06 a launch file opens straight into the install screen', async ({ page }) => {
  await page.goto('/?scenario=rpm');
  await expect(page).toHaveURL(/\/install/);
  await expect(page.getByText(tx('dmg.hint', { app: APP }))).toBeVisible();
});

test('F-06 without a launch file the library opens', async ({ page }) => {
  await page.goto('/?scenario=rpm&launch=none');
  await expect(page).toHaveURL(/\/library/);
});

test('F-06 dropping a file on the library opens the install screen', async ({ page }) => {
  await page.goto('/library?scenario=rpm&launch=none');
  await expect(page.getByRole('heading', { name: tx('library.title') })).toBeVisible();
  await page.evaluate(() =>
    (globalThis as unknown as { __mock: { emitOpenFile(p: string): void } }).__mock.emitOpenFile(
      '/x.rpm'
    )
  );
  await expect(page).toHaveURL(/\/install/);
});
