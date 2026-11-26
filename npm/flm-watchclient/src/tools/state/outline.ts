// The Outline tab's pure parts: the rows of its one list (each heading followed by the
// labels that stand under it, the labels before the first heading first), the kinds of the
// labels that its filter offers, how its toggles change it, and which rows the filter and
// the search keep.

import type { Heading, LabelEntry } from '../../shared/protocol';
import { referenceCode } from './description';
import { matches } from './settings-panel';

/** The kind under which the filter counts a label whose kind the server does not give. */
export const OTHER_KIND = 'other';

/** The kind under which the filter counts the label `entry`. */
export function kindOf(entry: LabelEntry): string {
  return entry.kind ?? OTHER_KIND;
}

/** A kind of label that the filter offers, with the number of the document's labels of that kind. */
export interface LabelKind {
  kind: string;
  count: number;
}

/** The kinds of `labels`, in the order they first occur, each with the number of its labels. */
export function labelKinds(labels: readonly LabelEntry[]): LabelKind[] {
  const kinds = new Map<string, number>();
  for (const entry of labels) kinds.set(kindOf(entry), (kinds.get(kindOf(entry)) ?? 0) + 1);
  return [...kinds].map(([kind, count]) => ({ kind, count }));
}

/**
 * Which labels the filter shows: all of them or none (`labels`), and, when they are shown,
 * those of every kind but the kinds in `hidden`. A kind that the document gains later is
 * shown.
 */
export interface LabelFilter {
  labels: boolean;
  hidden: readonly string[];
}

/** The filter that shows every label. */
export const ALL_LABELS: LabelFilter = { labels: true, hidden: [] };

/** Whether `filter` shows the label `entry`. */
export function shownBy(filter: LabelFilter, entry: LabelEntry): boolean {
  return filter.labels && !filter.hidden.includes(kindOf(entry));
}

/** `filter` with the kind `kind` shown or hidden. */
export function withKind(filter: LabelFilter, kind: string, shown: boolean): LabelFilter {
  const hidden = filter.hidden.filter((each) => each !== kind);
  return { ...filter, hidden: shown ? hidden : [...hidden, kind] };
}

/**
 * `filter` once the kind `kind` alone is asked for, among the kinds `kinds`: when `kind` is
 * the one kind shown, every kind is shown; otherwise `kind` is shown and the others are
 * hidden.
 */
export function onlyKind(filter: LabelFilter, kind: string, kinds: readonly string[]): LabelFilter {
  const others = kinds.filter((each) => each !== kind);
  const alone = !filter.hidden.includes(kind) && others.every((each) => filter.hidden.includes(each));
  return { ...filter, hidden: alone ? [] : others };
}

/**
 * A row of the outline: a heading, with its position in the outline; or a label, with the
 * code that refers to it and the position of the heading it stands under (`null` before
 * the first heading).
 */
export type OutlineRow =
  | { kind: 'heading'; at: number; heading: Heading }
  | { kind: 'label'; entry: LabelEntry; code: string; under: number | null };

/**
 * The position of the heading the label `entry` stands under, among `outline`: `null`
 * before the first heading, and for a position that the outline does not have.
 */
function headingOf(entry: LabelEntry, outline: readonly Heading[]): number | null {
  const at = entry.heading;
  return at !== null && Number.isInteger(at) && at >= 0 && at < outline.length ? at : null;
}

/**
 * The positions of the headings that the heading at `at` stands under, from the nearest
 * outward: each the last heading before the one inside it whose level is smaller.
 */
function outerHeadings(outline: readonly Heading[], at: number): number[] {
  const outer: number[] = [];
  let level = outline[at].level;
  for (let before = at - 1; before >= 0; before -= 1) {
    if (outline[before].level < level) {
      outer.push(before);
      level = outline[before].level;
    }
  }
  return outer;
}

/**
 * The rows of the outline: each heading of `outline`, in document order, followed by the
 * labels of `labels` that stand under it, in document order; the labels before the first
 * heading first. The labels are those that `filter` shows. A search `query` keeps the rows
 * that match it (a heading by its number and its title, a label by its label, its
 * designation and its code), each with the headings it stands under: its heading and that
 * heading's outer headings.
 */
export function outlineRows(
  outline: readonly Heading[],
  labels: readonly LabelEntry[],
  filter: LabelFilter,
  query: string,
): OutlineRow[] {
  const before: OutlineRow[] = [];
  const under: OutlineRow[][] = outline.map(() => []);
  for (const entry of labels) {
    if (!shownBy(filter, entry)) continue;
    const code = referenceCode(entry.label);
    if (!matches(query, entry.label, entry.designation, code)) continue;
    const at = headingOf(entry, outline);
    (at === null ? before : under[at]).push({ kind: 'label', entry, code, under: at });
  }
  // The headings kept: those that match, and the headings that a row kept stands under.
  const kept = new Set<number>();
  const keep = (at: number) => {
    if (kept.has(at)) return;
    kept.add(at);
    for (const outer of outerHeadings(outline, at)) kept.add(outer);
  };
  outline.forEach((heading, at) => {
    if (matches(query, heading.title, heading.number ?? '') || under[at].length > 0) keep(at);
  });
  const rows = [...before];
  outline.forEach((heading, at) => {
    if (kept.has(at)) rows.push({ kind: 'heading', at, heading }, ...under[at]);
  });
  return rows;
}
