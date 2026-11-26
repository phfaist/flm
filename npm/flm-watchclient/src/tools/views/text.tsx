// The text view: the output's entry file at `/`, when it is neither an HTML page nor a PDF
// file, in the monospace stack. It reads the file again at each change of the output
// served, and keeps what it shows, and so its scroll position, until the changed file has
// arrived. A file that is a text (`textOf`) is shown as text: a LaTeX file highlighted by
// the library highlight.js, any other with the colors and styles of its terminal sequences,
// which the library ansi_up turns into HTML. A file that is not a text shows `none` instead.

import type { ComponentChildren } from 'preact';
import { useEffect, useMemo, useRef, useState } from 'preact/hooks';
import { AnsiUp } from 'ansi_up';
import hljs from 'highlight.js/lib/core';
import latex from 'highlight.js/lib/languages/latex';
import { entryBytes } from '../requests';
import { languageOf, textOf } from '../state/build';
import { preview, viewer } from '../state/store';

hljs.registerLanguage('latex', latex);

/**
 * The sequences of the OSC convention (`ESC ]` … `ESC \` or BEL), among them the hyperlinks
 * of OSC 8. ansi_up reads a hyperlink only when its text holds no other sequence, and the
 * text of a hyperlink of a terminal output carries its colors: the sequences are removed,
 * and the text shows without its link.
 */
const OSC = /\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g;

/** The HTML of the text `text` of the media type `mediaType`: highlighted in its language, or with its terminal sequences applied. */
function htmlOf(text: string, mediaType: string): string {
  const language = languageOf(mediaType);
  if (language !== null) return hljs.highlight(text, { language, ignoreIllegals: true }).value;
  // A converter of its own for each text: ansi_up keeps the styles of a text for the next.
  const ansi = new AnsiUp();
  ansi.use_classes = true;
  return ansi.ansi_to_html(text.replace(OSC, ''));
}

export function TextView({ mediaType, none }: {
  mediaType: string;
  /** What the view shows when the file is not a text. */
  none: ComponentChildren;
}) {
  const revision = preview.value.revision;
  // The text of the file read last; `null` for a file that is not a text, `undefined` before the first.
  const [text, setText] = useState<string | null | undefined>(undefined);
  // Counts the reads started, so that the answer to an earlier read is dropped.
  const reads = useRef(0);
  useEffect(() => {
    const read = ++reads.current;
    void entryBytes('read the text output').then((bytes) => {
      if (bytes !== null && read === reads.current) setText(textOf(bytes));
    });
  }, [revision]);
  const html = useMemo(() => (typeof text === 'string' ? htmlOf(text, mediaType) : ''), [text, mediaType]);
  if (text === null) return <>{none}</>;
  return (
    <main class="flm-output flm-text-output">
      <div class="flm-text-head">
        <span>{mediaType}</span>
      </div>
      <pre
        class="flm-text"
        data-wrap={viewer.value.settings.wrapLines}
        tabIndex={0}
        role="region"
        aria-label="Text output"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </main>
  );
}
