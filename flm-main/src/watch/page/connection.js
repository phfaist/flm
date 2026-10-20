  // --- the connection -----------------------------------------------------------------
  //
  // The page listens to the stream of messages of the preview's server, a stream of
  // server-sent events the browser connects to again by itself when the connection is
  // lost (for instance when `flm watch` is started again). The messages:
  //
  // - `hello`, first on every connection: `{server, document, build, building, problems,
  //   references, editors, editor}` — which server, the document's file as the command
  //   line gave it, the number of the build whose output the server holds (`null` before
  //   the first output), whether a build is under way, the last build's problems, the
  //   labels of the elements of the output it holds (`gestures.js`), and the editors the
  //   server found with its default (the toolbar's menu of editors, `toolbar.js`);
  // - `building`, a build starts: `{changed}`;
  // - `update`, a build is done: the update (`regions.js`), with the build's number and
  //   its problems.
  //
  // A message that is not understood — not JSON, or without the fields that say what it
  // is — changes nothing.
  //
  // The page knows which server and which build it shows from the HTTP response that
  // carried it: the server gives them in the response's header `Server-Timing` (the
  // metrics `flm-server` and `flm-build`, whose descriptions are the server's name and the
  // build's number), which the browser exposes as the navigation entry's `serverTiming`. A
  // page that shows another server or build than the one `hello` gives loads itself anew.
  // A page whose browser does not expose that header takes `hello`'s server and build as
  // the ones it shows.

  // The server's own addresses.
  var EVENTS = '/.flm-preview/events';
  var REBUILD = '/.flm-preview/rebuild';

  // The attribute of the page the server serves while it holds no output of the
  // document: that page shows no build.
  var WAITING = 'data-flm-preview-waiting';

  var waiting = document.documentElement.hasAttribute(WAITING);

  // The server and the build that the HTTP response carrying this page gives:
  // `{server, build}`, or `null` when the browser does not expose them or the response
  // gives none (the waiting page).
  function servedBy() {
    var entries = window.performance && performance.getEntriesByType
      ? performance.getEntriesByType('navigation') : [];
    var timing = entries.length > 0 && Array.isArray(entries[0].serverTiming) ? entries[0].serverTiming : [];
    var server = null;
    var build = null;
    timing.forEach(function (metric) {
      if (metric.name === 'flm-server') {
        server = metric.description;
      } else if (metric.name === 'flm-build' && /^[0-9]+$/.test(metric.description)) {
        build = Number(metric.description);
      }
    });
    return isText(server) && build !== null ? { server: server, build: build } : null;
  }

  // What this page shows: the server that served it, and the number of the build whose
  // output it shows; `null` until the HTTP response or the first message gave them.
  var shown = waiting ? null : servedBy();

  // The message `text`, read as JSON, or `null` when it is not understood.
  function read(text) {
    try {
      var message = JSON.parse(text);
      return message !== null && typeof message === 'object' && !Array.isArray(message) ? message : null;
    } catch (error) {
      return null;
    }
  }

  // Asks the server to build the document again now; the build that follows is shown
  // by the messages it causes.
  function requestRebuild() {
    window.fetch(REBUILD, { method: 'POST' }).catch(function (error) {
      console.error('The preview’s server cannot be reached:', error);
    });
  }

  // Runs `handle` on the message of `event`, read as JSON; a message that is not
  // understood, or that `handle` cannot apply, changes nothing more.
  function listen(source, name, handle) {
    source.addEventListener(name, function (event) {
      var message = read(event.data);
      if (message === null) {
        console.error('The preview cannot read the message `' + name + '` from its server.');
        return;
      }
      try {
        handle(message);
      } catch (error) {
        console.error('The preview cannot apply the message `' + name + '` from its server:', error);
      }
    });
  }

  function problemsOf(message) {
    return Array.isArray(message.problems) ? message.problems : [];
  }

  // The ways an update can ask the page to change: see `regions.js`.
  var PAGE_CHANGES = ['same', 'regions', 'reload', 'absent'];

  function isHello(hello) {
    return isText(hello.server) && (typeof hello.build === 'number' || hello.build === null);
  }

  function isUpdate(update) {
    return typeof update.build === 'number' && PAGE_CHANGES.indexOf(update.page) !== -1;
  }

  function start() {
    makeToolbar(waiting ? 'problems' : null);
    var source = new EventSource(EVENTS);

    listen(source, 'hello', function (hello) {
      if (!isHello(hello)) {
        console.error('The preview cannot read the message `hello` from its server.');
        return;
      }
      if (isText(hello.document)) {
        view.document = hello.document;
      }
      var build = hello.build;
      // A page that shows another build than the one the server holds — the server was
      // started again, or a build came while the page was not connected — loads itself
      // anew; so does the waiting page once the server holds an output.
      if (waiting ? build !== null
                  : shown !== null && (shown.server !== hello.server || shown.build !== build)) {
        reload();
        return;
      }
      if (!waiting && shown === null) {
        shown = { server: hello.server, build: build };
      }
      view.building = hello.building === true;
      setEditors(hello.editors, hello.editor);
      setReferences(hello.references);
      setProblems(problemsOf(hello));
      render();
    });

    listen(source, 'building', function () {
      view.building = true;
      render();
    });

    listen(source, 'update', function (update) {
      if (!isUpdate(update)) {
        console.error('The preview cannot read the message `update` from its server.');
        return;
      }
      view.building = false;
      if (waiting) {
        if (update.page !== 'absent') {
          reload();
          return;
        }
      } else {
        applyUpdate(update);
        if (update.page !== 'absent' && shown !== null) {
          shown.build = update.build;
        }
      }
      setReferences(update.references);
      setProblems(problemsOf(update));
      render();
    });
  }

  if (document.body === null) {
    document.addEventListener('DOMContentLoaded', start);
  } else {
    start();
  }
