// The help: the ranking of the search over the index, and the history of the pages and searches shown.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { IndexEntry } from '../src/shared/help';
import { NO_HISTORY, back, canGoBack, canGoForward, currentPage, currentSearch, forward, search, visit } from '../src/tools/state/help';

function page(path: string, kind: string, title: string, names: string[], summary = ''): IndexEntry {
  return { path, kind, title, summary, names };
}

const PAGES: IndexEntry[] = [
  page('features/refs', 'feature', 'Feature refs', ['refs'], 'Labels and references.'),
  page('features/refs/macros/refstyle', 'command', 'Macro \\refstyle', ['\\refstyle', 'refstyle']),
  page('features/refs/macros/ref', 'command', 'Macro \\ref', ['\\ref', 'ref'], 'Refers to a labeled place.'),
  page('settings/refs', 'settings', 'Settings of refs', ['refs.ref_style']),
  page('features/headings', 'feature', 'Feature headings', ['headings'], 'Section headings, which a ref can name.'),
  page('workflows/', 'workflow-index', 'Workflows that prefer references', []),
  page('namespaces/element/flm.ref', 'entity', 'Element flm.ref', ['flm.ref']),
];

const paths = (pages: readonly IndexEntry[]) => pages.map((each) => each.path);

test('a whole name ranks before a name it begins, then titles, then summaries; commands and settings first among equals', () => {
  assert.deepEqual(paths(search(PAGES, 'ref', false)), [
    'features/refs/macros/ref',
    // Names that begin with the text: the command and the settings page before the feature.
    'features/refs/macros/refstyle',
    'settings/refs',
    'features/refs',
    'workflows/',
    'features/headings',
  ]);
  // A name is matched whatever the case, with its backslash.
  assert.deepEqual(paths(search(PAGES, ' \\REF', false)).slice(0, 2), ['features/refs/macros/ref', 'features/refs/macros/refstyle']);
  assert.deepEqual(search(PAGES, '  ', false), []);
});

test('the pages of the internal names are listed only while the toggle is on', () => {
  assert.equal(paths(search(PAGES, 'flm.ref', false)).length, 0);
  assert.deepEqual(paths(search(PAGES, 'flm.ref', true)), ['namespaces/element/flm.ref']);
  assert.ok(paths(search(PAGES, 'ref', true)).includes('namespaces/element/flm.ref'));
});

test('the history goes back and forward, and a page visited after going back drops the pages after it', () => {
  const page = (path: string) => ({ page: path });
  let history = visit(NO_HISTORY, page('a'));
  assert.equal(currentPage(history), 'a');
  assert.equal(canGoBack(history), false);
  history = visit(visit(history, page('b')), page('c'));
  // Showing the page shown adds nothing.
  assert.deepEqual(visit(history, page('c')), history);
  history = back(back(history));
  assert.equal(currentPage(history), 'a');
  assert.equal(canGoBack(history), false);
  assert.deepEqual(back(history), history);
  history = forward(history);
  assert.equal(currentPage(history), 'b');
  assert.equal(canGoForward(history), true);
  history = visit(history, page('d'));
  assert.deepEqual(history.visits, [page('a'), page('b'), page('d')]);
  assert.equal(canGoForward(history), false);
  assert.deepEqual(forward(history), history);
  assert.equal(currentPage(back(history)), 'b');
});

test('a search in the history comes back with its text, and shows no page', () => {
  const history = visit(visit(visit(NO_HISTORY, { page: 'a' }), { search: 'ref' }), { page: 'b' });
  const results = back(history);
  assert.equal(currentSearch(results), 'ref');
  assert.equal(currentPage(results), null);
  assert.equal(currentSearch(history), '');
});
