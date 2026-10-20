// flm-lecturenotes: the script of a book-length document shown as one page with
// a sidebar. The bundle defines one global name, `flmLecturenotes`, whose
// `setup(settings)` the page calls once.

import { findPage, readSettings } from './layout';
import { ReadingPosition } from './position';
import { Previews } from './previews';
import { Navigator } from './sidebar';

/** The event the live preview dispatches after it updated a part of the page in place. */
const REGION_UPDATED = 'flm:region-updated';

/**
 * Sets the page up: the sidebar, the bar and the chapters, the anchors, the
 * previews of the links, the reading position; and sets the sidebar up again
 * after each in-place update of the page (the popup closes, the position is
 * recorded again). `settings` is the settings object (`chapter_level`; other
 * fields are ignored). Runs when the document is parsed.
 */
export function setup(settings: unknown = {}): void {
  const run = (): void => {
    const page = findPage(document);
    if (page === null) return;
    const options = readSettings(settings);
    const navigator = new Navigator(page, options);
    navigator.attach();
    navigator.rebuild();
    const previews = new Previews(document);
    previews.attach();
    const position = new ReadingPosition(page);
    position.attach();
    navigator.onBeforeJump = () => position.record();
    position.restoreOnLoad();
    document.addEventListener(REGION_UPDATED, () => {
      navigator.rebuild();
      previews.hide();
      position.record();
    });
  };
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', run, { once: true });
  } else {
    run();
  }
}
