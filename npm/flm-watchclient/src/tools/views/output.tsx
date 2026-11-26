// The output view, chosen from the media type of the output's entry file (`view`): a frame
// on `/` for an HTML page; the PDF view for a PDF file; the text view for any other file,
// which shows the file when it is a text, and otherwise one line that states its media type,
// with a link to the Export tab; one line that says that the build type gives no output,
// with that link; before any output, the document's name and the build state. Over the
// frame, the Inspect card (`inspect.tsx`).

import { useLayoutEffect, useRef } from 'preact/hooks';
import { buildLine, view } from '../state/build';
import { frameClosed, InspectCard, inspectLoaded, inspectScrolled, inspectSelected } from '../inspect';
import { dispatch, preview, showTab, shown } from '../state/store';
import { reportUnopened } from '../requests';
import { connect } from './channel';
import { PdfView } from './pdf';
import { TextView } from './text';

/** The document's page at `/`, in a frame, and the channel to its page script. */
function Frame() {
  const frame = useRef<HTMLIFrameElement>(null);
  // Connected as soon as the frame is in the page, before its page can load.
  useLayoutEffect(() => {
    if (frame.current === null) return undefined;
    const channel = connect(frame.current, (message) => {
      switch (message.type) {
        case 'loaded':
          shown.value = true;
          inspectLoaded();
          break;
        case 'scrolled':
          inspectScrolled();
          break;
        case 'inspected':
          inspectSelected(message);
          break;
        case 'open-failed':
          // A place the page asked to open was refused: its line shows in the build state.
          dispatch({ type: 'failed', text: message.text });
          break;
        case 'unopened':
          // The server answered the place a click asked to open, for want of an editor; the
          // page script copied it to the clipboard, or could not.
          reportUnopened({ place: message.place, reason: message.reason }, message.copied);
          break;
      }
    });
    return () => {
      channel.close();
      shown.value = false;
      frameClosed();
    };
  }, []);
  return <iframe ref={frame} class="flm-frame" src="/" title="Document" />;
}

/** The link that opens the Export tab, under the line of an output the page cannot show. */
function ToExport() {
  return (
    <p>
      <button type="button" class="flm-link" onClick={() => showTab('export')}>
        Export document…
      </button>
    </p>
  );
}

export function Output() {
  const output = useRef<HTMLElement>(null);
  const state = preview.value;
  const shows = view(state);
  switch (shows.kind) {
    case 'frame':
      return (
        <main ref={output} class="flm-output flm-output-frame">
          <Frame />
          <InspectCard room={() => output.current} />
        </main>
      );
    case 'text':
      return (
        <TextView
          mediaType={shows.mediaType}
          none={(
            <main class="flm-output flm-output-line">
              <p>
                No view for the media type <code>{shows.mediaType}</code>
              </p>
              <ToExport />
            </main>
          )}
        />
      );
    case 'pdf':
      return <PdfView />;
    case 'empty':
      return (
        <main class="flm-output flm-output-line">
          <p>
            {shows.profile === null ? 'No output' : (
              <>
                No output produced for build type <code>{shows.profile}</code>
              </>
            )}
          </p>
          <ToExport />
        </main>
      );
    case 'none': {
      const line = buildLine(state);
      return (
        <main class="flm-output flm-output-line">
          {state.document !== null && (
            <p>
              <code>{state.document}</code>
            </p>
          )}
          {line !== null && (
            <p class="flm-build-state" data-tone={line.tone}>
              {line.text}
            </p>
          )}
        </main>
      );
    }
  }
}
