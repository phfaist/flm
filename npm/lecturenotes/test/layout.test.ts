// The settings.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { DEFAULT_NAVIGATION_LEVEL, readSettings } from '../src/layout';

test('the navigation level is 1 unless the settings give another', () => {
  assert.equal(DEFAULT_NAVIGATION_LEVEL, 1);
  assert.deepEqual(readSettings({}), { navigationLevel: 1 });
  assert.deepEqual(readSettings(undefined), { navigationLevel: 1 });
  assert.deepEqual(readSettings(null), { navigationLevel: 1 });
  assert.deepEqual(readSettings({ navigation_heading_level: 2, font_family: 'serif', links: [] }), { navigationLevel: 2 });
  assert.deepEqual(readSettings({ navigation_heading_level: 0 }), { navigationLevel: 0 });
});

test('a navigation level that is not a whole number from 0 up is reported and gives the default', (t) => {
  const reported: string[] = [];
  t.mock.method(console, 'error', (message: string) => { reported.push(message); });
  assert.deepEqual(readSettings({ navigation_heading_level: '2' }), { navigationLevel: 1 });
  assert.deepEqual(readSettings({ navigation_heading_level: -1 }), { navigationLevel: 1 });
  assert.deepEqual(readSettings({ navigation_heading_level: 1.5 }), { navigationLevel: 1 });
  assert.deepEqual(reported, [
    'flm-lecturenotes: the setting `navigation_heading_level` must be a whole number from 0 up, and it is `"2"`',
    'flm-lecturenotes: the setting `navigation_heading_level` must be a whole number from 0 up, and it is `-1`',
    'flm-lecturenotes: the setting `navigation_heading_level` must be a whole number from 0 up, and it is `1.5`',
  ]);
});
