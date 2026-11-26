// The Export tab's box, buttons and command line, for a document with errors, with warnings
// alone, with neither, and after an export refused for the document's errors.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { ExportResult } from '../src/tools/state/export';
import { commandShown, exportForm, resultAfterBuild, resultFor } from '../src/tools/state/export';

const PROBLEMS = { kind: 'problems' } as const;

const NAME = {
  command: 'flm render -p html -o paper.html paper.flm --view',
  commandContinuing: 'flm render -p html --continue-with-errors -o paper.html paper.flm --view',
};

test('with errors, the box holds the checkbox, which enables the buttons and the option of the command', () => {
  const unchecked = exportForm({ errors: 3, warnings: 1, notes: 2 }, false, null);
  assert.deepEqual(unchecked.box, { text: 'The document has 3 errors and 1 warning.', confirm: true, link: PROBLEMS });
  assert.equal(unchecked.enabled, false);
  assert.equal(commandShown(NAME, unchecked.continuing), NAME.command);
  const checked = exportForm({ errors: 1, warnings: 0, notes: 0 }, true, null);
  assert.deepEqual(checked.box, { text: 'The document has 1 error.', confirm: true, link: PROBLEMS });
  assert.equal(checked.enabled, true);
  assert.equal(commandShown(NAME, checked.continuing), NAME.commandContinuing);
});

test('with warnings alone, the box states their count, without the checkbox or the option', () => {
  for (const confirmed of [false, true]) {
    const form = exportForm({ errors: 0, warnings: 2, notes: 1 }, confirmed, null);
    assert.deepEqual(form.box, { text: 'The document has 2 warnings.', confirm: false, link: PROBLEMS });
    assert.equal(form.enabled, true);
    assert.equal(commandShown(NAME, form.continuing), NAME.command);
  }
});

test('with neither errors nor warnings, there is no box, and the command has no option', () => {
  for (const confirmed of [false, true]) {
    const form = exportForm({ errors: 0, warnings: 0, notes: 4 }, confirmed, null);
    assert.equal(form.box, null);
    assert.equal(form.enabled, true);
    assert.equal(commandShown(NAME, form.continuing), NAME.command);
  }
});

test('after an export refused for the document\'s errors, the box holds the server\'s line, the checkbox and the log, whatever the preview shows', () => {
  const reason = 'Document with 2 errors: an export despite the errors must be confirmed';
  const refused: ExportResult = { kind: 'refused', reason, log: 'error: one\nerror: two', documentErrors: true };
  for (const counts of [{ errors: 0, warnings: 0, notes: 0 }, { errors: 0, warnings: 1, notes: 0 }, { errors: 1, warnings: 0, notes: 0 }]) {
    const unchecked = exportForm(counts, false, refused);
    assert.deepEqual(unchecked.box, { text: reason, confirm: true, link: { kind: 'log', log: 'error: one\nerror: two' } });
    assert.equal(unchecked.enabled, false);
    assert.equal(commandShown(NAME, unchecked.continuing), NAME.command);
    const checked = exportForm(counts, true, refused);
    assert.equal(checked.enabled, true);
    assert.equal(commandShown(NAME, checked.continuing), NAME.commandContinuing);
  }
  // Another refusal, with the same line, is decided by its status alone: no box from it.
  const failed: ExportResult = { ...refused, documentErrors: false };
  assert.equal(exportForm({ errors: 0, warnings: 0, notes: 0 }, false, failed).box, null);
  assert.equal(exportForm({ errors: 0, warnings: 0, notes: 0 }, false, { kind: 'saved', name: 'paper.pdf' }).box, null);
  // The refusal shows under the build type exported alone, and goes once the stream reports
  // the problems again; any other result stays.
  const last = { profile: 'pdf', result: refused };
  assert.equal(resultFor(last, 'pdf'), refused);
  assert.equal(resultFor(last, 'html'), null);
  assert.equal(resultAfterBuild(last), null);
  for (const kept of [{ profile: 'pdf', result: failed }, { profile: 'pdf', result: { kind: 'saved', name: 'paper.pdf' } as const }]) {
    assert.equal(resultAfterBuild(kept), kept);
  }
});
