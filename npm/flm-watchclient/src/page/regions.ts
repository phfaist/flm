// The update of the page after a build. A build's update lists the output regions whose
// HTML changed (each an element of the page whose identifier the update gives) and the
// files beside the page that changed, or asks for the page to be loaded anew. A changed
// region is brought up to date in place: `morphdom` changes the element's content into
// the new one while keeping every element that is the same in both, so that the reader's
// place is kept. Then the event `flm:region-updated` is dispatched on the region's
// element; it reaches `document`.
//
// An element a script of the page transformed in the browser carries the attribute
// `data-flm-original-html`, the HTML it held before. Such an element is left as it is
// when its new HTML equals that record, and brought up to date otherwise; the script that
// transformed it transforms it again on the event.
//
// The update records the elements it changed: those added, and those whose attributes or
// own text changed. They are what *Scroll to changes* shows.

import morphdom from 'morphdom';
import type { Region, Update } from '../shared/protocol';
import { OWN_ATTRIBUTES } from './attributes';
import { outermost } from './places';

/** The attribute that records the HTML an element held before a script transformed it. */
export const ORIGINAL = 'data-flm-original-html';

/** The event dispatched on a region's element once its content was brought up to date. */
export const REGION_UPDATED = 'flm:region-updated';

export function reload(): void {
  window.location.reload();
}

/** The text of the nodes directly inside `element`, joined. */
function ownText(element: Element): string {
  let text = '';
  for (const node of Array.from(element.childNodes)) {
    if (node.nodeType === Node.TEXT_NODE) text += node.nodeValue ?? '';
  }
  return text;
}

/** The attributes of `element`, the page script's own left out, as `name=value` lines. */
function attributesOf(element: Element): string {
  return Array.from(element.attributes)
    .filter((attribute) => !OWN_ATTRIBUTES.includes(attribute.name))
    .map((attribute) => `${attribute.name}=${attribute.value}`)
    .sort()
    .join('\n');
}

/**
 * Brings the region `region` of an update up to date, `artifacts` the paths of the files
 * beside the page that changed, and adds the elements it changed to `changed`. Gives
 * `false` when the page has no place for the region (no element with the region's `id`,
 * or words other than the page's title, which is the only place for words that every page
 * has), so that the page must be loaded anew.
 */
function updateRegion(region: Region, artifacts: readonly string[], changed: Set<Element>): boolean {
  if (region.kind === 'words') {
    if (region.name !== 'title') return false;
    document.title = region.text;
    return true;
  }
  const element = document.getElementById(region.id);
  if (element === null) return false;
  const template = document.createElement('template');
  template.innerHTML = region.text;
  const target = element.cloneNode(false) as Element;
  target.appendChild(template.content);
  morphdom(element, target, {
    childrenOnly: true,
    onBeforeElUpdated(from, to) {
      // An element transformed in the browser whose new HTML equals the record keeps the
      // record, and its content is left alone (below).
      const original = from.getAttribute(ORIGINAL);
      if (original !== null && original === to.innerHTML) {
        to.setAttribute(ORIGINAL, original);
      } else if (original !== null || attributesOf(from) !== attributesOf(to) || ownText(from) !== ownText(to)) {
        changed.add(from);
      }
      return true;
    },
    onBeforeElChildrenUpdated(from, to) {
      return !(from.hasAttribute(ORIGINAL) && to.hasAttribute(ORIGINAL));
    },
    onNodeAdded(node) {
      // An added text node changes the element around it, except the region's element
      // itself: what lies directly in it is the whitespace between blocks, and the whole
      // region is no change to show.
      if (node instanceof Element) {
        changed.add(node);
      } else if (node.nodeType === Node.TEXT_NODE && (node.nodeValue ?? '').trim() !== '') {
        const parent = node.parentElement;
        if (parent !== null && parent !== element) changed.add(parent);
      }
      return node;
    },
  });
  element.dispatchEvent(new CustomEvent(REGION_UPDATED, {
    bubbles: true,
    detail: { region: region.id, artifacts: artifacts.slice() },
  }));
  return true;
}

/** The attribute of `element` that holds the address of the file it shows. */
function addressAttribute(element: Element): string {
  switch (element.tagName) {
    case 'LINK': return 'href';
    case 'OBJECT': return 'data';
    default: return 'src';
  }
}

/**
 * Loads anew, where the page shows them, the files at `paths` (paths in the output, as
 * the update lists them): each element that shows one gets its address again, with a
 * query parameter holding the build's number, so that the browser asks the server
 * instead of reusing its cached copy. The server ignores the query.
 */
function reloadArtifacts(paths: readonly string[], build: number): void {
  if (paths.length === 0) return;
  const root = window.location.origin + '/';
  const changed = paths.map((path) => new URL(path, root).href);
  const elements = document.querySelectorAll(
    'img[src], source[src], video[src], audio[src], iframe[src], embed[src], link[href], object[data]');
  for (const element of Array.from(elements)) {
    const name = addressAttribute(element);
    const address = new URL(element.getAttribute(name) ?? '', window.location.href);
    address.search = '';
    address.hash = '';
    if (changed.includes(address.href)) {
      address.searchParams.set('flm-preview-build', String(build));
      element.setAttribute(name, address.href);
    }
  }
}

/** What applying an update did to the page. */
export type Applied =
  | { kind: 'reloading' }
  | { kind: 'applied'; changed: Element[] };

/**
 * Applies `update` to the page: loads the page anew when it asks for it or when a region
 * has no place in the page; otherwise brings each changed region up to date and loads the
 * changed files anew. A build that gave no output changes nothing. Gives the elements the
 * update changed that are still in the page, none inside another.
 */
export function applyUpdate(update: Update): Applied {
  if (update.page === 'reload') {
    reload();
    return { kind: 'reloading' };
  }
  const changed = new Set<Element>();
  if (update.page === 'regions') {
    for (const region of update.regions) {
      if (!updateRegion(region, update.files, changed)) {
        reload();
        return { kind: 'reloading' };
      }
    }
  }
  if (update.page !== 'absent') reloadArtifacts(update.files, update.build);
  return { kind: 'applied', changed: outermost(Array.from(changed).filter((element) => element.isConnected)) };
}
