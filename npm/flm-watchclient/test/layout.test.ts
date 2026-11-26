// The panel's size limits: at the right at least 360 px and at most 60 % of the body's
// width; at the bottom at least 160 px, leaving the output view at least 160 px.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { keyed, limited } from '../src/tools/state/layout';

test('at the right the panel is at least 360 px and at most 60 % of the width', () => {
  assert.equal(limited('right', 100, 1200, 800), 360);
  assert.equal(limited('right', 900, 1200, 800), 720);
  assert.equal(limited('right', 500, 1200, 800), 500);
  // The window shrinks: the size asked for is shown within the new limits.
  assert.equal(limited('right', 700, 1000, 800), 600);
  // Its least width wins over the room it leaves.
  assert.equal(limited('right', 408, 560, 800), 360);
});

test('at the bottom the panel is at least 160 px and leaves the output view 160 px', () => {
  assert.equal(limited('bottom', 40, 1200, 800), 160);
  assert.equal(limited('bottom', 700, 1200, 800), 640);
  assert.equal(limited('bottom', 300, 1200, 800), 300);
  assert.equal(limited('bottom', 300, 1200, 280), 160);
});

test('the arrow toward the output view makes the panel larger, within the limits', () => {
  assert.equal(keyed('right', 'ArrowLeft', 408, 1200, 800), 424);
  assert.equal(keyed('right', 'ArrowRight', 408, 1200, 800), 392);
  assert.equal(keyed('right', 'ArrowRight', 362, 1200, 800), 360);
  assert.equal(keyed('right', 'ArrowLeft', 718, 1200, 800), 720);
  assert.equal(keyed('right', 'ArrowUp', 408, 1200, 800), null);
  assert.equal(keyed('bottom', 'ArrowUp', 320, 1200, 800), 336);
  assert.equal(keyed('bottom', 'ArrowDown', 320, 1200, 800), 304);
  assert.equal(keyed('bottom', 'ArrowLeft', 320, 1200, 800), null);
  assert.equal(keyed('bottom', 'Enter', 320, 1200, 800), null);
});
