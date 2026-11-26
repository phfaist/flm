// The tools page of the live preview: the page at `/.flm-preview/`, which shows the
// document's output beside the preview's tools. `PROTOCOL.md` specifies what it expects of
// the server and the messages it exchanges with the page script of the page in its frame.
// The same files, at `/.flm-preview/help`, show the help browser alone. Both take the
// theme of the viewer settings.

import { effect } from '@preact/signals';
import { render } from 'preact';
import { HELP_BROWSER } from '../shared/help';
import { App } from './app';
import { followState, showHelpIcon } from './browser-tab';
import { HelpBrowser } from './panels/help';
import { viewer } from './state/store';
import { connect } from './stream';

const root = document.getElementById('flm-tools');
if (root !== null) {
  connect();
  // The stylesheet takes the light or the dark colors by this attribute (`tools.css`).
  effect(() => {
    document.documentElement.dataset.theme = viewer.value.settings.theme;
  });
  if (window.location.pathname === HELP_BROWSER) {
    showHelpIcon();
    render(<HelpBrowser />, root);
  } else {
    followState();
    render(<App />, root);
  }
}
