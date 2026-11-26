// The requests of an export (`GET` and `POST /.flm-preview/export`, `POST
// /.flm-preview/export/open`), as the tools page reads their answers. `PROTOCOL.md`
// specifies them. The browser sends no path: the server names and places every export.

import { isObject } from './protocol';

/** The request of an export's name and command lines, and the request of an export. */
export const EXPORT = '/.flm-preview/export';

/** The request that opens the main file of the last export saved outside the browser. */
export const EXPORT_OPEN = '/.flm-preview/export/open';

/** The name of an export of a build type, and the command lines that write the same files. */
export interface ExportName {
  profile: string;
  /** The export's name, `STEM.EXT`. */
  name: string;
  /** The command line without `--continue-with-errors`; `null` when the program composes none. */
  command: string | null;
  /** The command line with `--continue-with-errors`; `null` when the program composes none. */
  commandContinuing: string | null;
}

/** What an export does with its files: write them beside the document, or answer them. */
export type ExportAction = 'save' | 'download';

/** The body of `POST /.flm-preview/export`. */
export interface ExportOrder {
  profile: string;
  action: ExportAction;
  continue_with_errors: boolean;
}

function textOrNull(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

/** The answer of `GET /.flm-preview/export`; `null` when it is not understood. */
export function readExportName(value: unknown): ExportName | null {
  if (!isObject(value) || typeof value.profile !== 'string' || typeof value.name !== 'string') return null;
  return {
    profile: value.profile,
    name: value.name,
    command: textOrNull(value.command),
    commandContinuing: textOrNull(value.command_continuing),
  };
}

/** The name of a saved export, from the answer of a save; `null` when it is not understood. */
export function readSavedName(value: unknown): string | null {
  return isObject(value) && typeof value.name === 'string' ? value.name : null;
}

/**
 * The file name of a download, from its header `Content-Disposition`
 * (`attachment; filename*=UTF-8''paper.html`); `null` when the header names none.
 */
export function downloadName(disposition: string | null): string | null {
  const encoded = disposition?.match(/filename\*=UTF-8''([^;]+)/i)?.[1];
  if (encoded === undefined) return null;
  try {
    return decodeURIComponent(encoded.trim());
  } catch {
    return null;
  }
}

/**
 * Why nothing was written, from the text of a refused export: its first line, and the log
 * that follows a blank line, or `null` when the answer carries none.
 */
export function refusalOf(text: string): { reason: string; log: string | null } {
  const [reason = '', blank, ...rest] = text.replace(/\r\n/g, '\n').split('\n');
  const log = blank === '' ? rest.join('\n').trim() : '';
  return { reason: reason.trim(), log: log !== '' ? log : null };
}
