// Shifu site — small progressive enhancements. The page works without this file
// (install tabs fall back to the first panel; long code blocks stay expanded).
(function () {
  'use strict';

  var root = document.documentElement;
  // Each language has its own pre-rendered page (/ is Chinese, /en/ is English); the URL decides.
  var META = {
    'zh-CN': { copy: '复制', copied: '已复制', failed: '复制失败' },
    en: { copy: 'Copy', copied: 'Copied', failed: 'Copy failed' }
  };
  function isEn() { return root.lang.indexOf('en') === 0; }

  // The toggle is a plain link to the other page; remember the choice for later visits.
  var toggle = document.getElementById('lang-toggle');
  if (toggle) {
    toggle.addEventListener('click', function () {
      try { localStorage.setItem('shifu-lang', toggle.getAttribute('data-lang')); } catch (e) {}
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
      navigator.clipboard.writeText(pre.textContent.replace(/\n$/, '')).then(
        function () { done(m.copied); },
        function () { done(m.failed); }
      );
    });
  });

  // Collapsible long blocks: show the first lines with a toggle. The full text stays in the DOM,
  // so copy and find-in-page still see all of it. Collapsing keeps the toggle where it was on screen.
  var TOGGLE = {
    zh: ['展开全部', '收起'],
    en: ['Show all', 'Show less']
  };
  var chevron = '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m4 6 4 4 4-4"/></svg>';
  function labelToggle(btn) {
    var open = btn.getAttribute('aria-expanded') === 'true';
    var lines = btn.getAttribute('data-lines');
    btn.innerHTML =
      '<span data-l="zh">' + TOGGLE.zh[open ? 1 : 0] + (open ? '' : '（共 ' + lines + ' 行）') + '</span>' +
      '<span data-l="en">' + TOGGLE.en[open ? 1 : 0] + (open ? '' : ' (' + lines + ' lines)') + '</span>' + chevron;
  }
  document.querySelectorAll('.code[data-collapse]').forEach(function (block, i) {
    var pre = block.querySelector('pre');
    pre.id = pre.id || 'code-block-' + i;
    var btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'code-toggle';
    btn.setAttribute('aria-controls', pre.id);
    btn.setAttribute('aria-expanded', 'false');
    btn.setAttribute('data-lines', String(pre.textContent.replace(/\n$/, '').split('\n').length));
    labelToggle(btn);
    block.appendChild(btn);
    block.classList.add('is-collapsed');
    btn.addEventListener('click', function () {
      var before = btn.getBoundingClientRect().top;
      var open = btn.getAttribute('aria-expanded') !== 'true';
      btn.setAttribute('aria-expanded', open ? 'true' : 'false');
      block.classList.toggle('is-collapsed', !open);
      labelToggle(btn);
      if (!open) {
        var shift = btn.getBoundingClientRect().top - before;
        if (shift) window.scrollBy({ top: shift, left: 0, behavior: 'instant' });
      }
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
