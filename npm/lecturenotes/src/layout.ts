// flm-lecturenotes: the page's parts and the settings.
//
// The page this script runs on holds these elements (the classes of the page's
// own elements begin with `ln-`; the content is FLM's HTML output):
//
//   .ln-app                       the frame
//     .ln-topbar                  the sticky bar
//       .ln-book-title            the title, linking to the top
//       .ln-current-unit          the title of the navigation unit being read (written here)
//       .ln-prev, .ln-next        the previous-unit and next-unit buttons
//       .ln-sidebar-toggle        the button hiding or showing the sidebar
//     .ln-sidebar                 the sidebar, holding the table of contents (.flm-toc)
//       .ln-toc-toggle            the unfold toggle: folded or all open
//     .ln-main                    the main column
//       #flm-content              the content (FLM's content region)
//
// Only #flm-content is required; a part the page does not have is left out.

/** The settings the page passes to `setup`; other fields are ignored. */
export interface Settings {
  /**
   * The navigation level (1 by default): the level of the headings that begin
   * the document's navigation units.
   */
  navigation_heading_level?: unknown;
}

/** The settings, checked. */
export interface Options {
  /** The navigation level. */
  navigationLevel: number;
}

/** The navigation level when the settings do not give one. */
export const DEFAULT_NAVIGATION_LEVEL = 1;

/** The elements of the page this script works with. */
export interface Page {
  doc: Document;
  /** The root element, which carries the page's state classes. */
  root: HTMLElement;
  content: HTMLElement;
  bar: HTMLElement | null;
  unitText: HTMLElement | null;
  prev: HTMLButtonElement | null;
  next: HTMLButtonElement | null;
  sidebarToggle: HTMLElement | null;
  sidebar: HTMLElement | null;
  tocToggle: HTMLElement | null;
}

/**
 * Reads the settings: `navigation_heading_level` must be a whole number from
 * 0 up. A missing value gives the default; a wrong one is reported on the
 * console and gives the default.
 */
export function readSettings(settings: unknown): Options {
  const given = (settings !== null && typeof settings === 'object')
    ? (settings as Settings).navigation_heading_level : undefined;
  if (given === undefined || given === null) {
    return { navigationLevel: DEFAULT_NAVIGATION_LEVEL };
  }
  if (typeof given === 'number' && Number.isInteger(given) && given >= 0) {
    return { navigationLevel: given };
  }
  console.error(`flm-lecturenotes: the setting \`navigation_heading_level\` must be a whole number from 0 up, and it is \`${JSON.stringify(given)}\``);
  return { navigationLevel: DEFAULT_NAVIGATION_LEVEL };
}

/** Finds the page's parts; `null`, reported on the console, when the page has no content. */
export function findPage(doc: Document): Page | null {
  const content = doc.getElementById('flm-content');
  if (content === null) {
    console.error('flm-lecturenotes: the page must hold an element with the id `flm-content`');
    return null;
  }
  const one = <T extends Element = HTMLElement>(selector: string): T | null => doc.querySelector<T>(selector);
  return {
    doc,
    root: doc.documentElement,
    content,
    bar: one('.ln-topbar'),
    unitText: one('.ln-current-unit'),
    prev: one<HTMLButtonElement>('.ln-prev'),
    next: one<HTMLButtonElement>('.ln-next'),
    sidebarToggle: one('.ln-sidebar-toggle'),
    sidebar: one('.ln-sidebar'),
    tocToggle: one('.ln-toc-toggle'),
  };
}

/** A heading's text, without the anchor this script adds, with its spaces collapsed. */
export function textOf(heading: Element): string {
  const copy = heading.cloneNode(true) as Element;
  copy.querySelectorAll('.ln-anchor').forEach((a) => a.remove());
  return (copy.textContent ?? '').replace(/\s+/g, ' ').trim();
}

/** Reads a value of the browser's storage; `null` when there is none or the storage is unavailable. */
export function loadStored(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** Writes a value into the browser's storage; nothing when the storage is unavailable. */
export function store(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // the storage is unavailable (a private window, blocked site data)
  }
}
