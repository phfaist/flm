/* Progressive reader enhancements. FLM owns content, numbering, references,
   syntax colors, and math rendering; this script owns only reader controls. */
(() => {
  'use strict';
  const config = JSON.parse(document.getElementById('bp-config').textContent);
  const root = document.documentElement;
  const storageKey = 'flm-blogpost-preferences-v1';
  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
  const systemDark = matchMedia('(prefers-color-scheme: dark)');
  const printMedia = matchMedia('print');
  const animations = new Set();
  const choose = (value, values, fallback) => values.includes(value) ? value : fallback;
  let saved = {};
  if (config.remember_preferences) {
    try {
      const value = JSON.parse(localStorage.getItem(storageKey));
      if (value && typeof value === 'object' && !Array.isArray(value)) saved = value;
    } catch { /* Storage is optional, including for local files. */ }
  }
  const prefs = {
    theme: choose(saved.theme, ['light', 'dark', 'system'], choose(config.theme, ['light', 'dark', 'system'], 'light')),
    textSize: choose(saved.textSize, ['small', 'standard', 'large'], choose(config.text_size, ['small', 'standard', 'large'], 'standard')),
    motion: choose(saved.motion, ['subtle', 'none'], config.motion)
  };
  const allowsMotion = config.motion !== 'none' && (config.section_reveal || config.heading_accent || config.smooth_scroll);
  let controls = null;
  function cancelEntrances() {
    for (const animation of animations) animation.cancel();
    animations.clear();
  }
  function applyPreferences() {
    const theme = prefs.theme === 'system' ? (systemDark.matches ? 'dark' : 'light') : prefs.theme;
    const wasMoving = root.dataset.bpMotion === 'on';
    const moving = allowsMotion && prefs.motion === 'subtle' && !reducedMotion.matches && !printMedia.matches;
    root.dataset.bpTheme = theme;
    root.dataset.bpTextSize = prefs.textSize;
    root.dataset.bpMotion = moving ? 'on' : 'off';
    root.dataset.bpReveal = moving && config.section_reveal ? 'on' : 'off';
    root.dataset.bpHeadingAccent = moving && config.heading_accent ? 'on' : 'off';
    root.dataset.bpSmoothScroll = moving && config.smooth_scroll ? 'on' : 'off';
    if (!moving) {
      cancelEntrances();
      if (wasMoving) window.scrollTo({ top: scrollY, left: scrollX, behavior: 'instant' });
    }
    if (controls) {
      const label = theme === 'dark' ? config.label_light_theme : config.label_dark_theme;
      controls.theme.setAttribute('aria-label', label);
      controls.theme.title = label;
      controls.theme.querySelector('.bp-moon').hidden = theme === 'dark';
      controls.theme.querySelector('.bp-sun').hidden = theme !== 'dark';
      for (const button of controls.sizes) button.setAttribute('aria-pressed', String(button.dataset.bpSize === prefs.textSize));
      controls.motion.checked = moving;
      controls.motion.disabled = reducedMotion.matches || !allowsMotion;
      controls.hint.textContent = reducedMotion.matches ? config.label_reduced_motion : !allowsMotion ? config.label_disabled_motion : '';
    }
  }
  function remember() {
    if (!config.remember_preferences) return;
    try { localStorage.setItem(storageKey, JSON.stringify(prefs)); } catch { /* Optional. */ }
  }
  // Theme and fixed-size choices are applied before the first paint.
  applyPreferences();
  reducedMotion.addEventListener('change', applyPreferences);
  systemDark.addEventListener('change', applyPreferences);
  printMedia.addEventListener('change', applyPreferences);

  function initialize() {
    const content = document.getElementById('flm-content');
    const region = document.getElementById('flm-region-content');
    const article = document.querySelector('.bp-article');
    const status = document.getElementById('bp-status');
    const announce = text => { status.textContent = text; };
    const tools = document.querySelector('.bp-tools');
    const dialog = document.querySelector('.bp-figure-dialog');
    const contents = document.querySelector('.bp-contents');
    const progress = document.querySelector('.bp-progress');
    const copyTemplate = document.getElementById('bp-copy-button');
    const expandTemplate = document.getElementById('bp-expand-button');
    const headingTemplate = document.getElementById('bp-heading-link');
    const narrow = matchMedia('(max-width: 900px)');
    const seen = new WeakSet();
    const copyTimers = new Map();
    let headings = [];
    let entries = [];
    let framePending = false;
    let navigating = Boolean(location.hash) || ['back_forward', 'reload'].includes(performance.getEntriesByType('navigation')[0]?.type);
    let navigationTimer;

    function endNavigation() { navigating = false; clearTimeout(navigationTimer); }
    function settleNavigation(delay = 180) {
      clearTimeout(navigationTimer);
      navigationTimer = setTimeout(endNavigation, delay);
    }
    if (navigating) settleNavigation();
    function closeSettings(focus = false) {
      if (!controls) return;
      controls.panel.hidden = true;
      controls.toggle.setAttribute('aria-expanded', 'false');
      if (focus) controls.toggle.focus();
    }
    if (tools) {
      controls = {
        panel: tools.querySelector('.bp-settings'), toggle: tools.querySelector('.bp-settings-toggle'),
        theme: tools.querySelector('.bp-theme-toggle'), motion: tools.querySelector('#bp-motion-toggle'),
        sizes: [...tools.querySelectorAll('[data-bp-size]')], hint: tools.querySelector('#bp-motion-hint')
      };
      controls.toggle.addEventListener('click', () => {
        const opening = controls.panel.hidden;
        controls.panel.hidden = !opening;
        controls.toggle.setAttribute('aria-expanded', String(opening));
        if (opening) controls.panel.querySelector('[aria-pressed="true"]').focus();
      });
      controls.theme.addEventListener('click', () => {
        prefs.theme = root.dataset.bpTheme === 'dark' ? 'light' : 'dark';
        applyPreferences(); remember();
      });
      controls.motion.addEventListener('change', () => {
        prefs.motion = controls.motion.checked ? 'subtle' : 'none';
        applyPreferences(); remember();
      });
      for (const button of controls.sizes) button.addEventListener('click', () => {
        prefs.textSize = button.dataset.bpSize;
        applyPreferences(); remember();
      });
      document.addEventListener('keydown', event => {
        if (event.key === 'Escape' && !controls.panel.hidden) closeSettings(true);
      });
      for (const name of ['click', 'focusin']) document.addEventListener(name, event => {
        if (!controls.panel.hidden && !controls.panel.contains(event.target) && !controls.toggle.contains(event.target)) closeSettings();
      });
      applyPreferences();
      tools.hidden = false;
    }
    const fitContents = () => { contents.open = !narrow.matches; };
    fitContents();
    narrow.addEventListener('change', fitContents);
    contents.addEventListener('click', event => {
      if (narrow.matches && event.target.closest('.flm-toc a')) contents.open = false;
    });

    function updateReading() {
      framePending = false;
      const headerHeight = document.querySelector('.bp-masthead').offsetHeight;
      if (progress) {
        const rect = article.getBoundingClientRect();
        const range = Math.max(1, rect.height - (innerHeight - headerHeight));
        const percentage = Math.max(0, Math.min(100, (headerHeight - rect.top) / range * 100));
        progress.firstElementChild.style.transform = `scaleX(${percentage / 100})`;
        progress.setAttribute('aria-valuenow', String(Math.round(percentage)));
      }
      let currentEntry = entries[0];
      for (const entry of entries) if (entry.target.getBoundingClientRect().top <= headerHeight + 100) currentEntry = entry;
      for (const entry of entries) {
        if (entry === currentEntry) entry.link.setAttribute('aria-current', 'location');
        else entry.link.removeAttribute('aria-current');
      }
      const primary = headings.filter(heading => heading.dataset.flmLevel === '1');
      let currentHeading = primary[0];
      for (const heading of primary) if (heading.getBoundingClientRect().top <= headerHeight + 100) currentHeading = heading;
      for (const heading of primary) heading.toggleAttribute('data-bp-reading', heading === currentHeading);
    }
    function scheduleReading() {
      if (framePending) return;
      framePending = true;
      requestAnimationFrame(updateReading);
    }
    document.addEventListener('scroll', () => {
      scheduleReading();
      if (navigating) settleNavigation();
    }, { passive: true });
    window.addEventListener('resize', scheduleReading, { passive: true });
    if ('ResizeObserver' in window) new ResizeObserver(scheduleReading).observe(article);
    document.addEventListener('scrollend', endNavigation);
    window.addEventListener('hashchange', () => { navigating = true; settleNavigation(); scheduleReading(); });
    document.addEventListener('click', event => {
      if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      const anchor = event.target.closest('a[href]');
      if (!anchor) return;
      const destination = new URL(anchor.href, document.baseURI);
      if (!destination.hash || destination.origin !== location.origin || destination.pathname !== location.pathname || destination.search !== location.search) return;
      navigating = true;
      cancelEntrances();
      settleNavigation(1500);
    });

    // An entrance never changes the DOM or leaves a hidden state behind.
    const observer = 'IntersectionObserver' in window ? new IntersectionObserver(records => {
      for (const record of records) {
        if (!record.isIntersecting) continue;
        observer.unobserve(record.target);
        seen.add(record.target);
        if (root.dataset.bpReveal !== 'on' || navigating || !record.target.animate || record.target.contains(document.activeElement) || record.target.getBoundingClientRect().top < innerHeight * .4) continue;
        const animation = record.target.animate([
          { opacity: 0, transform: 'translateY(16px)' }, { opacity: 1, transform: 'translateY(0)' }
        ], { duration: 560, easing: 'cubic-bezier(.2,.7,.2,1)' });
        animations.add(animation);
        animation.finished.then(() => animations.delete(animation), () => animations.delete(animation));
      }
    }, { threshold: 0, rootMargin: '0px 0px -36px 0px' }) : null;

    function prepareCode() {
      for (const pre of content.querySelectorAll('pre.flm-pres-verbatim[data-flm-kind="code"][data-flm-display="block"]')) {
        if (!pre.hasAttribute('tabindex')) pre.tabIndex = 0;
        if (!copyTemplate || !navigator.clipboard?.writeText || pre.parentElement.classList.contains('bp-code')) continue;
        const wrapper = document.createElement('div');
        wrapper.className = 'bp-code';
        const button = copyTemplate.content.firstElementChild.cloneNode(true);
        button.dataset.bpUi = '';
        pre.before(wrapper);
        wrapper.append(button, pre);
        button.addEventListener('click', async () => {
          try {
            await navigator.clipboard.writeText(pre.textContent);
            clearTimeout(copyTimers.get(button));
            button.setAttribute('data-copied', '');
            button.querySelector('.bp-copy-idle').hidden = true;
            button.querySelector('.bp-copy-done').hidden = false;
            button.setAttribute('aria-label', config.label_copied);
            button.title = config.label_copied;
            announce(config.label_copied);
            copyTimers.set(button, setTimeout(() => {
              button.removeAttribute('data-copied');
              button.querySelector('.bp-copy-idle').hidden = false;
              button.querySelector('.bp-copy-done').hidden = true;
              button.setAttribute('aria-label', config.label_copy);
              button.title = config.label_copy;
              copyTimers.delete(button);
            }, 2000));
          } catch { announce(config.label_copy_unavailable); }
        });
      }
    }
    function prepareFigures() {
      if (!dialog || !expandTemplate || typeof dialog.showModal !== 'function') return;
      for (const figure of document.querySelectorAll('.bp-content .flm-floats-float[data-flm-kind~="figure"], .bp-lead')) {
        const images = figure.querySelectorAll('.flm-floats-float-content img, :scope > img');
        if (images.length !== 1 || figure.querySelector(':scope > .bp-expand')) continue;
        const image = images[0];
        const button = expandTemplate.content.firstElementChild.cloneNode(true);
        button.dataset.bpUi = '';
        figure.append(button);
        button.addEventListener('click', () => {
          const clone = image.cloneNode(false);
          clone.removeAttribute('id');
          clone.loading = 'eager';
          dialog.querySelector('.bp-dialog-image').replaceChildren(clone);
          const caption = figure.querySelector('figcaption');
          const text = dialog.querySelector('.bp-dialog-caption');
          text.textContent = caption?.textContent.trim() || image.alt;
          text.hidden = !text.textContent;
          dialog.showModal();
        });
      }
    }
    if (dialog) {
      dialog.querySelector('.bp-close').addEventListener('click', () => dialog.close());
      dialog.addEventListener('click', event => {
        if (event.target !== dialog) return;
        const rect = dialog.getBoundingClientRect();
        if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) dialog.close();
      });
    }
    function refresh() {
      cancelEntrances();
      if (dialog?.open) dialog.close();
      observer?.disconnect();
      for (const [button, timer] of copyTimers) if (!button.isConnected) { clearTimeout(timer); copyTimers.delete(button); }
      headings = [...content.querySelectorAll('.flm-pres-block--heading[id]')];
      entries = [...contents.querySelectorAll('.flm-toc a[href^="#"]')].map(link => {
        let id;
        try { id = decodeURIComponent(link.hash.slice(1)); } catch { return null; }
        const target = document.getElementById(id);
        return target && content.contains(target) ? { link, target } : null;
      }).filter(Boolean);
      const time = document.querySelector('.bp-reading-time');
      if (time) {
        const words = region.textContent.trim().split(/\s+/u).filter(Boolean).length;
        time.textContent = `${Math.max(1, Math.ceil(words / 210))} ${config.label_minutes}`;
        time.hidden = words === 0;
      }
      if (headingTemplate) for (const heading of headings) {
        if (heading.querySelector(':scope > .bp-heading-link')) continue;
        const link = headingTemplate.content.firstElementChild.cloneNode(true);
        link.href = `#${encodeURIComponent(heading.id)}`;
        link.dataset.bpUi = '';
        heading.append(link);
      }
      prepareCode();
      prepareFigures();
      if (observer) for (const element of region.children) {
        if (!element.matches('.flm-pres-par, .flm-pres-block--heading, .flm-pres-block, .flm-pres-verbatim[data-flm-display="block"], .bp-code, [data-flm-display="display"]') || seen.has(element)) continue;
        if (element.getBoundingClientRect().top < innerHeight - 36) seen.add(element);
        else observer.observe(element);
      }
      if (progress) progress.hidden = false;
      scheduleReading();
    }
    refresh();
    document.addEventListener('flm:region-updated', refresh);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', initialize, { once: true });
  else initialize();
})();
