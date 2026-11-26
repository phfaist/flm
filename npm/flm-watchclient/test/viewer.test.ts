// The panel opens by itself only by the viewer setting, only when it is closed, and only after
// a build with an error; an open panel keeps the tab it shows.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Problem, Severity } from '../src/shared/protocol';
import { DEFAULT_VIEWER } from '../src/shared/viewer';
import { opensProblems } from '../src/tools/state/viewer';

function problem(severity: Severity): Problem {
  return {
    kind: 'diagnostic', severity, id: 'flm.x', data: {}, message: 'x',
    place: null, frames: [], bytes: null, excerpt: null,
  };
}

test('a build opens a closed panel on Problems only when the setting is on and it has an error', () => {
  const on = { ...DEFAULT_VIEWER, openProblemsOnErrors: true };
  const errors = [problem('warning'), problem('error')];
  assert.equal(opensProblems(on, false, errors), true);
  assert.equal(opensProblems(on, false, [problem('warning'), problem('note')]), false);
  assert.equal(opensProblems(on, false, []), false);
  // Off, the default: no build opens the panel.
  assert.equal(opensProblems(DEFAULT_VIEWER, false, errors), false);
  // An open panel keeps the tab it shows.
  assert.equal(opensProblems(on, true, errors), false);
});
