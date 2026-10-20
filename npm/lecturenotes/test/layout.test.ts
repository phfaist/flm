// The settings.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { DEFAULT_CHAPTER_LEVEL, readSettings } from '../src/layout';

test('the chapter level is 1 unless the settings give another', () => {
  assert.equal(DEFAULT_CHAPTER_LEVEL, 1);
  assert.deepEqual(readSettings({}), { chapterLevel: 1 });
  assert.deepEqual(readSettings(undefined), { chapterLevel: 1 });
  assert.deepEqual(readSettings(null), { chapterLevel: 1 });
  assert.deepEqual(readSettings({ chapter_level: 2, font_family: 'serif', links: [] }), { chapterLevel: 2 });
  assert.deepEqual(readSettings({ chapter_level: 0 }), { chapterLevel: 0 });
});

test('a chapter level that is not a whole number from 0 up is reported and gives the default', (t) => {
  const reported: string[] = [];
  t.mock.method(console, 'error', (message: string) => { reported.push(message); });
  assert.deepEqual(readSettings({ chapter_level: '2' }), { chapterLevel: 1 });
  assert.deepEqual(readSettings({ chapter_level: -1 }), { chapterLevel: 1 });
  assert.deepEqual(readSettings({ chapter_level: 1.5 }), { chapterLevel: 1 });
  assert.deepEqual(reported, [
    'flm-lecturenotes: the setting `chapter_level` must be a whole number from 0 up, and it is `"2"`',
    'flm-lecturenotes: the setting `chapter_level` must be a whole number from 0 up, and it is `-1`',
    'flm-lecturenotes: the setting `chapter_level` must be a whole number from 0 up, and it is `1.5`',
  ]);
});
