// Shifu site — small progressive enhancements. The page works without this file
// (Chinese shown by default, install tabs fall back to the first panel).
(function () {
  'use strict';

  var root = document.documentElement;
  var META = {
    'zh-CN': {
      title: 'Shifu — macOS 编程 Agent 工作台',
      desc: 'Shifu 把 Claude Code、Codex 等编程 Agent 放进同一个 macOS 窗口：一眼看到谁在等你，各家终端原样保留，关掉窗口会话继续运行。开源，数据留在本机。',
      copy: '复制', copied: '已复制', failed: '复制失败'
    },
    en: {
      title: 'Shifu — a macOS workbench for coding agents',
      desc: 'Shifu puts Claude Code, Codex and other coding agents in one macOS window: see which one is waiting for you, keep each agent\'s real terminal, and keep sessions running after you close the window. Open source and local-first.',
      copy: 'Copy', copied: 'Copied', failed: 'Copy failed'
    }
  };

  function isEn() { return root.lang.indexOf('en') === 0; }

  function applyLang(lang, persist) {
    root.lang = lang;
    var m = META[lang] || META['zh-CN'];
    var key = lang === 'en' ? 'en' : 'zh';
    document.title = m.title;
    var d = document.querySelector('meta[name="description"]');
    if (d) d.setAttribute('content', m.desc);
    document.querySelectorAll('[data-alt-' + key + ']').forEach(function (el) {
      el.setAttribute('alt', el.getAttribute('data-alt-' + key));
    });
    document.querySelectorAll('[data-label-' + key + ']').forEach(function (el) {
      el.setAttribute('aria-label', el.getAttribute('data-label-' + key));
    });
    document.querySelectorAll('.copy').forEach(function (b) {
      if (!b.querySelector('[data-l]')) b.textContent = m.copy;
    });
    if (persist) {
      try { localStorage.setItem('shifu-lang', lang); } catch (e) {}
    }
  }

  applyLang(isEn() ? 'en' : 'zh-CN', false);

  var toggle = document.getElementById('lang-toggle');
  if (toggle) {
    toggle.addEventListener('click', function () {
      applyLang(isEn() ? 'zh-CN' : 'en', true);
    });
  }

  // Header border once scrolled.
  var header = document.querySelector('.site-header');
  function onScroll() { header.classList.toggle('scrolled', window.scrollY > 8); }
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  // Install tabs (WAI-ARIA tabs pattern, manual activation with arrow keys).
  var tabs = Array.prototype.slice.call(document.querySelectorAll('[role="tab"]'));
  function select(tab, focus) {
    tabs.forEach(function (t) {
      var on = t === tab;
      t.setAttribute('aria-selected', on ? 'true' : 'false');
      t.tabIndex = on ? 0 : -1;
      document.getElementById(t.getAttribute('aria-controls')).hidden = !on;
    });
    if (focus) tab.focus();
  }
  tabs.forEach(function (t, i) {
    t.addEventListener('click', function () { select(t, false); });
    t.addEventListener('keydown', function (e) {
      var n = null;
      if (e.key === 'ArrowRight') n = tabs[(i + 1) % tabs.length];
      else if (e.key === 'ArrowLeft') n = tabs[(i - 1 + tabs.length) % tabs.length];
      else if (e.key === 'Home') n = tabs[0];
      else if (e.key === 'End') n = tabs[tabs.length - 1];
      if (n) { e.preventDefault(); select(n, true); }
    });
  });

  // Copy buttons.
  document.querySelectorAll('.code .copy').forEach(function (btn) {
    btn.addEventListener('click', function () {
      var pre = btn.parentNode.querySelector('pre');
      var m = META[isEn() ? 'en' : 'zh-CN'];
      var original = btn.innerHTML;
      function done(label) {
        btn.textContent = label;
        setTimeout(function () { btn.innerHTML = original; }, 1600);
      }
      if (!navigator.clipboard) { done(m.failed); return; }
      navigator.clipboard.writeText(pre.innerText.replace(/\n$/, '')).then(
        function () { done(m.copied); },
        function () { done(m.failed); }
      );
    });
  });

  // Reveal on scroll; skipped when the user prefers reduced motion.
  var reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var items = document.querySelectorAll('.reveal');
  if (reduce || !('IntersectionObserver' in window)) {
    items.forEach(function (el) { el.classList.add('in'); });
  } else {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) {
        if (en.isIntersecting) { en.target.classList.add('in'); io.unobserve(en.target); }
      });
    }, { rootMargin: '0px 0px -8% 0px', threshold: 0.08 });
    items.forEach(function (el) { io.observe(el); });
  }
})();
