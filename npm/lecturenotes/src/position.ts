// flm-lecturenotes: the reading position.
//
// The browser restores the page's position on back and forward by pixels; the
// page's height may have changed since (formulas typeset, images loaded, a part
// of the page updated in place), so this script also records the position as
// the nearest heading above the window's top plus a distance from it, and
// corrects the browser's restore against that record. The record is kept in
// the current history entry's state and in the browser's storage, where it
// survives to the next visit: a page opened without a fragment goes back to
// where the reader left it.

import { type Page, loadStored, store } from './layout';
import { contentHeadings, currentHeadingIndex } from './sidebar';

/** A reading position: the id of a heading and the distance from its top to the window's top, in pixels; or, above the first heading (`id` null), the distance from the page's top. */
export interface Position {
  id: string | null;
  offset: number;
}

/** The field of a history entry's state that holds the position; the state's other fields are kept. */
export const STATE_FIELD = 'flm-lecturenotes:position';

/** The beginning of the storage key of a page's position. */
export const POSITION_KEY_PREFIX = 'flm-lecturenotes:position:';

/** How far below the window's top a heading's top may be and still count as above it, in pixels. */
export const ABOVE_TOLERANCE_PX = 1;

/** How long the page must not scroll before a scroll counts as stopped, where the browser has no `scrollend`, in milliseconds. */
export const SCROLL_STOP_MS = 150;

/** The storage key of the position of the page at `address`: the address without its fragment. */
export function positionKey(address: string): string {
  const hash = address.indexOf('#');
  return POSITION_KEY_PREFIX + (hash === -1 ? address : address.slice(0, hash));
}

/**
 * The position, from the headings' ids and their tops measured from the
 * window's top, and the page's scroll offset: the last heading whose top is at
 * or above the window's top, or the page's offset above the first heading.
 */
export function positionAt(ids: readonly string[], tops: readonly number[], scrollY: number): Position {
  const index = currentHeadingIndex(tops, ABOVE_TOLERANCE_PX);
  return index >= 0 ? { id: ids[index], offset: -tops[index] } : { id: null, offset: scrollY };
}

/** A position read back from a history state's field or from the storage; `null` when the value is not one. */
export function readPosition(value: unknown): Position | null {
  if (value === null || typeof value !== 'object') return null;
  const { id, offset } = value as { id?: unknown; offset?: unknown };
  if ((id !== null && typeof id !== 'string') || typeof offset !== 'number' || !Number.isFinite(offset)) return null;
  return { id, offset };
}

/** The position a history entry's state holds, or `null`. */
export function positionInState(state: unknown): Position | null {
  if (state === null || typeof state !== 'object') return null;
  return readPosition((state as Record<string, unknown>)[STATE_FIELD]);
}

/** A history state holding `position`, with the other fields of `state` kept (a state that is not an object is replaced). */
export function withPosition(state: unknown, position: Position): Record<string, unknown> {
  const fields = state !== null && typeof state === 'object' ? state as Record<string, unknown> : {};
  return { ...fields, [STATE_FIELD]: position };
}

/** A stored position, parsed; `null` when there is none or it cannot be read. */
export function parseStored(stored: string | null): Position | null {
  if (stored === null) return null;
  try {
    return readPosition(JSON.parse(stored));
  } catch {
    return null;
  }
}

/** The part of MathJax's interface the restore waits for. */
interface MathJaxLike {
  startup?: { promise?: Promise<unknown> };
}

/** The reading position of one page. */
export class ReadingPosition {
  private readonly page: Page;
  private stopTimer = 0;

  constructor(page: Page) {
    this.page = page;
  }

  /** Adds the listeners; called once. */
  attach(): void {
    const win = this.page.doc.defaultView ?? window;
    if ('onscrollend' in (win as object)) {
      win.addEventListener('scrollend', () => this.record(), { passive: true });
    } else {
      win.addEventListener('scroll', () => {
        window.clearTimeout(this.stopTimer);
        this.stopTimer = window.setTimeout(() => this.record(), SCROLL_STOP_MS);
      }, { passive: true });
    }
    // Following a link to this page: the browser makes the new history entry
    // and jumps; the position is recorded into the entry being left first.
    this.page.doc.addEventListener('click', (event) => {
      const target = event.target as Element | null;
      if (target !== null && typeof target.closest === 'function'
          && target.closest('a[href^="#"], a[data-mjx-href^="#"]') !== null) {
        this.record();
      }
    }, { capture: true, passive: true });
    win.addEventListener('popstate', (event) => {
      const position = positionInState(event.state);
      if (position !== null) this.restore(position);
    });
  }

  /** Restores the position when the page opens: the history entry's, or, without a fragment in the address, the stored one. */
  restoreOnLoad(): void {
    const position = positionInState(history.state)
      ?? (location.hash === '' ? parseStored(loadStored(positionKey(location.href))) : null);
    if (position !== null) this.restore(position);
  }

  /** Records the current position into the history entry's state and into the storage. */
  record(): void {
    const headings = contentHeadings(this.page.content).map((h) => h.element);
    const position = positionAt(
      headings.map((h) => h.id),
      headings.map((h) => h.getBoundingClientRect().top),
      window.scrollY,
    );
    try {
      history.replaceState(withPosition(history.state, position), '');
    } catch {
      // the browser refused the update (too many in a short time); the next scroll stop records again
    }
    store(positionKey(location.href), JSON.stringify(position));
  }

  /**
   * Scrolls to `position` once the page has settled: after two animation
   * frames, and again once MathJax has finished typesetting when the page has
   * MathJax. A heading that is no longer on the page leaves the position as it is.
   */
  restore(position: Position): void {
    const apply = (): void => {
      let top = position.offset;
      if (position.id !== null) {
        const heading = this.page.doc.getElementById(position.id);
        if (heading === null) return;
        top += heading.getBoundingClientRect().top + window.scrollY;
      }
      window.scrollTo({ top, behavior: 'instant' });
    };
    requestAnimationFrame(() => requestAnimationFrame(() => {
      apply();
      const mathJax = (window as unknown as { MathJax?: MathJaxLike }).MathJax;
      if (mathJax && mathJax.startup && mathJax.startup.promise) {
        void mathJax.startup.promise.then(apply);
      }
    }));
  }
}
