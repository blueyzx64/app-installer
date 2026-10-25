import { expect, type Locator, type Page } from '@playwright/test';

export type Scenario =
  | 'rpm'
  | 'deb'
  | 'appimage'
  | 'incompatible-arch'
  | 'already-installed'
  | 'update'
  | 'fail-offline'
  | 'cancel-permission'
  | 'missing-fuse2'
  | 'empty-library';

export const APP = 'Lumen Notes';

/** Open the installer window on a mock scenario. Mock launch file makes `/` route to /install. */
export async function openScenario(page: Page, scenario: Scenario, extra = ''): Promise<void> {
  await page.goto(`/?scenario=${scenario}${extra}`);
}

export const icon = (page: Page): Locator =>
  page.getByRole('button', { name: new RegExp(`${APP}\\. Drag into Applications`) });
export const folder = (page: Page): Locator => page.getByTestId('folder-tile');

export async function waitReady(page: Page): Promise<void> {
  await dismissDefaultPrompt(page);
  await expect(icon(page)).toBeVisible();
}

/** The first-launch "make default" sheet may cover the window. Dismiss it if it shows. */
export async function dismissDefaultPrompt(page: Page): Promise<void> {
  const notNow = page.getByRole('button', { name: 'Not Now' });
  try {
    await notNow.waitFor({ state: 'visible', timeout: 1_500 });
    await notNow.click();
    await expect(notNow).toBeHidden();
  } catch {
    /* no prompt */
  }
}

async function center(l: Locator): Promise<{ x: number; y: number }> {
  const b = await l.boundingBox();
  if (!b) throw new Error('element has no box');
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
}

/** Real pointer drag with the mouse (no HTML5 DnD). */
export async function dragIconTo(page: Page, to: { x: number; y: number }, drop = true) {
  const from = await center(icon(page));
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 12, from.y + 4, { steps: 3 });
  await page.mouse.move(to.x, to.y, { steps: 12 });
  if (drop) await page.mouse.up();
}

export async function dragIconToFolder(page: Page, drop = true) {
  await dragIconTo(page, await center(folder(page)), drop);
}

export const progressbar = (page: Page) => page.getByRole('progressbar');
export const alertDialog = (page: Page) => page.getByRole('alertdialog');

/** Flip to true with E2E_UI_WIRED=1 once lanes E/F/G are merged. Then delete the gate. */
export const uiWired = process.env.E2E_UI_WIRED === '1';
export const UI_PENDING =
  'Waits for lanes E/F/G: /install is still a placeholder (no DmgView, alerts or window controls yet)';
