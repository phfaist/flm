// What the viewer settings drive in the tools page: the panel's side and sizes, and a closed
// panel opened on the Problems tab after a build with an error (an open panel keeps the tab
// it shows). Nothing here touches the page, so that the tests can run each case.

import type { Problem } from '../../shared/protocol';
import type { ViewerSettings } from '../../shared/viewer';
import { counts } from './build';
import type { Side, Sizes } from './layout';
import { FIRST_SIZES } from './layout';

/** The panel's side and its sizes as the settings give them; a size not given is the first. */
export function layoutOf(settings: ViewerSettings): { side: Side; sizes: Sizes } {
  return {
    side: settings.panelSide,
    sizes: { right: settings.panelWidth ?? FIRST_SIZES.right, bottom: settings.panelHeight ?? FIRST_SIZES.bottom },
  };
}

/**
 * Whether a build whose problems are `problems` opens the panel on the Problems tab: only
 * when the setting asks for it, the panel is closed (`panelOpen` false) and the build has an
 * error. An open panel keeps the tab it shows, and the panel opens by itself in no other case.
 */
export function opensProblems(
  settings: ViewerSettings,
  panelOpen: boolean,
  problems: readonly Problem[],
): boolean {
  return settings.openProblemsOnErrors && !panelOpen && counts(problems).errors > 0;
}
