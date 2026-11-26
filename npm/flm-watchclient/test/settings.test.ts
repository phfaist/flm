// The rows of the Metadata and Settings tabs: the control that follows from each shape of a
// key's schema and from the value of the authors, the icon of each origin, and the write a
// control sends for a change and for a revert.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Origin } from '../src/shared/settings';
import { METADATA } from '../src/tools/state/description';
import { bodyOf, changed, controlOf, metadataControl, reverted, savedYaml, sourceOf, textEdit, textsEdit } from '../src/tools/state/settings';

const root = {
  $defs: {
    'a/b~c.Mark': { oneOf: [{ type: 'string', const: 'bracketed' }, { type: 'string', const: 'superscript' }] },
    'x.Depth': { type: 'integer', minimum: 0, default: 3 },
    'x.Map': { type: 'object', additionalProperties: { type: 'string' } },
  },
};

test('the control of a key follows from the shape of its schema, and any other shape is edited as YAML', () => {
  assert.deepEqual(controlOf(root, { type: 'boolean', default: true }), { kind: 'switch' });
  assert.deepEqual(controlOf(root, { type: 'integer', format: 'uint32' }), { kind: 'number', integer: true });
  assert.deepEqual(controlOf(root, { type: 'number', exclusiveMinimum: 0 }), { kind: 'number', integer: false });
  assert.deepEqual(controlOf(root, { type: 'string' }), { kind: 'text' });
  assert.deepEqual(controlOf(root, { type: ['string', 'null'] }), { kind: 'text' });
  assert.deepEqual(controlOf(root, { type: 'array', items: { type: 'string' } }), { kind: 'texts' });
  assert.deepEqual(controlOf(root, { enum: ['a', 'b'] }), {
    kind: 'choice', options: [{ label: 'a', value: 'a' }, { label: 'b', value: 'b' }],
  });
  // An enumeration of alternatives, reached through a reference whose name holds `/` and `~`.
  assert.deepEqual(controlOf(root, { $ref: '#/$defs/a~1b~0c.Mark', description: 'How a mark looks' }), {
    kind: 'choice', options: [{ label: 'bracketed', value: 'bracketed' }, { label: 'superscript', value: 'superscript' }],
  });
  // One alternative beside `null` is that alternative.
  assert.deepEqual(controlOf(root, { anyOf: [{ $ref: '#/$defs/x.Depth' }, { type: 'null' }] }), { kind: 'number', integer: true });
  for (const other of [
    { $ref: '#/$defs/x.Map' },
    { type: 'object', properties: { raster: { type: 'number' } } },
    { type: 'array', items: { type: 'object' } },
    { anyOf: [{ type: 'string' }, { const: false }] },
    { type: ['string', 'number', 'boolean'] },
    { $ref: '#/$defs/nowhere' },
    {},
  ]) {
    assert.deepEqual(controlOf(root, other), { kind: 'yaml' }, JSON.stringify(other));
  }
});

function origin(source: Origin['source'], fields: Partial<Origin> = {}): Origin {
  return { source, text: 'the program\'s defaults', package: null, extension: null, workflow: null, ...fields };
}

test('the authors are edited as YAML as soon as one author is written as a map', () => {
  const authors = METADATA.find((row) => row.key === 'author');
  assert.ok(authors !== undefined);
  assert.equal(metadataControl(authors, ['A. Author', 'B. Author']), 'texts');
  assert.equal(metadataControl(authors, [{ name: 'A. Author', email: 'a@example.com' }, 'B. Author']), 'yaml');
  assert.equal(metadataControl(authors, { name: 'A. Author', affiliation: ['X', 'Y'] }), 'yaml');
});

test('the icon at the right end of a row tells where the value comes from', () => {
  const lecturenotes = origin('package', { text: 'the package `lecturenotes#_`', package: 'lecturenotes#_', extension: 'lecturenotes' });
  const workflow = origin('workflow', { text: 'the workflow `html`', package: 'std', workflow: 'html' });
  const option = origin('command_line', { text: 'the option `-O toc.depth=2`' });
  const front = origin('front_matter', { text: 'paper.flm:5:7' });
  // Written by the front matter: the active revert arrow.
  assert.deepEqual(sourceOf(true, front), { kind: 'written' });
  // A key written beside a value that a package gives: the front matter's value is in effect.
  assert.deepEqual(sourceOf(true, lecturenotes), { kind: 'written' });
  // The default, of the program or of the schema: the revert arrow, greyed out.
  assert.deepEqual(sourceOf(false, origin('defaults')), { kind: 'default' });
  assert.deepEqual(sourceOf(false, null), { kind: 'default' });
  // From a package: the package icon, with the origin in its tooltip.
  assert.deepEqual(sourceOf(false, lecturenotes), { kind: 'package', line: 'From the package ‘lecturenotes’' });
  // Decided by a source that ranks above the front matter, written there or not: its name.
  assert.deepEqual(sourceOf(false, workflow), { kind: 'decided', line: 'Decided by the build type ‘html’', written: false });
  assert.deepEqual(sourceOf(true, option), { kind: 'decided', line: 'Decided by the option ‘-O toc.depth=2’', written: true });
  assert.deepEqual(sourceOf(false, origin('command_line', { text: 'the option `--use`' })), { kind: 'decided', line: 'Decided by the option ‘--use’', written: false });
});

test('a control sends the value set at its path for a change, and the removal of its key for a revert', () => {
  const path = ['flm', 'features', 'toc', 'depth'];
  assert.deepEqual(bodyOf('0123456789abcdef', [changed(path, 2)]), { revision: '0123456789abcdef', changes: [{ path, value: 2 }] });
  assert.deepEqual(bodyOf('0123456789abcdef', [reverted(path)]), { revision: '0123456789abcdef', changes: [{ path, remove: true }] });
  assert.deepEqual(bodyOf('r', [savedYaml(['flm', 'features', 'citations', 'style'], 'number_format: "#{roman}"\n')]), {
    revision: 'r', changes: [{ path: ['flm', 'features', 'citations', 'style'], yaml: 'number_format: "#{roman}"\n' }],
  });
  // Metadata: an emptied text field removes its key.
  assert.deepEqual(textEdit(['title'], 'A paper'), { path: ['title'], change: { value: 'A paper' } });
  assert.deepEqual(textEdit(['date'], ''), { path: ['date'], change: { remove: true } });
  // The chips of the authors: the list, one text where the front matter wrote one, or no key.
  assert.deepEqual(textsEdit(['author'], ['A.', 'B.'], ['A.', 'B.', 'C.']), { path: ['author'], change: { value: ['A.', 'B.', 'C.'] } });
  assert.deepEqual(textsEdit(['author'], 'A.', ['A. Author']), { path: ['author'], change: { value: 'A. Author' } });
  assert.deepEqual(textsEdit(['author'], ['A.', 'B.'], ['B.']), { path: ['author'], change: { value: ['B.'] } });
  assert.deepEqual(textsEdit(['author'], 'A.', []), { path: ['author'], change: { remove: true } });
});
