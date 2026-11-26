// Inspect, which the tools page turns on and off. While it is on, the page script takes
// each click before the document does: it selects the innermost element around the click
// that has a place in a source, outlines it (the attribute `INSPECTED`), and reports to
// the tools page the element's place, its rectangle in the window and the label that
// refers to it. A click where no element has a place selects nothing, and is reported
// with none. A click with Ctrl or Cmd and Shift is left to the gesture that opens the
// place in the editor (`gestures.ts`). The tools page can also ask for the element of a
// part of a source (a construct around the one inspected), which is selected and reported
// in the same way.

import type { Part } from '../shared/messages';
import { post, scrollSeen } from './tools';
import { INSPECTED } from './attributes';
import { outermost, placedAround, placedElements, spanOf } from './places';

/** From the identifier of an element of the page to the label that refers to it. */
let references: Record<string, string> = {};

/** Whether Inspect is on. */
let on = false;

/**
 * Takes `given`, as a message of the server carries it, as the labels of the page's
 * elements; `null` (the build gave no page) keeps the ones there are.
 */
export function setReferences(given: Record<string, string> | null): void {
  if (given !== null) references = given;
}

/** Takes the outline off the element selected. */
function unselect(): void {
  for (const element of Array.from(document.querySelectorAll(`[${INSPECTED}]`))) element.removeAttribute(INSPECTED);
}

/** Turns Inspect on or off; either way, nothing stays selected. */
export function setInspect(value: boolean): void {
  on = value;
  unselect();
}

function samePart(one: Part | null, other: Part): boolean {
  return one !== null && one.file === other.file && one.start === other.start && one.end === other.end;
}

/**
 * The label that refers to `element`, whose place is `place`: the label of its `id`, else
 * of the nearest element around it written from the same place or from none (such as the
 * block of a theorem around its paragraph), up to the first written from another place;
 * `null` when none has one.
 */
function labelOf(element: Element, place: Part): string | null {
  for (let node: Element | null = element; node !== null; node = node.parentElement) {
    const span = spanOf(node);
    if (span !== null && !samePart(span, place)) return null;
    if (node.id !== '' && Object.prototype.hasOwnProperty.call(references, node.id)) return references[node.id]!;
  }
  return null;
}

/** Selects `element`, whose place is `own`, and reports it as the selection of `place`. */
function select(element: Element, own: Part, place: Part): void {
  unselect();
  element.setAttribute(INSPECTED, '');
  const rect = element.getBoundingClientRect();
  scrollSeen();
  post({
    type: 'inspected',
    place,
    rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
    label: labelOf(element, own),
  });
}

/** Selects what a click on `target` selects, and reports it. */
function selectClicked(target: Element): void {
  const found = placedAround(target);
  if (found === null) {
    unselect();
    post({ type: 'inspected', place: null, rect: null, label: null });
    return;
  }
  select(found.element, found.place, found.place);
}

/**
 * Selects the element of `part`, and reports it with `part` as its place: among the
 * elements whose place contains the whole part, those of the narrowest place, and the
 * outermost of them. When no element contains the part, nothing is selected, and the part
 * is reported without a rectangle. Nothing happens while Inspect is off.
 */
export function inspectPlace(part: Part): void {
  if (!on) return;
  const containing = placedElements(document).filter((entry) =>
    entry.place.file === part.file && entry.place.start <= part.start && part.end <= entry.place.end);
  const narrowest = Math.min(...containing.map((entry) => entry.place.end - entry.place.start));
  const tightest = containing.filter((entry) => entry.place.end - entry.place.start === narrowest);
  const element = outermost(tightest.map((entry) => entry.element))[0];
  const found = tightest.find((entry) => entry.element === element);
  if (found === undefined) {
    unselect();
    post({ type: 'inspected', place: part, rect: null, label: null });
    return;
  }
  select(found.element, found.place, part);
}

/** Whether `event` is the gesture that opens a place in the editor. */
function isOpenGesture(event: MouseEvent): boolean {
  return (event.ctrlKey || event.metaKey) && event.shiftKey;
}

/** Takes the clicks while Inspect is on, before the document's own handlers. */
export function startInspect(): void {
  const take = (event: MouseEvent): void => {
    if (!on || event.button !== 0 || isOpenGesture(event)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    if (event.type !== 'click') return;
    const target = event.target instanceof Element ? event.target
      : event.target instanceof Node ? event.target.parentElement : null;
    if (target !== null) selectClicked(target);
  };
  for (const type of ['mousedown', 'mouseup', 'click'] as const) window.addEventListener(type, take, true);
}

/** The document changed: nothing stays selected. */
export function documentChanged(): void {
  unselect();
}
