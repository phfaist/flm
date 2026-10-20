  // --- the update of the page ---------------------------------------------------------
  //
  // A build's update lists the output regions whose HTML changed (each an element of
  // the page whose identifier the update gives) and the files beside the page that
  // changed, or asks for the page to be loaded anew. A changed region is brought up to
  // date in place: `morphdom` changes the element's content into the new one while
  // keeping every element that is the same in both, so that the reader's place is kept.
  // Then the event `flm:region-updated` is dispatched on the region's element; it
  // reaches `document`.
  //
  // An element a script of the page transformed in the browser carries the attribute
  // `data-flm-original-html`, the HTML it held before. Such an element is left as it is
  // when its new HTML equals that record, and brought up to date otherwise; the script
  // that transformed it transforms it again on the event.

  var morphdom = module.exports;

  // The attribute that records the HTML an element held before a script transformed it.
  var ORIGINAL = 'data-flm-original-html';

  // The event dispatched on a region's element once its content was brought up to date.
  var REGION_UPDATED = 'flm:region-updated';

  function reload() {
    window.location.reload();
  }

  // Brings the region `region` of an update up to date: `{id, name, kind, text}`, its
  // `text` the HTML inside the element `id` (kind `html`) or plain words (kind `words`).
  // `artifacts` are the paths of the files beside the page that changed. Gives `false`
  // when the page has no place for the region (no element with the region's `id`, or
  // words other than the page's title, which is the only place for words that every
  // page has), so that the page must be loaded anew.
  function updateRegion(region, artifacts) {
    if (region.kind === 'words') {
      if (region.name !== 'title') {
        return false;
      }
      document.title = region.text;
      return true;
    }
    var element = document.getElementById(region.id);
    if (element === null) {
      return false;
    }
    var template = document.createElement('template');
    template.innerHTML = region.text;
    var target = element.cloneNode(false);
    target.appendChild(template.content);
    morphdom(element, target, {
      childrenOnly: true,
      onBeforeElUpdated: function (from, to) {
        // An element transformed in the browser whose new HTML equals the record keeps
        // the record, and its content is left alone (below).
        var original = from.getAttribute(ORIGINAL);
        if (original !== null && original === to.innerHTML) {
          to.setAttribute(ORIGINAL, original);
        }
        return true;
      },
      onBeforeElChildrenUpdated: function (from, to) {
        return !(from.hasAttribute(ORIGINAL) && to.hasAttribute(ORIGINAL));
      }
    });
    element.dispatchEvent(new CustomEvent(REGION_UPDATED, {
      bubbles: true,
      detail: { region: region.id, artifacts: artifacts.slice() }
    }));
    return true;
  }

  // The attribute of `element` that holds the address of the file it shows.
  function addressAttribute(element) {
    switch (element.tagName) {
      case 'LINK': return 'href';
      case 'OBJECT': return 'data';
      default: return 'src';
    }
  }

  // Loads anew, where the page shows them, the files at `paths` (paths in the output, as
  // the update lists them): each element that shows one gets its address again, with a
  // query parameter holding the build's number, so that the browser asks the server
  // instead of reusing its cached copy. The server ignores the query.
  function reloadArtifacts(paths, build) {
    if (paths.length === 0) {
      return;
    }
    var root = window.location.origin + '/';
    var changed = paths.map(function (path) {
      return new URL(path, root).href;
    });
    var elements = document.querySelectorAll(
      'img[src], source[src], video[src], audio[src], iframe[src], embed[src], ' +
      'link[href], object[data]');
    Array.prototype.forEach.call(elements, function (element) {
      var name = addressAttribute(element);
      var address = new URL(element.getAttribute(name), window.location.href);
      address.search = '';
      address.hash = '';
      if (changed.indexOf(address.href) !== -1) {
        address.searchParams.set('flm-preview-build', String(build));
        element.setAttribute(name, address.href);
      }
    });
  }

  // Applies the update `update` to the page: loads the page anew when it asks for it or
  // when a region has no place in the page; otherwise brings each changed region up to
  // date and loads the changed files anew. A build that gave no output changes nothing.
  function applyUpdate(update) {
    var artifacts = Array.isArray(update.files) ? update.files.filter(isText) : [];
    if (update.page === 'reload') {
      reload();
      return;
    }
    if (update.page === 'regions' && Array.isArray(update.regions)) {
      var regions = update.regions.filter(function (region) {
        return region !== null && isText(region.id) && isText(region.text);
      });
      for (var i = 0; i < regions.length; i++) {
        if (!updateRegion(regions[i], artifacts)) {
          reload();
          return;
        }
      }
    }
    if (update.page !== 'absent') {
      reloadArtifacts(artifacts, update.build);
    }
  }

  function isText(value) {
    return typeof value === 'string';
  }

