// Showing elements to the reader: a flash once on each, and the scroll that brings the
// first into view.

import { FLASH } from './attributes';

/** The animation the stylesheet runs on an element that carries `FLASH`. */
const FLASH_ANIMATION = 'flm-preview-flash';

/**
 * Makes each of `elements` flash once. The attribute comes off when the animation ends, so
 * that the flash lasts as long as the stylesheet says.
 */
export function flash(elements: readonly Element[]): void {
  if (elements.length === 0) return;
  for (const element of elements) element.removeAttribute(FLASH);
  // Read a layout value, so that the browser starts the animations anew: once for all the
  // elements, since each read between changes of the page lays the page out again.
  void (elements[0] as HTMLElement).offsetWidth;
  for (const element of elements) element.setAttribute(FLASH, '');
}

/** Takes off `FLASH` when an element's flash ends. */
export function endFlashes(): void {
  document.addEventListener('animationend', (event) => {
    const target = event.target;
    if (event.animationName === FLASH_ANIMATION && target instanceof Element && target.hasAttribute(FLASH)) {
      target.removeAttribute(FLASH);
    }
  });
}

/** Whether some part of `element` lies inside the window. */
export function inView(element: Element): boolean {
  const rect = element.getBoundingClientRect();
  return rect.bottom > 0 && rect.right > 0 && rect.top < window.innerHeight && rect.left < window.innerWidth;
}

/** The first of `elements` in document order, or `null` when there is none. */
export function first(elements: readonly Element[]): Element | null {
  let found: Element | null = null;
  for (const element of elements) {
    if (found === null || (element.compareDocumentPosition(found) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0) {
      found = element;
    }
  }
  return found;
}

/**
 * Shows `elements`: scrolls to the first of them in document order unless some part of
 * it is in view already (always when `always`), and flashes each once.
 */
export function show(elements: readonly Element[], always: boolean): void {
  const leading = first(elements);
  if (leading === null) return;
  if (always || !inView(leading)) leading.scrollIntoView({ block: 'center' });
  flash(elements);
}
