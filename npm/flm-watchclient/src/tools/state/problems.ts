// The rows of the Problems tab: the problems of the last build, by file, each row one
// problem or several alike. Nothing here touches the page, so that the tests can run it.

import type { Place, Problem } from '../../shared/protocol';

/**
 * One row: the problems it stands for, the first of which it shows, and the key by which
 * a selection finds it again after the next build.
 */
export interface Row {
  key: string;
  problems: Problem[];
}

/** The rows of the problems of one file, or of the problems placed in no file. */
export interface FileRows {
  file: string | null;
  rows: Row[];
}

/**
 * What makes problems alike: the same identifier, the same data and the same place. Only
 * diagnostics are alike; a problem that stopped the build has no identifier, and is never
 * merged.
 */
function likeness(problem: Problem): string | null {
  if (problem.kind !== 'diagnostic' || problem.id === null) return null;
  const { place, bytes } = problem;
  return JSON.stringify([problem.id, problem.data, place?.file ?? null, place?.label ?? null,
    place?.line ?? null, place?.column ?? null, bytes?.start ?? null, bytes?.end ?? null]);
}

/** Orders two places by line, then by column; a place with neither comes first. */
function byPlace(a: Place | null, b: Place | null): number {
  return (a?.line ?? 0) - (b?.line ?? 0) || (a?.column ?? 0) - (b?.column ?? 0);
}

/**
 * The rows of `problems`: the problems placed in no file first, then those of each file,
 * in the order the files first appear; in each, the rows by line and column, problems at
 * the same place in the order reported. Problems alike share one row.
 */
export function rowsOf(problems: readonly Problem[]): FileRows[] {
  const files = new Map<string | null, Row[]>([[null, []]]);
  const rows = new Map<string, Row>();
  problems.forEach((problem, index) => {
    const like = likeness(problem);
    const row = like === null ? undefined : rows.get(like);
    if (row !== undefined) {
      row.problems.push(problem);
      return;
    }
    const added = { key: like ?? `#${index}:${problem.message}`, problems: [problem] };
    if (like !== null) rows.set(like, added);
    const file = problem.place?.file ?? null;
    const list = files.get(file) ?? [];
    list.push(added);
    files.set(file, list);
  });
  return Array.from(files, ([file, list]) => ({
    file,
    rows: list.slice().sort((a, b) => byPlace(a.problems[0]!.place, b.problems[0]!.place)),
  })).filter((group) => group.rows.length > 0);
}

/** `place` as the row shows it, beside a problem of the file `file`. */
export function placeText(place: Place | null, file: string | null): string {
  if (place === null) return '';
  const at = place.line !== null
    ? place.column !== null ? `line ${place.line}, column ${place.column}` : `line ${place.line}`
    : '';
  const where = place.file ?? place.label;
  if (where === null || where === file) return at;
  return at === '' ? where : `${where}, ${at}`;
}
