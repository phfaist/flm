// What the Export tab shows of the preview's last build and of the last export: the box that
// states the problems, whether the two buttons export, and which command line is shown. The
// errors and warnings counted are those of the preview's last build; an export refused for
// the document's errors states its own. Nothing here touches the page, so that the tests can
// read each case.

import type { Counts } from './build';

/**
 * The box above the buttons: its line, whether it holds the checkbox that confirms an
 * export despite the errors, and what its link shows: the Problems tab (*Show problems*),
 * or the log of an export refused for the document's errors (*Show log*, `null` when the
 * answer carries none).
 */
export interface ProblemsBox {
  text: string;
  confirm: boolean;
  link: { kind: 'problems' } | { kind: 'log'; log: string | null };
}

/** What the Export tab shows: see `exportForm`. */
export interface ExportForm {
  /** The box, or `null` when the document has neither errors nor warnings. */
  box: ProblemsBox | null;
  /** Whether *Save as sibling* and *Download* export. */
  enabled: boolean;
  /** Whether the export continues with errors, and the command shown has the option. */
  continuing: boolean;
}

/** `count` and `word`, in its plural when the count is not one. */
function counted(count: number, word: string): string {
  return `${count} ${count === 1 ? word : `${word}s`}`;
}

/** The sentence of the box: "The document has 3 errors and 1 warning." */
function sentence(errors: number, warnings: number): string {
  const parts = [errors > 0 ? counted(errors, 'error') : null, warnings > 0 ? counted(warnings, 'warning') : null];
  return `The document has ${parts.filter((part) => part !== null).join(' and ')}.`;
}

/** Whether `result` is an export refused for the document's errors (status 428). */
export function refusedForErrors(result: ExportResult | null): result is Extract<ExportResult, { kind: 'refused' }> {
  return result?.kind === 'refused' && result.documentErrors;
}

/**
 * The Export tab's form, for the problems `counts` of the preview's last build, the
 * checkbox `confirmed`, and the result of the last export of the build type chosen,
 * `result`. When that export was refused for the document's errors, the box holds the
 * server's line, the checkbox and the link to the export's log, whatever the preview
 * shows, since the build type exported may have errors that the preview has not; that box
 * holds until the stream reports the document's problems again (`resultAfterBuild`).
 * Otherwise, from the preview's problems: with errors, the box with the checkbox; with
 * warnings alone, the box without it; with neither, no box. Notes are not counted. With
 * the checkbox, the buttons export only once it is checked.
 */
export function exportForm({ errors, warnings }: Counts, confirmed: boolean, result: ExportResult | null): ExportForm {
  if (refusedForErrors(result)) {
    const box: ProblemsBox = { text: result.reason, confirm: true, link: { kind: 'log', log: result.log } };
    return { box, enabled: confirmed, continuing: confirmed };
  }
  const problems = { kind: 'problems' } as const;
  if (errors > 0) {
    return { box: { text: sentence(errors, warnings), confirm: true, link: problems }, enabled: confirmed, continuing: confirmed };
  }
  const box = warnings > 0 ? { text: sentence(0, warnings), confirm: false, link: problems } : null;
  return { box, enabled: true, continuing: false };
}

/**
 * The result shown under the build type `profile`: that of the last export, `last`, when it
 * exported this build type, else none. The tab's build type follows the preview's until the
 * author chooses one, so it can change after an export without the tab's selector.
 */
export function resultFor(last: LastExport | null, profile: string | null): ExportResult | null {
  return last !== null && last.profile === profile ? last.result : null;
}

/**
 * The last export, `last`, once the stream reports the document's problems again (`hello`
 * or `update`): none when it was refused for the document's errors, else as it is. A
 * refusal for errors states the errors of the build it ran on; once the stream reports the
 * problems again, the box follows that report. Any other result records a past action and
 * stays.
 */
export function resultAfterBuild(last: LastExport | null): LastExport | null {
  return last !== null && refusedForErrors(last.result) ? null : last;
}

/** The command line shown: the server's, with `--continue-with-errors` exactly when `continuing`. */
export function commandShown(name: { command: string | null; commandContinuing: string | null }, continuing: boolean): string | null {
  return continuing ? name.commandContinuing : name.command;
}

/** The result of the last export, as the Export tab shows it. */
export type ExportResult =
  /** The files were written beside the document; `name` is the export's name. */
  | { kind: 'saved'; name: string }
  /**
   * Nothing was written: the reason, the log when the answer carries one, and whether the
   * server refused the export for the document's errors (status 428).
   */
  | { kind: 'refused'; reason: string; log: string | null; documentErrors: boolean };

/** The last export: the build type it exported, and its result. */
export interface LastExport {
  profile: string;
  result: ExportResult;
}
