// The tools page's requests to the server. A request that fails gives the line of text the
// server answered, which the status bar shows as it is.

import type { OpenTarget, Unopened } from '../shared/protocol';
import { ENTRY, OPEN, PROFILE, REBUILD, UNREACHED, failure, isObject, readUnopened, unopenedLine } from '../shared/protocol';
import type { ConstructsAt, HelpIndex, PageAnswer } from '../shared/help';
import { AT, HELP_INDEX, HELP_PAGE, readConstructsAt, readHelpIndex, readPageAnswer } from '../shared/help';
import type { ExportName, ExportOrder } from '../shared/export';
import { EXPORT, EXPORT_OPEN, downloadName, readExportName, readSavedName, refusalOf } from '../shared/export';
import type { Copied, FrontMatterWrite, SettingsState, Sources } from '../shared/settings';
import { FRONT_MATTER, SETTINGS, SETTINGS_SOURCES, SETTINGS_YAML, pointerOf, readCopied, readSettings, readSources } from '../shared/settings';
import type { ViewerChange } from '../shared/viewer';
import { VIEWER } from '../shared/viewer';
import type { ExportResult } from './state/export';
import { dispatch } from './state/store';

/**
 * Sends the request `POST path`, which does `action`, with `body` as JSON when given; a
 * failure changes the state. Gives whether the request succeeded.
 */
async function post(path: string, action: string, body?: unknown): Promise<boolean> {
  const init: RequestInit = body === undefined ? { method: 'POST' }
    : { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
  try {
    const response = await fetch(path, init);
    if (response.ok) return true;
    dispatch({ type: 'failed', text: await failure(response, action) });
  } catch {
    dispatch({ type: 'failed', text: UNREACHED });
  }
  return false;
}

/** Asks the server to build the document again. */
export function rebuild(): void {
  void post(REBUILD, 'build the document again');
}

/**
 * Says in the status bar that the place `unopened`, which the server answered for want of
 * an editor, was copied to the clipboard (`copied`) or could not be: with the server's
 * reason, as a notice when copied, and with the place as well, as a failure when not.
 */
export function reportUnopened(unopened: Unopened, copied: boolean): void {
  dispatch({ type: copied ? 'noted' : 'failed', text: unopenedLine(unopened, copied) });
}

/** Copies the place `unopened` to the clipboard, for *Open in editor*, and says so (`reportUnopened`). */
async function copyPlace(unopened: Unopened): Promise<void> {
  const copied = await navigator.clipboard.writeText(unopened.place).then(() => true, () => false);
  reportUnopened(unopened, copied);
}

/**
 * Asks the server to open the place `target` in the editor. The request names no editor:
 * the server opens the one of the viewer settings. When no editor can be opened, the server
 * answers the place, which is copied to the clipboard (`copyPlace`).
 */
export async function openPlace(target: OpenTarget): Promise<void> {
  try {
    const response = await fetch(OPEN, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(target),
    });
    const unopened = await readUnopened(response.clone());
    if (unopened !== null) await copyPlace(unopened);
    else if (!response.ok) dispatch({ type: 'failed', text: await failure(response, 'open the editor') });
  } catch {
    dispatch({ type: 'failed', text: UNREACHED });
  }
}

/**
 * Asks the server to build the document with the profile `name` from now on. Gives whether
 * the server accepted the request; the build type in use is the one the stream reports.
 */
export function chooseProfile(name: string): Promise<boolean> {
  return post(PROFILE, 'change the build type', { profile: name });
}

/**
 * Asks the server to change the viewer settings named in `change`. Gives whether the server
 * made the change; the settings in effect are those the stream reports.
 */
export function changeViewer(change: ViewerChange): Promise<boolean> {
  return post(VIEWER, 'change the viewer settings', change);
}

/**
 * The output's entry file, served at `/`, as `read` gives it from the response; `null`
 * when it cannot be read, which shows a line that names what failed (`action`).
 */
async function entry<T>(read: (response: Response) => Promise<T>, action: string): Promise<T | null> {
  try {
    const response = await fetch(ENTRY, { cache: 'no-store' });
    if (response.ok) return await read(response);
    dispatch({ type: 'failed', text: await failure(response, action) });
  } catch {
    dispatch({ type: 'failed', text: UNREACHED });
  }
  return null;
}

/**
 * The bytes of the output's entry file, served at `/`; `null` when it cannot be read, the
 * failure's line then saying that the tools page could not do `action` ("read the PDF output").
 */
export function entryBytes(action: string): Promise<Uint8Array | null> {
  return entry(async (response) => new Uint8Array(await response.arrayBuffer()), action);
}

/** The settings state; `null` when it cannot be read. */
export async function settingsState(): Promise<SettingsState | null> {
  try {
    const response = await fetch(SETTINGS, { cache: 'no-store' });
    if (!response.ok) {
      dispatch({ type: 'failed', text: await failure(response, 'read the settings') });
      return null;
    }
    const state = readSettings(await response.json());
    if (state === null) dispatch({ type: 'failed', text: 'Failed to read the settings: the server’s answer is not a settings state' });
    return state;
  } catch {
    dispatch({ type: 'failed', text: UNREACHED });
  }
  return null;
}

/**
 * The value at `path` of the front matter as YAML text, which the YAML editor starts from;
 * `null` when it cannot be read.
 */
export async function valueAsYaml(path: readonly string[]): Promise<string | null> {
  try {
    const response = await fetch(`${SETTINGS_YAML}?path=${encodeURIComponent(pointerOf(path))}`, { cache: 'no-store' });
    if (response.ok) return await response.text();
    dispatch({ type: 'failed', text: await failure(response, 'read the value as YAML') });
  } catch {
    dispatch({ type: 'failed', text: UNREACHED });
  }
  return null;
}

/**
 * Asks the server to make the changes of `write` to the front matter, together. Gives the
 * values that the server copied into the front matter beside them, or `null` when it did
 * not make them; refused changes show the server's line of text.
 */
export async function changeFrontMatter(write: FrontMatterWrite): Promise<Copied[] | null> {
  try {
    const response = await fetch(FRONT_MATTER, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(write),
    });
    if (response.ok) {
      const answer: unknown = await response.json().catch(() => null);
      return readCopied(isObject(answer) ? answer.copied : null);
    }
    dispatch({ type: 'failed', text: await failure(response, 'change the front matter') });
  } catch {
    dispatch({ type: 'failed', text: UNREACHED });
  }
  return null;
}

/** What a request that reads gave: the value read, or the line of text that says why it failed. */
export type Answer<T> = { value: T } | { failure: string };

/**
 * Reads `address` as JSON, understood by `read`; a failure gives the server's line of text,
 * or one that names what failed (`action`). The status bar is left as it is: the part that
 * asked shows the failure where it is needed.
 */
async function readJson<T>(address: string, read: (value: unknown) => T | null, action: string): Promise<Answer<T>> {
  try {
    const response = await fetch(address, { cache: 'no-store' });
    if (!response.ok) return { failure: await failure(response, action) };
    const value = read(await response.json());
    return value !== null ? { value } : { failure: `Failed to ${action}: the server’s answer is not understood` };
  } catch {
    return { failure: UNREACHED };
  }
}

/** What each settings source writes at `path` of the front matter. */
export function settingSources(path: readonly string[]): Promise<Answer<Sources>> {
  return readJson(`${SETTINGS_SOURCES}?path=${encodeURIComponent(pointerOf(path))}`, readSources, 'read the sources of the setting');
}

/** The constructs at the byte `start` of the source file `file`. */
export function constructsAt(file: string, start: number): Promise<Answer<ConstructsAt>> {
  const query = `file=${encodeURIComponent(file)}&start=${start}`;
  return readJson(`${AT}?${query}`, readConstructsAt, 'read the constructs at the place');
}

/** The index of the help pages. */
export function helpIndex(): Promise<Answer<HelpIndex>> {
  return readJson(HELP_INDEX, readHelpIndex, 'read the index of the help pages');
}

/** The help page at `path`, or the pages nearest to it. */
export function helpPage(path: string): Promise<Answer<PageAnswer>> {
  return readJson(`${HELP_PAGE}?path=${encodeURIComponent(path)}`, readPageAnswer, 'read the help page');
}

/** The name of an export of the build type `profile`, and its command lines. */
export function exportName(profile: string): Promise<Answer<ExportName>> {
  return readJson(`${EXPORT}?profile=${encodeURIComponent(profile)}`, readExportName, 'read the name of the export');
}

/** Hands the browser the file `blob`, named `name`, to save. */
function hand(blob: Blob, name: string): void {
  const address = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = address;
  link.download = name;
  link.click();
  window.setTimeout(() => URL.revokeObjectURL(address), 0);
}

/**
 * Exports the document as `order` says. A save gives the name written; a download hands
 * the browser the file, or the archive of the files, and gives `null`; an export refused
 * gives why nothing was written.
 */
export async function exportDocument(order: ExportOrder): Promise<ExportResult | null> {
  const action = order.action === 'save' ? 'save the export' : 'download the export';
  try {
    const response = await fetch(EXPORT, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(order),
    });
    if (!response.ok) {
      const { reason, log } = refusalOf(await response.text().catch(() => ''));
      const documentErrors = response.status === 428;
      return { kind: 'refused', reason: reason !== '' ? reason : `Failed to ${action} (the server answered with the status ${response.status})`, log, documentErrors };
    }
    if (order.action === 'download') {
      const name = downloadName(response.headers.get('Content-Disposition'));
      hand(await response.blob(), name ?? 'export');
      return null;
    }
    const name = readSavedName(await response.json());
    return name !== null ? { kind: 'saved', name } : { kind: 'refused', reason: `Failed to ${action}: the server’s answer is not understood`, log: null, documentErrors: false };
  } catch {
    return { kind: 'refused', reason: UNREACHED, log: null, documentErrors: false };
  }
}

/** Asks the server to open the main file of the last export saved with the system's default application. */
export function openExported(): void {
  void post(EXPORT_OPEN, 'open the exported file');
}
