// The reading position: the record, its storage key, its place in the history state.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  POSITION_KEY_PREFIX, STATE_FIELD,
  parseStored, positionAt, positionInState, positionKey, readPosition, withPosition,
} from '../src/position';
import { contentHeadings } from '../src/sidebar';
import { h } from './dom';

test("the record's key is the page's address without its fragment", () => {
  assert.equal(POSITION_KEY_PREFIX, 'flm-lecturenotes:position:');
  assert.equal(positionKey('https://example.org/notes.html'), 'flm-lecturenotes:position:https://example.org/notes.html');
  assert.equal(positionKey('https://example.org/notes.html#sec-recap'), 'flm-lecturenotes:position:https://example.org/notes.html');
  assert.equal(positionKey('http://localhost:8000/notes.html?v=2#x'), 'flm-lecturenotes:position:http://localhost:8000/notes.html?v=2');
  assert.equal(positionKey('file:///tmp/notes.html#'), 'flm-lecturenotes:position:file:///tmp/notes.html');
});

test('the position is the nearest heading above the window top plus the distance from it', () => {
  const ids = ['a', 'b', 'c'];
  assert.deepEqual(positionAt(ids, [-900, -120, 300], 2000), { id: 'b', offset: 120 });
  assert.deepEqual(positionAt(ids, [-900, 0.5, 300], 2000), { id: 'b', offset: -0.5 }, 'a heading at the top counts');
  assert.deepEqual(positionAt(ids, [40, 400, 900], 75), { id: null, offset: 75 }, 'above the first heading');
  assert.deepEqual(positionAt([], [], 0), { id: null, offset: 0 });
});

test('a record is read back only when it has the shape of one', () => {
  assert.deepEqual(readPosition({ id: 'x', offset: 12 }), { id: 'x', offset: 12 });
  assert.deepEqual(readPosition({ id: null, offset: 0 }), { id: null, offset: 0 });
  assert.equal(readPosition({ id: 3, offset: 0 }), null);
  assert.equal(readPosition({ id: 'x' }), null);
  assert.equal(readPosition({ id: 'x', offset: Infinity }), null);
  assert.equal(readPosition('x'), null);
  assert.deepEqual(parseStored('{"id":"sec-a","offset":40}'), { id: 'sec-a', offset: 40 });
  assert.equal(parseStored('{oops'), null);
  assert.equal(parseStored(null), null);
});

test("the record goes into the history state with the state's other fields kept", () => {
  const position = { id: 'sec-a', offset: 40 };
  assert.deepEqual(withPosition({ other: 1 }, position), { other: 1, [STATE_FIELD]: position });
  assert.deepEqual(withPosition(null, position), { [STATE_FIELD]: position });
  assert.deepEqual(withPosition({ [STATE_FIELD]: { id: null, offset: 3 } }, position), { [STATE_FIELD]: position });
  assert.deepEqual(positionInState(withPosition({ other: 1 }, position)), position);
  assert.equal(positionInState(null), null);
  assert.equal(positionInState({ other: 1 }), null);
});

test('the headings of the content are those with an id and a level, outside the pieces', () => {
  const content = h('article#flm-content', {},
    h('h2.flm-pres-block--heading#a', { 'data-flm-level': '1' }),
    h('h3.flm-pres-block--heading', { 'data-flm-level': '2' }),
    h('div', { id: 'flm-region-flm.toc' }, h('h2.flm-pres-block--heading#t', { 'data-flm-level': '1' })),
    h('h3.flm-pres-block--heading#b', { 'data-flm-level': '2' }),
    h('h3.flm-pres-block--heading#c'));
  const headings = contentHeadings(content as unknown as HTMLElement);
  assert.deepEqual(headings.map((x) => [x.element.id, x.level]), [['a', 1], ['b', 2]]);
});
