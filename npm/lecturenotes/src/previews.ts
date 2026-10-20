// flm-lecturenotes: the hover previews.
//
// A link to a place of this page (a reference, a defined term, a citation, a
// footnote) shows a popup with a copy of what it points to: when the pointer
// rests on it, after a long press on a touch screen, and when it gets the
// keyboard's focus. The functions at the top choose and copy the content; the
// class `Previews` shows the popup. The popup is one element of this script's
// own, added at the end of the page's body; nothing of the document is moved:
// the popup holds copies.

import { fragmentId } from './sidebar';

// ── The links ──────────────────────────────────────────────────────────────

/** The links that show a preview: a reference, a defined term, a citation, a footnote. */
export const PREVIEW_LINKS = [
  'a.flm-pres-link[data-flm-kind="ref"]',
  'a.flm-pres-link[data-flm-kind="term"]',
  'a.flm-pres-link[data-flm-kind="cite"]',
  'a.flm-pres-link[data-flm-kind="xnote"]',
].join(', ');

/** The class of the popup element. */
export const POPUP_CLASS = 'ln-preview';

/**
 * The id a link points to on this page, or `null` when its address is not a
 * fragment of this page. A link inside a formula typeset by MathJax keeps its
 * address in `data-mjx-href`.
 */
export function linkTarget(link: Element): string | null {
  const address = link.getAttribute('href') ?? link.getAttribute('data-mjx-href');
  if (address === null || !address.startsWith('#') || address.length < 2) return null;
  return fragmentId(address);
}

// ── The citations ──────────────────────────────────────────────────────────

/** A part of a citation mark, in order: a link to a work (the id it points to) or the text between links. */
export type MarkPart = { link: string } | { text: string };

/** The works from `first` to `last` in the bibliography's order (one work when they are the same). */
export interface CitedRange {
  first: string;
  last: string;
}

/** Whether the text between two links of a citation mark joins them into a range: a dash, with spaces or none. */
export function joinsRange(text: string): boolean {
  return /^\s*[–-]\s*$/.test(text);
}

/**
 * The works a citation mark covers, from its parts: a dash (`–` or `-`)
 * between two links joins them into a range; any other text between them
 * separates them. Text before the first link and after the last is ignored.
 */
export function citedRanges(parts: readonly MarkPart[]): CitedRange[] {
  const ranges: CitedRange[] = [];
  let between = '';
  for (const part of parts) {
    if ('text' in part) {
      between += part.text;
      continue;
    }
    const previous = ranges[ranges.length - 1];
    if (previous !== undefined && joinsRange(between)) {
      previous.last = part.link;
    } else {
      ranges.push({ first: part.link, last: part.link });
    }
    between = '';
  }
  return ranges;
}

/** The parts of a citation mark, in document order: its citation links and the text around them. */
export function markParts(mark: Element): MarkPart[] {
  const parts: MarkPart[] = [];
  const walk = (node: Node): void => {
    node.childNodes.forEach((child) => {
      if (child.nodeType === 3) {
        parts.push({ text: child.textContent ?? '' });
      } else if (child.nodeType === 1) {
        const element = child as Element;
        const id = element.matches('a[data-flm-kind="cite"]') ? linkTarget(element) : null;
        if (id !== null) parts.push({ link: id });
        else walk(element);
      }
    });
  };
  walk(mark);
  return parts;
}

/**
 * The entries from `first` to `last` among their siblings, both included; when
 * `last` does not come after `first` in the same list, the two entries alone.
 */
export function entriesBetween(first: Element, last: Element): Element[] {
  const out: Element[] = [];
  for (let entry: Element | null = first; entry !== null; entry = entry.nextElementSibling) {
    out.push(entry);
    if (entry === last) return out;
  }
  return [first, last];
}

/**
 * The bibliography entries (`li` elements) a citation link's mark covers, in
 * the mark's order, ranges expanded. Every link of one mark gives the same
 * entries.
 */
export function citationEntries(link: Element, doc: Document): Element[] {
  const mark = link.closest('.flm-citations-mark');
  const own = linkTarget(link);
  const parts: MarkPart[] = mark !== null ? markParts(mark) : own !== null ? [{ link: own }] : [];
  const entryOf = (id: string): Element | null => doc.getElementById(id)?.closest('li') ?? null;
  const out: Element[] = [];
  for (const { first, last } of citedRanges(parts)) {
    const a = entryOf(first);
    const b = entryOf(last);
    if (a !== null && b !== null) out.push(...entriesBetween(a, b));
    else if (a !== null) out.push(a);
    else if (b !== null) out.push(b);
  }
  return out;
}

// ── The content ────────────────────────────────────────────────────────────

/** A heading of the document. */
const HEADING = '.flm-pres-block--heading';
/** The blocks an inline target is shown in: the nearest of them that encloses it. */
const BLOCKS = '.flm-pres-par, .flm-pres-block, li, figure, figcaption, blockquote';
/** The elements that are inline: a target of one of these is shown in its enclosing block. */
const INLINE_TAGS = new Set(['SPAN', 'A', 'EM', 'STRONG', 'SUP']);
/** A displayed formula: its source element, or the container MathJax typeset it into. */
const DISPLAY_SOURCE = '.flm-pres-math--source[data-flm-display="display"]';
const DISPLAY_TYPESET = 'mjx-container[display="true"]';
/** A paragraph. */
const PARAGRAPH = '.flm-pres-par';
/** How many siblings after a heading its preview shows at most. */
export const HEADING_SIBLINGS = 3;
/** How many entries a citation's preview lists at most. */
export const MAX_ENTRIES = 8;

/**
 * The elements a preview of `target` copies, in order:
 * - a heading: the heading and up to three following siblings, until the next heading;
 * - a list entry: the entry;
 * - an inline element: the nearest enclosing block (a paragraph, a block, a
 *   list entry, a figure, a caption, a quotation); when there is none and the
 *   target stands in a displayed formula (between paragraphs, outside any
 *   paragraph), the formula with the paragraph before it and the paragraph
 *   after it, each when there is one;
 * - anything else: the target itself.
 */
export function previewElements(target: Element): Element[] {
  if (target.matches(HEADING)) {
    const out = [target];
    for (let next = target.nextElementSibling; next !== null && out.length <= HEADING_SIBLINGS;
      next = next.nextElementSibling) {
      if (next.matches(HEADING)) break;
      out.push(next);
    }
    return out;
  }
  if (target.tagName === 'LI' || !INLINE_TAGS.has(target.tagName)) return [target];
  const block = target.closest(BLOCKS);
  if (block !== null) return [block];
  const formula = target.closest(DISPLAY_SOURCE) ?? target.closest(DISPLAY_TYPESET);
  if (formula === null) return [target];
  const before = formula.previousElementSibling;
  const after = formula.nextElementSibling;
  return [
    ...(before !== null && before.matches(PARAGRAPH) ? [before] : []),
    formula,
    ...(after !== null && after.matches(PARAGRAPH) ? [after] : []),
  ];
}

/** The texts the popup writes. */
const TEXTS = {
  more: (count: number): string => `and ${count} more`,
};

/**
 * Removes from a copy what must not be shown twice on the page: every id (the
 * copy's root's too), the `§` anchors this script adds to headings, and a
 * footnote's link back to its mark.
 */
export function cleanCopy(copy: Element): void {
  copy.removeAttribute('id');
  copy.querySelectorAll('[id]').forEach((element) => element.removeAttribute('id'));
  copy.querySelectorAll('.ln-anchor, a[data-flm-kind="xnote-back"]').forEach((element) => element.remove());
}

/**
 * The content of the preview of `link`: a `div` holding copies of the
 * elements to show (a list entry inside a copy of its list, without the
 * list's other entries); `null` when the link points to nothing on the page.
 * A citation's preview lists the first eight entries and a line saying how many
 * more there are.
 */
export function buildPreview(link: Element, doc: Document): HTMLElement | null {
  const id = linkTarget(link);
  if (id === null) return null;
  let elements: Element[];
  let more = 0;
  if (link.getAttribute('data-flm-kind') === 'cite') {
    const entries = citationEntries(link, doc);
    elements = entries.slice(0, MAX_ENTRIES);
    more = entries.length - elements.length;
  } else {
    const target = doc.getElementById(id);
    if (target === null) return null;
    elements = previewElements(target);
  }
  if (elements.length === 0) return null;

  const content = doc.createElement('div');
  content.className = 'ln-preview-content';
  let list: Element | null = null;
  let listOf: Element | null = null;
  for (const element of elements) {
    const copy = element.cloneNode(true) as Element;
    const parent = element.parentElement;
    if (element.tagName === 'LI' && parent !== null) {
      if (list === null || listOf !== parent) {
        list = parent.cloneNode(false) as Element;
        listOf = parent;
        content.appendChild(list);
      }
      list.appendChild(copy);
    } else {
      list = null;
      listOf = null;
      content.appendChild(copy);
    }
  }
  if (more > 0) {
    const line = doc.createElement('p');
    line.className = 'ln-preview-more';
    line.textContent = TEXTS.more(more);
    content.appendChild(line);
  }
  cleanCopy(content);
  return content;
}

// ── The placement ──────────────────────────────────────────────────────────

/** The space between the link and the popup, in pixels. */
export const POPUP_GAP_PX = 6;
/** The space the popup keeps from the window's edges, in pixels. */
export const EDGE_PX = 8;

/** A box, as `getBoundingClientRect` gives it (relative to the window). */
export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/**
 * Where the popup goes, relative to the window: below the link, centered on
 * it; above it when the popup does not fit below and does fit above; when it
 * fits on neither side, its top is clamped to the window (from the edge margin
 * to the window's height less the margin and the popup's height, the edge
 * margin winning for a popup taller than the window); moved sideways to stay
 * inside the window.
 */
export function placePopup(
  link: Box, width: number, height: number, windowWidth: number, windowHeight: number,
): { left: number; top: number } {
  let top = link.bottom + POPUP_GAP_PX;
  const above = link.top - POPUP_GAP_PX - height;
  if (top + height > windowHeight - EDGE_PX) {
    if (above >= EDGE_PX) {
      top = above;
    } else {
      top = Math.min(top, windowHeight - EDGE_PX - height);
      top = Math.max(top, EDGE_PX);
    }
  }
  let left = (link.left + link.right) / 2 - width / 2;
  left = Math.min(left, windowWidth - EDGE_PX - width);
  left = Math.max(left, EDGE_PX);
  return { left, top };
}

// ── The popup ──────────────────────────────────────────────────────────────

/** How long the pointer rests on a link before its preview shows, in milliseconds. */
export const SHOW_DELAY_MS = 150;
/** How long after the pointer left the link and the popup the popup hides, in milliseconds. */
export const HIDE_DELAY_MS = 250;
/** How long a touch on a link lasts before it counts as a long press, in milliseconds. */
export const LONG_PRESS_MS = 500;
/** How long after a long press ends the click it produces is not followed, in milliseconds. */
export const CLICK_GUARD_MS = 700;
/** How far a touch may move and still count as a long press, in pixels. */
export const PRESS_TOLERANCE_PX = 10;

/** The part of MathJax's interface the popup uses. */
export interface MathJaxLike {
  typesetPromise?: (elements: Element[]) => Promise<unknown>;
  typesetClear?: (elements: Element[]) => void;
  startup?: { promise?: Promise<unknown> };
}

/**
 * Typesets the formulas of `popup`. When MathJax has a startup promise, the
 * call is chained after it (as the library's `mathjax-config.js` does), so that
 * it never runs alongside the page's own typesetting or a previous popup's,
 * and the chain is stored back. A failure ends the chain without an error.
 */
export function typesetPopup(mathJax: MathJaxLike, popup: Element): Promise<void> {
  const typeset = mathJax.typesetPromise;
  if (typeset === undefined) return Promise.resolve();
  const run = () => typeset.call(mathJax, [popup]);
  const startup = mathJax.startup;
  const work = startup?.promise !== undefined ? startup.promise.then(run) : run();
  const chain = work.then(() => undefined, () => undefined);
  if (startup?.promise !== undefined) startup.promise = chain;
  return chain;
}

/** The part of an element `positionPopup` uses. */
export interface Placeable {
  readonly offsetWidth: number;
  readonly offsetHeight: number;
  readonly style: { left: string; top: string };
}

/**
 * Places `popup` for `link` (both relative to the window), on a page scrolled
 * by `scrollX` and `scrollY`. The popup is first moved to the page's top left
 * corner, so that its size is measured with the whole width of the window
 * available, not at the place of the preview shown before.
 */
export function positionPopup(
  popup: Placeable, link: Box, windowWidth: number, windowHeight: number, scrollX: number, scrollY: number,
): void {
  popup.style.left = '0px';
  popup.style.top = '0px';
  const { left, top } = placePopup(link, popup.offsetWidth, popup.offsetHeight, windowWidth, windowHeight);
  popup.style.left = `${left + scrollX}px`;
  popup.style.top = `${top + scrollY}px`;
}

/** The touch on a link that may become a long press. */
interface Press {
  link: Element;
  pointerId: number;
  x: number;
  y: number;
  timer: number;
  fired: boolean;
}

/** The previews of one page. */
export class Previews {
  private readonly doc: Document;
  private popup: HTMLElement | null = null;
  /** The link whose preview the popup shows. */
  private shownFor: Element | null = null;
  private showTimer = 0;
  private pendingFor: Element | null = null;
  private hideTimer = 0;
  private press: Press | null = null;
  /** The link whose next click is not followed, and until when. */
  private guard: { link: Element; until: number } | null = null;

  constructor(doc: Document) {
    this.doc = doc;
  }

  /** Adds the listeners; called once. */
  attach(): void {
    const { doc } = this;
    doc.addEventListener('pointerover', (event) => this.onOver(event), { passive: true });
    doc.addEventListener('pointerout', (event) => this.onOut(event), { passive: true });
    doc.addEventListener('pointerdown', (event) => this.onDown(event), { passive: true });
    doc.addEventListener('pointermove', (event) => this.onMove(event), { passive: true });
    doc.addEventListener('pointerup', (event) => this.endPress(event), { passive: true });
    doc.addEventListener('pointercancel', (event) => this.endPress(event), { passive: true });
    doc.addEventListener('click', (event) => this.onClick(event));
    doc.addEventListener('contextmenu', (event) => {
      if (this.press !== null) event.preventDefault();
    });
    doc.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') this.hide();
    }, { passive: true });
    doc.addEventListener('focusin', (event) => {
      const link = this.previewLink(event.target);
      if (link !== null && focusVisible(link)) this.scheduleShow(link);
    }, { passive: true });
    doc.addEventListener('focusout', (event) => {
      const link = this.previewLink(event.target);
      if (link !== null && (link === this.shownFor || link === this.pendingFor)) this.hide();
    }, { passive: true });
  }

  /** Hides the popup now and forgets any pending preview. */
  hide(): void {
    this.cancelShow();
    this.cancelHide();
    this.shownFor = null;
    this.popup?.classList.remove('ln-preview-shown');
  }

  /** The previewed link that holds `target`, or `null` (a link inside the popup is not previewed). */
  private previewLink(target: EventTarget | null): Element | null {
    const element = target as Element | null;
    if (element === null || typeof element.closest !== 'function') return null;
    const link = element.closest(PREVIEW_LINKS);
    if (link === null || link.closest('.' + POPUP_CLASS) !== null || linkTarget(link) === null) return null;
    return link;
  }

  private inPopup(target: EventTarget | null): boolean {
    const element = target as Element | null;
    return this.popup !== null && element !== null && typeof element.closest === 'function'
      && this.popup.contains(element);
  }

  private onOver(event: PointerEvent): void {
    if (event.pointerType === 'touch') return;
    if (this.inPopup(event.target)) {
      this.cancelHide();
      return;
    }
    const link = this.previewLink(event.target);
    if (link === null) return;
    this.cancelHide();
    if (link !== this.shownFor) this.scheduleShow(link);
  }

  private onOut(event: PointerEvent): void {
    if (event.pointerType === 'touch') return;
    const from = this.previewLink(event.target);
    if (from === null && !this.inPopup(event.target)) return;
    const to = event.relatedTarget as Element | null;
    if (to !== null && ((from !== null && from.contains(to)) || this.inPopup(to))) return;
    if (from !== null && from === this.pendingFor) this.cancelShow();
    if (this.shownFor !== null) this.scheduleHide();
  }

  private onDown(event: PointerEvent): void {
    const link = this.previewLink(event.target);
    if (!this.inPopup(event.target) && (link === null || link !== this.shownFor)) this.hide();
    if (event.pointerType !== 'touch' || link === null) return;
    this.cancelPress();
    this.press = {
      link,
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      timer: window.setTimeout(() => this.firePress(), LONG_PRESS_MS),
      fired: false,
    };
  }

  private onMove(event: PointerEvent): void {
    const press = this.press;
    if (press === null || press.pointerId !== event.pointerId || press.fired) return;
    if (Math.hypot(event.clientX - press.x, event.clientY - press.y) > PRESS_TOLERANCE_PX) this.cancelPress();
  }

  private firePress(): void {
    const press = this.press;
    if (press === null) return;
    press.fired = true;
    this.guard = { link: press.link, until: Infinity };
    this.show(press.link);
  }

  private endPress(event: PointerEvent): void {
    const press = this.press;
    if (press === null || press.pointerId !== event.pointerId) return;
    if (press.fired) this.guard = { link: press.link, until: Date.now() + CLICK_GUARD_MS };
    this.cancelPress();
  }

  private cancelPress(): void {
    if (this.press !== null) window.clearTimeout(this.press.timer);
    this.press = null;
  }

  private onClick(event: MouseEvent): void {
    const guard = this.guard;
    if (guard === null) return;
    this.guard = null;
    const link = this.previewLink(event.target);
    if (link === guard.link && Date.now() <= guard.until) event.preventDefault();
  }

  private scheduleShow(link: Element): void {
    this.cancelShow();
    this.pendingFor = link;
    this.showTimer = window.setTimeout(() => {
      this.pendingFor = null;
      this.show(link);
    }, SHOW_DELAY_MS);
  }

  private cancelShow(): void {
    window.clearTimeout(this.showTimer);
    this.pendingFor = null;
  }

  private scheduleHide(): void {
    this.cancelHide();
    this.hideTimer = window.setTimeout(() => this.hide(), HIDE_DELAY_MS);
  }

  private cancelHide(): void {
    window.clearTimeout(this.hideTimer);
  }

  private getPopup(): HTMLElement {
    if (this.popup === null) {
      this.popup = this.doc.createElement('div');
      this.popup.className = POPUP_CLASS;
      this.doc.body.appendChild(this.popup);
    }
    return this.popup;
  }

  /** Shows the preview of `link` now. */
  private show(link: Element): void {
    this.cancelHide();
    const content = buildPreview(link, this.doc);
    if (content === null) return;
    const popup = this.getPopup();
    const mathJax = (window as unknown as { MathJax?: MathJaxLike }).MathJax;
    mathJax?.typesetClear?.([popup]);
    popup.replaceChildren(content);
    popup.scrollTop = 0;
    popup.classList.add('ln-preview-shown');
    this.shownFor = link;
    this.place(link);
    const untypeset = Array.from(popup.querySelectorAll('.flm-pres-math--source'))
      .some((formula) => formula.querySelector('mjx-container') === null);
    if (untypeset && mathJax !== undefined) {
      void typesetPopup(mathJax, popup).then(() => {
        if (this.shownFor === link) this.place(link);
      });
    }
  }

  private place(link: Element): void {
    const popup = this.popup;
    if (popup === null) return;
    const view = this.doc.documentElement;
    positionPopup(popup, link.getBoundingClientRect(), view.clientWidth, view.clientHeight,
      window.scrollX, window.scrollY);
  }
}

/** Whether `element` has the focus the browser shows (the keyboard's, not a click's). */
function focusVisible(element: Element): boolean {
  try {
    return element.matches(':focus-visible');
  } catch {
    return true;
  }
}
