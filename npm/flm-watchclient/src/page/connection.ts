// The connection to the preview's server, which drives the other parts of the page
// script. The page listens to the stream of the server's messages, a stream of
// server-sent events the browser connects to again by itself when the connection is lost
// (for instance when `flm watch` is started again):
//
// - `hello`, first on every connection: which server, the number of the build whose
//   output the server holds (`null` before the first output), the last build's problems
//   and the labels of the page's elements;
// - `update`, a build is done: the update (`regions.ts`), with the build's number, its
//   problems and labels;
// - `viewer`, the viewer settings changed. Of these, as of those `hello` carries, the page
//   script reads whether it scrolls to what an update changed and flashes it.
//
// A message that is not understood changes nothing.
//
// The page knows which server and which build it shows from the HTTP response that
// carried it: the server gives them in the response's header `Server-Timing`, which the
// browser exposes as the navigation entry's `serverTiming`. A page that shows another
// server or build than the one `hello` gives loads itself anew. A page whose browser does
// not expose that header takes `hello`'s server and build as the ones it shows.

import type { Problem } from '../shared/protocol';
import { EVENTS, on, readHello, readUpdate, servedBy } from '../shared/protocol';
import { endFlashes, show } from './flash';
import { startGestures } from './gestures';
import { documentChanged, inspectPlace, setInspect, setReferences, startInspect } from './inspect';
import { mark } from './marks';
import { elementsOf, placedElements } from './places';
import { applyUpdate, reload } from './regions';
import { listen } from './tools';
import { DEFAULT_VIEWER, readViewer } from '../shared/viewer';

/**
 * The attribute of the page the server serves while it holds no output of the document:
 * that page shows no build.
 */
const WAITING = 'data-flm-preview-waiting';

/** The server and the build the HTTP response carrying this page names, or `null`. */
function servedByNavigation(): { server: string; build: number } | null {
  const entries = typeof performance.getEntriesByType === 'function' ? performance.getEntriesByType('navigation') : [];
  const entry = entries[0] as PerformanceNavigationTiming | undefined;
  return entry !== undefined && Array.isArray(entry.serverTiming) ? servedBy(entry.serverTiming) : null;
}

/** Marks the elements `problems` lie in. */
function markProblems(problems: readonly Problem[]): void {
  mark(problems, placedElements(document));
}

/** Starts the page script. */
export function start(): void {
  const waiting = document.documentElement.hasAttribute(WAITING);
  // What this page shows: the server that served it, and the number of the build whose
  // output it shows; `null` until the HTTP response or the first message gave them.
  let shown: { server: string; build: number | null } | null = waiting ? null : servedByNavigation();
  // Whether the page scrolls to what an update changed and flashes it: the viewer setting.
  let scrollToChanges = DEFAULT_VIEWER.scrollToChanges;

  endFlashes();
  startGestures();
  startInspect();
  listen(waiting, (message) => {
    switch (message.type) {
      case 'show-place':
        show(elementsOf(message.place, placedElements(document)), true);
        break;
      case 'show-element': {
        const element = document.getElementById(message.id);
        if (element !== null) show([element], true);
        break;
      }
      case 'inspect':
        setInspect(message.on);
        break;
      case 'inspect-place':
        inspectPlace(message.place);
        break;
    }
  });

  const source = new EventSource(EVENTS);
  on(source, 'hello', (message) => {
    const hello = readHello(message);
    if (hello === null) return false;
    // A page that shows another build than the one the server holds (the server was
    // started again, or a build came while the page was not connected) loads itself anew;
    // so does the waiting page once the server holds an output.
    if (waiting ? hello.build !== null
                : shown !== null && (shown.server !== hello.server || shown.build !== hello.build)) {
      reload();
      return true;
    }
    if (!waiting && shown === null) shown = { server: hello.server, build: hello.build };
    scrollToChanges = (readViewer(message.viewer) ?? DEFAULT_VIEWER).scrollToChanges;
    setReferences(hello.references);
    markProblems(hello.problems);
    return true;
  });
  on(source, 'update', (message) => {
    const update = readUpdate(message);
    if (update === null) return false;
    if (waiting) {
      if (update.page !== 'absent') reload();
      return true;
    }
    const applied = applyUpdate(update);
    if (applied.kind === 'reloading') return true;
    if (update.page !== 'absent' && shown !== null) shown.build = update.build;
    if (update.page === 'regions') documentChanged();
    setReferences(update.references);
    markProblems(update.problems);
    if (scrollToChanges) show(applied.changed, false);
    return true;
  });
  on(source, 'viewer', (message) => {
    const settings = readViewer(message.viewer);
    if (settings === null) return false;
    scrollToChanges = settings.scrollToChanges;
    return true;
  });
}
