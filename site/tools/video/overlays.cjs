// Renders the composited overlays (notification banner, key hints, focus ring) as transparent 1440x924 PNGs.
const { chromium } = require(process.env.HOME + '/.shifu/browser/runtime/node_modules/playwright');
const fs = require('fs');
const dir = process.argv[2];
const icon = 'data:image/png;base64,' + fs.readFileSync(dir + '/icon.png').toString('base64');
const T = {
  zh: { now: '现在', title: '升级依赖 · 等你批准', body: 'git push origin chore/upgrade-deps', jump: '跳到在等你', approve: '批准' },
  en: { now: 'now', title: 'Dependency upgrade · waiting for approval', body: 'git push origin chore/upgrade-deps', jump: 'Jump to what\'s waiting', approve: 'Approve' },
};
const css = `html,body{margin:0;background:transparent;width:1440px;height:924px;overflow:hidden;
  font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text","PingFang SC",sans-serif;-webkit-font-smoothing:antialiased}
.banner{position:absolute;right:18px;top:18px;width:372px;padding:14px 16px;border-radius:18px;display:grid;
  grid-template-columns:40px 1fr;gap:12px;align-items:start;color:#f2f2f4;
  background:rgba(48,48,54,.94);box-shadow:0 0 0 .5px rgba(255,255,255,.18) inset,0 18px 50px rgba(0,0,0,.55)}
.banner img{width:40px;height:40px;border-radius:9px}
.row{display:flex;justify-content:space-between;font-size:15px}
.row b{font-weight:650}.row span{color:#a7a7ad;font-size:13px}
.t{font-size:14.5px;margin-top:3px;font-weight:600}.d{font-size:14px;color:#c8c8cf;margin-top:2px;font-family:ui-monospace,Menlo,monospace}
.hint{position:absolute;left:${869 + 571 / 2}px;top:820px;transform:translateX(-50%);display:flex;align-items:center;gap:12px;
  padding:12px 18px;border-radius:16px;background:rgba(20,20,24,.86);box-shadow:0 0 0 1px rgba(255,255,255,.14) inset,0 14px 40px rgba(0,0,0,.5);
  color:#f2f2f4;font-size:17px;font-weight:600;white-space:nowrap}
.k{display:inline-flex;align-items:center;justify-content:center;min-width:40px;height:40px;padding:0 10px;border-radius:9px;
  background:linear-gradient(#3a3a42,#2a2a30);box-shadow:0 2px 0 #121215,0 0 0 1px rgba(255,255,255,.16) inset;font-size:21px;font-weight:600}
.ring{position:absolute;left:870px;top:44px;width:566px;height:876px;border-radius:6px;
  box-shadow:0 0 0 3px #f2b544 inset,0 0 26px 2px rgba(242,181,68,.45) inset}`;
(async () => {
  const b = await chromium.launch({ channel: 'chrome', headless: true });
  const p = await (await b.newContext({ viewport: { width: 1440, height: 924 }, deviceScaleFactor: 1 })).newPage();
  for (const L of ['zh', 'en']) {
    const t = T[L];
    const pages = {
      banner: `<div class="banner"><img src="${icon}"><div><div class="row"><b>Shifu</b><span>${t.now}</span></div><div class="t">${t.title}</div><div class="d">${t.body}</div></div></div>`,
      cmdj: `<div class="hint"><span class="k">⌘</span><span class="k">J</span><span>${t.jump}</span></div>`,
      enter: `<div class="hint"><span class="k">↵</span><span>${t.approve}</span></div>`,
      ring: `<div class="ring"></div>`,
    };
    for (const [n, html] of Object.entries(pages)) {
      await p.setContent(`<style>${css}</style>${html}`);
      await p.waitForTimeout(100);
      await p.screenshot({ path: `${dir}/${n}-${L}.png`, omitBackground: true });
    }
  }
  await b.close();
})();
