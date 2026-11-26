// The state of the preview as the stream reports it, and what the status bar, the output
// view and the browser tab show of it. `step` gives the state after an event; everything
// else is read from the state. Nothing here touches the page, so that the tests can run
// each sequence of events.

import type { Heading, Hello, LabelEntry, Problem, Update } from '../../shared/protocol';

/** How long a build runs before the status bar says that it is building, in milliseconds. */
export const SLOW_BUILD_MS = 300;

/** The state of the connection to the server's stream. */
export type Connection = 'connecting' | 'connected' | 'disconnected';

/** What the tools page knows of the server and of its builds. */
export interface BuildState {
  connection: Connection;
  /** The server's identifier, from the first `hello`; `null` before it. */
  server: string | null;
  /** The document's path, as given to the server. */
  document: string | null;
  /** The number of the build whose output the server holds, or `null` while it holds none. */
  build: number | null;
  /** Whether a build runs. */
  building: boolean;
  /** Counts the builds started, so that a timer of an earlier build is told apart. */
  started: number;
  /** Whether the build that runs has lasted longer than `SLOW_BUILD_MS`. */
  slow: boolean;
  /**
   * The text of the progress line of the build that runs, from its last `progress` message;
   * `null` before the first, and once the build is over.
   */
  progress: string | null;
  /** How long the last build that finished took, in milliseconds. */
  durationMs: number | null;
  /** Whether the output served is from an earlier build than the last one that finished. */
  earlierOutput: boolean;
  /** The media type of the file served at `/`, or `null` while there is none. */
  mediaType: string | null;
  /**
   * Counts the changes of the output served: once for each `update` that reports a change of
   * the page, and for each `hello` that names another build than the one held.
   */
  revision: number;
  /** The build type in use, as the server reports it, or `null` before it knows one. */
  profile: string | null;
  /** The default build type, as the server reports it, or `null` before it knows one. */
  defaultProfile: string | null;
  /** The build types available, as the server reports them. */
  profiles: string[];
  /** The digest of the document's environment, as the server reports it. */
  environment: string | null;
  /** The counter of the settings state, as the server reports it. */
  settings: number;
  /** The problems of the last build. */
  problems: Problem[];
  /** The headings of the document that the output shows, or `null` while there is none. */
  outline: Heading[] | null;
  /** The labels of the document that the output shows, or `null` while there is none. */
  labels: LabelEntry[] | null;
  /** Whether the program gives the help pages. */
  help: boolean;
  /**
   * The line of the last request that failed, or that ended otherwise than asked without
   * failing (a place copied for want of an editor), until the next build or connection.
   */
  notice: Line | null;
}

/** What changes the state: the stream's messages, its connection, a timer, a request. */
export type BuildEvent =
  | { type: 'hello'; hello: Hello }
  | { type: 'building' }
  | { type: 'progress'; text: string }
  | { type: 'update'; update: Update }
  | { type: 'disconnected' }
  | { type: 'slow'; started: number }
  | { type: 'failed'; text: string }
  | { type: 'noted'; text: string };

export const INITIAL: BuildState = {
  connection: 'connecting',
  server: null,
  document: null,
  build: null,
  building: false,
  started: 0,
  slow: false,
  progress: null,
  durationMs: null,
  earlierOutput: false,
  mediaType: null,
  revision: 0,
  profile: null,
  defaultProfile: null,
  profiles: [],
  environment: null,
  settings: 0,
  problems: [],
  outline: null,
  labels: null,
  help: false,
  notice: null,
};

/** The state after `event`. */
export function step(state: BuildState, event: BuildEvent): BuildState {
  switch (event.type) {
    case 'hello': {
      const { hello } = event;
      const started = hello.building && !state.building ? state.started + 1 : state.started;
      return {
        ...state,
        connection: 'connected',
        server: hello.server,
        document: hello.document,
        build: hello.build,
        building: hello.building,
        started,
        slow: hello.building && state.building && state.slow,
        progress: hello.building && state.building ? state.progress : null,
        durationMs: hello.durationMs,
        earlierOutput: hello.earlierOutput,
        mediaType: hello.mediaType,
        revision: hello.build !== state.build ? state.revision + 1 : state.revision,
        profile: hello.profile,
        defaultProfile: hello.defaultProfile,
        profiles: hello.profiles,
        environment: hello.environment,
        settings: hello.settings,
        problems: hello.problems,
        outline: hello.outline,
        labels: hello.labels,
        help: hello.help,
        notice: null,
      };
    }
    case 'building':
      // A build that starts while one runs follows a build that was cancelled: the
      // progress text is that of the cancelled build, and is cleared.
      if (state.building) return state.progress === null ? state : { ...state, progress: null };
      return { ...state, building: true, started: state.started + 1, slow: false, progress: null, notice: null };
    case 'progress':
      return state.building ? { ...state, progress: event.text } : state;
    case 'update': {
      const { update } = event;
      return {
        ...state,
        build: update.page === 'absent' ? state.build : update.build,
        building: false,
        slow: false,
        progress: null,
        durationMs: update.durationMs,
        earlierOutput: update.earlierOutput,
        mediaType: update.mediaType,
        revision: update.page === 'regions' || update.page === 'reload' ? state.revision + 1 : state.revision,
        profile: update.profile,
        defaultProfile: update.defaultProfile,
        profiles: update.profiles,
        environment: update.environment,
        settings: update.settings,
        problems: update.problems,
        outline: update.outline,
        labels: update.labels,
        notice: null,
      };
    }
    case 'disconnected':
      return { ...state, connection: 'disconnected', building: false, slow: false, progress: null };
    case 'slow':
      return state.building && event.started === state.started ? { ...state, slow: true } : state;
    case 'failed':
      return { ...state, notice: { text: event.text, tone: 'error' } };
    case 'noted':
      return { ...state, notice: { text: event.text, tone: 'plain' } };
  }
}

/** Whether `next` comes from another server than the one `state` describes. */
export function isOtherServer(state: BuildState, next: Hello): boolean {
  return state.server !== null && state.server !== next.server;
}

/** The tone of a text of the status bar: its color. */
export type Tone = 'muted' | 'plain' | 'active' | 'warning' | 'error';

/** The build state of the status bar: a few words and their tone; `null` for none. */
export interface Line {
  text: string;
  tone: Tone;
}

/** The build state that the status bar shows. */
export function buildLine(state: BuildState): Line | null {
  if (state.connection === 'disconnected') return { text: 'Disconnected', tone: 'error' };
  if (state.notice !== null) return state.notice;
  if (state.building && state.slow) return { text: state.progress ?? 'Building…', tone: 'active' };
  if (state.earlierOutput) return { text: 'Last successful build', tone: 'warning' };
  if (state.durationMs !== null) return { text: `Built in ${duration(state.durationMs)}`, tone: 'muted' };
  return null;
}

/** A duration in seconds, as the status bar writes it: `0.4 s`, `0.05 s`, `12 s`. */
export function duration(ms: number): string {
  const seconds = ms / 1000;
  if (seconds < 0.1) return `${seconds.toFixed(2)} s`;
  if (seconds < 10) return `${seconds.toFixed(1)} s`;
  return `${Math.round(seconds)} s`;
}

/** The counts of the last build's problems by severity, which add up to their number. */
export interface Counts {
  errors: number;
  warnings: number;
  notes: number;
}

export function counts(problems: readonly Problem[]): Counts {
  return {
    errors: problems.filter((problem) => problem.severity === 'error').length,
    warnings: problems.filter((problem) => problem.severity === 'warning').length,
    notes: problems.filter((problem) => problem.severity === 'note').length,
  };
}

/**
 * What the output view shows: nothing yet (`none`), the page in a frame, the PDF view, the
 * text view (which shows the file's text when it is a text, `textOf`, and otherwise one
 * line that names the media type), or one line that says that the build type gives no
 * output (`empty`).
 */
export type View =
  | { kind: 'none' }
  | { kind: 'frame' }
  | { kind: 'text'; mediaType: string }
  | { kind: 'pdf' }
  | { kind: 'empty'; profile: string | null };

/** The view of the output served, chosen from the media type of its entry file. */
export function view(state: BuildState): View {
  if (state.mediaType === null) {
    // An output is held, and it has no entry file: the build type writes no file. The line
    // names the build type only when that output is the last build's: an output kept from
    // an earlier build can be another build type's.
    if (state.build === null) return { kind: 'none' };
    return { kind: 'empty', profile: state.earlierOutput ? null : state.profile };
  }
  // The type itself, without its parameters (`text/html; charset=utf-8`).
  const essence = state.mediaType.split(';')[0].trim().toLowerCase();
  if (essence === 'text/html') return { kind: 'frame' };
  if (essence === 'application/pdf') return { kind: 'pdf' };
  return { kind: 'text', mediaType: essence };
}

/**
 * Control characters that a text does not hold: those of C0 but the tab, the line feed, the
 * form feed, the carriage return and the escape (which begins a terminal's sequences of
 * colors and styles), and DEL.
 */
const NOT_IN_TEXT = /[\x00-\x08\x0b\x0e-\x1a\x1c-\x1f\x7f]/;

/** The text of `bytes` when they are a text: valid UTF-8 without the control characters of `NOT_IN_TEXT`; else `null`. */
export function textOf(bytes: Uint8Array): string | null {
  let text: string;
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    return null;
  }
  return NOT_IN_TEXT.test(text) ? null : text;
}

/** The media types of LaTeX, which the text view highlights as LaTeX. */
const LATEX_TYPES: readonly string[] = ['application/x-latex', 'application/x-tex', 'text/x-latex', 'text/x-tex'];

/** The language in which the text view highlights a text of the media type `mediaType` (without parameters), or `null` for none. */
export function languageOf(mediaType: string): 'latex' | null {
  return LATEX_TYPES.includes(mediaType) ? 'latex' : null;
}

/** The state that the browser tab's icon draws: the worst severity present, or `none` for no problem. */
export type Mark = 'connecting' | 'disconnected' | 'building' | 'errors' | 'warnings' | 'notes' | 'none';

export function mark(state: BuildState): Mark {
  if (state.connection !== 'connected') return state.connection;
  if (state.building) return 'building';
  const { errors, warnings, notes } = counts(state.problems);
  if (errors > 0) return 'errors';
  if (warnings > 0) return 'warnings';
  if (notes > 0) return 'notes';
  return 'none';
}

/** The short name of the document's path `path`: its last part, without the folders. */
export function shortName(path: string): string {
  return path.slice(Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1);
}

/** The browser tab's title. */
export function title(state: BuildState): string {
  return state.document === null ? 'FLM preview' : `${state.document} – FLM preview`;
}
