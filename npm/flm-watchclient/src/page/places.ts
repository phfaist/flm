// The places of the page's elements in the document's sources: the attribute in which the
// library writes an element's place, and the elements of a place.

import type { Part } from '../shared/messages';

/**
 * The attribute in which the library writes an element's place in its source: a JSON
 * array of the source's name and two byte offsets.
 */
export const SOURCE_SPAN = 'data-flm-source-span';

/** The place an element's attribute `SOURCE_SPAN` holds, or `null`. */
export function spanOf(element: Element): Part | null {
  const text = element.getAttribute(SOURCE_SPAN);
  if (text === null) return null;
  try {
    const span: unknown = JSON.parse(text);
    if (Array.isArray(span) && span.length === 3 && typeof span[0] === 'string'
        && typeof span[1] === 'number' && typeof span[2] === 'number') {
      return { file: span[0], start: span[1], end: span[2] };
    }
  } catch {
    // An attribute that is not understood places nothing.
  }
  return null;
}

/** An element with its place. */
export interface Placed {
  element: Element;
  place: Part;
}

/** The elements under `root` that have a place, in document order. */
export function placedElements(root: ParentNode): Placed[] {
  const placed: Placed[] = [];
  for (const element of Array.from(root.querySelectorAll(`[${SOURCE_SPAN}]`))) {
    const place = spanOf(element);
    if (place !== null) placed.push({ element, place });
  }
  return placed;
}

/**
 * The elements of `place`: among the elements of `placed` whose place overlaps its bytes
 * (a place that covers no bytes counts as covering the byte at its start), those that
 * contain no other such element.
 */
export function elementsOf(place: Part, placed: readonly Placed[]): Element[] {
  const end = Math.max(place.end, place.start + 1);
  const overlapping = placed
    .filter((entry) => entry.place.file === place.file && entry.place.start < end && place.start < entry.place.end)
    .map((entry) => entry.element);
  return innermost(overlapping);
}

// `innermost` and `outermost` walk from each element up to the root, so that their time
// grows with the number of elements times the depth of the tree: an update can change
// thousands of elements at once.

/** The elements of `elements` that contain no other of them, in their order. */
export function innermost(elements: readonly Element[]): Element[] {
  const given = new Set(elements);
  // The elements of `elements` that contain another of them.
  const around = new Set<Element>();
  for (const element of given) {
    for (let node = element.parentElement; node !== null && !around.has(node); node = node.parentElement) {
      if (given.has(node)) around.add(node);
    }
  }
  return elements.filter((element) => !around.has(element));
}

/** The elements of `elements` that no other of them contains, in their order. */
export function outermost(elements: readonly Element[]): Element[] {
  const given = new Set(elements);
  return elements.filter((element) => {
    for (let node = element.parentElement; node !== null; node = node.parentElement) {
      if (given.has(node)) return false;
    }
    return true;
  });
}

/**
 * The place of `element` in its source: its own, else that of the nearest element before
 * it, else the same for the element around it, and so on up to `body`; `null` when none
 * has one.
 */
export function placeAt(element: Element, body: Element | null): Part | null {
  for (let node: Element | null = element; node !== null && node !== body; node = node.parentElement) {
    for (let sibling: Element | null = node; sibling !== null; sibling = sibling.previousElementSibling) {
      const place = spanOf(sibling);
      if (place !== null) return place;
    }
  }
  return null;
}

/** The nearest element around `element`, itself included, that has a place, or `null`. */
export function placedAround(element: Element): Placed | null {
  for (let node: Element | null = element; node !== null; node = node.parentElement) {
    const place = spanOf(node);
    if (place !== null) return { element: node, place };
  }
  return null;
}
