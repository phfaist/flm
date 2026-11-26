// The constructs at a place of the document (`GET /.flm-preview/at`) and the help pages
// (`GET /.flm-preview/help/index` and `GET /.flm-preview/help/page`), as the tools page reads
// them. `PROTOCOL.md` specifies both. A help page's path is the program's: the tools page
// reads paths from the index, from a page's links, from a page's path line and from the
// constructs, makes none, and does not read a path's form.

import { isObject } from './protocol';

/** The request of the constructs at a place of a source. */
export const AT = '/.flm-preview/at';

/** The request of the index of the help pages. */
export const HELP_INDEX = '/.flm-preview/help/index';

/** The request of one help page, by its path. */
export const HELP_PAGE = '/.flm-preview/help/page';

/** The help browser alone: the tools page's files, showing the page of the address's fragment. */
export const HELP_BROWSER = '/.flm-preview/help';

/** A command, an environment or a special character sequence written in a source. */
export interface Construct {
  kind: string;
  /** The name it is written with, without the backslash, or the characters of a special. */
  name: string;
  file: string;
  start: number;
  end: number;
  /** The feature that declares it, or `null`. */
  feature: string | null;
  /** The path of its help page, or `null` when it has none. */
  help: string | null;
}

/** The constructs at a place: its line and column, and the constructs from the innermost outward. */
export interface ConstructsAt {
  line: number;
  column: number;
  constructs: Construct[];
}

/** A page of the index of the help pages. */
export interface IndexEntry {
  path: string;
  kind: string;
  title: string;
  /** A short text that says what the page shows, as plain text; empty when it has none. */
  summary: string;
  /** The names the page is looked up by. */
  names: string[];
}

/** The index of the help pages, for one environment. */
export interface HelpIndex {
  environment: string | null;
  pages: IndexEntry[];
}

/** A help page. */
export interface HelpPage {
  path: string;
  kind: string;
  title: string;
  /** The page's content, as HTML. */
  content: string;
  /** The feature, for the page of a feature and for the page of a feature's settings. */
  settingsFeature: string | null;
  /** The levels from the page's section down to the page itself, each with its page's path; empty for `/`. */
  pathLine: { text: string; path: string }[];
}

/** The answer for a path: the page, or the pages whose paths are nearest when it has none. */
export interface PageAnswer {
  path: string;
  page: HelpPage | null;
  nearest: { path: string; title: string }[];
}

function text(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

function count(value: unknown): number | null {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0 ? value : null;
}

function readConstruct(value: unknown): Construct | null {
  if (!isObject(value)) return null;
  const [kind, name, file, start, end] = [text(value.kind), text(value.name), text(value.file), count(value.start), count(value.end)];
  if (kind === null || name === null || file === null || start === null || end === null) return null;
  return { kind, name, file, start, end, feature: text(value.feature), help: text(value.help) };
}

/** The answer of `GET at`, or `null` when it is not understood. */
export function readConstructsAt(value: unknown): ConstructsAt | null {
  if (!isObject(value) || !Array.isArray(value.constructs)) return null;
  const [line, column] = [count(value.line), count(value.column)];
  if (line === null || column === null) return null;
  return { line, column, constructs: value.constructs.map(readConstruct).filter((each) => each !== null) };
}

function readEntry(value: unknown): IndexEntry | null {
  if (!isObject(value)) return null;
  const [path, kind, title] = [text(value.path), text(value.kind), text(value.title)];
  if (path === null || kind === null || title === null) return null;
  const names = Array.isArray(value.names) ? value.names.filter((name): name is string => typeof name === 'string') : [];
  return { path, kind, title, summary: text(value.summary) ?? '', names };
}

/** The answer of `GET help/index`, or `null` when it is not understood. */
export function readHelpIndex(value: unknown): HelpIndex | null {
  if (!isObject(value) || !Array.isArray(value.pages)) return null;
  return { environment: text(value.environment), pages: value.pages.map(readEntry).filter((each) => each !== null) };
}

/** The answer of `GET help/page`, or `null` when it is not understood. */
export function readPageAnswer(value: unknown): PageAnswer | null {
  if (!isObject(value) || typeof value.path !== 'string') return null;
  let page: HelpPage | null = null;
  if (isObject(value.page)) {
    const { path, kind, title, content } = value.page;
    if (typeof path !== 'string' || typeof kind !== 'string' || typeof title !== 'string' || typeof content !== 'string') return null;
    const pathLine = Array.isArray(value.page.path_line)
      ? value.page.path_line.filter(isObject).flatMap((each) => (typeof each.text === 'string' && typeof each.path === 'string' ? [{ text: each.text, path: each.path }] : []))
      : [];
    page = { path, kind, title, content, settingsFeature: text(value.page.settings_feature), pathLine };
  }
  const nearest = Array.isArray(value.nearest)
    ? value.nearest.filter(isObject).flatMap((each) => (typeof each.path === 'string' && typeof each.title === 'string' ? [{ path: each.path, title: each.title }] : []))
    : [];
  return { path: value.path, page, nearest };
}
