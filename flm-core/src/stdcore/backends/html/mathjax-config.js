/* flm.html.mathjax */
/* The configuration of the formula renderer (MathJax), the script that
   typesets in the browser the formulas that the HTML backend writes into the
   page.  The renderer's program, which the next part of the script
   definition flm.html.mathjax loads, reads this configuration when it
   starts, so the page writes this part first.  The configuration holds what
   the backend's own output needs and nothing that restricts what an author
   may write inside a formula, because FLM does not inspect the content of a
   formula.  A host that renders mathematics written by authors it does not
   trust replaces this part with a configuration of its own; the
   documentation of the Rust module flm_core::stdcore::backends::html::mathjax
   gives that configuration.  After the configuration object come a listener
   that follows a link inside a formula when it is clicked, and the code that
   keeps the formulas typeset while a script of the page updates parts of the
   page in place. */
window.MathJax = {
  loader: {load: ['[tex]/texhtml']},
  tex: {
    packages: {'[+]': ['texhtml']},
    allowTexHTML: true,
    inlineMath: [['\\(', '\\)']],
    displayMath: [['\\[', '\\]']],
    processEnvironments: true,
    processRefs: false,
    tags: 'none'
  },
  chtml: {mtextInheritFont: true},
  startup: {elements: ['.flm-pres-math--passthrough']}
};
/* A reference written in text inside a formula is a link that the HTML
   backend writes into the HTML that the formula carries.  The renderer
   removes the address from every link in its output, keeping it in
   `data-mjx-href`, and follows only the links that it made itself, so a
   click on such a link would otherwise do nothing.  This listener follows
   the link, and acts only on a plain primary click (no modifier key held) on
   one of the backend's links inside a typeset formula. */
document.addEventListener('click', function (event) {
  if (event.defaultPrevented || event.button !== 0 ||
      event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) { return; }
  var from = event.target;
  var link = from && from.closest &&
    from.closest('mjx-container mjx-html a.flm-pres-link[data-mjx-href]');
  if (!link) { return; }
  event.preventDefault();
  window.location.href = link.getAttribute('data-mjx-href');
});
/* A page updated in place.  A script that updates an output region of the
   page (a named part of the page, such as its content) without loading the
   page again follows two conventions: it dispatches the event
   `flm:region-updated` on the region's element once the region's content is
   new, and it leaves unchanged every element that carries the attribute
   `data-flm-original-html` when the new HTML inside that element equals the
   attribute's value.  This code writes that attribute on each formula,
   holding the HTML inside the formula's element, just before the renderer
   typesets the formula; and on the event, it typesets the formulas of the
   region that carry no such attribute: the new formulas, and those whose
   source changed.  Before the renderer has started, there is nothing to do,
   because the renderer typesets every formula of the page when it starts. */
(function () {
  var ORIGINAL = 'data-flm-original-html';
  var formulas = window.MathJax.startup.elements.join(', ');
  function untypeset(root) {
    return Array.prototype.filter.call(root.querySelectorAll(formulas), function (element) {
      return !element.hasAttribute(ORIGINAL);
    });
  }
  function record(elements) {
    elements.forEach(function (element) {
      element.setAttribute(ORIGINAL, element.innerHTML);
    });
    return elements;
  }
  window.MathJax.startup.pageReady = function () {
    record(untypeset(document));
    return window.MathJax.startup.defaultPageReady();
  };
  document.addEventListener('flm:region-updated', function (event) {
    var renderer = window.MathJax;
    if (!renderer.startup || !renderer.startup.promise) { return; }
    renderer.startup.promise = renderer.startup.promise.then(function () {
      var fresh = record(untypeset(event.target));
      if (fresh.length > 0) { return renderer.typesetPromise(fresh); }
    });
  });
})();
