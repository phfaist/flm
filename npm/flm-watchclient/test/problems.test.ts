// The rows of the Problems tab: problems alike share a row, nothing else is merged.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Problem } from '../src/shared/protocol';
import { rowsOf } from '../src/tools/state/problems';

function diagnostic(id: string, line: number, start: number, more: Partial<Problem> = {}): Problem {
  return {
    kind: 'diagnostic', severity: 'error', id, data: { name: 'x' }, message: `${id} at ${line}`,
    place: { file: 'a.flm', label: null, line, column: 3, offset: start + 30 }, frames: [], bytes: { start, end: start + 2 }, excerpt: null,
    ...more,
  };
}

test('problems with the same identifier, data and place share one row, the others do not', () => {
  const frames = [{ title: 'an argument', place: { file: 'a.flm', label: null, line: 2, column: 1, offset: 10 } }];
  const first = diagnostic('flm.a', 4, 40);
  const again = diagnostic('flm.a', 4, 40, { frames });
  const otherData = diagnostic('flm.a', 4, 40, { data: { name: 'y' } });
  const otherPlace = diagnostic('flm.a', 4, 41);
  const otherId = diagnostic('flm.b', 4, 40);
  const stopped: Problem = { ...diagnostic('', 1, 0), kind: 'problem', id: null, bytes: null };
  const files = rowsOf([first, again, otherData, otherPlace, otherId, stopped, { ...stopped }]);
  assert.equal(files.length, 1);
  const counts = files[0]!.rows.map((row) => row.problems.length);
  assert.deepEqual(counts, [1, 1, 2, 1, 1, 1]);
  const merged = files[0]!.rows.find((row) => row.problems.length === 2)!;
  assert.deepEqual(merged.problems, [first, again]);
});
