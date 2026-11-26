// The Inspect card: which of the constructs at the start of the part inspected are the
// element's and which are around it.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Construct } from '../src/shared/help';
import { constructsOf } from '../src/tools/state/inspect';

function construct(kind: string, name: string, start: number, end: number, file = 'paper.flm'): Construct {
  return { kind, name, file, start, end, feature: null, help: null };
}

// `\begin{theorem}\textbf{Remark.} Some text.\end{theorem}`, innermost first, as `at`
// answers at the start of the paragraph.
const THEOREM = construct('environment', 'theorem', 0, 57);
const TEXTBF = construct('macro', 'textbf', 15, 31);

test('a command at the start of a paragraph is inside the paragraph, not its construct', () => {
  const paragraph = { file: 'paper.flm', start: 15, end: 43 };
  assert.deepEqual(constructsOf([TEXTBF, THEOREM], paragraph), [THEOREM]);
});

test('the construct whose range is the element is the first, the others are around it', () => {
  assert.deepEqual(constructsOf([TEXTBF, THEOREM], { file: 'paper.flm', start: 15, end: 31 }), [TEXTBF, THEOREM]);
  assert.deepEqual(constructsOf([THEOREM], { file: 'paper.flm', start: 0, end: 57 }), [THEOREM]);
});

test('a construct of another file is never the element\'s', () => {
  const other = construct('macro', 'input', 0, 57, 'main.flm');
  assert.deepEqual(constructsOf([other], { file: 'paper.flm', start: 15, end: 43 }), []);
});
