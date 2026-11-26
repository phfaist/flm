/* Enhance the existing FLM output. No content generation or code execution. */
(() => {
  const paper = document.querySelector('.nb-paper');
  if (!paper) return;
  const foldStates = new Map();
  let entries = [];
  let pending = false;

  function markCurrent() {
    pending = false;
    if (!entries.length) return;
    let current = entries[0];
    for (const entry of entries) {
      if (entry.target.getBoundingClientRect().top <= 100) current = entry;
    }
    if (window.scrollY > 0 && Math.ceil(window.scrollY + window.innerHeight) >= document.documentElement.scrollHeight - 2) {
      current = entries[entries.length - 1];
    }
    for (const entry of entries) {
      if (entry === current) entry.link.setAttribute('aria-current', 'location');
      else entry.link.removeAttribute('aria-current');
    }
  }

  function scheduleCurrent() {
    if (pending) return;
    pending = true;
    requestAnimationFrame(markCurrent);
  }

  function setup() {
    entries = Array.from(document.querySelectorAll('.nb-sidebar .flm-toc a[href^="#"]'))
      .map(link => {
        let id;
        try { id = decodeURIComponent(link.hash.slice(1)); }
        catch { return null; }
        const target = document.getElementById(id);
        return target && paper.contains(target) ? { link, target } : null;
      })
      .filter(Boolean);

    if (document.body.dataset.nbFoldCode === 'true') {
      paper.querySelectorAll('pre.flm-pres-verbatim[data-flm-kind="code"][data-flm-display="block"]').forEach((pre, index) => {
        if (pre.parentElement.classList.contains('nb-code')) return;
        const details = document.createElement('details');
        details.className = 'nb-code';
        details.open = foldStates.get(index) ?? true;
        const summary = document.createElement('summary');
        summary.textContent = document.body.dataset.nbCodeLabel;
        pre.before(details);
        details.append(summary, pre);
        details.addEventListener('toggle', () => foldStates.set(index, details.open));
      });
    }
    scheduleCurrent();
  }

  setup();
  window.addEventListener('scroll', scheduleCurrent, { passive: true });
  window.addEventListener('resize', scheduleCurrent);
  window.addEventListener('hashchange', scheduleCurrent);
  document.addEventListener('flm:region-updated', setup);

  // Print the whole document, including code the reader folded on screen.
  let closedForPrint = [];
  window.addEventListener('beforeprint', () => {
    closedForPrint = Array.from(paper.querySelectorAll('details.nb-code:not([open])'));
    for (const details of closedForPrint) details.open = true;
  });
  window.addEventListener('afterprint', () => {
    for (const details of closedForPrint) details.open = false;
    closedForPrint = [];
  });
})();
