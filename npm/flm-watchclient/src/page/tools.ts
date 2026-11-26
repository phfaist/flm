// The page script's side of the messages with the tools page. The tools page shows the
// document's page in a frame, and sends `attach` once the frame has loaded the page; from
// then on the page script sends its messages to the tools page. A page that is not in the
// tools page's frame receives no `attach`, and sends nothing. A message is accepted only
// from the window around the page's (the frame's parent) and from the page's own origin,
// the server's.

import type { ToPage, ToTools } from '../shared/messages';
import { envelope, isFrom, readToPage } from '../shared/messages';

/** The tools page's window, once it sent `attach`; `null` until then. */
let tools: Window | null = null;

/**
 * The window's scroll position as the tools page last saw it: the one of the last scroll
 * reported, or of the last message that carried a rectangle. A scroll is reported only away
 * from it, so that a report sent after a rectangle, of a scroll made before it, does not
 * tell the tools page that the rectangle is out of date.
 */
let seen = { x: 0, y: 0 };

/** Marks the window's scroll position as seen by the tools page: call it with a rectangle sent. */
export function scrollSeen(): void {
  seen = { x: window.scrollX, y: window.scrollY };
}

/** Sends `message` to the tools page; nothing when the page is not in its frame. */
export function post(message: ToTools): void {
  if (tools !== null) tools.postMessage(envelope(message), window.location.origin);
}

/** The window from which a message to the page script is accepted, or `null` when none is. */
function parentWindow(): Window | null {
  return window.parent !== window ? window.parent : null;
}

/**
 * Listens to the tools page: `handle` receives each message accepted other than
 * `attach`, which the page script answers with `loaded`, `waiting` telling whether the
 * page is the server's waiting page. Reports each scroll of the window to the tools page,
 * once per frame of the window's drawing, when the window is at another position than the
 * one the tools page last saw.
 */
export function listen(waiting: boolean, handle: (message: Exclude<ToPage, { type: 'attach' }>) => void): void {
  window.addEventListener('message', (event) => {
    const from = parentWindow();
    if (!isFrom(event, from, window.location.origin)) return;
    const message = readToPage(event.data);
    if (message === null) return;
    if (message.type === 'attach') {
      tools = from;
      scrollSeen();
      post({ type: 'loaded', waiting });
      return;
    }
    handle(message);
  });
  let pending = false;
  window.addEventListener('scroll', () => {
    if (tools === null || pending) return;
    pending = true;
    window.requestAnimationFrame(() => {
      pending = false;
      if (window.scrollX === seen.x && window.scrollY === seen.y) return;
      scrollSeen();
      post({ type: 'scrolled', x: window.scrollX, y: window.scrollY });
    });
  }, { passive: true });
}
