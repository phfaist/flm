// The state of the tools page, held in signals: what the stream reports (`build.ts`), what
// the frame shows, the panel and the tab asked to take the focus, and what the tabs keep
// while hidden (the problem selected, the Outline's filter, the Export tab's state). The
// parts of the page read these signals; the stream and the requests change the first
// through `dispatch`, the controls change the others.

import { signal } from '@preact/signals';
import type { BuildEvent, BuildState } from './build';
import type { ExportAction } from '../../shared/export';
import type { LastExport } from './export';
import { INITIAL, SLOW_BUILD_MS, step } from './build';
import type { Side, Sizes } from './layout';
import type { LabelFilter } from './outline';
import { ALL_LABELS } from './outline';
import { FIRST_SIZES } from './layout';
import type { Editor, ViewerSettings } from '../../shared/viewer';
import { DEFAULT_VIEWER } from '../../shared/viewer';
import { layoutOf } from './viewer';

/** The state of the server and of its builds. */
export const preview = signal<BuildState>(INITIAL);

/** Changes `preview` by `event`; starts the timer after which a build counts as slow. */
export function dispatch(event: BuildEvent): void {
  const before = preview.value;
  const after = step(before, event);
  preview.value = after;
  if (after.building && after.started !== before.started) {
    const started = after.started;
    window.setTimeout(() => dispatch({ type: 'slow', started }), SLOW_BUILD_MS);
  }
}

/** Whether the frame's page has loaded and its page script answered. */
export const shown = signal<boolean>(false);

/** Whether Inspect is on: only while the frame shows the document's page. */
export const inspecting = signal<boolean>(false);

/** The panel: whether it is open, the edge and the sizes asked for, and the tab used last. */
export interface Panel {
  open: boolean;
  side: Side;
  sizes: Sizes;
  /** The tab shown when the panel is open, or `null` while the panel has no tab. */
  tab: string | null;
}

export const panel = signal<Panel>({ open: false, side: 'right', sizes: FIRST_SIZES, tab: null });

/** The viewer settings, and the editors that the server can open a place in. */
export interface Viewer {
  editors: Editor[];
  settings: ViewerSettings;
}

export const viewer = signal<Viewer>({ editors: [], settings: DEFAULT_VIEWER });

/**
 * Takes `settings` as the viewer settings and `editors` as the editors: the panel takes the
 * side and the sizes that the settings give.
 */
export function setViewer(settings: ViewerSettings, editors: Editor[]): void {
  viewer.value = { editors, settings };
  panel.value = { ...panel.value, ...layoutOf(settings) };
}

/** The key of the row of the Problems tab that is selected (`problems.ts`), or `null`. */
export const selectedProblem = signal<string | null>(null);

/** Which labels the Outline tab shows (`outline.ts`): kept for the session, not stored. */
export const labelFilter = signal<LabelFilter>(ALL_LABELS);

/** The feature that the Settings tab is asked to show, until it shows it. */
export const settingsTarget = signal<string | null>(null);

/** Opens the panel on the tab `id`. */
export function showTab(id: string): void {
  panel.value = { ...panel.value, open: true, tab: id };
}

/** The tab that is asked to take the focus, until the panel shows it (`focusTab`). */
export const focusTarget = signal<string | null>(null);

/** Opens the panel on the tab `id`, and moves the focus into the tab once it is shown. */
export function focusTab(id: string): void {
  focusTarget.value = id;
  showTab(id);
}

/** Opens the panel on the Settings tab, at the feature `name` of its advanced list. */
export function showSettingsOf(name: string): void {
  settingsTarget.value = name;
  showTab('settings');
}

/** The Export tab's state, kept while the tab is hidden. */
export interface Export {
  /** The build type chosen in the tab, or `null` for the build type shown. */
  profile: string | null;
  /** Whether the checkbox that confirms an export despite the errors is checked. */
  confirmed: boolean;
  /** The action of the export under way, or `null` while none runs. */
  running: ExportAction | null;
  /**
   * The last export and its result, or `null` before any, after a download, and after a
   * refusal for the document's errors once the stream reports the problems again (the next
   * `hello` or `update`: `resultAfterBuild`). The tab shows the result only under the build
   * type exported (`resultFor`).
   */
  last: LastExport | null;
}

export const exporting = signal<Export>({ profile: null, confirmed: false, running: null, last: null });
