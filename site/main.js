// Shifu site — small progressive enhancements. The page works without this file
// (the install prompt stays open to select by hand; long code blocks stay expanded).
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

  // Demo video: a visible pause / play control. With reduced motion it starts paused on its poster.
  var video = document.getElementById('hero-video');
  var vToggle = document.querySelector('.video-toggle');
  if (video && vToggle) {
    var sync = function () { vToggle.classList.toggle('is-paused', video.paused); };
    vToggle.hidden = false;
    sync();
    video.addEventListener('play', sync);
    video.addEventListener('pause', sync);
    vToggle.addEventListener('click', function () {
      if (video.paused) {
        var p = video.play();
        if (p && p.catch) p.catch(function () {});
      } else {
        video.pause();
      }
    });
  }

  // Phone menu (a <details>, works without JS): close it after a link is chosen.
  var menu = document.querySelector('.nav-menu');
  if (menu) {
    menu.addEventListener('click', function (e) {
      if (e.target.closest && e.target.closest('a')) menu.open = false;
    });
  }

  // Header border once scrolled.
  var header = document.querySelector('.site-header');
  function onScroll() { header.classList.toggle('scrolled', window.scrollY > 8); }
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  // Install: "Copy installation prompt" copies the generated prompt (#install-prompt). Without this script the
  // button stays hidden and the prompt's <details> stays open, so it can be selected by hand.
  function copyText(text, ok, bad) {
    if (!navigator.clipboard) { bad(); return; }
    navigator.clipboard.writeText(text).then(ok, bad);
  }
  document.querySelectorAll('.copy-prompt').forEach(function (btn) {
    var pre = document.getElementById(btn.getAttribute('data-copy-target'));
    if (!pre) return;
    var label = btn.querySelector('.copy-label'), done = btn.querySelector('.copy-done'), fail = btn.querySelector('.copy-fail');
    var timer = null;
    function show(which) {
      label.hidden = which !== label; done.hidden = which !== done; fail.hidden = which !== fail;
      clearTimeout(timer);
      if (which !== label) timer = setTimeout(function () { show(label); }, 2400);
    }
    btn.hidden = false;
    btn.addEventListener('click', function () {
      copyText(pre.textContent.replace(/\n$/, ''),
        function () { show(done); },
        function () {
          show(fail);
          var d = pre.closest('details');
          if (d) d.open = true;
        });
    });
  });
  // "Download ZIP and view instructions" opens the release in a new tab and the steps here.
  var manual = document.getElementById('manual-download');
  if (manual) {
    manual.addEventListener('click', function () {
      var d = document.getElementById(manual.getAttribute('aria-controls'));
      if (d) d.open = true;
    });
  }

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
      copyText(pre.textContent.replace(/\n$/, ''), function () { done(m.copied); }, function () { done(m.failed); });
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
