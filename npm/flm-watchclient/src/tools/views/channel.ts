// The tools page's side of the messages with the page script, inside the document's page
// that the frame shows. Once the frame has loaded a page, the tools page sends it
// `attach`, and the page script answers `loaded`; the page script sends its messages only
// after `attach`. A message is accepted only from the frame's window and from the tools
// page's own origin, the server's.

import type { ToPage, ToTools } from '../../shared/messages';
import { envelope, isFrom, readToTools } from '../../shared/messages';

/** The channel to the page script of the page in `frame`. */
export interface Channel {
  /** Sends `message` to the page script. */
  send(message: ToPage): void;
  /** Stops listening. */
  close(): void;
}

/** The channel to the page shown in the output view's frame, while there is one. */
let current: Channel | null = null;

/** Sends `message` to the page script of the page the frame shows; nothing without a frame. */
export function toPage(message: ToPage): void {
  current?.send(message);
}

/**
 * Opens the channel to the page in `frame`, which `toPage` sends to until it is closed;
 * `handle` receives each message accepted.
 */
export function connect(frame: HTMLIFrameElement, handle: (message: ToTools) => void): Channel {
  const origin = window.location.origin;
  const send = (message: ToPage): void => {
    frame.contentWindow?.postMessage(envelope(message), origin);
  };
  const attach = (): void => send({ type: 'attach' });
  const receive = (event: MessageEvent): void => {
    if (!isFrom(event, frame.contentWindow, origin)) return;
    const message = readToTools(event.data);
    if (message !== null) handle(message);
  };
  frame.addEventListener('load', attach);
  window.addEventListener('message', receive);
  const channel: Channel = {
    send,
    close() {
      frame.removeEventListener('load', attach);
      window.removeEventListener('message', receive);
      if (current === channel) current = null;
    },
  };
  current = channel;
  return channel;
}
