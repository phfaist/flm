  // --- the gestures -------------------------------------------------------------------
  //
  // Two clicks on the document with modifier keys, which act for the author:
  //
  // - with Ctrl or Cmd, and Shift: the place of the clicked element in the source is
  //   opened in the editor, and the element flashes. The place is the one the library
  //   wrote into the element (`spanOf`), or, where it wrote none, that of the nearest
  //   element before it or around it that has one;
  // - with Ctrl or Cmd alone: `\ref{LABEL}` is copied for the nearest element around the
  //   click that a label refers to, and that element flashes; nothing is copied where no
  //   element around the click has a label.
  //
  // A click on the toolbar is the toolbar's own. The labels of the page's elements come
  // with the server's messages (`references`: from an element's identifier to the label
  // that refers to it). *Open in editor* on a problem of the toolbar's list (`problems.js`)
  // sends the same request as the first gesture, with the problem's place. Each request
  // carries the editor chosen in the toolbar's menu of editors (`toolbar.js`), or no editor
  // when the menu lists none, and the server then opens its default. When the server opens
  // nothing, the toolbar shows why.

  // The server's address for a place to open in the editor.
  var OPEN = '/.flm-preview/open';

  // From the identifier of an element of the page to the label that refers to it.
  var references = {};

  // Takes `given`, as a message of the server carries it, as the labels of the page's
  // elements; anything else (`null`: the build gave no page) keeps the ones there are.
  function setReferences(given) {
    if (given !== null && typeof given === 'object' && !Array.isArray(given)) {
      references = given;
    }
  }

  // Asks the server to open the source file `file` at the byte `start`, in the editor
  // chosen in the toolbar's menu.
  function requestOpen(file, start) {
    var place = { file: file, start: start };
    if (view.editor !== null) {
      place.editor = view.editor;
    }
    window.fetch(OPEN, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(place)
    }).then(function (answer) {
      if (answer.ok) {
        tell(null);
        return null;
      }
      return answer.text().then(function (text) {
        tell(text !== '' ? text : 'The editor could not be opened (the server answered with status ' + answer.status + ').');
      });
    }).catch(function () {
      tell('The preview’s server cannot be reached.');
    });
  }

  openPlace = requestOpen;

  // The place `[name, start, end]` of `element` in the source: its own, else that of the
  // nearest element before it, else the same for the element around it, and so on.
  function placeAt(element) {
    for (var node = element; node !== null && node !== document.body; node = node.parentElement) {
      for (var sibling = node; sibling !== null; sibling = sibling.previousElementSibling) {
        var span = spanOf(sibling);
        if (span !== null) {
          return span;
        }
      }
    }
    return null;
  }

  // The nearest element around `element`, itself included, that a label refers to, with
  // that label: `{element, label}`, or `null`.
  function referenceAt(element) {
    for (var node = element; node !== null; node = node.parentElement) {
      if (node.id && Object.prototype.hasOwnProperty.call(references, node.id) && isText(references[node.id])) {
        return { element: node, label: references[node.id] };
      }
    }
    return null;
  }

  // Copies `text`; the toolbar says so when the browser refuses.
  function copy(text) {
    var refused = function () {
      tell('The browser did not allow copying ' + text + ' to the clipboard.');
    };
    if (!navigator.clipboard) {
      refused();
      return;
    }
    navigator.clipboard.writeText(text).then(function () {
      tell(null);
    }, refused);
  }

  // The element a mouse event `event` is a gesture on, or `null` when it is none: the
  // left button with Ctrl or Cmd held, on the document and not on the toolbar.
  function gestureTarget(event) {
    if (event.button !== 0 || !(event.ctrlKey || event.metaKey)) {
      return null;
    }
    var target = event.target instanceof Element ? event.target : event.target.parentElement;
    if (target === null || (view.root !== null && view.root.contains(target))) {
      return null;
    }
    return target;
  }

  // The gesture acts when the button goes down, as the browser's own Ctrl+click does on
  // some systems; it keeps the browser from selecting text and the page's own handlers
  // from seeing that event. The click that follows is the browser's: a Ctrl or Cmd click
  // on a link still opens it in a new tab, as on any page.
  document.addEventListener('mousedown', function (event) {
    var target = gestureTarget(event);
    if (target === null) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.shiftKey) {
      var span = placeAt(target);
      if (span !== null) {
        requestOpen(span[0], span[1]);
        flash(target);
      }
      return;
    }
    var found = referenceAt(target);
    if (found !== null) {
      copy('\\ref{' + found.label + '}');
      flash(found.element);
    }
  }, true);

