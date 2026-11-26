// The output view chosen from the media type of the output's entry file, and the changes of
// the output that make the text view read its file again.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Hello, Update } from '../src/shared/protocol';
import type { BuildState } from '../src/tools/state/build';
import { INITIAL, step, textOf, view } from '../src/tools/state/build';

function hello(fields: Partial<Hello> = {}): Hello {
  return {
    server: 's', document: 'paper.flm', build: 1, building: false, problems: [], references: null,
    mediaType: 'text/plain', earlierOutput: false, durationMs: 400, profile: 'text', defaultProfile: 'html', profiles: ['html', 'text'],
    ...fields,
  };
}

function update(fields: Partial<Update> = {}): Update {
  return {
    build: 2, page: 'reload', regions: [], files: [], problems: [], references: null, mediaType: 'text/plain',
    earlierOutput: false, durationMs: 180, profile: 'text', defaultProfile: 'html', profiles: ['html', 'text'], ...fields,
  };
}

function shown(fields: Partial<Hello>): BuildState {
  return step(INITIAL, { type: 'hello', hello: hello(fields) });
}

test('the view follows the media type of the entry file, never the name of the build type', () => {
  assert.deepEqual(view(shown({ mediaType: 'text/html; charset=utf-8', profile: 'text' })), { kind: 'frame' });
  assert.deepEqual(view(shown({ mediaType: 'text/plain; charset=utf-8', profile: 'html' })),
    { kind: 'text', mediaType: 'text/plain' });
  assert.deepEqual(view(shown({ mediaType: 'text/x-tex' })), { kind: 'text', mediaType: 'text/x-tex' });
  assert.deepEqual(view(shown({ mediaType: 'application/pdf', profile: 'pdf-latex' })), { kind: 'pdf' });
  assert.deepEqual(view(shown({ mediaType: 'application/x-latex' })), { kind: 'text', mediaType: 'application/x-latex' });
  // An output without an entry file, and no output at all.
  assert.deepEqual(view(shown({ mediaType: null, profile: 'check' })), { kind: 'empty', profile: 'check' });
  assert.deepEqual(view(shown({ mediaType: null, build: null })), { kind: 'none' });
});

test('a file is a text when it is UTF-8 without control characters other than white space and the escape', () => {
  const bytes = (text: string) => new TextEncoder().encode(text);
  assert.equal(textOf(bytes('Caf\u00e9\t\\section{A}\r\n\x1b[0;1mbold\x1b[0m\n')), 'Caf\u00e9\t\\section{A}\r\n\x1b[0;1mbold\x1b[0m\n');
  assert.equal(textOf(bytes('PK\x03\x04')), null);
  assert.equal(textOf(new Uint8Array([0x25, 0x50, 0x44, 0x46, 0xe2, 0xe3])), null);
});

test('the output changes with a reload or with a hello of another build, not with an unchanged or absent page', () => {
  const first = shown({});
  const same = step(first, { type: 'update', update: update({ page: 'same' }) });
  assert.equal(same.revision, first.revision);
  const absent = step(same, { type: 'update', update: update({ build: 3, page: 'absent' }) });
  assert.equal(absent.revision, first.revision);
  const reloaded = step(absent, { type: 'update', update: update({ build: 4 }) });
  assert.equal(reloaded.revision, first.revision + 1);
  const again = step(reloaded, { type: 'hello', hello: hello({ build: 4 }) });
  assert.equal(again.revision, reloaded.revision);
  const missed = step(again, { type: 'hello', hello: hello({ build: 6 }) });
  assert.equal(missed.revision, reloaded.revision + 1);
});
