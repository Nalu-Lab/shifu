import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createRequire } from 'node:module';
import { createInterface } from 'node:readline';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

// Reuse the Chromium dependency already used by Shifu's web tests.
const { chromium } = createRequire(new URL('../../web/package.json', import.meta.url))('playwright-core');

test(
  'desktop signup, invite retry, and returning users in the local pilot',
  { timeout: 45_000 },
  async (t) => {
    const preview = spawn(
      process.execPath,
      [fileURLToPath(new URL('../preview/server.mjs', import.meta.url))],
      {
        stdio: ['ignore', 'pipe', 'inherit']
      }
    );
    const exited = once(preview, 'exit');
    t.after(async () => {
      preview.kill('SIGTERM');
      await exited;
    });
    let url;
    let invite;
    const output = createInterface({ input: preview.stdout });
    for await (const line of output) {
      if (line.startsWith('Local Shifu pilot:')) url = line.split(' ').at(-1);
      if (line.startsWith('Single-use sample invite for alex:')) {
        invite = line.split(' ').at(-1);
        break;
      }
    }
    assert.ok(url && invite);
    const browser = await chromium.launch({ executablePath: process.env.SHIFU_TEST_CHROME || undefined });
    t.after(() => browser.close());
    const page = await browser.newPage({ locale: 'en-US' });
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.goto(url);
    const panel = page.getByRole('dialog', { name: 'My Macs' });
    const open = () => page.getByRole('button', { name: 'Open My Macs' }).click();
    const submit = async (code) => {
      await page.getByLabel('Invite code', { exact: true }).fill(code);
      await page.getByRole('button', { name: 'Redeem invite' }).click();
    };
    const signIn = async (name) => {
      await page.getByLabel('Name for this Mac').fill(name);
      await page.getByRole('button', { name: 'Sign in with Google' }).click();
    };
    await open();
    await signIn('Studio');
    await submit('WRONG');
    await page.getByRole('alert').filter({ hasText: 'Check the invite code' }).waitFor();
    await page.keyboard.press('Escape');
    await open();
    await submit(invite);
    await panel.getByText('Studio (this Mac)', { exact: true }).waitFor();

    await page.keyboard.press('Escape');
    await page.getByLabel('Simulated Mac').selectOption('b');
    await open();
    await signIn('Laptop');
    await panel.getByText('Laptop (this Mac)', { exact: true }).waitFor();
    await panel.getByText('Studio', { exact: true }).waitFor();
    await page.getByRole('button', { name: 'Remove Studio' }).click();
    await page.getByRole('button', { name: 'Remove', exact: true }).click();
    await panel.getByText('Studio', { exact: true }).waitFor({ state: 'detached' });

    await page.keyboard.press('Escape');
    await page.getByLabel('Simulated Mac').selectOption('a');
    await page.getByLabel('Sample identity').selectOption('sam');
    await open();
    await signIn('Studio');
    await submit(invite);
    await page.getByRole('alert').filter({ hasText: 'Check the invite code' }).waitFor();
    assert.equal(await panel.getByText('Laptop', { exact: true }).count(), 0);
    assert.deepEqual(errors, []);
  }
);
