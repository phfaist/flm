// The help pages that the Help tab and the help browser show: the index, read once for each
// environment that the stream reports, and what the panel shows (the history of the pages
// shown, the search). A problem's Related line opens the Help tab at a page or on a search
// through this module.

import { signal } from '@preact/signals';
import { useEffect } from 'preact/hooks';
import type { HelpIndex } from '../shared/help';
import type { Answer } from './requests';
import { helpIndex } from './requests';
import type { History } from './state/help';
import { NO_HISTORY, currentSearch, visit } from './state/help';
import { panel, preview } from './state/store';

/** The index read, and the environment it was read for; `null` before the first reading. */
const index = signal<{ environment: string | null; answer: Answer<HelpIndex> } | null>(null);

/** The environment of the reading of the index that runs, if one runs. */
let reading: string | null | undefined;

/** Counts the readings started, so that the answer of an earlier one is dropped. */
let readings = 0;

/**
 * The index of the help pages, for a part that shows them: read when the part first needs
 * it, and again when the environment that the stream reports changes. `null` while it is
 * read for the first time; the index read last while it is read again. When the program
 * gives no help pages, the server's line that says so.
 */
export function useHelpIndex(): Answer<HelpIndex> | null {
  const { environment } = preview.value;
  useEffect(() => {
    if (index.value?.environment === environment || reading === environment) return;
    reading = environment;
    readings += 1;
    const number = readings;
    void helpIndex().then((answer) => {
      if (number !== readings) return;
      reading = undefined;
      index.value = { environment, answer };
    });
  }, [environment]);
  return index.value?.answer ?? null;
}

/** What the help shows: the history of the pages and searches shown, the search's text, and whether it lists the internal names. */
export interface HelpView {
  history: History;
  query: string;
  internal: boolean;
}

export const helpView = signal<HelpView>({ history: NO_HISTORY, query: '', internal: false });

/** Opens the panel on the Help tab. */
function openHelpTab(): void {
  panel.value = { ...panel.value, open: true, tab: 'help' };
}

/** Shows the help page at `path`, after the place shown in the history. */
export function showPage(path: string): void {
  const view = helpView.value;
  helpView.value = { ...view, history: visit(view.history, { page: path }), query: '' };
}

/**
 * Shows the help page at `path`, a result of the search shown: the search goes into the
 * history before the page, so that Back shows its results again.
 */
export function showResult(path: string): void {
  const view = helpView.value;
  const history = visit(visit(view.history, { search: view.query }), { page: path });
  helpView.value = { ...view, history, query: '' };
}

/** Shows the place of the history `history` (after Back or Forward): a page, or a search and its results. */
export function showVisit(history: History): void {
  helpView.value = { ...helpView.value, history, query: currentSearch(history) };
}

/**
 * Shows the page at `path` when no place was shown yet, the search's text kept: the first
 * page the help shows.
 */
export function showFirst(path: string): void {
  const view = helpView.value;
  if (view.history.visits.length === 0) helpView.value = { ...view, history: visit(view.history, { page: path }) };
}

/** Opens the Help tab on the page at `path`. */
export function showHelp(path: string): void {
  showPage(path);
  openHelpTab();
}

/** Opens the Help tab on a search of `text`. */
export function searchHelp(text: string): void {
  helpView.value = { ...helpView.value, query: text };
  openHelpTab();
}
