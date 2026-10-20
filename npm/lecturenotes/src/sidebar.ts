// flm-lecturenotes: the sidebar, the bar and the chapters.
//
// The pure functions at the top compute over plain lists (the levels of the
// content's headings, their positions); the class `Navigator` applies them to
// the page. The script never wraps or moves an element of the document: it sets
// attributes and classes, adds the `§` anchors, and does all of it again after
// the live preview updated the page in place.

import { type Options, type Page, loadStored, store, textOf } from './layout';

// ── The storage ────────────────────────────────────────────────────────────

/** The keys of the browser's storage this script uses; the values are kept across pages and visits. */
export const STORAGE_KEYS = {
  /** The folding of the table of contents: `folded` or `all`. */
  folding: 'flm-lecturenotes:toc-folding',
  /** Whether the sidebar is hidden on a wide window: `1` or `0`. */
  sidebarHidden: 'flm-lecturenotes:sidebar-hidden',
} as const;

// ── The structure ──────────────────────────────────────────────────────────

/** The indices, among the headings, of the chapters: the headings whose level is the chapter level. */
export function chapterIndices(levels: readonly number[], chapterLevel: number): number[] {
  const out: number[] = [];
  levels.forEach((level, i) => {
    if (level === chapterLevel) out.push(i);
  });
  return out;
}

/**
 * The index of the current heading: the last whose top is at or above `line`
 * (the tops in document order, as measured from the window's top); -1 above the
 * first heading.
 */
export function currentHeadingIndex(tops: readonly number[], line: number): number {
  let best = -1;
  for (let i = 0; i < tops.length; i++) {
    if (tops[i] <= line) best = i;
    else break;
  }
  return best;
}

/**
 * The index of the chapter heading that holds the heading `index`: the nearest
 * chapter at or before it, unless a part (a heading above the chapter level)
 * comes in between; -1 when there is none.
 */
export function chapterOf(levels: readonly number[], index: number, chapterLevel: number): number {
  for (let i = index; i >= 0; i--) {
    if (levels[i] === chapterLevel) return i;
    if (levels[i] < chapterLevel) return -1;
  }
  return -1;
}

/**
 * The headings the arrows lead to, as indices among the headings (-1 for none),
 * with `index` the current heading: the next chapter is the first that begins
 * after the current heading; the previous chapter is the one before the chapter
 * being read, or, outside a chapter, the last chapter before the current heading.
 */
export function navigationTargets(
  levels: readonly number[], index: number, chapterLevel: number,
): { prev: number; next: number } {
  const chapters = chapterIndices(levels, chapterLevel);
  const started = chapters.filter((c) => c <= index).length;
  const inChapter = chapterOf(levels, index, chapterLevel) !== -1;
  const prevPosition = inChapter ? started - 2 : started - 1;
  return {
    prev: prevPosition >= 0 ? chapters[prevPosition] : -1,
    next: started < chapters.length ? chapters[started] : -1,
  };
}

// ── The folding ────────────────────────────────────────────────────────────

/** The table of contents: only the chapter being read shows its sections (`folded`), or every chapter does (`all`). */
export type Folding = 'folded' | 'all';

/** The folding a stored value gives; `folded` for anything else. */
export function parseFolding(stored: string | null): Folding {
  return stored === 'all' ? 'all' : 'folded';
}

/** The folding after a click on the triangle toggle. */
export function toggleFolding(folding: Folding): Folding {
  return folding === 'all' ? 'folded' : 'all';
}

/** Whether a stored value hides the sidebar. */
export function parseHidden(stored: string | null): boolean {
  return stored === '1';
}

// ── Following the reader ───────────────────────────────────────────────────

/** How long the sidebar stops following the reader after the reader scrolled it, in milliseconds. */
export const FOLLOW_PAUSE_MS = 4000;
/** How long a scroll of the sidebar after this script scrolled it counts as this script's, in milliseconds. */
export const OWN_SCROLL_MS = 1000;
/** How close to the sidebar's edge the current entry may come before the sidebar follows, in pixels. */
export const FOLLOW_MARGIN_PX = 40;
/**
 * How far below the point the browser scrolls a target to (the root's
 * `scroll-padding-top`) a heading's top may be and the heading still be the
 * current one, in pixels.
 */
export const CURRENT_MARGIN_PX = 4;

/**
 * The line a heading's top must be at or above to be the current heading, as
 * measured from the window's top: the root's computed `scroll-padding-top`
 * (`auto` counts as 0) plus `CURRENT_MARGIN_PX`. A heading the arrows or a
 * link scrolled to is thus the current one.
 */
export function currentLine(scrollPaddingTop: string): number {
  const padding = parseFloat(scrollPaddingTop);
  return (Number.isFinite(padding) ? padding : 0) + CURRENT_MARGIN_PX;
}

/** Whether the sidebar follows the reader now, the reader having last scrolled it at `userScrolledAt`. */
export function followAllowed(now: number, userScrolledAt: number): boolean {
  return now - userScrolledAt >= FOLLOW_PAUSE_MS;
}

/** Whether an entry spanning `top` to `bottom` lies outside the box `boxTop` to `boxBottom`, with the margin. */
export function outsideView(top: number, bottom: number, boxTop: number, boxBottom: number): boolean {
  return top < boxTop + FOLLOW_MARGIN_PX || bottom > boxBottom - FOLLOW_MARGIN_PX;
}

// ── The page ───────────────────────────────────────────────────────────────

/** The texts of the buttons and anchors this script writes. */
const TEXTS = {
  expandAll: 'Expand all sections',
  foldToCurrent: "Show only the current chapter's sections",
  anchor: 'Link to this heading',
};

/** A heading of the content, with its level. */
export interface Heading {
  element: HTMLElement;
  level: number;
}

/**
 * The headings of the content, in document order: every heading with an id
 * and a level, outside the regions of the pieces placed in the content.
 */
export function contentHeadings(content: HTMLElement): Heading[] {
  const headings: Heading[] = [];
  content.querySelectorAll<HTMLElement>('.flm-pres-block--heading[data-flm-level]').forEach((element) => {
    const level = parseInt(element.getAttribute('data-flm-level') ?? '', 10);
    if (element.id && !Number.isNaN(level) && !element.closest('[id^="flm-region-flm."]')) {
      headings.push({ element, level });
    }
  });
  return headings;
}

/** The sidebar, the bar and the chapters of one page. */
export class Navigator {
  private readonly page: Page;
  private readonly chapterLevel: number;
  private headings: Heading[] = [];
  private levels: number[] = [];
  /** The link of each heading's entry in the table of contents, by heading id. */
  private entries = new Map<string, HTMLAnchorElement>();
  private current = -1;
  private userScrolledAt = -Infinity;
  private ownScrollUntil = -Infinity;
  private ticking = false;
  /** Called before the arrows or the keys scroll to a chapter (the reading position records itself there). */
  onBeforeJump: (() => void) | null = null;

  constructor(page: Page, options: Options) {
    this.page = page;
    this.chapterLevel = options.chapterLevel;
  }

  /** Adds the listeners and restores the stored states; called once. */
  attach(): void {
    const { page } = this;
    const win = page.doc.defaultView ?? window;
    this.setFolding(parseFolding(loadStored(STORAGE_KEYS.folding)));
    page.root.classList.toggle('ln-sidebar-collapsed', parseHidden(loadStored(STORAGE_KEYS.sidebarHidden)));

    win.addEventListener('scroll', () => this.schedule(), { passive: true });
    win.addEventListener('resize', () => this.schedule(), { passive: true });
    page.sidebar?.addEventListener('scroll', () => {
      if (Date.now() > this.ownScrollUntil) this.userScrolledAt = Date.now();
    }, { passive: true });

    page.prev?.addEventListener('click', () => this.go('prev'), { passive: true });
    page.next?.addEventListener('click', () => this.go('next'), { passive: true });
    page.doc.addEventListener('keydown', (event) => this.onKey(event), { passive: true });

    page.tocToggle?.addEventListener('click', () => {
      const folding = toggleFolding(page.root.classList.contains('ln-toc-all') ? 'all' : 'folded');
      this.setFolding(folding);
      store(STORAGE_KEYS.folding, folding);
    }, { passive: true });
    page.sidebarToggle?.addEventListener('click', () => {
      if (this.drawerMode()) {
        page.root.classList.toggle('ln-drawer-open');
      } else {
        const hidden = !page.root.classList.contains('ln-sidebar-collapsed');
        page.root.classList.toggle('ln-sidebar-collapsed', hidden);
        store(STORAGE_KEYS.sidebarHidden, hidden ? '1' : '0');
      }
    }, { passive: true });
    page.sidebar?.addEventListener('click', (event) => {
      if ((event.target as Element).closest('a') && this.drawerMode()) {
        page.root.classList.remove('ln-drawer-open');
      }
    }, { passive: true });
    page.doc.addEventListener('click', (event) => {
      const target = event.target as Element;
      if (page.root.classList.contains('ln-drawer-open')
          && !target.closest('.ln-sidebar, .ln-sidebar-toggle')) {
        page.root.classList.remove('ln-drawer-open');
      }
    }, { passive: true });
  }

  /** Reads the headings and the table of contents again, adds the anchors, and updates the page. */
  rebuild(): void {
    const { page } = this;
    this.headings = contentHeadings(page.content);
    this.levels = this.headings.map((h) => h.level);

    this.entries = new Map();
    page.sidebar?.querySelectorAll<HTMLAnchorElement>('.flm-toc a[href^="#"]').forEach((link) => {
      const target = page.doc.getElementById(fragmentId(link.getAttribute('href') ?? '#'));
      const item = link.closest('li');
      const level = target?.getAttribute('data-flm-level');
      if (target === null || item === null || level === null || level === undefined) return;
      item.setAttribute('data-ln-level', level);
      item.classList.toggle('ln-chapter', parseInt(level, 10) === this.chapterLevel);
      this.entries.set(target.id, link);
    });

    this.addAnchors();
    this.current = -2;
    this.update();
  }

  private addAnchors(): void {
    const { page } = this;
    page.content.querySelectorAll('.ln-anchor').forEach((a) => a.remove());
    for (const { element } of this.headings) {
      const anchor = page.doc.createElement('a');
      anchor.className = 'ln-anchor';
      anchor.href = '#' + element.id;
      anchor.textContent = '§';
      anchor.setAttribute('aria-label', TEXTS.anchor);
      element.appendChild(anchor);
    }
  }

  private schedule(): void {
    if (this.ticking) return;
    this.ticking = true;
    requestAnimationFrame(() => {
      this.ticking = false;
      this.update();
    });
  }

  private currentIndex(): number {
    const line = currentLine(getComputedStyle(this.page.root).scrollPaddingTop);
    const tops = this.headings.map((h) => h.element.getBoundingClientRect().top);
    return currentHeadingIndex(tops, line);
  }

  /** Updates the current entry, the folding, the bar's text and the arrows. */
  update(): void {
    const { page } = this;
    const index = this.currentIndex();
    const chapter = index >= 0 ? chapterOf(this.levels, index, this.chapterLevel) : -1;
    const changed = index !== this.current;
    this.current = index;
    const heading = index >= 0 ? this.headings[index].element : null;
    const chapterHeading = chapter >= 0 ? this.headings[chapter].element : null;

    if (changed) {
      for (const link of this.entries.values()) link.removeAttribute('aria-current');
      const link = heading ? this.entries.get(heading.id) : undefined;
      link?.setAttribute('aria-current', 'true');
      page.sidebar?.querySelectorAll('li.ln-open').forEach((li) => li.classList.remove('ln-open'));
      const chapterLink = chapterHeading ? this.entries.get(chapterHeading.id) : undefined;
      chapterLink?.closest('li')?.classList.add('ln-open');
      if (link) this.follow(link);
    }

    if (page.chapterText) page.chapterText.textContent = chapterHeading ? textOf(chapterHeading) : '';
    const targets = navigationTargets(this.levels, index, this.chapterLevel);
    if (page.prev) page.prev.disabled = targets.prev === -1;
    if (page.next) page.next.disabled = targets.next === -1;
  }

  /** Scrolls the sidebar so that the current entry is in its view, unless the reader scrolled it lately. */
  private follow(link: HTMLElement): void {
    const sidebar = this.page.sidebar;
    if (!sidebar || sidebar.getClientRects().length === 0) return;
    const now = Date.now();
    if (!followAllowed(now, this.userScrolledAt)) return;
    const entry = link.getBoundingClientRect();
    const box = sidebar.getBoundingClientRect();
    if (!outsideView(entry.top, entry.bottom, box.top, box.bottom)) return;
    this.ownScrollUntil = now + OWN_SCROLL_MS;
    const top = sidebar.scrollTop + (entry.top - box.top) - (box.height - entry.height) / 2;
    sidebar.scrollTo({ top, behavior: 'smooth' });
  }

  /** Scrolls to the previous or the next chapter and puts its fragment into the address. */
  private go(direction: 'prev' | 'next'): void {
    const target = navigationTargets(this.levels, this.currentIndex(), this.chapterLevel)[direction];
    if (target === -1) return;
    const element = this.headings[target].element;
    this.onBeforeJump?.();
    element.scrollIntoView({ block: 'start' });
    history.pushState(null, '', '#' + element.id);
    this.update();
  }

  private onKey(event: KeyboardEvent): void {
    if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) return;
    const target = event.target as HTMLElement | null;
    if (target && (target.isContentEditable || /^(input|select|textarea)$/i.test(target.tagName))) return;
    if (event.key === '[') this.go('prev');
    else if (event.key === ']') this.go('next');
  }

  private setFolding(folding: Folding): void {
    const { page } = this;
    page.root.classList.toggle('ln-toc-all', folding === 'all');
    if (page.tocToggle) {
      const label = folding === 'all' ? TEXTS.foldToCurrent : TEXTS.expandAll;
      page.tocToggle.title = label;
      page.tocToggle.setAttribute('aria-label', label);
    }
  }

  /** Whether the sidebar is a drawer (a narrow window): the stylesheet then fixes its position. */
  private drawerMode(): boolean {
    const sidebar = this.page.sidebar;
    if (!sidebar) return false;
    return getComputedStyle(sidebar).position === 'fixed';
  }
}

/** The id a fragment address (`#id`) names, decoded. */
export function fragmentId(href: string): string {
  const raw = href.startsWith('#') ? href.slice(1) : href;
  try {
    return decodeURIComponent(raw);
  } catch {
    return raw;
  }
}
