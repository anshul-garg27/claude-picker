/* claude-picker landing — minimal JS.
   No frameworks. No trackers. No cookies.
   Just: copy-button feedback + smooth-scroll for nav anchors. */

(function () {
  'use strict';

  /* Copy-to-clipboard buttons */
  document.querySelectorAll('[data-copy]').forEach(function (btn) {
    btn.addEventListener('click', async function () {
      var target = document.querySelector(btn.getAttribute('data-copy'));
      if (!target) return;
      var text = target.getAttribute('data-raw') || target.innerText.trim();

      try {
        if (navigator.clipboard && window.isSecureContext) {
          await navigator.clipboard.writeText(text);
        } else {
          var ta = document.createElement('textarea');
          ta.value = text;
          ta.setAttribute('readonly', '');
          ta.style.position = 'fixed';
          ta.style.top = '-1000px';
          document.body.appendChild(ta);
          ta.select();
          document.execCommand('copy');
          document.body.removeChild(ta);
        }

        var label = btn.querySelector('[data-label]');
        var prev = label ? label.textContent : null;
        btn.setAttribute('data-copied', 'true');
        if (label) label.textContent = 'Copied';

        setTimeout(function () {
          btn.removeAttribute('data-copied');
          if (label && prev !== null) label.textContent = prev;
        }, 1800);
      } catch (err) {
        console.warn('Copy failed', err);
      }
    });
  });

  /* Smooth-scroll for in-page anchors (skip if user prefers reduced motion) */
  var reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (!reduce) {
    document.querySelectorAll('a[href^="#"]').forEach(function (link) {
      var href = link.getAttribute('href');
      if (href === '#' || href.length < 2) return;
      link.addEventListener('click', function (e) {
        var el = document.querySelector(href);
        if (!el) return;
        e.preventDefault();
        el.scrollIntoView({ behavior: 'smooth', block: 'start' });
        history.replaceState(null, '', href);
      });
    });
  }
})();
