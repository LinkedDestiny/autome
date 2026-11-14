import { test as base, expect, type Page } from '@playwright/test';
import { state } from './fixture.ts';
export const test = base.extend<{ consoleErrors: string[] }>({
  consoleErrors: [async ({ page }, use) => {
    const errors: string[] = [];
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    page.on('pageerror', error => errors.push(error.message));
    await use(errors);
    expect(errors, 'Console/hydration errors').toEqual([]);
  }, { auto: true }],
});
export { expect };
export async function visit(page: Page, path: string) {
  await page.goto(state().url + path);
  await expect(page.getByTestId('shell-main')).toBeVisible();
  await expect(page.getByTestId('live-banner')).toHaveCount(0);
  await page.evaluate(() => document.fonts.ready);
}
export async function oneScreen(page: Page) {
  const overflows = await page.evaluate(() => {
    const main = document.querySelector('[data-testid=shell-main]');
    if (!main) throw Error('Missing shell-main');
    return [document.documentElement, main, ...main.children].filter(element => !element.closest('[data-scroll-region]'))
      .filter(element => element.scrollHeight > element.clientHeight)
      .map(element => ({ tag: element.tagName, className: element.className, height: element.clientHeight, scroll: element.scrollHeight }));
  });
  expect(overflows).toEqual([]);
}
export async function expandLog(page: Page) {
  const log = page.getByTestId('session-log').first();
  if (await log.getAttribute('open') === null) await log.locator('summary').click();
  await expect(log.getByTestId('session-log-body')).toBeVisible();
}
