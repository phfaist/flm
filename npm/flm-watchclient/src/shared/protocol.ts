// What the preview's server and its browser parts share: the server's addresses, and the
// messages of its stream as the browser parts read them. `PROTOCOL.md` specifies both.

import type { Part } from './messages';

/** The stream of the server's messages (server-sent events). */
export const EVENTS = '/.flm-preview/events';

/** The request to build the document again. */
export const REBUILD = '/.flm-preview/rebuild';

/** The request to open a place of a source in an editor. */
export const OPEN = '/.flm-preview/open';

/** The request to change the build type: the profile that the builds run. */
export const PROFILE = '/.flm-preview/profile';

/** The output's entry file, served at `/`. */
export const ENTRY = '/';

/** The library pdf.js, a module among the files of the tools page, which the PDF view loads. */
export const PDFJS = '/.flm-preview/assets/pdfjs/pdf.min.mjs';

/** The worker of pdf.js, a module. */
export const PDFJS_WORKER = '/.flm-preview/assets/pdfjs/pdf.worker.min.mjs';

/** The folder of the fonts of pdf.js, with its final `/`. */
export const PDFJS_FONTS = '/.flm-preview/assets/pdfjs/standard_fonts/';

/** The severities of a problem, the worst first. */
export const SEVERITIES = ['error', 'warning', 'note'] as const;

export type Severity = (typeof SEVERITIES)[number];

/** Where a problem lies, as the terminal shows it: a file, a line and a column. */
export interface Place {
  /** The name of the source file, as in `data-flm-source-span`. */
  file: string | null;
  /** The label of content that the library made, for a place in such content. */
  label: string | null;
  line: number | null;
  column: number | null;
  /**
   * The byte offset of the place counted from the top of the file, given only for a place
   * in the file's content: a place in a text parsed under the file's name (the document's
   * title, for one) has none, and its `bytes` count from the start of that text.
   */
  offset: number | null;
}

/** One part of the "inside" chain of a problem: its title and its place. */
export interface Frame {
  title: string;
  place: Place;
}

/** The line a problem lies on, cut in three around the part the problem covers. */
export interface Excerpt {
  line: number;
  before: string;
  covered: string;
  after: string;
}

/** A problem of a build, as the stream carries it: its record and two fields more. */
export interface Problem {
  /** `diagnostic` for a diagnostic of the document, `problem` for one that stopped the build. */
  kind: 'diagnostic' | 'problem';
  severity: Severity;
  /** The identifier of a diagnostic's condition. */
  id: string | null;
  /** The data of a diagnostic's condition, a JSON value; `null` when there is none. */
  data: unknown;
  message: string;
  place: Place | null;
  frames: Frame[];
  /** The byte offsets of its start and of its end, counted as in `data-flm-source-span`. */
  bytes: { start: number; end: number } | null;
  excerpt: Excerpt | null;
}

/** A heading of the document, in the outline. */
export interface Heading {
  /** The smaller the number, the outer the heading. */
  level: number;
  /** The heading's number as plain text, or `null` for a heading without one. */
  number: string | null;
  /** The title, as plain text. */
  title: string;
  /** The `id` of the heading's element in the page, or `null`. */
  id: string | null;
}

/** A label of the document, and what a reference to it shows. */
export interface LabelEntry {
  label: string;
  /** What a reference to the label shows, as plain text, such as "Thm. 1". */
  designation: string;
  /** The `id` of the element the label refers to, or `null`. */
  id: string | null;
  /** The kind of what the label refers to, such as "equation" (an open set), or `null`. */
  kind: string | null;
  /** The position in the outline of the heading the label stands under, or `null` before the first heading. */
  heading: number | null;
}

/** The first message of every connection: the server's state. */
export interface Hello {
  server: string;
  document: string | null;
  build: number | null;
  building: boolean;
  problems: Problem[];
  references: Record<string, string> | null;
  /** The headings of the document that the output shows, or `null` while there is no output. */
  outline: Heading[] | null;
  /** The labels of the document that the output shows, or `null` while there is no output. */
  labels: LabelEntry[] | null;
  /** Whether the program gives the help pages. */
  help: boolean;
  /** The media type of the file served at `/`, or `null` while the server holds no output. */
  mediaType: string | null;
  /** Whether the output served is from an earlier build than the last one that finished. */
  earlierOutput: boolean;
  /** How long the last build that finished took, in milliseconds, or `null` before the first. */
  durationMs: number | null;
  /** The build type in use, as the server reports it, or `null` before it knows one. */
  profile: string | null;
  /** The default build type: the one the builds run when no build type is chosen, or `null` before the server knows one. */
  defaultProfile: string | null;
  /** The build types available, in name order. */
  profiles: string[];
  /** The digest of the document's environment, or `null` before a preparation has succeeded. */
  environment: string | null;
  /** The counter of the settings state: it changes whenever the settings state may have changed. */
  settings: number;
}

/** How the page must change to show a build's output. */
export const PAGE_CHANGES = ['same', 'regions', 'reload', 'absent'] as const;

export type PageChange = (typeof PAGE_CHANGES)[number];

/** An output region of an update: the HTML inside an element, or plain words. */
export interface Region {
  id: string;
  name: string | null;
  kind: string | null;
  text: string;
}

/** A build is done: how the page changes, and the build's problems. */
export interface Update {
  build: number;
  page: PageChange;
  regions: Region[];
  files: string[];
  problems: Problem[];
  references: Record<string, string> | null;
  /** The headings, as in `Hello`. */
  outline: Heading[] | null;
  /** The labels, as in `Hello`. */
  labels: LabelEntry[] | null;
  /** The media type of the file served at `/` after this build, or `null`. */
  mediaType: string | null;
  /** Whether that output is from an earlier build, because this one gave none. */
  earlierOutput: boolean;
  /** How long this build took, in milliseconds, or `null` when the server does not say. */
  durationMs: number | null;
  /** The build type in use, as in `Hello`. */
  profile: string | null;
  /** The default build type, as in `Hello`. */
  defaultProfile: string | null;
  /** The build types available, as in `Hello`. */
  profiles: string[];
  /** The digest of the document's environment, as in `Hello`. */
  environment: string | null;
  /** The counter of the settings state, as in `Hello`. */
  settings: number;
}

type Fields = Record<string, unknown>;

/** The JSON object of a message's data, or `null` when the data is not one. */
export function readObject(text: string): Fields | null {
  try {
    const value: unknown = JSON.parse(text);
    return isObject(value) ? value : null;
  } catch {
    return null;
  }
}

/**
 * Runs `handle` on each message `name` of `source`, read as a JSON object. A message that
 * is not one, that `handle` does not understand (it answers `false`) or whose handling
 * throws is written to the console and changes nothing more.
 */
export function on(source: EventSource, name: string, handle: (message: Fields) => boolean): void {
  source.addEventListener(name, (event) => {
    const message = readObject((event as MessageEvent<string>).data);
    try {
      if (message === null || !handle(message)) console.error(`Unreadable message ‘${name}’ from the preview’s server`);
    } catch (error) {
      console.error(`Failed to apply the message ‘${name}’ from the preview’s server:`, error);
    }
  });
}

/** The line of text that says a request reached no server. */
export const UNREACHED = 'Failed to reach the preview’s server';

/**
 * The line of text that says why the request answered `response` failed: the first line
 * the server answered, or one that names what failed (`action`, as in "build the document
 * again").
 */
export async function failure(response: Response, action: string): Promise<string> {
  const text = (await response.text().catch(() => '')).trim().split('\n')[0] ?? '';
  return text !== '' ? text : `Failed to ${action} (the server answered with the status ${response.status})`;
}

export function isObject(value: unknown): value is Fields {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function textOrNull(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

function numberOrNull(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function readPlace(value: unknown): Place | null {
  if (!isObject(value)) return null;
  return { file: textOrNull(value.file), label: textOrNull(value.label), line: numberOrNull(value.line), column: numberOrNull(value.column), offset: numberOrNull(value.offset) };
}

function readBytes(value: unknown): { start: number; end: number } | null {
  if (!isObject(value)) return null;
  const start = numberOrNull(value.start);
  const end = numberOrNull(value.end);
  return start !== null && end !== null && start <= end ? { start, end } : null;
}

function readExcerpt(value: unknown): Excerpt | null {
  if (!isObject(value)) return null;
  const line = numberOrNull(value.line);
  const [before, covered, after] = [value.before, value.covered, value.after];
  if (line === null || typeof before !== 'string' || typeof covered !== 'string' || typeof after !== 'string') return null;
  return { line, before, covered, after };
}

function readFrames(value: unknown): Frame[] {
  if (!Array.isArray(value)) return [];
  return value.filter(isObject).filter((frame) => typeof frame.title === 'string').map((frame) => ({
    title: frame.title as string,
    place: readPlace(frame.place) ?? { file: null, label: null, line: null, column: null, offset: null },
  }));
}

/** The problems of a message's field `problems`: those that are understood. */
export function readProblems(value: unknown): Problem[] {
  if (!Array.isArray(value)) return [];
  return value.filter(isObject).filter((problem) => typeof problem.message === 'string').map((problem) => ({
    kind: problem.kind === 'problem' ? 'problem' : 'diagnostic',
    severity: SEVERITIES.find((severity) => severity === problem.severity) ?? 'error',
    id: textOrNull(problem.id),
    data: problem.data ?? null,
    message: problem.message as string,
    place: readPlace(problem.place),
    frames: readFrames(problem.frames),
    bytes: readBytes(problem.bytes),
    excerpt: readExcerpt(problem.excerpt),
  }));
}

/**
 * The part of its source that `problem` covers, as the page's elements carry their places:
 * `null` when the problem lies in no part of a file's content. A place without an `offset`
 * is in a text that the library parsed under the file's name, such as the title, whose
 * `bytes` count from the start of that text, not as the content's elements count theirs.
 */
export function partOf(problem: Problem): Part | null {
  const file = problem.place?.file ?? null;
  return file !== null && problem.place?.offset != null && problem.bytes !== null ? { file, start: problem.bytes.start, end: problem.bytes.end } : null;
}

/**
 * A place to open in the editor, as `POST open` takes it: a byte offset into the file's
 * content, as the page's elements carry their places, or a line and a column (the column in
 * bytes), as a problem in a file's front matter gives them.
 */
export type OpenTarget = { file: string; start: number } | { file: string; line: number; column: number };

/**
 * The place of `problem` to open in the editor: its part of a file's content (`partOf`);
 * else, for a problem placed by a line and a column alone (in a
 * file's front matter: it has neither an `offset` nor `bytes`), that line and column;
 * `null` otherwise. A problem in a text parsed under the file's name, such as the title,
 * has `bytes` without an `offset`, and is not opened.
 */
export function openTargetOf(problem: Problem): OpenTarget | null {
  const part = partOf(problem);
  if (part !== null) return { file: part.file, start: part.start };
  const place = problem.place;
  if (place === null || place.file === null || place.line === null || place.column === null) return null;
  return place.offset === null && problem.bytes === null ? { file: place.file, line: place.line, column: place.column } : null;
}

/**
 * What `POST open` answers when the place is one to open but no editor opened (status
 * 200): the place, `PATH:LINE:COLUMN`, and the line that says why no editor opened.
 */
export interface Unopened {
  place: string;
  reason: string;
}

/** The place and the reason of the answer `response` of `POST open`; `null` for any other answer. */
export async function readUnopened(response: Response): Promise<Unopened | null> {
  if (response.status !== 200) return null;
  const answer: unknown = await response.json().catch(() => null);
  if (!isObject(answer) || typeof answer.place !== 'string' || typeof answer.reason !== 'string') return null;
  return { place: answer.place, reason: answer.reason };
}

/**
 * The line that tells of the place `unopened`: that it was copied to the clipboard, and
 * why no editor opened; when it could not be copied, the reason and the place.
 */
export function unopenedLine({ place, reason }: Unopened, copied: boolean): string {
  return copied ? `Copied ‘${place}’ to the clipboard (${reason})` : `${reason} (place ‘${place}’)`;
}

/** The labels of a message's field `references`, or `null` when it holds none. */
export function readReferences(value: unknown): Record<string, string> | null {
  if (!isObject(value)) return null;
  const references: Record<string, string> = {};
  for (const [id, label] of Object.entries(value)) {
    if (typeof label === 'string') references[id] = label;
  }
  return references;
}

/**
 * The headings of a message's field `outline`, or `null` when it holds none. Each heading
 * keeps its position, which a label's `heading` counts in: one that is not understood is
 * read with an empty title.
 */
export function readOutline(value: unknown): Heading[] | null {
  if (!Array.isArray(value)) return null;
  return value.map((each) => {
    const heading = isObject(each) ? each : {};
    return {
      level: numberOrNull(heading.level) ?? 1,
      number: textOrNull(heading.number),
      title: typeof heading.title === 'string' ? heading.title : '',
      id: textOrNull(heading.id),
    };
  });
}

/** The labels of a message's field `labels`: those that are understood, or `null` when it holds none. */
export function readLabels(value: unknown): LabelEntry[] | null {
  if (!Array.isArray(value)) return null;
  return value.filter(isObject).filter((entry) => typeof entry.label === 'string').map((entry) => ({
    label: entry.label as string,
    designation: typeof entry.designation === 'string' ? entry.designation : (entry.label as string),
    id: textOrNull(entry.id),
    kind: textOrNull(entry.kind),
    heading: numberOrNull(entry.heading),
  }));
}

/** `hello` as read from its data, or `null` when it is not understood. */
export function readHello(message: Fields): Hello | null {
  const build = message.build;
  if (typeof message.server !== 'string' || !(build === null || typeof build === 'number')) return null;
  return {
    server: message.server,
    document: textOrNull(message.document),
    build,
    building: message.building === true,
    problems: readProblems(message.problems),
    references: readReferences(message.references),
    help: message.help === true,
    ...readOutput(message),
  };
}

/**
 * The fields of `hello` and `update` that describe the document that the output shows, the
 * output served, the build types, the environment and the counter of the settings state.
 */
type Output = Pick<Hello, 'outline' | 'labels' | 'mediaType' | 'earlierOutput' | 'durationMs' | 'profile' | 'defaultProfile' | 'profiles' | 'environment' | 'settings'>;

function readOutput(message: Fields): Output {
  const duration = numberOrNull(message.duration_ms);
  const profile = textOrNull(message.profile);
  const defaultProfile = textOrNull(message.default_profile);
  const environment = textOrNull(message.environment);
  return {
    outline: readOutline(message.outline),
    labels: readLabels(message.labels),
    mediaType: textOrNull(message.media_type),
    earlierOutput: message.earlier_output === true,
    durationMs: duration !== null && duration >= 0 ? duration : null,
    profile: profile !== '' ? profile : null,
    defaultProfile: defaultProfile !== '' ? defaultProfile : null,
    profiles: Array.isArray(message.profiles)
      ? message.profiles.filter((name): name is string => typeof name === 'string' && name !== '') : [],
    environment: environment !== '' ? environment : null,
    settings: numberOrNull(message.settings) ?? 0,
  };
}

/** `update` as read from its data, or `null` when it is not understood. */
export function readUpdate(message: Fields): Update | null {
  const page = PAGE_CHANGES.find((change) => change === message.page);
  if (typeof message.build !== 'number' || page === undefined) return null;
  const regions = Array.isArray(message.regions) ? message.regions : [];
  return {
    build: message.build,
    page,
    regions: regions.filter(isObject).filter((region) => typeof region.id === 'string' && typeof region.text === 'string')
      .map((region) => ({
        id: region.id as string,
        name: textOrNull(region.name),
        kind: textOrNull(region.kind),
        text: region.text as string,
      })),
    files: Array.isArray(message.files) ? message.files.filter((file): file is string => typeof file === 'string') : [],
    problems: readProblems(message.problems),
    references: readReferences(message.references),
    ...readOutput(message),
  };
}

/**
 * The server and the build that the HTTP response carrying a page names in its header
 * `Server-Timing` (the metrics `flm-server` and `flm-build`), given the navigation
 * entry's `serverTiming`; `null` when it names none.
 */
export function servedBy(timing: readonly { name: string; description: string }[]): { server: string; build: number } | null {
  let server: string | null = null;
  let build: number | null = null;
  for (const metric of timing) {
    if (metric.name === 'flm-server') server = metric.description;
    else if (metric.name === 'flm-build' && /^[0-9]+$/.test(metric.description)) build = Number(metric.description);
  }
  return server !== null && build !== null ? { server, build } : null;
}
