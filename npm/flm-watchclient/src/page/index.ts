// The live preview's page script: it keeps the document's page in step with the document
// while `flm watch` runs, marks the elements the last build's problems lie in, opens a
// clicked element's place in the editor, and exchanges messages with the tools page when
// the page is shown in its frame. It adds no visible element to the page, defines no
// global name and changes no built-in object. `PROTOCOL.md` specifies what it expects of
// the page and of the server.

import { start } from './connection';

if (document.body === null) {
  document.addEventListener('DOMContentLoaded', start);
} else {
  start();
}
