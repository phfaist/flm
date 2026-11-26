// The Outline tab: the code that refers to a label, the one list of headings and labels, the
// filter of the labels by kind, and the search that keeps the headings a match stands under.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Heading, LabelEntry } from '../src/shared/protocol';
import { readOutline } from '../src/shared/protocol';
import { referenceCode } from '../src/tools/state/description';
import type { OutlineRow } from '../src/tools/state/outline';
import { ALL_LABELS, labelKinds, onlyKind, outlineRows, withKind } from '../src/tools/state/outline';

function heading(level: number, title: string): Heading {
  return { level, number: null, title, id: title.toLowerCase() };
}

function label(name: string, designation: string, kind: string | null, under: number | null): LabelEntry {
  return { label: name, designation, id: null, kind, heading: under };
}

const OUTLINE = [heading(1, 'Intro'), heading(2, 'Setting'), heading(1, 'Results')];
const LABELS = [
  label('eq:first', '(1)', 'equation', null),
  label('sec:intro', '§1', 'section', 0),
  label('defterm:Entropy', 'Entropy', 'defterm', 1),
  label('defterm:Entropies', 'Entropy', 'defterm', 1),
  label('fig:plot', 'Fig. 1', 'figure', 2),
  label('x:odd', 'odd', null, 2),
];

/** A row as a short text: `# title` for a heading, the label for a label. */
function shown(rows: readonly OutlineRow[]): string[] {
  return rows.map((row) => (row.kind === 'heading' ? `# ${row.heading.title}` : row.entry.label));
}

test('a term is referred to with \\term and its label proper, every other label with \\ref', () => {
  assert.equal(referenceCode('defterm:Markov chains'), '\\term{Markov chains}');
  assert.equal(referenceCode('term:mc'), '\\ref{term:mc}');
  assert.equal(referenceCode('sec:a:b'), '\\ref{sec:a:b}');
});

test('each heading is followed by the labels under it, the labels before the first heading first', () => {
  assert.deepEqual(shown(outlineRows(OUTLINE, LABELS, ALL_LABELS, '')), [
    'eq:first',
    '# Intro',
    'sec:intro',
    '# Setting',
    'defterm:Entropy',
    'defterm:Entropies',
    '# Results',
    'fig:plot',
    'x:odd',
  ]);
  const rows = outlineRows(OUTLINE, LABELS, ALL_LABELS, '');
  const entropy = rows.find((row) => row.kind === 'label' && row.entry.label === 'defterm:Entropy');
  assert.equal(entropy?.kind === 'label' && entropy.code, '\\term{Entropy}');
});

test('the filter offers the kinds in the order they first occur, a label without a kind counted as other', () => {
  assert.deepEqual(labelKinds(LABELS), [
    { kind: 'equation', count: 1 },
    { kind: 'section', count: 1 },
    { kind: 'defterm', count: 2 },
    { kind: 'figure', count: 1 },
    { kind: 'other', count: 1 },
  ]);
});

test('a toggle off hides the labels of its kind, and the first one hides every label', () => {
  const noTerms = withKind(withKind(ALL_LABELS, 'defterm', false), 'other', false);
  assert.deepEqual(shown(outlineRows(OUTLINE, LABELS, noTerms, '')), ['eq:first', '# Intro', 'sec:intro', '# Setting', '# Results', 'fig:plot']);
  assert.deepEqual(withKind(noTerms, 'defterm', true).hidden, ['other']);
  assert.deepEqual(shown(outlineRows(OUTLINE, LABELS, { ...ALL_LABELS, labels: false }, '')), ['# Intro', '# Setting', '# Results']);
});

test('a kind asked for alone hides the others, and asked for again shows them all', () => {
  const kinds = labelKinds(LABELS).map((each) => each.kind);
  const figures = onlyKind(ALL_LABELS, 'figure', kinds);
  assert.deepEqual(figures.hidden, ['equation', 'section', 'defterm', 'other']);
  assert.deepEqual(onlyKind(figures, 'figure', kinds).hidden, []);
  assert.deepEqual(onlyKind(figures, 'section', kinds).hidden, ['equation', 'defterm', 'figure', 'other']);
});

test('a row that matches the search is shown with its heading and that heading\'s outer headings', () => {
  // A label matches by its code.
  assert.deepEqual(shown(outlineRows(OUTLINE, LABELS, ALL_LABELS, '\\term{entropies')), ['# Intro', '# Setting', 'defterm:Entropies']);
  // By its designation, before the first heading.
  assert.deepEqual(shown(outlineRows(OUTLINE, LABELS, ALL_LABELS, '(1)')), ['eq:first']);
  // A heading that matches keeps its outer headings, not its labels.
  assert.deepEqual(shown(outlineRows(OUTLINE, LABELS, ALL_LABELS, 'setting')), ['# Intro', '# Setting']);
  // A label that the filter hides does not match.
  assert.deepEqual(shown(outlineRows(OUTLINE, LABELS, withKind(ALL_LABELS, 'figure', false), 'fig')), []);
});

test('a heading that is not understood keeps its place, so that a label stands under the heading its position names', () => {
  const outline = readOutline([{ level: 1, title: 'Intro' }, { level: 2 }, { level: 1, title: 'Results' }]) ?? [];
  assert.equal(outline.length, 3);
  assert.deepEqual(shown(outlineRows(outline, [label('fig:plot', 'Fig. 1', 'figure', 2)], ALL_LABELS, '')), ['# Intro', '# ', '# Results', 'fig:plot']);
});
