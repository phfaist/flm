// The build state the status bar shows, for each sequence of the stream's messages.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Hello, Update } from '../src/shared/protocol';
import type { BuildEvent, BuildState } from '../src/tools/state/build';
import { INITIAL, buildLine, duration, shortName, step } from '../src/tools/state/build';

function helloData(fields: Partial<Hello> = {}): Hello {
  return {
    server: 's', document: 'paper.flm', build: 1, building: false, problems: [], references: null,
    mediaType: 'text/html', earlierOutput: false, durationMs: 400, profile: 'html', defaultProfile: 'html', profiles: ['html', 'text'], ...fields,
  };
}

function hello(fields: Partial<Hello> = {}): BuildEvent {
  return { type: 'hello', hello: helloData(fields) };
}

function update(fields: Partial<Update> = {}): BuildEvent {
  return {
    type: 'update',
    update: {
      build: 2, page: 'regions', regions: [], files: [], problems: [], references: null,
      mediaType: 'text/html', earlierOutput: false, durationMs: 180, profile: 'html', defaultProfile: 'html', profiles: ['html', 'text'], ...fields,
    },
  };
}

const building: BuildEvent = { type: 'building' };

function run(events: BuildEvent[]): BuildState {
  return events.reduce(step, INITIAL);
}

/** The text of the build state after `events`, or `null` when it shows none. */
function shown(events: BuildEvent[]): string | null {
  return buildLine(run(events))?.text ?? null;
}

test('a finished build shows how long it took', () => {
  assert.equal(shown([]), null);
  assert.equal(shown([hello()]), 'Built in 0.4 s');
  assert.equal(shown([hello(), building, update()]), 'Built in 0.2 s');
  assert.equal(buildLine(run([hello()]))?.tone, 'muted');
});

test('a build shows Building only once it has lasted longer than 0.3 s', () => {
  const started = run([hello(), building]);
  assert.equal(buildLine(started)?.text, 'Built in 0.4 s');
  const slow = step(started, { type: 'slow', started: started.started });
  assert.deepEqual(buildLine(slow), { text: 'Building…', tone: 'active' });
  assert.equal(buildLine(step(slow, update({ durationMs: 1250 })))?.text, 'Built in 1.3 s');
});

test('a build shows the text of its progress, and its update clears it', () => {
  const started = run([hello(), building]);
  const slow = step(started, { type: 'slow', started: started.started });
  const progressing = step(slow, { type: 'progress', text: '42% Converting graphics figure-3.pdf' });
  assert.equal(progressing.progress, '42% Converting graphics figure-3.pdf');
  assert.deepEqual(buildLine(progressing), { text: '42% Converting graphics figure-3.pdf', tone: 'active' });
  const done = step(progressing, update({ durationMs: 1250 }));
  assert.equal(done.progress, null);
  assert.equal(buildLine(done)?.text, 'Built in 1.3 s');
});

test('the timer of an earlier build does not make a later one slow', () => {
  const first = run([hello(), building]);
  const later = run([hello(), building, update(), building]);
  assert.notEqual(first.started, later.started);
  assert.equal(buildLine(step(later, { type: 'slow', started: first.started }))?.text, 'Built in 0.2 s');
  assert.equal(buildLine(step(run([hello(), building, update()]), { type: 'slow', started: first.started }))?.text, 'Built in 0.2 s');
});

test('a hello during a build counts the build as started once', () => {
  const state = run([hello({ building: true, build: null, durationMs: null, mediaType: null })]);
  assert.equal(state.building, true);
  assert.equal(buildLine(state), null);
  const slow = step(state, { type: 'slow', started: state.started });
  assert.equal(buildLine(slow)?.text, 'Building…');
  // The browser connected again while the same build ran: it stays slow.
  assert.equal(buildLine(step(slow, hello({ building: true })))?.text, 'Building…');
  // A second message `building` for the same build changes nothing.
  assert.equal(step(slow, building), slow);
});

test('an output kept from an earlier build is said so, in the warning tone', () => {
  const state = run([hello(), building, update({ page: 'absent', earlierOutput: true })]);
  assert.deepEqual(buildLine(state), { text: 'Last successful build', tone: 'warning' });
  assert.equal(state.build, 1);
  assert.equal(shown([hello({ earlierOutput: true })]), 'Last successful build');
  assert.equal(shown([hello(), update({ page: 'absent', earlierOutput: true }), building, update()]), 'Built in 0.2 s');
});

test('a lost connection shows Disconnected until the next hello', () => {
  const lost = run([hello(), building, { type: 'disconnected' }]);
  assert.deepEqual(buildLine(lost), { text: 'Disconnected', tone: 'error' });
  assert.equal(lost.building, false);
  assert.equal(buildLine(step(lost, { type: 'slow', started: lost.started }))?.text, 'Disconnected');
  assert.equal(shown([hello(), { type: 'disconnected' }, hello({ durationMs: 90 })]), 'Built in 0.09 s');
  assert.equal(shown([{ type: 'disconnected' }]), 'Disconnected');
});

test('a failed request shows its text until the next build', () => {
  const failed = run([hello(), { type: 'failed', text: 'The build loop has stopped' }]);
  assert.deepEqual(buildLine(failed), { text: 'The build loop has stopped', tone: 'error' });
  assert.equal(buildLine(step(failed, building))?.text, 'Built in 0.4 s');
  assert.equal(buildLine(step(failed, update()))?.text, 'Built in 0.2 s');
});

test('a duration is written with the precision that reads well', () => {
  assert.equal(duration(42), '0.04 s');
  assert.equal(duration(400), '0.4 s');
  assert.equal(duration(1949), '1.9 s');
  assert.equal(duration(12400), '12 s');
});

test('the status bar names the document by its short name, without the folders', () => {
  assert.equal(shortName('paper.flm'), 'paper.flm');
  assert.equal(shortName('notes/2026/paper.flm'), 'paper.flm');
  assert.equal(shortName('C:\\notes\\paper.flm'), 'paper.flm');
});
