(function () {
  function escapeHtml(value) {
    return value
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  function ensureMount(block) {
    var sibling = block.nextElementSibling;
    if (sibling && sibling.hasAttribute('data-aoc-see-mermaid-output')) {
      sibling.className = 'mermaid-rendered';
      return sibling;
    }

    var mount = document.createElement('div');
    mount.className = 'mermaid-rendered';
    mount.setAttribute('data-aoc-see-mermaid-output', 'true');
    block.insertAdjacentElement('afterend', mount);
    return mount;
  }

  async function resolveSource(block) {
    var inlineSource = (block.textContent || '').trim();
    if (inlineSource) return inlineSource;

    var src = block.getAttribute('data-aoc-see-mermaid-src');
    if (!src) return '';

    var response = await fetch(src, { cache: 'no-cache' });
    if (!response.ok) {
      throw new Error('Unable to load Mermaid source: ' + src + ' (' + response.status + ')');
    }
    return (await response.text()).trim();
  }

  async function renderMermaidBlocks() {
    if (!window.mermaid) return;

    var blocks = Array.prototype.slice.call(
      document.querySelectorAll('script[data-aoc-see-mermaid], script[data-aoc-see-mermaid-src]')
    );
    if (!blocks.length) return;

    window.mermaid.initialize({
      startOnLoad: false,
      theme: 'dark',
      securityLevel: 'loose',
      fontFamily: 'Inter, ui-sans-serif, system-ui, sans-serif',
      flowchart: {
        useMaxWidth: true,
        htmlLabels: true,
        curve: 'basis'
      }
    });

    for (var index = 0; index < blocks.length; index += 1) {
      var block = blocks[index];
      var mount = ensureMount(block);

      try {
        var source = await resolveSource(block);
        if (!source) {
          mount.innerHTML = '';
          continue;
        }

        var renderId = 'aoc-see-mermaid-' + index + '-' + Math.random().toString(36).slice(2, 8);
        var result = await window.mermaid.render(renderId, source);
        mount.innerHTML = result.svg;
      } catch (error) {
        var message = error && error.message ? error.message : String(error);
        mount.innerHTML =
          '<div style="border:1px solid #5b2b2b;background:#1a1114;color:#ffcabf;border-radius:16px;padding:14px;">' +
          '<strong style="display:block;color:#fff;margin-bottom:8px;">Mermaid render error</strong>' +
          '<pre style="margin:0;white-space:pre-wrap;font:12px/1.5 ui-monospace,SFMono-Regular,Menlo,monospace;">' +
          escapeHtml(message) +
          '</pre>' +
          '</div>';
      }
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', renderMermaidBlocks);
  } else {
    renderMermaidBlocks();
  }
})();
