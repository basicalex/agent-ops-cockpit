/* docs.intrface.eu page comments.
   Vanilla, no dependencies. Reads /_comments for the page named in the
   script's data-page attribute, anchors each thread back to the text it was
   left on, and renders bubbles in a right rail (>= 1100px) or inline under the
   anchored block (below that). Never throws into the host page: every entry
   point is wrapped, and an anchor that cannot be found falls back to the
   General comments panel. */
(function () {
  'use strict';

  var script = document.currentScript || document.querySelector('script[data-page]');
  var PAGE = script && script.getAttribute('data-page');
  if (!PAGE) return;

  var API = '/_comments';
  var RAIL_W = 300;      // bubble width in the rail
  var RESERVE = 348;     // right padding added to <html> when the rail is on
  var RAIL_MIN = 1100;   // viewport width at which the rail appears
  var POLL_MS = 30000;
  var NAME_KEY = 'docs_author';

  var S = {
    viewer: { role: 'anon', name: null, canWrite: false },
    threads: [],
    sig: '',
    expanded: {},      // accepted thread id -> shown in full
    openInline: null,  // thread id whose inline panel is open (narrow)
    active: null,      // hovered / focused thread id
    drafts: {},        // field key -> text
    composer: null,    // { anchor, left, top }
    generalOpen: false,
    error: null,
    booted: false
  };

  var root = null, floatBox = null, generalBox = null;
  var markers = [];      // { id, node }
  var railBubbles = [];  // { id, node, mark }

  /* ---------------------------------------------------------------- utils */

  function el(tag, cls, text) {
    var n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text != null) n.textContent = text;
    return n;
  }

  function ui(node) {
    node.setAttribute('data-cm-ui', '');
    return node;
  }

  function clamp(n, lo, hi) { return Math.max(lo, Math.min(hi, n)); }

  function norm(s) { return String(s == null ? '' : s).replace(/\s+/g, ' ').trim(); }

  function rel(ts) {
    var d = (Date.now() - Number(ts)) / 1000;
    if (!isFinite(d)) return '';
    if (d < 60) return 'just now';
    if (d < 3600) return Math.floor(d / 60) + ' min ago';
    if (d < 86400) return Math.floor(d / 3600) + ' h ago';
    if (d < 604800) return Math.floor(d / 86400) + ' d ago';
    try { return new Date(Number(ts)).toLocaleDateString(); } catch (e) { return ''; }
  }

  function guard(fn) {
    return function () {
      try { return fn.apply(this, arguments); } catch (e) { log(e); };
    };
  }

  function log(e) {
    if (window.console && console.warn) console.warn('[comments]', e);
  }

  /* ------------------------------------------------------------------ api */

  function req(method, url, body) {
    var opts = { method: method, credentials: 'same-origin', headers: { accept: 'application/json' } };
    if (body) {
      opts.headers['content-type'] = 'application/json';
      opts.body = JSON.stringify(body);
    }
    return fetch(url, opts).then(function (r) {
      return r.text().then(function (t) {
        var data = null;
        try { data = t ? JSON.parse(t) : null; } catch (e) { data = null; }
        if (!r.ok) throw new Error((data && data.error) || ('request failed (' + r.status + ')'));
        return data;
      });
    });
  }

  function load() {
    return req('GET', API + '?page=' + encodeURIComponent(PAGE)).then(function (data) {
      if (!data) return;
      S.viewer = data.viewer || S.viewer;
      S.threads = Array.isArray(data.threads) ? data.threads : [];
      S.error = null;
      render();
    }).catch(function (e) {
      log(e);
      if (S.booted) { S.error = e.message; render(); }
    });
  }

  /* --------------------------------------------------------- text indexing */

  var SKIP_TAGS = { SCRIPT: 1, STYLE: 1, NOSCRIPT: 1, TEMPLATE: 1, TEXTAREA: 1, HEAD: 1, TITLE: 1 };
  var XHTML = 'http://www.w3.org/1999/xhtml';

  function usableParent(node) {
    var p = node.parentNode;
    if (!p || p.nodeType !== 1) return null;
    if (p.namespaceURI && p.namespaceURI !== XHTML) return null; // SVG / MathML
    for (var e = p; e && e.nodeType === 1; e = e.parentNode) {
      if (SKIP_TAGS[e.nodeName]) return null;
      if (e.hasAttribute && e.hasAttribute('data-cm-ui')) return null;
    }
    return p;
  }

  /* Flat, whitespace-collapsed view of the page text plus a map back into the
     DOM: map[i] = [nodeIndex, offsetInNode]. */
  function buildIndex() {
    var nodes = [], chars = [], map = [], lastSpace = true;
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null, false);
    var n;
    while ((n = walker.nextNode())) {
      if (!n.nodeValue || !usableParent(n)) continue;
      var ni = nodes.push(n) - 1;
      var t = n.nodeValue;
      for (var i = 0; i < t.length; i++) {
        var c = t.charCodeAt(i);
        var ws = c === 32 || c === 9 || c === 10 || c === 13 || c === 12 || c === 160;
        if (ws) {
          if (lastSpace) continue;
          chars.push(' ');
          lastSpace = true;
        } else {
          chars.push(t[i]);
          lastSpace = false;
        }
        map.push([ni, i]);
      }
    }
    return { nodes: nodes, text: chars.join(''), map: map, index: null };
  }

  function nodeIndexOf(idx, node) {
    if (!idx.index) {
      idx.index = new Map();
      for (var i = 0; i < idx.nodes.length; i++) idx.index.set(idx.nodes[i], i);
    }
    var v = idx.index.get(node);
    return v == null ? -1 : v;
  }

  /* Best occurrence of anchor.exact, scored by how well the text around it
     matches the stored prefix / suffix. */
  function findAnchor(idx, anchor) {
    var exact = norm(anchor && anchor.exact);
    if (!exact) return null;
    var hay = idx.text, best = null, bestScore = -1, from = 0, at;
    var pre = norm(anchor.prefix), suf = norm(anchor.suffix);
    while ((at = hay.indexOf(exact, from)) !== -1) {
      var score = 0;
      if (pre) score += tailMatch(hay.slice(Math.max(0, at - pre.length - 4), at), pre);
      if (suf) score += headMatch(hay.slice(at + exact.length, at + exact.length + suf.length + 4), suf);
      if (score > bestScore) { bestScore = score; best = at; }
      from = at + 1;
      if (from > hay.length) break;
    }
    if (best == null) return null;
    return { start: best, end: best + exact.length };
  }

  function tailMatch(a, b) {
    var n = 0, i = a.length - 1, j = b.length - 1;
    while (i >= 0 && j >= 0 && a[i] === b[j]) { n++; i--; j--; }
    return n;
  }

  function headMatch(a, b) {
    var n = 0;
    while (n < a.length && n < b.length && a[n] === b[n]) n++;
    return n;
  }

  /* Per-text-node slices covering [start, end) of the flat index. */
  function slicesFor(idx, start, end) {
    var out = [], cur = null;
    for (var i = start; i < end && i < idx.map.length; i++) {
      var m = idx.map[i];
      if (!cur || cur.ni !== m[0]) {
        cur = { ni: m[0], s: m[1], e: m[1] + 1 };
        out.push(cur);
      } else {
        cur.e = m[1] + 1;
      }
    }
    return out;
  }

  function wrapSlice(node, s, e, thread) {
    var target = node;
    if (e < target.nodeValue.length) target.splitText(e);
    if (s > 0) target = target.splitText(s);
    var m = document.createElement('mark');
    m.className = 'cm-anchor';
    m.setAttribute('data-cm-state', thread.state || 'open');
    m.setAttribute('data-cm-id', thread.id);
    target.parentNode.insertBefore(m, target);
    m.appendChild(target);
    return m;
  }

  function clearAnchors() {
    var marks = document.querySelectorAll('mark.cm-anchor');
    for (var i = 0; i < marks.length; i++) {
      var m = marks[i], p = m.parentNode;
      if (!p) continue;
      while (m.firstChild) p.insertBefore(m.firstChild, m);
      p.removeChild(m);
      p.normalize();
    }
  }

  /* Wrap every resolvable anchor; returns a map id -> first mark element. */
  function applyAnchors(threads) {
    var found = {};
    try {
      clearAnchors();
      var idx = buildIndex();
      var hits = [];
      threads.forEach(function (t) {
        if (!t.anchor) return;
        var a = typeof t.anchor === 'string' ? safeJSON(t.anchor) : t.anchor;
        if (!a) return;
        var r = findAnchor(idx, a);
        if (r) hits.push({ t: t, start: r.start, end: r.end });
      });
      hits.sort(function (a, b) { return b.start - a.start; });  // apply back to front
      var taken = [];
      hits.forEach(function (h) {
        for (var i = 0; i < taken.length; i++) {
          if (h.start < taken[i][1] && taken[i][0] < h.end) return; // overlapping anchors
        }
        taken.push([h.start, h.end]);
        var parts = slicesFor(idx, h.start, h.end), made = [];
        for (var k = parts.length - 1; k >= 0; k--) {
          var p = parts[k];
          made.unshift(wrapSlice(idx.nodes[p.ni], p.s, p.e, h.t));
        }
        if (made.length) found[h.t.id] = made;
      });
    } catch (e) {
      log(e);
    }
    return found;
  }

  function safeJSON(s) {
    try { return JSON.parse(s); } catch (e) { return null; }
  }

  /* ------------------------------------------------- anchor from selection */

  function anchorFromRange(text, range) {
    var exact = norm(text).slice(0, 300);
    if (!exact) return null;
    var a = { exact: exact, prefix: '', suffix: '', heading: headingFor(range.startContainer) };
    try {
      var idx = buildIndex();
      var g = flatOffset(idx, range.startContainer, range.startOffset);
      var at = null;
      if (g != null) {
        var near = idx.text.indexOf(exact, Math.max(0, g - 4));
        at = near === -1 ? idx.text.indexOf(exact) : near;
      } else {
        at = idx.text.indexOf(exact);
      }
      if (at != null && at !== -1) {
        a.prefix = idx.text.slice(Math.max(0, at - 32), at);
        a.suffix = idx.text.slice(at + exact.length, at + exact.length + 32);
      }
    } catch (e) {
      log(e);
    }
    return a;
  }

  function flatOffset(idx, node, offset) {
    var target = node, off = offset;
    if (node.nodeType === 1) {
      target = node.childNodes[offset] || node.lastChild;
      off = 0;
      while (target && target.nodeType === 1) target = target.firstChild;
    }
    if (!target || target.nodeType !== 3) return null;
    var ni = nodeIndexOf(idx, target);
    if (ni < 0) return null;
    for (var i = 0; i < idx.map.length; i++) {
      if (idx.map[i][0] === ni && idx.map[i][1] >= off) return i;
    }
    return null;
  }

  function headingFor(node) {
    var e = node.nodeType === 1 ? node : node.parentNode;
    while (e && e !== document.body) {
      for (var p = e.previousElementSibling; p; p = p.previousElementSibling) {
        if (/^H[1-3]$/.test(p.nodeName)) return norm(p.textContent).slice(0, 120);
        var inner = p.querySelector && p.querySelector('h1, h2, h3');
        if (inner) return norm(inner.textContent).slice(0, 120);
      }
      if (/^H[1-3]$/.test(e.nodeName)) return norm(e.textContent).slice(0, 120);
      e = e.parentNode;
    }
    return '';
  }

  /* --------------------------------------------------------------- writing */

  function viewerName() {
    if (S.viewer.name) return S.viewer.name;
    try { return localStorage.getItem(NAME_KEY) || ''; } catch (e) { return ''; }
  }

  function needsName() {
    return S.viewer.role === 'client' && !viewerName();
  }

  function rememberName(n) {
    try { localStorage.setItem(NAME_KEY, n); } catch (e) { /* private mode */ }
    S.viewer.name = n;
  }

  function post(payload, btn) {
    if (btn) { btn.disabled = true; btn.textContent = 'Sending…'; }
    var body = { page: PAGE, body: payload.body };
    if (payload.anchor) body.anchor = payload.anchor;
    if (payload.parent) body.parent = payload.parent;
    var name = viewerName();
    if (name) body.author = name;
    return req('POST', API, body);
  }

  function patch(id, state) {
    return req('PATCH', API, { id: id, state: state });
  }

  function del(id) {
    return req('DELETE', API + '?id=' + encodeURIComponent(id));
  }

  /* ---------------------------------------------------------------- render */

  function signature() {
    return JSON.stringify([S.viewer, S.threads.map(function (t) {
      return [t.id, t.state, t.updated, t.body, (t.replies || []).map(function (r) { return [r.id, r.body]; })];
    })]);
  }

  function saveDrafts() {
    var fields = document.querySelectorAll('[data-cm-draft]');
    for (var i = 0; i < fields.length; i++) {
      S.drafts[fields[i].getAttribute('data-cm-draft')] = fields[i].value;
    }
    var a = document.activeElement;
    return a && a.getAttribute && a.getAttribute('data-cm-draft');
  }

  function restoreDrafts(focusKey) {
    var fields = document.querySelectorAll('[data-cm-draft]');
    for (var i = 0; i < fields.length; i++) {
      var k = fields[i].getAttribute('data-cm-draft');
      if (S.drafts[k]) fields[i].value = S.drafts[k];
      if (focusKey && k === focusKey) {
        try { fields[i].focus(); fields[i].selectionStart = fields[i].value.length; } catch (e) { /* ignore */ }
      }
    }
  }

  function ensureRoot() {
    if (!root || !root.isConnected) {
      root = ui(el('div', 'cm-root'));
      document.body.appendChild(root);
    }
    if (!floatBox || !floatBox.isConnected) {
      floatBox = ui(el('div', 'cm-float'));
      floatBox.hidden = true;
      root.appendChild(floatBox);
    }
  }

  function teardown() {
    clearAnchors();
    if (root && root.parentNode) root.parentNode.removeChild(root);
    if (generalBox && generalBox.parentNode) generalBox.parentNode.removeChild(generalBox);
    root = floatBox = generalBox = null;
    document.documentElement.classList.remove('cm-rail-on');
  }

  function render() {
    var focusKey = saveDrafts();
    S.sig = signature();

    if (S.viewer.role === 'anon' && !S.threads.length) { teardown(); return; }

    ensureRoot();
    document.querySelectorAll('.cm-inline-host').forEach(function (n) { n.remove(); });

    var anchored = applyAnchors(S.threads);
    markers = [];
    railBubbles = [];

    // markers + rail bubbles live in the document-coordinate layer
    Array.prototype.slice.call(root.children).forEach(function (n) {
      if (n !== floatBox) n.remove();
    });

    var general = [];
    var wide = document.documentElement.clientWidth >= RAIL_MIN;
    var seq = 0;

    S.threads.forEach(function (t) {
      var marks = anchored[t.id];
      var mark = marks && marks[0];
      if (!mark) {
        general.push(t);
        return;
      }
      seq++;
      var m = ui(el('button', 'cm-marker', String(seq)));
      m.type = 'button';
      m.setAttribute('data-cm-state', t.state || 'open');
      m.setAttribute('aria-label', 'Comment ' + seq + ', ' + (t.state || 'open'));
      m.addEventListener('click', guard(function (ev) {
        ev.preventDefault();
        ev.stopPropagation();
        onMarker(t.id);
      }));
      m.addEventListener('mouseenter', guard(function () { setActive(t.id); }));
      m.addEventListener('mouseleave', guard(function () { setActive(null); }));
      root.appendChild(m);
      markers.push({ id: t.id, node: m, mark: mark, endMark: marks[marks.length - 1] });

      mark.addEventListener('click', guard(function () { onMarker(t.id); }));

      if (wide) {
        var b = bubble(t, { rail: true });
        root.appendChild(b);
        railBubbles.push({ id: t.id, node: b, mark: mark });
      } else if (S.openInline === t.id) {
        var host = ui(el('div', 'cm-inline-host'));
        host.appendChild(bubble(t, { inline: true }));
        var block = blockOf(mark);
        if (block && block.parentNode) block.parentNode.insertBefore(host, block.nextSibling);
      }
    });

    renderGeneral(general);
    markRevisions();
    restoreDrafts(focusKey);
    requestAnimationFrame(position);
    S.booted = true;
  }

  function blockOf(node) {
    var e = node.parentNode;
    while (e && e !== document.body) {
      var d = '';
      try { d = getComputedStyle(e).display; } catch (err) { d = 'block'; }
      if (d.indexOf('inline') !== 0) return e;
      e = e.parentNode;
    }
    return node.parentNode;
  }

  function onMarker(id) {
    if (document.documentElement.clientWidth >= RAIL_MIN) {
      setActive(id);
      var found = railBubbles.filter(function (b) { return b.id === id; })[0];
      if (found) found.node.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    } else {
      S.openInline = S.openInline === id ? null : id;
      render();
    }
  }

  function setActive(id) {
    S.active = id;
    document.querySelectorAll('.cm-on').forEach(function (n) { n.classList.remove('cm-on'); });
    document.querySelectorAll('.cm-target-on').forEach(function (n) { n.classList.remove('cm-target-on'); });
    if (!id) return;
    document.querySelectorAll('mark.cm-anchor[data-cm-id="' + id + '"]').forEach(function (n) { n.classList.add('cm-on'); });
    markers.forEach(function (m) { if (m.id === id) m.node.classList.add('cm-on'); });
    railBubbles.forEach(function (b) { if (b.id === id) b.node.classList.add('cm-on'); });
    revisionsFor(id).forEach(function (n) { n.classList.add('cm-target-on'); });
  }

  function revisionsFor(id) {
    return Array.prototype.slice.call(document.querySelectorAll('[data-comment="' + id + '"]'));
  }

  function markRevisions() {
    S.threads.forEach(function (t) {
      revisionsFor(t.id).forEach(function (n) { n.setAttribute('data-cm-state', t.state || 'open'); });
    });
  }

  /* ------------------------------------------------------------- one bubble */

  function bubble(t, opts) {
    var state = t.state || 'open';
    var wrap = ui(el('div', 'cm-bubble' + (opts.inline ? ' cm-inline' : '')));
    wrap.setAttribute('data-cm-state', state);
    wrap.setAttribute('data-cm-thread', t.id);
    wrap.addEventListener('mouseenter', guard(function () { setActive(t.id); }));
    wrap.addEventListener('mouseleave', guard(function () { setActive(null); }));
    wrap.addEventListener('focusin', guard(function () { setActive(t.id); }));

    if (state === 'accepted' && !S.expanded[t.id]) {
      wrap.classList.add('cm-collapsed');
      var line = el('button', 'cm-expand');
      line.type = 'button';
      line.appendChild(pill(state));
      line.appendChild(el('span', 'cm-who', t.author));
      line.appendChild(el('span', 'cm-body', norm(t.body)));
      line.addEventListener('click', guard(function () {
        S.expanded[t.id] = true;
        render();
      }));
      wrap.appendChild(line);
      return wrap;
    }

    var head = el('div', 'cm-head');
    head.appendChild(pill(state));
    head.appendChild(el('span', 'cm-who', t.author));
    head.appendChild(el('span', 'cm-role', t.role));
    head.appendChild(el('span', 'cm-time', rel(t.created)));
    wrap.appendChild(head);

    var a = typeof t.anchor === 'string' ? safeJSON(t.anchor) : t.anchor;
    if (a && a.exact) {
      if (opts.orphan) {
        var q = el('div', 'cm-quote', '“' + norm(a.exact).slice(0, 140) + '”');
        wrap.appendChild(q);
        wrap.appendChild(el('div', 'cm-note', 'anchor text no longer on the page'));
      }
    }

    wrap.appendChild(el('div', 'cm-body', t.body));

    var revs = revisionsFor(t.id);
    if (revs.length) {
      var link = el('button', 'cm-changes', 'Changes on page' + (revs.length > 1 ? ' (' + revs.length + ')' : ''));
      link.type = 'button';
      link.addEventListener('click', guard(function () {
        revs[0].scrollIntoView({ block: 'center', behavior: 'smooth' });
        setActive(t.id);
      }));
      wrap.appendChild(link);
    }

    var replies = t.replies || [];
    if (replies.length) {
      var list = el('div', 'cm-replies');
      replies.forEach(function (r) {
        var item = el('div', 'cm-reply');
        var rh = el('div', 'cm-head');
        rh.appendChild(el('span', 'cm-who', r.author));
        rh.appendChild(el('span', 'cm-role', r.role));
        rh.appendChild(el('span', 'cm-time', rel(r.created)));
        item.appendChild(rh);
        item.appendChild(el('div', 'cm-body', r.body));
        list.appendChild(item);
      });
      wrap.appendChild(list);
    }

    var acts = el('div', 'cm-actions');
    var canReview = S.viewer.role === 'owner' || S.viewer.role === 'client';

    if (S.viewer.canWrite) {
      var replyBtn = el('button', 'cm-btn', 'Reply');
      replyBtn.type = 'button';
      replyBtn.addEventListener('click', guard(function () {
        replyBtn.remove();
        wrap.insertBefore(replyForm(t), acts);
        var f = wrap.querySelector('[data-cm-draft]');
        if (f) f.focus();
      }));
      acts.appendChild(replyBtn);
    }

    if (canReview && (state === 'open' || state === 'addressed')) {
      var acc = el('button', 'cm-btn cm-accept', '✓ Accept');
      acc.type = 'button';
      acc.addEventListener('click', guard(function () {
        acc.disabled = true;
        patch(t.id, 'accepted').then(load).catch(fail(wrap, acc, '✓ Accept'));
      }));
      acts.appendChild(acc);
    }

    if (canReview && state === 'accepted') {
      var re = el('button', 'cm-btn cm-quiet', 'Reopen');
      re.type = 'button';
      re.addEventListener('click', guard(function () {
        re.disabled = true;
        patch(t.id, 'open').then(load).catch(fail(wrap, re, 'Reopen'));
      }));
      acts.appendChild(re);
    }

    if (state === 'accepted' && S.expanded[t.id]) {
      var less = el('button', 'cm-btn cm-quiet', 'Collapse');
      less.type = 'button';
      less.addEventListener('click', guard(function () {
        S.expanded[t.id] = false;
        render();
      }));
      acts.appendChild(less);
    }

    if (S.viewer.role === 'owner') {
      var rm = el('button', 'cm-btn cm-quiet cm-danger cm-spacer', 'Delete');
      rm.type = 'button';
      rm.addEventListener('click', guard(function () {
        if (!window.confirm('Delete this thread?')) return;
        rm.disabled = true;
        del(t.id).then(load).catch(fail(wrap, rm, 'Delete'));
      }));
      acts.appendChild(rm);
    }

    if (acts.children.length) wrap.appendChild(acts);
    return wrap;
  }

  function pill(state) {
    var p = el('span', 'cm-pill', state);
    p.setAttribute('data-cm-state', state);
    return p;
  }

  function fail(wrap, btn, label) {
    return function (e) {
      log(e);
      if (btn) { btn.disabled = false; btn.textContent = label; }
      var line = wrap.querySelector('.cm-error') || el('div', 'cm-error');
      line.className = 'cm-error';
      line.textContent = e.message;
      wrap.appendChild(line);
    };
  }

  function replyForm(t) {
    var form = el('div', 'cm-form');
    var name = nameField(form);
    var ta = el('textarea', 'cm-field');
    ta.setAttribute('data-cm-draft', 'reply:' + t.id);
    ta.placeholder = 'Reply…';
    ta.id = 'cm-reply-' + t.id;
    form.appendChild(ta);
    var row = el('div', 'cm-actions');
    var send = el('button', 'cm-btn cm-primary', 'Reply');
    send.type = 'button';
    send.addEventListener('click', guard(function () {
      var text = ta.value.trim();
      if (!text) { ta.focus(); return; }
      if (name && !commitName(name, form)) return;
      post({ body: text, parent: t.id }, send).then(function () {
        delete S.drafts['reply:' + t.id];
        load();
      }).catch(fail(form, send, 'Reply'));
    }));
    row.appendChild(send);
    var cancel = el('button', 'cm-btn cm-quiet', 'Cancel');
    cancel.type = 'button';
    cancel.addEventListener('click', guard(function () {
      delete S.drafts['reply:' + t.id];
      render();
    }));
    row.appendChild(cancel);
    form.appendChild(row);
    return form;
  }

  /* Client name, asked once and kept in localStorage. */
  function nameField(form) {
    if (!needsName()) return null;
    form.appendChild(el('div', 'cm-label', 'Your name'));
    var input = el('input', 'cm-field');
    input.type = 'text';
    input.maxLength = 40;
    input.placeholder = 'Name shown on your comments';
    input.id = 'cm-name';
    input.setAttribute('data-cm-draft', 'name');
    form.appendChild(input);
    return input;
  }

  function commitName(input, form) {
    var n = input.value.trim().slice(0, 40);
    if (!n) {
      var line = form.querySelector('.cm-error') || el('div', 'cm-error');
      line.className = 'cm-error';
      line.textContent = 'Add your name so the owner knows who commented.';
      form.appendChild(line);
      input.focus();
      return false;
    }
    rememberName(n);
    delete S.drafts.name;
    return true;
  }

  /* ------------------------------------------------------- general comments */

  function renderGeneral(list) {
    var show = list.length || S.viewer.canWrite;
    if (!show) {
      if (generalBox && generalBox.parentNode) generalBox.remove();
      generalBox = null;
      return;
    }
    if (!generalBox || !generalBox.isConnected) {
      generalBox = ui(el('section', 'cm-general'));
      document.body.appendChild(generalBox);
    } else {
      generalBox.textContent = '';
      document.body.appendChild(generalBox); // keep it last
    }

    var head = el('div', 'cm-general-head');
    head.appendChild(el('h2', 'cm-general-title', 'General comments'));
    if (S.viewer.canWrite) {
      var add = el('button', 'cm-btn', 'General comment');
      add.type = 'button';
      add.addEventListener('click', guard(function () {
        S.generalOpen = true;
        render();
      }));
      head.appendChild(add);
    }
    var who = S.viewer.role === 'anon'
      ? 'Reading as a visitor'
      : (viewerName() ? 'Commenting as ' + viewerName() : 'Commenting as a guest');
    head.appendChild(el('span', 'cm-viewer', who));
    generalBox.appendChild(head);

    var body = el('div', 'cm-general-list');
    list.forEach(function (t) {
      body.appendChild(bubble(t, { orphan: !!t.anchor }));
    });
    if (!list.length) body.appendChild(el('div', 'cm-role', 'No general comments yet.'));
    generalBox.appendChild(body);

    if (S.generalOpen && S.viewer.canWrite) {
      generalBox.appendChild(generalForm());
    }
    if (S.error) {
      generalBox.appendChild(el('div', 'cm-error', S.error));
    }
  }

  function generalForm() {
    var form = el('div', 'cm-form');
    form.style.marginTop = '12px';
    var name = nameField(form);
    var ta = el('textarea', 'cm-field');
    ta.setAttribute('data-cm-draft', 'general');
    ta.placeholder = 'A comment about the whole page…';
    ta.id = 'cm-general-body';
    form.appendChild(ta);
    var row = el('div', 'cm-actions');
    var send = el('button', 'cm-btn cm-primary', 'Comment');
    send.type = 'button';
    send.addEventListener('click', guard(function () {
      var text = ta.value.trim();
      if (!text) { ta.focus(); return; }
      if (name && !commitName(name, form)) return;
      post({ body: text }, send).then(function () {
        delete S.drafts.general;
        S.generalOpen = false;
        load();
      }).catch(fail(form, send, 'Comment'));
    }));
    row.appendChild(send);
    var cancel = el('button', 'cm-btn cm-quiet', 'Cancel');
    cancel.type = 'button';
    cancel.addEventListener('click', guard(function () {
      delete S.drafts.general;
      S.generalOpen = false;
      render();
    }));
    row.appendChild(cancel);
    form.appendChild(row);
    return form;
  }

  /* ---------------------------------------------- selection button + composer */

  /* Positioned from the live range: selecting text can make the browser
     scroll, so the document coordinates are read at the last moment and once
     more on the next frame. */
  function showFloat(node, range) {
    ensureRoot();
    floatBox.textContent = '';
    floatBox.hidden = false;
    floatBox.appendChild(node);
    var place = function () {
      try {
        var r = range.getBoundingClientRect();
        if (!r || (!r.width && !r.height)) return;
        var w = floatBox.offsetWidth || RAIL_W;
        var cw = document.documentElement.clientWidth;
        floatBox.style.left = clamp(r.left + (window.scrollX || 0), 8, Math.max(8, cw - w - 12)) + 'px';
        floatBox.style.top = (r.bottom + (window.scrollY || 0) + 8) + 'px';
      } catch (e) { log(e); }
    };
    place();
    requestAnimationFrame(place);
  }

  function hideFloat() {
    if (!floatBox) return;
    floatBox.hidden = true;
    floatBox.textContent = '';
  }

  function checkSelection() {
    if (!S.viewer.canWrite) return;
    var sel = window.getSelection();
    if (!sel || sel.isCollapsed || !sel.rangeCount) {
      if (!S.composer) hideFloat();
      return;
    }
    var range = sel.getRangeAt(0);
    var host = range.commonAncestorContainer;
    host = host.nodeType === 1 ? host : host.parentNode;
    if (!host || (host.closest && host.closest('[data-cm-ui]'))) return;
    if (!norm(sel.toString())) return;

    var rect = range.getBoundingClientRect();
    if (!rect || (!rect.width && !rect.height)) return;
    var snapText = sel.toString();
    var snapRange = range.cloneRange();
    var btn = el('button', 'cm-sel-btn', 'Comment');
    btn.type = 'button';
    btn.addEventListener('mousedown', function (ev) { ev.preventDefault(); });
    btn.addEventListener('click', guard(function () {
      openComposer(anchorFromRange(snapText, snapRange), snapRange);
    }));
    S.composer = null;
    showFloat(btn, range);
  }

  function openComposer(anchor, range) {
    S.composer = { anchor: anchor };
    var box = el('div', 'cm-composer');
    box.appendChild(el('div', 'cm-composer-title', anchor && anchor.heading ? anchor.heading : 'New comment'));
    if (anchor && anchor.exact) {
      box.appendChild(el('div', 'cm-quote', '“' + anchor.exact.slice(0, 140) + (anchor.exact.length > 140 ? '…' : '') + '”'));
    }
    var form = el('div', 'cm-form');
    var name = nameField(form);
    var ta = el('textarea', 'cm-field');
    ta.setAttribute('data-cm-draft', 'composer');
    ta.placeholder = 'What should change here?';
    ta.id = 'cm-composer-body';
    form.appendChild(ta);
    var row = el('div', 'cm-actions');
    var send = el('button', 'cm-btn cm-primary', 'Comment');
    send.type = 'button';
    send.addEventListener('click', guard(function () {
      var text = ta.value.trim();
      if (!text) { ta.focus(); return; }
      if (name && !commitName(name, form)) return;
      post({ body: text, anchor: anchor }, send).then(function () {
        delete S.drafts.composer;
        S.composer = null;
        hideFloat();
        load();
      }).catch(fail(form, send, 'Comment'));
    }));
    row.appendChild(send);
    var cancel = el('button', 'cm-btn cm-quiet', 'Cancel');
    cancel.type = 'button';
    cancel.addEventListener('click', guard(function () {
      delete S.drafts.composer;
      S.composer = null;
      hideFloat();
    }));
    row.appendChild(cancel);
    form.appendChild(row);
    box.appendChild(form);
    showFloat(box, range);
    try { ta.focus({ preventScroll: true }); } catch (e) { ta.focus(); }
  }

  /* -------------------------------------------------------------- position */

  function position() {
    try {
      var wide = document.documentElement.clientWidth >= RAIL_MIN;
      document.documentElement.classList.toggle('cm-rail-on', wide && railBubbles.length > 0);
      var cw = document.documentElement.clientWidth;
      var sy = window.scrollY || window.pageYOffset || 0;
      var sx = window.scrollX || window.pageXOffset || 0;

      var rows = markers.map(function (m) {
        var r = m.mark.getBoundingClientRect();
        // narrow: sit at the end of the anchored phrase, like a footnote mark
        var list = (m.endMark || m.mark).getClientRects();
        var last = list.length ? list[list.length - 1] : r;
        return { m: m, top: r.top + sy, endTop: last.top + sy, endRight: last.right + sx };
      }).sort(function (a, b) { return a.top - b.top; });

      rows.forEach(function (row) {
        var node = row.m.node;
        if (wide) {
          node.style.left = (cw - RESERVE + 4) + 'px';
          node.style.top = (row.top - 2) + 'px';
        } else {
          node.style.left = clamp(row.endRight + 2, 4, cw - 26) + 'px';
          node.style.top = (row.endTop - 2) + 'px';
        }
      });

      if (!wide) return;
      var bubblesByTop = rows.map(function (row) {
        var b = railBubbles.filter(function (x) { return x.id === row.m.id; })[0];
        return b ? { node: b.node, top: row.top } : null;
      }).filter(Boolean);

      var cursor = 0;
      bubblesByTop.forEach(function (item) {
        var top = Math.max(item.top - 2, cursor);
        item.node.style.left = (cw - RAIL_W - 12) + 'px';
        item.node.style.top = top + 'px';
        cursor = top + item.node.offsetHeight + 10;
      });
    } catch (e) {
      log(e);
    }
  }

  /* ----------------------------------------------------------------- wiring */

  var resizeTimer = null;
  window.addEventListener('resize', function () {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(guard(function () {
      var wide = document.documentElement.clientWidth >= RAIL_MIN;
      var hasRail = railBubbles.length > 0;
      if (wide !== hasRail && markers.length) render();
      else position();
    }), 120);
  });

  window.addEventListener('load', function () {
    setTimeout(guard(position), 60);
    setTimeout(guard(position), 600);
  });

  document.addEventListener('mouseup', function (ev) {
    if (ev.target && ev.target.closest && ev.target.closest('[data-cm-ui]')) return;
    setTimeout(guard(checkSelection), 10);
  });
  document.addEventListener('touchend', function () { setTimeout(guard(checkSelection), 10); });
  document.addEventListener('keyup', function (ev) {
    if (ev.key && ev.key.indexOf('Arrow') === 0) setTimeout(guard(checkSelection), 10);
    if (ev.key === 'Escape' && S.composer) { S.composer = null; hideFloat(); }
  });

  setInterval(guard(function () {
    if (document.visibilityState !== 'visible') return;
    req('GET', API + '?page=' + encodeURIComponent(PAGE)).then(function (data) {
      if (!data) return;
      var before = S.sig;
      S.viewer = data.viewer || S.viewer;
      S.threads = Array.isArray(data.threads) ? data.threads : [];
      if (signature() !== before) render();
    }).catch(log);
  }), POLL_MS);

  try {
    load();
  } catch (e) {
    log(e);
  }
})();
