// What the Help tab and the help browser make of the index of the help pages: the search,
// the history of the pages and the searches shown, the pages a link leads to, and the links
// of a problem's Related line, from the constructs at its place. Every path comes from the
// index, from a page or from a link's address; nothing here makes one or reads its form, and nothing
// touches the page, so that the tests can run each case.

import type { Construct, IndexEntry } from '../../shared/help';

/** The kinds of the pages of the registry's internal names, listed only on demand. */
const INTERNAL_KINDS: readonly string[] = ['namespace-index', 'namespace', 'entity'];

/** The kinds of the pages that rank first among the results of equal standing. */
const FIRST_KINDS: readonly string[] = ['command', 'settings'];

/** Whether `entry` is the page of one of the registry's internal names (a stage, a table…). */
export function isInternal(entry: IndexEntry): boolean {
  return INTERNAL_KINDS.includes(entry.kind);
}

/**
 * How well `entry` matches `wanted` (lowercase, trimmed): 0 for a name that matches whole,
 * 1 for a name that begins with it, 2 for a title that holds it, 3 for a summary that holds
 * it; `null` when it does not match.
 */
function standing(entry: IndexEntry, wanted: string): number | null {
  const names = entry.names.map((name) => name.toLowerCase());
  if (names.includes(wanted)) return 0;
  if (names.some((name) => name.startsWith(wanted))) return 1;
  if (entry.title.toLowerCase().includes(wanted)) return 2;
  if (entry.summary.toLowerCase().includes(wanted)) return 3;
  return null;
}

/**
 * The pages of `pages` that match `query`, best first: a name that matches whole, then a
 * name that begins with the text, then a title, then a summary; among pages of equal
 * standing, commands and settings pages first, then the order of the index. The pages of
 * the internal names are left out unless `internal`. Letter case is ignored; an empty query
 * matches nothing.
 */
export function search(pages: readonly IndexEntry[], query: string, internal: boolean): IndexEntry[] {
  const wanted = query.trim().toLowerCase();
  if (wanted === '') return [];
  const found: { entry: IndexEntry; rank: number; first: number; at: number }[] = [];
  pages.forEach((entry, at) => {
    if (!internal && isInternal(entry)) return;
    const rank = standing(entry, wanted);
    if (rank !== null) found.push({ entry, rank, first: FIRST_KINDS.includes(entry.kind) ? 0 : 1, at });
  });
  found.sort((a, b) => a.rank - b.rank || a.first - b.first || a.at - b.at);
  return found.map((each) => each.entry);
}

// --- the history --------------------------------------------------------------------------

/** A place of the history: a help page, by its path, or the results of a search, by its text. */
export type Visit = { page: string } | { search: string };

/** The places shown, in order, and the one shown now (`at`, `-1` before the first). */
export interface History {
  visits: readonly Visit[];
  at: number;
}

export const NO_HISTORY: History = { visits: [], at: -1 };

/** The place shown, or `null` before the first. */
export function current(history: History): Visit | null {
  return history.visits[history.at] ?? null;
}

/** The path of the page shown, or `null` before the first and while the results of a search are shown. */
export function currentPage(history: History): string | null {
  const shown = current(history);
  return shown !== null && 'page' in shown ? shown.page : null;
}

/** The text of the search whose results are shown, or `''` while a page is shown. */
export function currentSearch(history: History): string {
  const shown = current(history);
  return shown !== null && 'search' in shown ? shown.search : '';
}

function same(a: Visit | null, b: Visit): boolean {
  if (a === null) return false;
  return 'page' in a ? 'page' in b && a.page === b.page : 'search' in b && a.search === b.search;
}

/**
 * The history once the place `next` is shown: the places after the one shown are dropped,
 * and `next` follows it, unless it is the place shown.
 */
export function visit(history: History, next: Visit): History {
  if (same(current(history), next)) return history;
  const visits = [...history.visits.slice(0, history.at + 1), next];
  return { visits, at: visits.length - 1 };
}

export function canGoBack(history: History): boolean {
  return history.at > 0;
}

export function canGoForward(history: History): boolean {
  return history.at < history.visits.length - 1;
}

/** The history once Back is taken: the place before the one shown. */
export function back(history: History): History {
  return canGoBack(history) ? { ...history, at: history.at - 1 } : history;
}

/** The history once Forward is taken: the place after the one shown. */
export function forward(history: History): History {
  return canGoForward(history) ? { ...history, at: history.at + 1 } : history;
}

// --- links and paths ----------------------------------------------------------------------

/** The suffix of the address of a link of the kind `flmdoc.page` after the page's path. */
const LINK_SUFFIX = '.html';

/** The path of the page that the address `href` of a link of the kind `flmdoc.page` leads to. */
export function pathOfLink(href: string): string {
  return href.endsWith(LINK_SUFFIX) ? href.slice(0, -LINK_SUFFIX.length) : href;
}

/**
 * The path of the home page of the help: the first page of the index, the root of the tree
 * of the pages; `null` for an index without pages.
 */
export function homePage(pages: readonly IndexEntry[]): string | null {
  return pages[0]?.path ?? null;
}

/** The page of the index at `path`, if there is one. */
export function entryAt(pages: readonly IndexEntry[], path: string): IndexEntry | undefined {
  return pages.find((entry) => entry.path === path);
}

/** The page of the feature `name`: the page of the kind `feature` looked up by that name. */
export function featurePage(pages: readonly IndexEntry[], name: string): IndexEntry | undefined {
  return pages.find((entry) => entry.kind === 'feature' && entry.names.includes(name));
}

/** The fragment of an address that names the page at `path`, without its `#`. */
export function fragmentOf(path: string): string {
  return encodeURI(path).replace(/#/g, '%23');
}

/** The path that the fragment `fragment` of an address names (with or without its `#`), or `null` for none. */
export function pathOfFragment(fragment: string): string | null {
  const text = fragment.startsWith('#') ? fragment.slice(1) : fragment;
  if (text === '') return null;
  try {
    return decodeURIComponent(text);
  } catch {
    return text;
  }
}

/** A link of a problem's Related line: its text, and the help page it shows or the text it searches. */
export type Related = { text: string; page: string } | { text: string; search: string };

/** The text a construct is written with: a macro with its backslash. */
function written(construct: Construct): string {
  return construct.kind === 'macro' ? `\\${construct.name}` : construct.name;
}

/**
 * The links of the Related line of a problem at the constructs `constructs` (the innermost
 * first): the help page of the innermost command or environment written there, then the page
 * of its feature; for one without a help page (an unknown command), a search of the help for
 * its name. A link is named by the title of its page in the index.
 */
export function relatedOf(constructs: readonly Construct[], pages: readonly IndexEntry[]): Related[] {
  const construct = constructs.find((each) => each.kind === 'macro' || each.kind === 'environment');
  if (construct === undefined) return [];
  const { help, feature } = construct;
  if (help === null) {
    const name = written(construct);
    return [{ text: `Search the help for ‘${name}’`, search: name }];
  }
  const links: Related[] = [{ text: entryAt(pages, help)?.title ?? written(construct), page: help }];
  const ofFeature = feature === null ? undefined : featurePage(pages, feature);
  if (ofFeature !== undefined && ofFeature.path !== help) links.push({ text: ofFeature.title, page: ofFeature.path });
  return links;
}
