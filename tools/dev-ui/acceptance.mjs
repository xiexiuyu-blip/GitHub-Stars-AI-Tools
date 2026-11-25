/** 可重复的浏览器旅程验收；测试已构建页面，所有数据和调用由隔离夹具提供。 */
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { installFixture } from './acceptance-fixture.js';

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.FSL_PLAYWRIGHT_PATH || 'playwright');
const root = path.resolve(process.env.FSL_BUILD_DIR || 'apps/desktop/dist');
const output = path.resolve(process.env.FSL_ACCEPTANCE_OUT || '/tmp/fsl-acceptance');
await mkdir(output, { recursive: true });
const server = createServer(async (request, response) => {
  try {
    const requested = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    const file = path.resolve(root, '.' + (requested === '/' ? '/index.html' : requested));
    if (!file.startsWith(root + path.sep)) { response.writeHead(403).end(); return; }
    const data = await readFile(file);
    const contentType = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.woff2': 'font/woff2', '.svg': 'image/svg+xml' }[path.extname(file)] || 'application/octet-stream';
    response.writeHead(200, { 'Content-Type': contentType }); response.end(data);
  } catch { response.writeHead(404).end(); }
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const url = `http://127.0.0.1:${server.address().port}`;
let browser;
const results = [];
try {
  browser = await chromium.launch({ headless: true, ...(process.env.FSL_BROWSER_CHANNEL ? { channel: process.env.FSL_BROWSER_CHANNEL } : {}) });
  const run = async (name, test, options = {}) => {
    const context = await browser.newContext({ viewport: { width: 1400, height: 900 }, ...options });
    await context.addInitScript(installFixture);
    await context.route('**/*', (route) => route.request().url().startsWith(url + '/') ? route.continue() : route.abort());
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.setDefaultTimeout(5000);
    try {
      await page.goto(url);
      await page.getByRole('heading', { name: /验收数据/ }).waitFor();
      await test(page);
      assert.deepEqual(errors, [], '未处理的页面异常');
      results.push({ name, passed: true });
    } catch (error) {
      results.push({ name, passed: false, error: error.message, pageErrors: errors });
      await page.screenshot({ path: path.join(output, `${name}-failed.png`), fullPage: true }).catch(() => {});
    } finally { await context.close(); }
    console.log(`${results.at(-1).passed ? 'PASS' : 'FAIL'} ${name}`);
  };
  const nav = (page, label) => page.getByRole('button', { name: label, exact: true }).first().click();
  const find = async (page, query = '制作封面') => {
    await nav(page, '找方案');
    await page.getByRole('textbox', { name: '你想解决什么' }).fill(query);
    await page.getByRole('button', { name: '找', exact: true }).click();
    await page.getByRole('button', { name: '复制交接单给 AI 助手', exact: true }).waitFor();
    await page.waitForFunction(() => window.__FSL_ACCEPTANCE__.calls.some((call) => call.command === 'build_reference_pack_v2'));
  };
  const copyButton = (page) => page.getByRole('button', { name: /^(复制交接单给 AI 助手|已复制)$/ });
  await run('01-today-and-library', async (page) => {
    await nav(page, '资料库');
    await page.getByText('cover', { exact: true }).first().waitFor();
    await page.screenshot({ path: path.join(output, 'library.png'), fullPage: true });
    assert(await page.getByText('已归档', { exact: true }).count() > 0);
  });
  await run('02-redo-confirm-cancel', async (page) => {
    await nav(page, '资料库');
    await page.getByRole('button', { name: '不太准，让 AI 重写（会花一点钱）', exact: true }).click();
    await page.getByRole('button', { name: '确认，重新整理', exact: true }).waitFor();
    await page.getByRole('button', { name: '先不了', exact: true }).click();
    assert.equal(await page.evaluate(() => window.__FSL_ACCEPTANCE__.calls.filter((call) => call.command === 'regenerate_library_card').length), 0);
  });
  await run('03-plain-search-enter', async (page) => {
    await page.keyboard.press('Meta+k');
    await page.getByPlaceholder(/直接搜工具|搜工具；/).fill('封面工具');
    await page.keyboard.press('Enter');
    await page.getByRole('heading', { name: '资料库', exact: true }).waitFor();
    await page.waitForFunction(() => document.querySelector('input[aria-label="搜索资料库"]')?.value === '封面工具');
  });
  await run('04-palette-escape', async (page) => {
    await page.keyboard.press('Meta+k');
    const input = page.getByPlaceholder(/直接搜工具|搜工具；/);
    await input.waitFor(); await page.keyboard.press('Escape'); await input.waitFor({ state: 'hidden' });
  });
  await run('05-ai-settings-destination', async (page) => {
    await page.getByRole('button', { name: '去设置 AI', exact: true }).first().click();
    await page.getByText('AI 设置', { exact: true }).first().waitFor();
    await page.getByRole('heading', { name: 'AI 设置', exact: true }).waitFor();
  });
  await run('06-pack-updating-blocks-copy', async (page) => {
    await find(page);
    await copyButton(page).click();
    await page.evaluate(() => { window.__FSL_ACCEPTANCE__.delayPacks = true; });
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /程序用/ }) }).click();
    await page.waitForFunction(() => window.__FSL_ACCEPTANCE__.pendingPacks.length > 0);
    assert.equal(await copyButton(page).isEnabled(), false, '更新时禁止复制旧稿');
    await page.evaluate(() => window.__FSL_ACCEPTANCE__.resolvePacks());
    await copyButton(page).click();
    const copied = await page.evaluate(() => window.__FSL_ACCEPTANCE__.clipboard);
    assert.equal(JSON.parse(copied).task, '制作封面');
  });
  await run('07-pack-failure-blocks-copy', async (page) => {
    await find(page); await copyButton(page).click();
    await page.evaluate(() => { window.__FSL_ACCEPTANCE__.failPacks = true; });
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /程序用/ }) }).click();
    await page.getByText('验收：交接单生成失败', { exact: true }).waitFor();
    assert.equal(await copyButton(page).isEnabled(), false);
  });
  await run('08-return-keeps-selection', async (page) => {
    await page.keyboard.press('Meta+k');
    await page.getByPlaceholder(/直接搜工具|搜工具；/).fill('?制作封面');
    await page.keyboard.press('Enter');
    await page.getByRole('button', { name: '加入交接单 fixture/archived', exact: true }).click();
    await nav(page, '今日'); await nav(page, '找方案');
    await page.getByRole('button', { name: '从交接单移除 fixture/archived', exact: true }).waitFor();
    assert.equal(await page.evaluate(() => window.__FSL_ACCEPTANCE__.calls.filter((call) => call.command === 'solve_v2').length), 1);
  });
  await run('09-maintenance-in-solve', async (page) => {
    await find(page);
    assert(await page.getByText('已归档', { exact: true }).count() > 0);
    assert(await page.getByText('疑似停更', { exact: true }).count() > 0);
    await page.getByRole('heading', { name: '当前交接单', exact: true }).waitFor();
    await page.screenshot({ path: path.join(output, 'solve-maintenance.png'), fullPage: true });
  });
  await run('10-readme-refresh-open-detail', async (page) => {
    await nav(page, '资料库');
    await page.getByRole('tab', { name: /项目说明/ }).click();
    await page.getByText('README验收版本1', { exact: true }).waitFor();
    await page.evaluate(() => {
      const state = window.__FSL_ACCEPTANCE__; state.readmeVersion = 2;
      state.emit('task-progress', { taskId: 'fetch-readmes', taskType: 'readme', status: 'succeeded', stage: 'readme', current: 4, total: 4, message: '完成', createdAt: new Date().toISOString() });
    });
    await page.getByText('README验收版本2', { exact: true }).waitFor();
  });
  await run('11-narrow-solve', async (page) => {
    await find(page); await copyButton(page).click();
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1), false, '页面不横向溢出');
    await page.getByText(/^已复制。/).waitFor();
    // 窄屏提示条会覆盖刚点过的复制位置；移开鼠标才恢复其正常消失计时。
    await page.mouse.move(10, 10);
    await page.getByText(/^已复制。/).waitFor({ state: 'hidden', timeout: 10000 });
    await page.screenshot({ path: path.join(output, 'solve-narrow.png'), fullPage: true });
  }, { viewport: { width: 720, height: 600 } });
  await run('12-dark-today', async (page) => {
    assert.equal(await page.evaluate(() => document.documentElement.classList.contains('dark')), true);
    await page.screenshot({ path: path.join(output, 'today-dark.png'), fullPage: true });
  }, { colorScheme: 'dark' });
  await run('13-palette-keyboard-and-focus', async (page) => {
    const opener = page.getByRole('button', { name: '打开命令面板', exact: true });
    await opener.focus(); await page.keyboard.press('Meta+k');
    const input = page.getByRole('combobox');
    await input.fill('fixture/');
    await page.getByRole('option').first().waitFor();
    await page.keyboard.press('ArrowDown'); await page.keyboard.press('ArrowDown');
    await page.keyboard.press('ArrowUp');
    assert.match(await page.getByRole('option', { selected: true }).innerText(), /fixture\/cover/);
    await page.keyboard.press('Tab');
    assert.equal(await page.getByRole('button', { name: '关闭', exact: true }).evaluate((element) => element === document.activeElement), true);
    await page.keyboard.press('Tab');
    assert.equal(await input.evaluate((element) => element === document.activeElement), true);
    await page.keyboard.press('Escape');
    assert.equal(await opener.evaluate((element) => element === document.activeElement), true);
    await page.keyboard.press('Meta+k'); await input.fill('fixture/');
    await page.getByRole('option').first().waitFor();
    await page.keyboard.press('ArrowDown'); await page.keyboard.press('ArrowDown'); await page.keyboard.press('Enter');
    await page.waitForFunction(() => document.querySelector('input[aria-label="搜索资料库"]')?.value === 'fixture/legacy');
  });
  await run('14-pack-race-uses-latest-selection', async (page) => {
    await find(page); await copyButton(page).click();
    await page.evaluate(() => { window.__FSL_ACCEPTANCE__.delayPacks = true; });
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /程序用/ }) }).click();
    await page.waitForFunction(() => window.__FSL_ACCEPTANCE__.pendingPacks.length === 1);
    await page.getByRole('button', { name: '加入交接单 fixture/archived', exact: true }).click();
    await page.waitForFunction(() => window.__FSL_ACCEPTANCE__.pendingPacks.length === 2);
    await page.evaluate(() => { const pending = window.__FSL_ACCEPTANCE__.pendingPacks.pop(); pending.resolve(pending.output); });
    await copyButton(page).click();
    const copied = await page.evaluate(() => window.__FSL_ACCEPTANCE__.clipboard);
    assert.equal(JSON.parse(copied).items.length, 3);
    await page.evaluate(() => window.__FSL_ACCEPTANCE__.resolvePacks());
    await copyButton(page).click();
    assert.equal(await page.evaluate(() => window.__FSL_ACCEPTANCE__.clipboard), copied, '旧请求不能盖掉当前勾选');
  });
  await run('15-new-question-with-same-tools', async (page) => {
    await find(page); await copyButton(page).click();
    await find(page, '制作第二张海报'); await copyButton(page).click();
    const copied = await page.evaluate(() => window.__FSL_ACCEPTANCE__.clipboard);
    assert(copied.includes('制作第二张海报')); assert(!copied.includes('制作封面'));
  });
  await run('16-palette-stale-response', async (page) => {
    await page.evaluate(() => { window.__FSL_ACCEPTANCE__.delaySearches = true; });
    await page.keyboard.press('Meta+k');
    const input = page.getByRole('combobox');
    await input.fill('cover');
    await page.waitForFunction(() => window.__FSL_ACCEPTANCE__.pendingSearches.length === 1);
    await input.fill('legacy');
    await page.waitForFunction(() => window.__FSL_ACCEPTANCE__.pendingSearches.length === 2);
    await page.evaluate(() => { const pending = window.__FSL_ACCEPTANCE__.pendingSearches.pop(); pending.resolve(pending.output); });
    await page.getByRole('option', { name: /fixture\/legacy/ }).waitFor();
    await page.evaluate(() => { const pending = window.__FSL_ACCEPTANCE__.pendingSearches.pop(); pending.resolve(pending.output); });
    assert.equal(await page.getByRole('option', { name: /fixture\/cover/ }).count(), 0);
    await page.keyboard.press('ArrowDown'); await page.keyboard.press('Enter');
    await page.waitForFunction(() => document.querySelector('input[aria-label="搜索资料库"]')?.value === 'fixture/legacy');
  });
  await run('17-explicit-same-question-is-new-task', async (page) => {
    await page.keyboard.press('Meta+k');
    await page.getByRole('combobox').fill('?制作封面'); await page.keyboard.press('Enter');
    await page.getByRole('button', { name: '加入交接单 fixture/archived', exact: true }).click();
    await nav(page, '今日'); await page.keyboard.press('Meta+k');
    await page.getByRole('combobox').fill('?制作封面'); await page.keyboard.press('Enter');
    await page.getByRole('button', { name: '加入交接单 fixture/archived', exact: true }).waitFor();
    assert.equal(await page.evaluate(() => window.__FSL_ACCEPTANCE__.calls.filter((call) => call.command === 'solve_v2').length), 2);
  });
  await run('18-pack-retry', async (page) => {
    await page.evaluate(() => { window.__FSL_ACCEPTANCE__.failPacks = true; });
    await find(page);
    await page.getByText('验收：交接单生成失败', { exact: true }).waitFor();
    assert.equal(await copyButton(page).isEnabled(), false);
    await page.evaluate(() => { window.__FSL_ACCEPTANCE__.failPacks = false; });
    await page.getByRole('button', { name: /重试/ }).click();
    await copyButton(page).click();
    assert((await page.evaluate(() => window.__FSL_ACCEPTANCE__.clipboard)).includes('制作封面'));
  });
  await run('19-solve-pending-ai-settings', async (page) => {
    await find(page);
    await page.getByText('fixture/pending', { exact: true }).click();
    await page.getByRole('dialog').getByRole('button', { name: '去设置 AI', exact: true }).click();
    await page.getByRole('heading', { name: 'AI 设置', exact: true }).waitFor();
  });
  await run('20-redo-confirmation-request', async (page) => {
    await nav(page, '资料库');
    await page.getByRole('button', { name: '不太准，让 AI 重写（会花一点钱）', exact: true }).click();
    await page.getByRole('button', { name: '确认，重新整理', exact: true }).click();
    await page.getByText('验收：还没配置 AI 服务', { exact: true }).waitFor();
    const calls = await page.evaluate(() => window.__FSL_ACCEPTANCE__.calls.filter((call) => call.command === 'regenerate_library_card'));
    assert.equal(calls.length, 1); assert.equal(calls[0].args.confirmed, true);
  });
} finally {
  if (browser) await browser.close();
  await new Promise((resolve) => server.close(resolve));
  await writeFile(path.join(output, 'results.json'), JSON.stringify({ build: root, fixtureOnly: true, results }, null, 2));
}
console.log(JSON.stringify(results.map(({ name, passed, error }) => ({ name, passed, error })), null, 2));
process.exitCode = results.every((result) => result.passed) ? 0 : 1;
