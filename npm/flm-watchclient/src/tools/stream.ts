// The tools page's connection to the server's stream, a connection of its own beside the
// page script's. Each message understood changes the state (`state/store.ts`); a lost
// connection makes the state disconnected until the browser has connected again, which it
// does by itself. While a build runs, each `progress` message gives the text that the status
// bar shows of it. On a `hello` of another server (`flm watch` started again), the tools page
// loads itself again. The viewer settings and the editors come with `hello` and with each
// message `viewer`; after a build with an error, a closed panel opens on the Problems tab
// when the settings ask for it. Each `hello` and `update` reports the document's problems
// again, and drops an export's refusal for the document's errors, so that the Export tab's
// box follows that report.

import { EVENTS, on, readHello, readUpdate } from '../shared/protocol';
import { DEFAULT_VIEWER, readEditors, readViewer } from '../shared/viewer';
import { isOtherServer } from './state/build';
import { resultAfterBuild } from './state/export';
import { dispatch, exporting, panel, preview, setViewer, showTab, viewer } from './state/store';
import { opensProblems } from './state/viewer';

/** Connects to the stream. */
export function connect(): void {
  const source = new EventSource(EVENTS);
  on(source, 'hello', (message) => {
    const hello = readHello(message);
    if (hello === null) return false;
    if (isOtherServer(preview.value, hello)) {
      window.location.reload();
      return true;
    }
    dispatch({ type: 'hello', hello });
    exporting.value = { ...exporting.value, last: resultAfterBuild(exporting.value.last) };
    setViewer(readViewer(message.viewer) ?? DEFAULT_VIEWER, readEditors(message.editors));
    return true;
  });
  on(source, 'building', () => {
    dispatch({ type: 'building' });
    return true;
  });
  on(source, 'progress', (message) => {
    if (typeof message.text !== 'string') return false;
    dispatch({ type: 'progress', text: message.text });
    return true;
  });
  on(source, 'update', (message) => {
    const update = readUpdate(message);
    if (update === null) return false;
    dispatch({ type: 'update', update });
    exporting.value = { ...exporting.value, last: resultAfterBuild(exporting.value.last) };
    if (opensProblems(viewer.value.settings, panel.value.open, update.problems)) showTab('problems');
    return true;
  });
  on(source, 'viewer', (message) => {
    const settings = readViewer(message.viewer);
    if (settings === null) return false;
    setViewer(settings, readEditors(message.editors));
    return true;
  });
  source.addEventListener('error', () => dispatch({ type: 'disconnected' }));
}
