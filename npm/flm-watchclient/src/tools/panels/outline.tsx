// The Outline tab: one list of the document's headings, each followed by the labels that
// stand under it (`state/outline.ts`), under a search field and a row of toggles that show or
// hide the labels by kind (with Shift, one kind alone). A heading's row is indented by its
// level; a label's row stands below its heading and shows what a reference to it shows,
// then the code that refers to it with a button that copies it. A click on a heading or on what a label shows scrolls the
// document to the element and flashes it.

import { useState } from 'preact/hooks';
import type { Heading, LabelEntry } from '../../shared/protocol';
import { CopyCode } from '../controls/copy-ref';
import { Icon } from '../controls/icon';
import type { LabelFilter, OutlineRow } from '../state/outline';
import { labelKinds, onlyKind, outlineRows, withKind } from '../state/outline';
import { labelFilter, preview } from '../state/store';
import { toPage } from '../views/channel';

/** The indentation of one level of headings, in pixels. */
const INDENT = 14;

/** The indentation of a heading of level `level`, the outermost level being `outer`. */
function indentOf(level: number, outer: number): number {
  return INDENT * (level - outer);
}

/** Scrolls the document to the element `id` and flashes it. */
function show(id: string | null): void {
  if (id !== null) toPage({ type: 'show-element', id });
}

function HeadingRow({ heading, outer }: { heading: Heading; outer: number }) {
  return (
    <li class="flm-outline-row-heading" data-level={heading.level - outer}>
      <button
        type="button"
        class="flm-outline-heading"
        style={{ paddingLeft: `${6 + indentOf(heading.level, outer)}px` }}
        disabled={heading.id === null}
        onClick={() => show(heading.id)}
      >
        {heading.number !== null && <span class="flm-outline-number">{heading.number}</span>}
        <span class="flm-outline-title">{heading.title}</span>
      </button>
    </li>
  );
}

function LabelRow({ entry, code, indent }: { entry: LabelEntry; code: string; indent: number }) {
  return (
    <li class="flm-outline-label" style={{ marginLeft: `${indent}px` }}>
      <button
        type="button"
        class="flm-outline-designation"
        disabled={entry.id === null}
        title={entry.label}
        onClick={() => show(entry.id)}
      >
        {entry.designation}
      </button>
      <span class="flm-outline-code">{code}</span>
      <CopyCode code={code} />
    </li>
  );
}

/**
 * The row of toggles that show or hide the labels: all of them, then those of each kind. A
 * click with Shift on a kind shows that kind alone, or, when it is shown alone, every kind.
 */
function Filter({ labels, filter }: { labels: readonly LabelEntry[]; filter: LabelFilter }) {
  const set = (next: LabelFilter) => {
    labelFilter.value = next;
  };
  const kinds = labelKinds(labels);
  return (
    <div class="flm-outline-filter" role="group" aria-label="Labels shown">
      <Icon name="list-filter" size={13} />
      <button
        type="button"
        class="flm-outline-toggle"
        aria-pressed={filter.labels}
        title={filter.labels ? 'Hide the labels' : 'Show the labels'}
        onClick={() => set({ ...filter, labels: !filter.labels })}
      >
        Labels <span class="flm-outline-count">{labels.length}</span>
      </button>
      {kinds.map(({ kind, count }) => {
        const shown = !filter.hidden.includes(kind);
        return (
          <button
            key={kind}
            type="button"
            class="flm-outline-toggle"
            aria-pressed={shown}
            disabled={!filter.labels}
            title={`${shown ? 'Hide' : 'Show'} the labels of kind ${kind} (Shift-click: show this kind alone, or all)`}
            onClick={(event) => set(event.shiftKey
              ? onlyKind(filter, kind, kinds.map((each) => each.kind))
              : withKind(filter, kind, !shown))}
          >
            {kind} <span class="flm-outline-count">{count}</span>
          </button>
        );
      })}
    </div>
  );
}

export function OutlineTab() {
  const { outline, labels } = preview.value;
  const filter = labelFilter.value;
  const [query, setQuery] = useState('');
  if (outline === null && labels === null) return null;
  const headings = outline ?? [];
  const entries = labels ?? [];
  const outer = Math.min(...headings.map((heading) => heading.level));
  const rows = outlineRows(headings, entries, filter, query);
  // A label's row is indented one level below its heading's.
  const indentUnder = (row: OutlineRow & { kind: 'label' }) =>
    row.under === null ? 0 : indentOf(headings[row.under].level, outer) + INDENT;
  return (
    <div class="flm-outline">
      <div class="flm-settings-search">
        <label class="flm-search">
          <Icon name="search" size={14} />
          <input
            type="search"
            aria-label="Filter the headings and the labels"
            placeholder="Filter headings and labels"
            spellcheck={false}
            value={query}
            onInput={(event) => setQuery((event.currentTarget as HTMLInputElement).value)}
          />
        </label>
        {entries.length > 0 && <Filter labels={entries} filter={filter} />}
      </div>
      <div class="flm-outline-body">
        {headings.length === 0 && entries.length === 0 ? (
          <p class="flm-outline-none">No headings and no labels</p>
        ) : rows.length === 0 ? (
          <p class="flm-outline-none">Nothing matches</p>
        ) : (
          <ul class="flm-outline-list" aria-label="Outline">
            {rows.map((row) =>
              row.kind === 'heading' ? (
                <HeadingRow key={`h${row.at}`} heading={row.heading} outer={outer} />
              ) : (
                <LabelRow key={`l${row.entry.label}`} entry={row.entry} code={row.code} indent={indentUnder(row)} />
              ),
            )}
          </ul>
        )}
      </div>
    </div>
  );
}
