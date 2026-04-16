/* claude-picker landing — self-contained progressive enhancement.
   No frameworks. No trackers. No cookies. No external CDNs at runtime.
   Modules below (in order of declaration):
     1. Copy-to-clipboard buttons
     2. Smooth-scroll for in-page anchors
     3. Scroll-reveal via IntersectionObserver
     4. Interactive picker simulation (hero centerpiece)
     5. Command palette (Cmd/Ctrl+K or /)
     6. Leader-key site navigation (g g, g i, g c, g f, g h, ?)
     7. Live GitHub stats + animated count-up
     8. Keyboard hint fade-out on first use
*/

(function () {
  'use strict';

  var reduceMotion = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // ----------------------------------------------------------------
  // 1. Copy-to-clipboard
  // ----------------------------------------------------------------

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

  // ----------------------------------------------------------------
  // 2. Smooth-scroll for in-page anchors
  // ----------------------------------------------------------------

  if (!reduceMotion) {
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

  // ----------------------------------------------------------------
  // 3. Scroll-reveal via IntersectionObserver
  // ----------------------------------------------------------------

  if ('IntersectionObserver' in window && !reduceMotion) {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          entry.target.classList.add('is-visible');
          io.unobserve(entry.target);
        }
      });
    }, { rootMargin: '0px 0px -8% 0px', threshold: 0.06 });
    document.querySelectorAll('.reveal').forEach(function (el) { io.observe(el); });
  } else {
    // no IO or reduced motion -> make everything visible immediately
    document.querySelectorAll('.reveal').forEach(function (el) { el.classList.add('is-visible'); });
  }

  // ----------------------------------------------------------------
  // 4. Interactive picker simulation
  // ----------------------------------------------------------------

  (function pickerSim() {
    var root = document.getElementById('picker-sim');
    if (!root) return;

    var rows = Array.prototype.slice.call(root.querySelectorAll('.picker-row'));
    var qEl = document.getElementById('ps-query');
    var countEl = document.getElementById('ps-count');
    var previewTitle = document.getElementById('pp-title');
    var previewBody = document.getElementById('pp-body');
    var previewId = document.getElementById('pp-id');
    var heroSection = document.querySelector('.hero');

    var PREVIEWS = {
      'auth-refactor': {
        id: '4a2e8f1c',
        body: '<span class="user">user</span> <span class="muted">→</span> Move session auth from cookies to signed JWTs.\n  Keep backwards compat with existing sessions for 30d.\n\n<span class="asst">claude</span> <span class="muted">→</span> I\'ll split this into three steps:\n  1. Add a new /auth/jwt route that accepts the old cookie\n  2. Migrate session middleware to read both\n  3. Expire cookie-only sessions after 30d…',
        stats: { msgs: 42, tokens: '18.2k', model: 'sonnet', cost: '$0.41' }
      },
      'fix-race-condition': {
        id: 'b7c9d2e0',
        body: '<span class="user">user</span> <span class="muted">→</span> Stripe webhook sometimes marks the order twice.\n  Logs show two concurrent POSTs with the same event id.\n\n<span class="asst">claude</span> <span class="muted">→</span> Classic idempotency gap. Options:\n  1. Postgres UPSERT on stripe_event_id with unique constraint\n  2. Redis SETNX guard (cheap but eventually inconsistent)\n  3. Process inside a SERIALIZABLE txn + retry on 40001\n  I\'d pick (1) for durability…',
        stats: { msgs: 38, tokens: '52.9k', model: 'opus', cost: '$1.24' }
      },
      'drizzle-migration': {
        id: 'e5f8a3b1',
        body: '<span class="user">user</span> <span class="muted">→</span> Switch the user_preferences table from prisma\n  to drizzle. Preserve existing rows, keep snake_case.\n\n<span class="asst">claude</span> <span class="muted">→</span> drizzle schema written. Diff:\n  + src/db/schema/user-preferences.ts\n  ~ src/db/schema/index.ts\n  Running <span class="muted">drizzle-kit generate</span> now…',
        stats: { msgs: 19, tokens: '7.4k', model: 'haiku', cost: '$0.18' }
      },
      'mcp-postgres-setup': {
        id: '9c1d4e7f',
        body: '<span class="user">user</span> <span class="muted">→</span> Wire up the postgres MCP server locally.\n  Claude Desktop says "server not responding".\n\n<span class="asst">claude</span> <span class="muted">→</span> Check claude_desktop_config.json — if the\n  path to <span class="muted">mcp-server-postgres</span> uses <span class="muted">~</span>, expand it.\n  Also: the DATABASE_URL must be in env, not a literal…',
        stats: { msgs: 27, tokens: '24.1k', model: 'sonnet', cost: '$0.62' }
      },
      'fix-payment-webhook': {
        id: '1f8e3a7b',
        body: '<span class="user">user</span> <span class="muted">→</span> Users in EU timezones see "Payment failed"\n  but card was actually charged. See attached log.\n\n<span class="asst">claude</span> <span class="muted">→</span> Your webhook timeout is 30s but PG settlement\n  in the EU cluster p95 is 42s. Two changes:\n  1. Bump webhook timeout to 60s\n  2. Treat 504s as "unknown" not "failed"…',
        stats: { msgs: 63, tokens: '81.3k', model: 'opus', cost: '$2.07' }
      },
      'session-tree-feature': {
        id: '3b5d8f2a',
        body: '<span class="user">user</span> <span class="muted">→</span> Add a --tree flag that groups sessions\n  by project and shows fork relationships.\n\n<span class="asst">claude</span> <span class="muted">→</span> Parse /branch markers from JSONL, build a\n  DAG keyed on session_id -> parent_id, then\n  render with rich.tree. Roots first, leaves last…',
        stats: { msgs: 31, tokens: '22.6k', model: 'sonnet', cost: '$0.53' }
      }
    };

    var query = '';
    var selectedIdx = 0;
    var visibleRows = rows.slice();
    var resumeTimer = null;

    function escapeHtml(s) {
      return s.replace(/[&<>"']/g, function (c) {
        return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
      });
    }

    function highlight(name, q) {
      if (!q) return escapeHtml(name);
      var lower = name.toLowerCase();
      var ql = q.toLowerCase();
      var result = '';
      var qi = 0;
      for (var i = 0; i < name.length; i++) {
        if (qi < ql.length && lower[i] === ql[qi]) {
          result += '<span class="hl">' + escapeHtml(name[i]) + '</span>';
          qi++;
        } else {
          result += escapeHtml(name[i]);
        }
      }
      return result;
    }

    function fuzzyMatch(name, q) {
      if (!q) return true;
      var lower = name.toLowerCase();
      var ql = q.toLowerCase();
      var qi = 0;
      for (var i = 0; i < lower.length && qi < ql.length; i++) {
        if (lower[i] === ql[qi]) qi++;
      }
      return qi === ql.length;
    }

    function render() {
      // update query display
      if (query) {
        qEl.innerHTML = escapeHtml(query);
      } else {
        qEl.innerHTML = '<span class="ps-placeholder">type to filter…</span>';
      }

      // filter + highlight
      visibleRows = [];
      rows.forEach(function (row) {
        var name = row.getAttribute('data-name');
        var matches = fuzzyMatch(name, query);
        if (matches) {
          row.classList.remove('is-filtered-out');
          visibleRows.push(row);
        } else {
          row.classList.add('is-filtered-out');
        }
        var nameEl = row.querySelector('.pr-name');
        if (nameEl) nameEl.innerHTML = highlight(name, query);
      });

      countEl.textContent = visibleRows.length + ' / ' + rows.length;

      // clamp selection
      if (visibleRows.length === 0) {
        rows.forEach(function (r) {
          r.classList.remove('is-selected');
          r.setAttribute('aria-selected', 'false');
        });
        return;
      }
      if (selectedIdx >= visibleRows.length) selectedIdx = visibleRows.length - 1;
      if (selectedIdx < 0) selectedIdx = 0;

      rows.forEach(function (r) {
        r.classList.remove('is-selected');
        r.setAttribute('aria-selected', 'false');
      });
      var sel = visibleRows[selectedIdx];
      sel.classList.add('is-selected');
      sel.setAttribute('aria-selected', 'true');
      updatePreview(sel.getAttribute('data-name'));
    }

    function updatePreview(name) {
      var data = PREVIEWS[name];
      if (!data) return;
      previewTitle.textContent = name;
      previewBody.innerHTML = data.body;
      previewId.textContent = data.id;
      var statsContainer = root.querySelector('.pp-stats');
      if (statsContainer) {
        statsContainer.innerHTML =
          '<span><span class="k">msgs</span>&nbsp;' + data.stats.msgs + '</span>' +
          '<span><span class="k">tokens</span>&nbsp;' + data.stats.tokens + '</span>' +
          '<span><span class="k">model</span>&nbsp;' + data.stats.model + '</span>' +
          '<span><span class="k">cost</span>&nbsp;' + data.stats.cost + '</span>';
      }
    }

    function playResume() {
      if (resumeTimer) return;
      var sel = visibleRows[selectedIdx];
      if (!sel) return;
      sel.classList.add('is-resuming');
      var name = sel.getAttribute('data-name');
      var prev = previewBody.innerHTML;
      previewBody.innerHTML = '<span class="muted">resuming</span> <span class="asst">' + escapeHtml(name) + '</span>\n<span class="muted">→ handing off to claude…</span>';
      resumeTimer = setTimeout(function () {
        sel.classList.remove('is-resuming');
        previewBody.innerHTML = prev;
        resumeTimer = null;
      }, 1400);
    }

    function onKey(e) {
      // don't swallow keys if typing elsewhere or the command palette is open
      var overlay = document.getElementById('cmdk-overlay');
      if (overlay && overlay.getAttribute('data-open') === 'true') return;
      var activeTag = document.activeElement && document.activeElement.tagName;
      if (activeTag === 'INPUT' || activeTag === 'TEXTAREA') return;

      if (e.key === 'ArrowDown') {
        e.preventDefault();
        selectedIdx = Math.min(selectedIdx + 1, visibleRows.length - 1);
        render();
        if (heroSection) heroSection.classList.add('paused');
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        selectedIdx = Math.max(selectedIdx - 1, 0);
        render();
        if (heroSection) heroSection.classList.add('paused');
      } else if (e.key === 'Enter') {
        if (document.activeElement === root || root.contains(document.activeElement)) {
          e.preventDefault();
          playResume();
          if (heroSection) heroSection.classList.add('paused');
        }
      } else if (e.key === 'Escape') {
        query = '';
        selectedIdx = 0;
        render();
      } else if (e.key === 'Backspace') {
        if (document.activeElement === root) {
          e.preventDefault();
          query = query.slice(0, -1);
          selectedIdx = 0;
          render();
          if (heroSection) heroSection.classList.add('paused');
        }
      } else if (e.key.length === 1 && /[a-z0-9\- ]/i.test(e.key) && !e.metaKey && !e.ctrlKey && !e.altKey) {
        if (document.activeElement === root) {
          e.preventDefault();
          query += e.key.toLowerCase();
          selectedIdx = 0;
          render();
          if (heroSection) heroSection.classList.add('paused');
        }
      }
    }

    rows.forEach(function (row, i) {
      row.addEventListener('click', function () {
        selectedIdx = visibleRows.indexOf(row);
        if (selectedIdx === -1) selectedIdx = 0;
        render();
        root.focus();
        if (heroSection) heroSection.classList.add('paused');
      });
      row.addEventListener('dblclick', function () {
        playResume();
      });
    });

    // Global listener — works whether picker is focused or not when hovered/focused.
    document.addEventListener('keydown', function (e) {
      // only act on keys when picker has focus OR user has engaged with it
      if (document.activeElement === root || root.contains(document.activeElement)) {
        onKey(e);
      }
    });

    render();
  })();

  // ----------------------------------------------------------------
  // 5. Command palette (Cmd/Ctrl+K or /)
  // ----------------------------------------------------------------

  var PALETTE_ITEMS = [
    { title: 'Features', shortcut: 'g f', href: '#features', icon: 'grid' },
    { title: 'How it compares', shortcut: '', href: '#compare', icon: 'scales' },
    { title: 'Install', shortcut: 'g i', href: '#install', icon: 'download' },
    { title: 'Commands', shortcut: 'g c', href: '#commands', icon: 'terminal' },
    { title: 'Stats dashboard', shortcut: '', href: '#stats', icon: 'chart' },
    { title: 'Keyboard shortcuts', shortcut: '', href: '#shortcuts', icon: 'key' },
    { title: 'GitHub stats', shortcut: '', href: '#gh', icon: 'github' },
    { title: 'Philosophy', shortcut: '', href: '#problem', icon: 'book' },
    { title: 'View on GitHub', shortcut: 'g h', href: 'https://github.com/anshul-garg27/claude-picker', external: true, icon: 'github' },
    { title: 'Report an issue', shortcut: '', href: 'https://github.com/anshul-garg27/claude-picker/issues/new', external: true, icon: 'bug' },
    { title: 'Top of page', shortcut: 'g g', href: '#top', icon: 'arrow-up' },
    { title: 'Copy install command', shortcut: '', action: 'copy-install', icon: 'copy' }
  ];

  var ICONS = {
    grid: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="5" height="5" rx="1"/><rect x="9" y="2" width="5" height="5" rx="1"/><rect x="2" y="9" width="5" height="5" rx="1"/><rect x="9" y="9" width="5" height="5" rx="1"/></svg>',
    scales: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M8 2v12"/><path d="M4 14h8"/><path d="M2 6l3-3 3 3"/><path d="M8 6l3-3 3 3"/></svg>',
    download: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M8 2v9"/><path d="M4 7l4 4 4-4"/><path d="M2 14h12"/></svg>',
    terminal: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 5l3 3-3 3"/><path d="M8 11h5"/><rect x="1" y="2" width="14" height="12" rx="1.5"/></svg>',
    chart: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 13V3"/><path d="M14 13H2"/><rect x="4" y="8" width="2" height="5"/><rect x="8" y="5" width="2" height="8"/><rect x="12" y="10" width="2" height="3"/></svg>',
    key: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="5" cy="11" r="3"/><path d="M7 9l6-6"/><path d="M11 5l2 2"/></svg>',
    github: '<svg viewBox="0 0 16 16" fill="currentColor"><path d="M8 0a8 8 0 0 0-2.53 15.59c.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.13 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.11.16 1.93.08 2.13.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8a8 8 0 0 0-8-8"/></svg>',
    book: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3.5C2 3 2.5 2 4 2h10v11H4c-.75 0-1.5.5-2 1"/><path d="M2 3.5V14c.5-.5 1.25-1 2-1h10"/></svg>',
    bug: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="5" width="6" height="8" rx="3"/><path d="M2 7h3"/><path d="M11 7h3"/><path d="M2 12h3"/><path d="M11 12h3"/><path d="M8 3V2"/><path d="M6 3l-1-1"/><path d="M10 3l1-1"/></svg>',
    'arrow-up': '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M8 13V3"/><path d="M4 7l4-4 4 4"/></svg>',
    copy: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="4" width="9" height="9" rx="1.5"/><path d="M10.5 4V3a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h1"/></svg>'
  };

  (function commandPalette() {
    var overlay = document.getElementById('cmdk-overlay');
    if (!overlay) return;

    var input = document.getElementById('cmdk-input');
    var list = document.getElementById('cmdk-list');
    var hintBtn = document.getElementById('cmdk-hint');

    var selectedIdx = 0;
    var filtered = PALETTE_ITEMS.slice();
    var lastFocus = null;

    function renderList() {
      if (filtered.length === 0) {
        list.innerHTML = '<li class="cmdk-empty">No matches. Try "install" or "github".</li>';
        return;
      }
      list.innerHTML = filtered.map(function (item, i) {
        var sel = (i === selectedIdx) ? 'true' : 'false';
        var shortcut = item.shortcut ? '<span class="cmdk-item-shortcut">' + item.shortcut + '</span>' : '';
        var icon = ICONS[item.icon] || ICONS.book;
        return '<li class="cmdk-item" role="option" aria-selected="' + sel + '" data-selected="' + sel + '" data-idx="' + i + '">' +
          icon +
          '<span class="cmdk-item-title">' + item.title + '</span>' +
          shortcut +
        '</li>';
      }).join('');
    }

    function filter(q) {
      var ql = q.trim().toLowerCase();
      if (!ql) {
        filtered = PALETTE_ITEMS.slice();
      } else {
        filtered = PALETTE_ITEMS.filter(function (it) {
          return it.title.toLowerCase().indexOf(ql) !== -1;
        });
      }
      selectedIdx = 0;
      renderList();
    }

    function selectItem(item) {
      if (!item) return;
      close();
      if (item.action === 'copy-install') {
        var btn = document.querySelector('.copy-btn');
        if (btn) btn.click();
        return;
      }
      if (item.external) {
        window.open(item.href, '_blank', 'noopener');
        return;
      }
      if (item.href && item.href.indexOf('#') === 0) {
        var el = document.querySelector(item.href);
        if (el) {
          el.scrollIntoView({ behavior: reduceMotion ? 'auto' : 'smooth', block: 'start' });
          history.replaceState(null, '', item.href);
        }
      }
    }

    function open() {
      lastFocus = document.activeElement;
      overlay.hidden = false;
      overlay.setAttribute('data-open', 'true');
      input.value = '';
      filter('');
      // defer focus so transition applies
      requestAnimationFrame(function () { input.focus(); });
      // fade out hint once seen
      if (hintBtn && !hintBtn.classList.contains('fade-out')) {
        hintBtn.classList.add('fade-out');
        try { sessionStorage.setItem('cmdk-seen', '1'); } catch (e) {}
      }
    }

    function close() {
      overlay.setAttribute('data-open', 'false');
      setTimeout(function () {
        if (overlay.getAttribute('data-open') === 'false') overlay.hidden = true;
      }, 200);
      if (lastFocus && typeof lastFocus.focus === 'function') lastFocus.focus();
    }

    input.addEventListener('input', function () { filter(input.value); });

    input.addEventListener('keydown', function (e) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        selectedIdx = Math.min(selectedIdx + 1, filtered.length - 1);
        renderList();
        scrollSelectedIntoView();
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        selectedIdx = Math.max(selectedIdx - 1, 0);
        renderList();
        scrollSelectedIntoView();
      } else if (e.key === 'Enter') {
        e.preventDefault();
        selectItem(filtered[selectedIdx]);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        close();
      } else if (e.key === 'Tab') {
        // keep focus inside dialog (only one input here)
        e.preventDefault();
      }
    });

    function scrollSelectedIntoView() {
      var el = list.querySelector('[data-selected="true"]');
      if (el && typeof el.scrollIntoView === 'function') {
        el.scrollIntoView({ block: 'nearest' });
      }
    }

    list.addEventListener('click', function (e) {
      var item = e.target.closest('.cmdk-item');
      if (!item) return;
      var idx = parseInt(item.getAttribute('data-idx'), 10);
      selectItem(filtered[idx]);
    });

    list.addEventListener('mousemove', function (e) {
      var item = e.target.closest('.cmdk-item');
      if (!item) return;
      var idx = parseInt(item.getAttribute('data-idx'), 10);
      if (idx !== selectedIdx) {
        selectedIdx = idx;
        renderList();
      }
    });

    overlay.addEventListener('click', function (e) {
      if (e.target === overlay) close();
    });

    if (hintBtn) {
      hintBtn.addEventListener('click', open);
      // If already seen, start faded out
      try {
        if (sessionStorage.getItem('cmdk-seen')) hintBtn.classList.add('fade-out');
      } catch (e) {}
    }

    // expose for keyboard nav
    window.__cmdk = { open: open, close: close, isOpen: function () { return overlay.getAttribute('data-open') === 'true'; } };
  })();

  // ----------------------------------------------------------------
  // 6. Leader-key site navigation (g g, g i, etc.) + global hotkeys
  // ----------------------------------------------------------------

  (function keyboardNav() {
    var leader = null;
    var leaderTimer = null;

    function showToast(html) {
      var toast = document.getElementById('kb-toast');
      if (!toast) return;
      toast.innerHTML = html;
      toast.classList.add('show');
      clearTimeout(toast._t);
      toast._t = setTimeout(function () { toast.classList.remove('show'); }, 1200);
    }

    function jumpTo(sel, label) {
      var el = document.querySelector(sel);
      if (!el) return;
      el.scrollIntoView({ behavior: reduceMotion ? 'auto' : 'smooth', block: 'start' });
      history.replaceState(null, '', sel);
      showToast('<kbd>' + label + '</kbd><span class="msg">' + sel + '</span>');
    }

    var LEADER_MAP = {
      'g': function () { jumpTo('#top', 'g g'); },
      'i': function () { jumpTo('#install', 'g i'); },
      'c': function () { jumpTo('#commands', 'g c'); },
      'f': function () { jumpTo('#features', 'g f'); },
      's': function () { jumpTo('#shortcuts', 'g s'); },
      'h': function () {
        showToast('<kbd>g h</kbd><span class="msg">opening github…</span>');
        window.open('https://github.com/anshul-garg27/claude-picker', '_blank', 'noopener');
      }
    };

    document.addEventListener('keydown', function (e) {
      // Always allow Cmd/Ctrl+K to open palette
      if ((e.metaKey || e.ctrlKey) && (e.key === 'k' || e.key === 'K')) {
        e.preventDefault();
        if (window.__cmdk) window.__cmdk.open();
        return;
      }

      // Skip leader keys when typing in inputs or when palette is open
      var activeTag = document.activeElement && document.activeElement.tagName;
      var inInput = activeTag === 'INPUT' || activeTag === 'TEXTAREA' || (document.activeElement && document.activeElement.isContentEditable);
      if (inInput) return;
      if (window.__cmdk && window.__cmdk.isOpen()) return;

      // Picker focused? picker owns keys.
      var pickerSim = document.getElementById('picker-sim');
      if (pickerSim && (document.activeElement === pickerSim || pickerSim.contains(document.activeElement))) return;

      // '/' opens the palette (like GitHub/Linear/Raycast)
      if (e.key === '/') {
        e.preventDefault();
        if (window.__cmdk) window.__cmdk.open();
        return;
      }

      // '?' shows shortcuts help (opens palette pre-seeded)
      if (e.key === '?') {
        e.preventDefault();
        if (window.__cmdk) window.__cmdk.open();
        return;
      }

      if (e.metaKey || e.ctrlKey || e.altKey) return;

      // leader key: 'g' then letter
      if (leader === 'g') {
        var fn = LEADER_MAP[e.key.toLowerCase()];
        if (fn) {
          e.preventDefault();
          fn();
        }
        leader = null;
        clearTimeout(leaderTimer);
        return;
      }

      if (e.key === 'g') {
        leader = 'g';
        showToast('<kbd>g</kbd><span class="msg">…</span>');
        clearTimeout(leaderTimer);
        leaderTimer = setTimeout(function () { leader = null; }, 900);
      }
    });
  })();

  // ----------------------------------------------------------------
  // 7. Live GitHub stats + animated count-up
  // ----------------------------------------------------------------

  (function githubStats() {
    var starEl = document.getElementById('gh-stars');
    if (!starEl) return;

    var forksEl = document.getElementById('gh-forks');
    var issuesEl = document.getElementById('gh-issues');
    var commitEl = document.getElementById('gh-commit');
    var container = document.getElementById('gh-stats');

    var REPO = 'anshul-garg27/claude-picker';
    var CACHE_KEY = 'ghstats:' + REPO;
    var CACHE_TTL_MS = 10 * 60 * 1000; // 10 min

    function formatRelative(iso) {
      if (!iso) return '—';
      var then = new Date(iso).getTime();
      var now = Date.now();
      var diff = Math.max(0, now - then);
      var d = Math.floor(diff / 86400000);
      if (d >= 30) return Math.floor(d / 30) + 'mo ago';
      if (d >= 1) return d + 'd ago';
      var h = Math.floor(diff / 3600000);
      if (h >= 1) return h + 'h ago';
      var m = Math.floor(diff / 60000);
      if (m >= 1) return m + 'm ago';
      return 'just now';
    }

    function animateCount(el, from, to, duration) {
      if (reduceMotion || duration <= 0) {
        el.textContent = String(to);
        el.classList.remove('loading');
        return;
      }
      var start = performance.now();
      function step(now) {
        var p = Math.min(1, (now - start) / duration);
        // easeOutCubic
        var eased = 1 - Math.pow(1 - p, 3);
        var val = Math.round(from + (to - from) * eased);
        el.textContent = String(val);
        if (p < 1) requestAnimationFrame(step);
        else el.classList.remove('loading');
      }
      requestAnimationFrame(step);
    }

    function render(data) {
      if (forksEl) {
        forksEl.textContent = String(data.forks || 0);
        forksEl.classList.remove('loading');
      }
      if (issuesEl) {
        issuesEl.textContent = String(data.issues || 0);
        issuesEl.classList.remove('loading');
      }
      if (commitEl) {
        commitEl.textContent = formatRelative(data.pushed_at);
        commitEl.classList.remove('loading');
      }

      // count-up for stars, triggered by IntersectionObserver
      var target = data.stars || 0;
      if ('IntersectionObserver' in window) {
        var sio = new IntersectionObserver(function (entries, obs) {
          entries.forEach(function (e) {
            if (e.isIntersecting) {
              animateCount(starEl, 0, target, 900);
              obs.disconnect();
            }
          });
        }, { threshold: 0.4 });
        sio.observe(container);
      } else {
        starEl.textContent = String(target);
        starEl.classList.remove('loading');
      }
    }

    // session cache
    try {
      var cached = sessionStorage.getItem(CACHE_KEY);
      if (cached) {
        var parsed = JSON.parse(cached);
        if (parsed.ts && Date.now() - parsed.ts < CACHE_TTL_MS) {
          render(parsed.data);
          return;
        }
      }
    } catch (e) {}

    fetch('https://api.github.com/repos/' + REPO, {
      headers: { 'Accept': 'application/vnd.github+json' }
    })
      .then(function (r) {
        if (!r.ok) throw new Error('gh api failed: ' + r.status);
        return r.json();
      })
      .then(function (json) {
        var data = {
          stars: json.stargazers_count,
          forks: json.forks_count,
          issues: json.open_issues_count,
          pushed_at: json.pushed_at
        };
        try {
          sessionStorage.setItem(CACHE_KEY, JSON.stringify({ ts: Date.now(), data: data }));
        } catch (e) {}
        render(data);
      })
      .catch(function () {
        // graceful fallback
        if (starEl) starEl.textContent = '—';
        if (forksEl) forksEl.textContent = '—';
        if (issuesEl) issuesEl.textContent = '—';
        if (commitEl) commitEl.textContent = '—';
      });
  })();

})();
