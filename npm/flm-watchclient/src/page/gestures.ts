// The click with Ctrl or Cmd and Shift on the document, which opens the place of the
// clicked element in the editor and flashes the element. The place is the one the library
// wrote into the element, or, where it wrote none, that of the nearest element before it
// or around it that has one. The request names no editor: the server opens the one of the
// viewer settings. When the server opens nothing, the page script writes why to the
// browser's console and tells the tools page; when it answers the place instead, because
// no editor can be opened, the page script copies the place to the clipboard, writes the
// place to the console, and tells the tools page whether it copied it. Alone at `/`, out of
// the tools page's frame, the page script copies and writes to the console all the same.

import { OPEN, UNREACHED, failure, readUnopened, unopenedLine } from '../shared/protocol';
import { flash } from './flash';
import { placeAt } from './places';
import { post } from './tools';

/** Tells why no editor opened: on the console, and to the tools page. */
function refused(text: string): void {
  console.error(text);
  post({ type: 'open-failed', text });
}

/** Asks the server to open the source file `file` at the byte `start`. */
export function requestOpen(file: string, start: number): void {
  window.fetch(OPEN, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ file, start }),
  }).then(async (answer) => {
    const unopened = await readUnopened(answer.clone());
    if (unopened !== null) {
      // Written by the window that received the click, which holds its user activation.
      const copied = await Promise.resolve()
        .then(() => navigator.clipboard.writeText(unopened.place))
        .then(() => true, () => false);
      const line = unopenedLine(unopened, copied);
      if (copied) console.info(line);
      else console.error(line);
      post({ type: 'unopened', ...unopened, copied });
    } else if (!answer.ok) {
      refused(await failure(answer, 'open the editor'));
    }
  }).catch(() => {
    refused(UNREACHED);
  });
}

/**
 * Starts the gesture. It acts when the button goes down, as the browser's own Ctrl+click
 * does on some systems; it keeps the browser from selecting text and the page's own
 * handlers from seeing that event. The click that follows is the browser's.
 */
export function startGestures(): void {
  document.addEventListener('mousedown', (event) => {
    if (event.button !== 0 || !(event.ctrlKey || event.metaKey) || !event.shiftKey) return;
    const target = event.target instanceof Element ? event.target
      : event.target instanceof Node ? event.target.parentElement : null;
    if (target === null) return;
    event.preventDefault();
    event.stopPropagation();
    const place = placeAt(target, document.body);
    if (place !== null) {
      requestOpen(place.file, place.start);
      flash([target]);
    }
  }, true);
}
