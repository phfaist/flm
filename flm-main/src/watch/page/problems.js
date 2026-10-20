  // --- the problems ---------------------------------------------------------------------
  //
  // The last build's problems, as data and as they show in the page: the elements each
  // problem lies in are marked (the innermost elements whose place in the source, which
  // the library writes in an attribute, overlaps the problem's), and the list shows each
  // problem with its severity, its text and its file, line and column; a click on one
  // scrolls to the first element it lies in and flashes it. Nothing here needs the
  // preview's server, so that a page written without the preview can carry this part
  // too; the toolbar (`toolbar.js`) shows the list as its tool *Problems*.

  // The attribute marking an element a problem lies in; its value is the worst severity
  // of the problems that lie in it.
  var MARK = 'data-flm-preview-problem';

  // The attribute an element carries while it flashes.
  var FLASH = 'data-flm-preview-flash';

  // The attribute in which the library writes an element's place in its source: a JSON
  // array of the source's name and two byte offsets.
  var SOURCE_SPAN = 'data-flm-source-span';

  // The severities, the worst first.
  var SEVERITIES = ['error', 'warning', 'note'];

  // The last build's problems, each as `setProblems` takes it, with `elements`, the
  // elements of the page it lies in.
  var lastProblems = [];

  // The function that opens a place of a source in the editor, `openPlace(file, start)`,
  // when the page has a server to ask (`gestures.js` sets it); without it, the list offers
  // no *Open in editor*. See decision rationale at [§flm-dd-dr:host-watch-open-problem].
  var openPlace = null;

  // An element `tag` with the class `name` (without its prefix) and the text `text`; the
  // toolbar makes its elements with it too.
  function make(tag, name, text) {
    var element = document.createElement(tag);
    if (name) {
      element.className = 'flm-preview-' + name;
    }
    if (text !== undefined) {
      element.textContent = text;
    }
    return element;
  }

  // The worst severity among `problems`, or `null` when there is none.
  function worst(problems) {
    for (var i = 0; i < SEVERITIES.length; i++) {
      for (var j = 0; j < problems.length; j++) {
        if (problems[j].severity === SEVERITIES[i]) {
          return SEVERITIES[i];
        }
      }
    }
    return problems.length > 0 ? 'note' : null;
  }

  // The place of `problem` in its source, as `file:line:column`, as much as is known.
  function placeOf(problem) {
    var parts = [];
    if (isText(problem.file)) {
      parts.push(problem.file);
    }
    if (typeof problem.line === 'number') {
      parts.push(String(problem.line));
      if (typeof problem.column === 'number') {
        parts.push(String(problem.column));
      }
    }
    return parts.join(':');
  }

  // Shows the list of the last build's problems in `panel`.
  function showProblems(panel) {
    if (lastProblems.length === 0) {
      panel.appendChild(make('p', 'empty', 'The last build reported no problems.'));
      return;
    }
    var list = make('ol', 'problems');
    lastProblems.forEach(function (problem) {
      list.appendChild(problemItem(problem));
    });
    panel.appendChild(list);
  }

  // One problem of the list: a button that shows where it lies in the page, and, for a
  // problem with a place in a source when a place can be opened, *Open in editor*.
  function problemItem(problem) {
    var item = make('li', 'problem');
    item.setAttribute('data-flm-preview-severity', problem.severity);
    var show = make('button', 'problem-show');
    show.type = 'button';
    show.appendChild(make('span', 'severity', problem.severity));
    var place = placeOf(problem);
    if (place !== '') {
      show.appendChild(make('span', 'place', place));
    }
    show.appendChild(make('span', 'text', problem.text));
    show.addEventListener('click', function () {
      if (problem.elements.length > 0) {
        problem.elements[0].scrollIntoView({ block: 'center' });
        flash(problem.elements[0]);
      }
    });
    item.appendChild(show);
    if (openPlace !== null && isText(problem.file) && typeof problem.start === 'number') {
      var open = make('button', 'problem-open', 'Open in editor');
      open.type = 'button';
      open.addEventListener('click', function () {
        openPlace(problem.file, problem.start);
      });
      item.appendChild(open);
    }
    return item;
  }

  // The animation the stylesheet runs on an element that carries `FLASH`.
  var FLASH_ANIMATION = 'flm-preview-flash';

  // Makes `element` flash once. The attribute comes off when the animation ends, so that
  // the flash lasts as long as the stylesheet says.
  function flash(element) {
    element.removeAttribute(FLASH);
    // Read a layout value, so that the browser starts the animation anew.
    void element.offsetWidth;
    element.setAttribute(FLASH, '');
  }

  document.addEventListener('animationend', function (event) {
    if (event.animationName === FLASH_ANIMATION && event.target.hasAttribute(FLASH)) {
      event.target.removeAttribute(FLASH);
    }
  });

  // The place `[name, start, end]` an element's attribute holds, or `null`.
  function spanOf(element) {
    try {
      var span = JSON.parse(element.getAttribute(SOURCE_SPAN));
      if (Array.isArray(span) && span.length === 3 && isText(span[0]) &&
          typeof span[1] === 'number' && typeof span[2] === 'number') {
        return span;
      }
    } catch (error) {
      // An attribute that is not understood places nothing.
    }
    return null;
  }

  // The elements `problem` lies in: among the elements whose place in the source overlaps
  // the problem's bytes (a problem that covers no bytes counts as covering the byte at its
  // position), those that contain no other such element.
  function elementsOf(problem, placed) {
    if (!isText(problem.file) || typeof problem.start !== 'number' ||
        typeof problem.end !== 'number') {
      return [];
    }
    var start = problem.start;
    var end = Math.max(problem.end, problem.start + 1);
    var overlapping = placed.filter(function (entry) {
      return entry.span[0] === problem.file && entry.span[1] < end && start < entry.span[2];
    }).map(function (entry) {
      return entry.element;
    });
    return overlapping.filter(function (element) {
      return !overlapping.some(function (other) {
        return other !== element && element.contains(other);
      });
    });
  }

  // Takes `problems` (each as the update gives it) as the last build's and marks the
  // elements each lies in; what lists them (the toolbar's `render`) shows them anew.
  function setProblems(problems) {
    var marked = document.querySelectorAll('[' + MARK + ']');
    Array.prototype.forEach.call(marked, function (element) {
      element.removeAttribute(MARK);
    });
    var placed = [];
    Array.prototype.forEach.call(document.querySelectorAll('[' + SOURCE_SPAN + ']'), function (element) {
      var span = spanOf(element);
      if (span !== null) {
        placed.push({ element: element, span: span });
      }
    });
    lastProblems = problems.filter(function (problem) {
      return problem !== null && typeof problem === 'object' && isText(problem.text);
    }).map(function (problem) {
      var shown = {
        severity: isText(problem.severity) ? problem.severity : 'error',
        text: problem.text,
        file: problem.file,
        line: problem.line,
        column: problem.column,
        start: problem.start,
        end: problem.end
      };
      shown.elements = elementsOf(shown, placed);
      return shown;
    });
    lastProblems.forEach(function (problem) {
      problem.elements.forEach(function (element) {
        var now = element.getAttribute(MARK);
        if (now === null || SEVERITIES.indexOf(problem.severity) < SEVERITIES.indexOf(now)) {
          element.setAttribute(MARK, problem.severity);
        }
      });
    });
  }
