// The navigation units, the folding and the storage keys.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  STORAGE_KEYS, FOLLOW_PAUSE_MS, CURRENT_MARGIN_PX,
  currentHeadingIndex, currentLine, followAllowed, fragmentId, navigationTargets,
  outsideView, parseFolding, parseHidden, toggleFolding, unitIndices, unitOf,
} from '../src/sidebar';

// The sample's structure, with the navigation level 1: a preface (a unit) with two
// sections, a part with two units, another part with one unit.
//              0  1  2  3  4  5  6  7  8  9 10 11 12 13 14
const LEVELS = [1, 2, 2, 0, 1, 2, 2, 2, 1, 2, 2, 0, 1, 2, 2];

test('the units are the headings at the navigation level', () => {
  assert.deepEqual(unitIndices(LEVELS, 1), [0, 4, 8, 12]);
  assert.deepEqual(unitIndices(LEVELS, 0), [3, 11]);
  assert.deepEqual(unitIndices(LEVELS, 2), [1, 2, 5, 6, 7, 9, 10, 13, 14]);
  assert.deepEqual(unitIndices([], 1), []);
});

test('the current heading is the last whose top is at or above the line', () => {
  const tops = [-500, -100, 30, 400, 900];
  assert.equal(currentHeadingIndex(tops, 60), 2);
  assert.equal(currentHeadingIndex(tops, 30), 2);
  assert.equal(currentHeadingIndex(tops, 29), 1);
  assert.equal(currentHeadingIndex([100, 200], 60), -1);
  assert.equal(currentHeadingIndex([], 60), -1);
});

test('a heading belongs to the nearest unit before it, unless a heading above the navigation level comes in between', () => {
  assert.equal(unitOf(LEVELS, 0, 1), 0);
  assert.equal(unitOf(LEVELS, 2, 1), 0);
  assert.equal(unitOf(LEVELS, 3, 1), -1, 'a heading above the navigation level is in no unit');
  assert.equal(unitOf(LEVELS, 7, 1), 4);
  assert.equal(unitOf(LEVELS, 10, 1), 8);
  assert.equal(unitOf(LEVELS, 14, 1), 12);
  assert.equal(unitOf(LEVELS, -1, 1), -1, 'above the first heading');
  assert.equal(unitOf([2, 1, 2], 0, 1), -1, 'a heading before any unit');
  assert.equal(unitOf(LEVELS, 7, 0), 3, 'the navigation level 0');
});

test('the arrows lead to the previous and the next unit', () => {
  // above the first heading: no previous, the first unit next
  assert.deepEqual(navigationTargets(LEVELS, -1, 1), { prev: -1, next: 0 });
  // in the first unit
  assert.deepEqual(navigationTargets(LEVELS, 0, 1), { prev: -1, next: 4 });
  assert.deepEqual(navigationTargets(LEVELS, 2, 1), { prev: -1, next: 4 });
  // on a heading above the navigation level: the unit before it, the unit after it
  assert.deepEqual(navigationTargets(LEVELS, 3, 1), { prev: 0, next: 4 });
  // in the second unit
  assert.deepEqual(navigationTargets(LEVELS, 6, 1), { prev: 0, next: 8 });
  // in the last unit
  assert.deepEqual(navigationTargets(LEVELS, 14, 1), { prev: 8, next: -1 });
  // no units at all
  assert.deepEqual(navigationTargets([2, 2], 1, 1), { prev: -1, next: -1 });
});

test('the folding toggles between folded and all open, folded by default', () => {
  assert.equal(parseFolding(null), 'folded');
  assert.equal(parseFolding('folded'), 'folded');
  assert.equal(parseFolding('all'), 'all');
  assert.equal(parseFolding('something else'), 'folded');
  assert.equal(toggleFolding('folded'), 'all');
  assert.equal(toggleFolding('all'), 'folded');
  assert.equal(toggleFolding(toggleFolding('folded')), 'folded');
});

test('the sidebar is shown unless the stored value hides it', () => {
  assert.equal(parseHidden(null), false);
  assert.equal(parseHidden('0'), false);
  assert.equal(parseHidden('1'), true);
  assert.equal(parseHidden(''), false);
});

test('the storage keys', () => {
  assert.deepEqual(STORAGE_KEYS, {
    folding: 'flm-lecturenotes:toc-folding',
    sidebarHidden: 'flm-lecturenotes:sidebar-hidden',
  });
});

test('the sidebar follows the reader unless the reader scrolled it in the last four seconds', () => {
  assert.equal(FOLLOW_PAUSE_MS, 4000);
  assert.equal(followAllowed(10_000, -Infinity), true);
  assert.equal(followAllowed(10_000, 7_000), false);
  assert.equal(followAllowed(10_000, 6_000), true);
});

test('an entry near the edge of the sidebar is outside its view', () => {
  assert.equal(outsideView(100, 120, 0, 500), false);
  assert.equal(outsideView(20, 40, 0, 500), true);
  assert.equal(outsideView(470, 490, 0, 500), true);
});

test('a fragment address names its id, decoded', () => {
  assert.equal(fragmentId('#sec-encoding'), 'sec-encoding');
  assert.equal(fragmentId('#sec--What_we-ll_cover_here~22090f'), 'sec--What_we-ll_cover_here~22090f');
  assert.equal(fragmentId('#caf%C3%A9'), 'café');
  assert.equal(fragmentId('#100%'), '100%');
});

test("the current heading's line is the root's scroll padding plus a few pixels", () => {
  assert.equal(currentLine('60px'), 60 + CURRENT_MARGIN_PX);
  assert.equal(currentLine('76.5px'), 76.5 + CURRENT_MARGIN_PX);
  assert.equal(currentLine('auto'), CURRENT_MARGIN_PX);
  assert.equal(currentLine(''), CURRENT_MARGIN_PX);
});
