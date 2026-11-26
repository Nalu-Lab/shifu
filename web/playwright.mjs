import { randomUUID } from 'node:crypto';
import { findText } from './find.mjs';

export class BrowserError extends Error {
  constructor(code, message, phase = 'not_dispatched') {
    super(message);
    this.code = code;
    this.phase = phase;
  }
}

export function targetKey({ role, name }) { return JSON.stringify([role, name]); }

export class PlaywrightProvider {
  constructor(connect, { actionTimeout = 5000, navigationTimeout = 15000, settleTimeout = 5000 } = {}) {
    this.connect = connect;
    this.actionTimeout = actionTimeout;
    this.navigationTimeout = navigationTimeout;
    this.settleTimeout = settleTimeout;
    this.owned = new Set();
  }

  async browser() {
    if (!this.connection) {
      this.connection = this.connect().then(async browser => {
        try {
          const session = await browser.newBrowserCDPSession();
          try {
            const version = await session.send('Browser.getVersion');
            if (!version.userAgent.includes('HeadlessChrome')) {
              throw new BrowserError('HEADLESS_REQUIRED', '最小版只连接独立无界面 Chromium，不接管本人浏览器。');
            }
          } finally { await session.detach(); }
          return browser;
        } catch (error) {
          // This Browser was connected, never launched: close only our connection.
          await browser.close();
          throw error;
        }
      });
    }
    return this.connection;
  }

  async open(url) {
    const browser = await this.browser();
    const context = await browser.newContext({ acceptDownloads: false });
    this.owned.add(context);
    context.setDefaultTimeout(this.actionTimeout);
    context.setDefaultNavigationTimeout(this.navigationTimeout);
    try {
      const page = await context.newPage();
      const cdp = await context.newCDPSession(page);
      const resource = { context, page, cdp, pageId: randomUUID(), observation: null };
      // Popups stay in this owned context and are included in its eventual disposal.
      // A popup is never silently selected as the task's page.
      await this.goto(resource, url);
      return resource;
    } catch (error) {
      await context.close();
      this.owned.delete(context);
      throw error;
    }
  }

  async documentId(resource) {
    const { frameTree } = await resource.cdp.send('Page.getFrameTree');
    return frameTree.frame.loaderId;
  }

  async requireDocument(resource, documentId) {
    if (await this.documentId(resource) !== documentId) {
      throw new BrowserError('PAGE_CHANGED', '页面已跳转或重载，请重新观察后再操作。');
    }
  }

  async observe(resource, scope) {
    resource.observation = null;
    const documentId = await this.documentId(resource);
    if (await resource.page.evaluate(() => document.body === null)) {
      throw new BrowserError('PAGE_NOT_READY', '页面主体尚未建成，暂不能观察控件；可用find读取已到正文，或稍后再观察。', 'not_dispatched');
    }
    const locator = scope ? resource.page.getByRole(scope.role, { name: scope.name, exact: true }) : resource.page.locator('body');
    const tree = await locator.ariaSnapshotJSON({ mode: 'ai', timeout: this.actionTimeout });
    await this.requireDocument(resource, documentId);
    const targets = new Map();
    const refs = new Map();
    const visit = node => {
      if (!node || typeof node !== 'object') return;
      if (Array.isArray(node)) { node.forEach(visit); return; }
      // AI snapshots can expand frames. The task pool observes and acts in the main document.
      if (node.role === 'iframe') { delete node.children; return; }
      if (node.role && typeof node.name === 'string') {
        const key = targetKey(node);
        targets.set(key, (targets.get(key) || 0) + 1);
        if (node.ref) refs.set(key, node.ref);
      }
      if (node.children) visit(node.children);
    };
    visit(tree);
    const text = JSON.stringify(tree, null, 2);
    if (Buffer.byteLength(text) > 256 * 1024) {
      throw new BrowserError('OBSERVATION_TOO_LARGE', '观察内容超过256 KiB，请指定正文等区域，例如scope:{role:"main",name:""}。');
    }
    resource.observation = { id: randomUUID(), pageId: resource.pageId, documentId, text, targets, refs, locator, scope };
    return resource.observation;
  }

  async find(resource, queries, scope) {
    const url = resource.page.url();
    const fallback = await this.htmlText(resource, scope);
    if (fallback !== null) {
      if (Buffer.byteLength(fallback) > 256 * 1024) throw new BrowserError('READ_TOO_LARGE', '正文超过256 KiB，请缩小scope后查找。');
      return { url, source: 'html', results: findText(fallback, queries) };
    }
    const locator = scope ? resource.page.getByRole(scope.role, { name: scope.name, exact: true }) : resource.page.locator('body');
    const text = await locator.innerText({ timeout: this.actionTimeout });
    if (resource.page.url() !== url) {
      throw new BrowserError('PAGE_CHANGED', '读取期间页面已跳转，请在当前页重新查找。');
    }
    if (Buffer.byteLength(text) > 256 * 1024) {
      throw new BrowserError('READ_TOO_LARGE', '正文超过256 KiB，请缩小scope后查找。');
    }
    return { url, results: findText(text, queries) };
  }

  async act(resource, operation) {
    const observation = resource.observation;
    const handles = [];
    try {
      if (operation.type === 'navigate') {
        await this.goto(resource, operation.url);
        return;
      }
      await this.requireDocument(resource, observation.documentId);
      const targets = operation.type === 'fill_form' ? operation.fields : [operation.target];
      for (const target of targets) {
        const key = targetKey(target);
        const ref = observation.refs.get(key);
        if (!ref) throw new BrowserError('TARGET_CHANGED', '原控件已无法定位，请重新观察。');
        const named = observation.scope && targetKey(observation.scope) === key
          ? observation.locator : observation.locator.getByRole(target.role, { name: target.name, exact: true });
        const matches = await named.and(resource.page.locator(`aria-ref=${ref}`)).elementHandles();
        handles.push(...matches);
        if (matches.length !== 1) {
          throw new BrowserError('TARGET_CHANGED', '原控件已更换，或已离开观察范围，请重新观察。');
        }
      }
      await this.requireDocument(resource, observation.documentId);
      // Pin every element before starting writes. A detached handle cannot resolve to a replacement.
      if (operation.type === 'fill_form') {
        for (let i = 0; i < handles.length; i++) await handles[i].fill(operation.fields[i].value);
      } else {
        await handles[0].click();
      }
    } finally {
      resource.observation = null;
      await Promise.allSettled(handles.map(handle => handle.dispose()));
    }
  }

  // Some real pages (e.g. MDN behind slow third-party scripts) take 15s+ to reach DOMContentLoaded
  // although their text has arrived. Navigation succeeds once the response commits; a short settle
  // wait then reports whether loading finished, instead of turning the task into an unknown result.
  async goto(resource, url) {
    resource.response = null;
    resource.response = await resource.page.goto(url, { waitUntil: 'commit' });
    resource.loaded = await resource.page.waitForLoadState('domcontentloaded', { timeout: this.settleTimeout }).then(() => true, () => false);
  }

  // On some networks a stylesheet or script in <head> stalls for 15s+ inside the browser, so <body> is
  // never built although the HTML has fully arrived. Reading then parses that HTML without running or
  // fetching anything. Only the unnamed main region or the whole page can be selected this way.
  async htmlText(resource, scope) {
    const { page, response } = resource;
    if (await page.evaluate(() => document.body !== null)) return null;
    if (!response || response.url() !== page.url() || scope && !(scope.role === 'main' && scope.name === '')) {
      throw new BrowserError('PAGE_NOT_READY', '页面主体尚未建成，暂不能按此范围读取；可稍后再读、改用整页或正文主区域查找，或换页。', 'not_dispatched');
    }
    const html = await Promise.race([response.text(), new Promise((_, reject) => setTimeout(() => reject(new BrowserError(
      'PAGE_NOT_READY', '页面HTML尚未接收完；可稍后再读或换页。', 'not_dispatched')), this.actionTimeout))]);
    if (Buffer.byteLength(html) > 4 * 1024 * 1024) throw new BrowserError('READ_TOO_LARGE', '页面HTML过大，未解析。');
    return page.evaluate(([source, main]) => {
      const document = new DOMParser().parseFromString(source, 'text/html');
      document.querySelectorAll('script, style, noscript, template').forEach(element => element.remove());
      const root = main ? document.querySelector('main, [role=main]') : document.body;
      return (root?.textContent ?? '').replace(/\s+/g, ' ').trim();
    }, [html, Boolean(scope)]);
  }

  async close(resource) {
    await resource.context.close();
    if (!resource.context.pages().every(page => page.isClosed())) {
      throw new BrowserError('DISPOSE_FAILED', '任务页面仍未关闭，未归还资源额度。', 'outcome_unknown');
    }
    this.owned.delete(resource.context);
  }

  async disconnect() {
    for (const context of [...this.owned]) {
      await context.close();
      this.owned.delete(context);
    }
    if (this.connection) {
      let browser;
      try { browser = await this.connection; } catch { return; } // Failed connections already reported and closed in browser().
      await browser.close();
    }
  }
}
