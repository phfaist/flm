// The PDF view: the output's entry file at `/`, when it is a PDF file, drawn by pdf.js
// (`pdf-pages.ts`), which is loaded when the view first shows. A line above the pages gives
// the page read, the number of pages and the zoom, with the buttons that change the zoom.
// The view reads the file again at each change of the output served; the reading position
// and the zoom stay, and the earlier file shows until the changed one is drawn.

import { useEffect, useRef, useState } from 'preact/hooks';
import { Icon } from '../controls/icon';
import { entryBytes } from '../requests';
import { dispatch, preview } from '../state/store';
import type { Reading } from './pdf-pages';
import { PdfPages, loadPdfJs, steppedZoom } from './pdf-pages';

/** The text of a failure reported by pdf.js, or by the loading of its module. */
function failureText(error: unknown): string {
  const detail = error instanceof Error ? error.message : String(error);
  return `Failed to show the PDF output: ${detail}`;
}

function failed(error: unknown): void {
  dispatch({ type: 'failed', text: failureText(error) });
}

export function PdfView() {
  const revision = preview.value.revision;
  const scroller = useRef<HTMLDivElement>(null);
  const pages = useRef<PdfPages | null>(null);
  const [reading, setReading] = useState<Reading | null>(null);

  useEffect(() => {
    if (scroller.current === null) return undefined;
    const drawn = new PdfPages(scroller.current, setReading, failed);
    pages.current = drawn;
    return () => {
      drawn.close();
      pages.current = null;
    };
  }, []);

  // Counts the reads started, so that the answer to an earlier read is dropped.
  const reads = useRef(0);
  useEffect(() => {
    const read = ++reads.current;
    void (async () => {
      const [library, data] = await Promise.all([loadPdfJs(), entryBytes('read the PDF output')]);
      if (data === null || read !== reads.current || pages.current === null) return;
      await pages.current.show(library, data);
    })().catch(failed);
  }, [revision]);

  const zoom = (direction: 1 | -1) => {
    if (reading !== null) pages.current?.setZoom(steppedZoom(reading.zoom, direction));
  };
  return (
    <main class="flm-output flm-pdf-output">
      <div class="flm-pdf-head">
        <span>application/pdf</span>
        <span class="flm-pdf-spacer" />
        {reading !== null && (
          <>
            <span class="flm-pdf-pages-read">
              Page {reading.page} of {reading.pages}
            </span>
            <span class="flm-pdf-zoom">
              <button type="button" aria-label="Zoom out" title="Zoom out" disabled={steppedZoom(reading.zoom, -1) === reading.zoom}
                onClick={() => zoom(-1)}>
                <Icon name="zoom-out" size={14} />
              </button>
              <span class="flm-pdf-zoom-value">{Math.round(reading.zoom * 100)}%</span>
              <button type="button" aria-label="Zoom in" title="Zoom in" disabled={steppedZoom(reading.zoom, 1) === reading.zoom}
                onClick={() => zoom(1)}>
                <Icon name="zoom-in" size={14} />
              </button>
            </span>
          </>
        )}
      </div>
      <div ref={scroller} class="flm-pdf-view" tabIndex={0} role="region" aria-label="PDF output" />
    </main>
  );
}
