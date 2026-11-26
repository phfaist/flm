// The pages of a PDF file, drawn by pdf.js into a scrolling element: one under the other,
// each a canvas with the text layer above it, so that the text can be selected. A page is
// drawn when it comes near the visible part, and its drawing is released when it moves away
// from it, so that only the pages near the view hold a canvas. A changed file is drawn
// beside the one shown, which stays until the changed pages at the reading position are
// drawn; the reading position (the page and the place in it) and the zoom are kept across
// files.

import type * as PdfJs from 'pdfjs-dist';
import type { PDFDocumentProxy, PDFPageProxy, PageViewport, RenderTask, TextLayer } from 'pdfjs-dist';
import { PDFJS, PDFJS_FONTS, PDFJS_WORKER } from '../../shared/protocol';

type Library = typeof PdfJs;

/** The CSS pixels of a PDF unit at the zoom 1: a PDF unit is 1/72 in, a CSS pixel 1/96 in. */
const CSS_UNITS = 96 / 72;

/** The zooms the buttons step through. */
export const ZOOMS = [0.5, 0.67, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3];

/** The smallest zoom, for a page fitted to a narrow view. */
const LEAST_ZOOM = 0.25;

/** The margin around the pages, in CSS pixels, as the stylesheet sets it. */
const MARGIN = 24;

/** The next zoom of `ZOOMS` above `zoom`, or below it when `direction` is -1; `zoom` at the end. */
export function steppedZoom(zoom: number, direction: 1 | -1): number {
  const next = direction > 0 ? ZOOMS.find((step) => step > zoom + 1e-9) : [...ZOOMS].reverse().find((step) => step < zoom - 1e-9);
  return next ?? zoom;
}

let loading: Promise<Library> | null = null;

/** pdf.js, loaded from the server the first time a PDF is shown; tried again after a failure. */
export function loadPdfJs(): Promise<Library> {
  if (loading === null) {
    // An address held as a `string`, so that neither the type checker nor the bundler looks
    // for the module among the sources.
    const address: string = PDFJS;
    loading = (import(address) as Promise<Library>).then((library) => {
      library.GlobalWorkerOptions.workerSrc = PDFJS_WORKER;
      return library;
    });
    loading.catch(() => {
      loading = null;
    });
  }
  return loading;
}

/** What the view shows: the page read, the number of pages, the zoom. */
export interface Reading {
  page: number;
  pages: number;
  zoom: number;
}

/** A page of the file, its element, and the drawing it shows. */
interface Page {
  proxy: PDFPageProxy;
  element: HTMLDivElement;
  /** The size of the page in PDF units, rotated as the file says. */
  width: number;
  height: number;
  /** The zoom of the drawing shown; `null` before the first. */
  drawn: number | null;
  /** The drawing under way, and the zoom it is for. */
  drawing: { zoom: number; task: RenderTask | null; text: TextLayer | null } | null;
}

/** A place in the pages: the page at the top of the visible part, and how far down it. */
interface Position {
  index: number;
  fraction: number;
  left: number;
}

/** A file, the element that holds its pages, and its pages. */
interface Sheet {
  file: PDFDocumentProxy;
  element: HTMLDivElement;
  pages: Page[];
}

export class PdfPages {
  private readonly scroller: HTMLElement;
  private readonly report: (reading: Reading) => void;
  private readonly failed: (error: unknown) => void;
  private readonly observer: IntersectionObserver;
  private library: Library | null = null;
  private sheet: Sheet | null = null;
  /** The zoom; `null` until the first file is shown, which sets it. */
  private zoom: number | null = null;
  /** Counts the files given, so that a file given later replaces the drawing of one before it. */
  private given = 0;
  private closed = false;

  /**
   * The pages drawn into `scroller`, which tells `report` what it shows, and `failed` of a
   * page that pdf.js could not draw.
   */
  constructor(scroller: HTMLElement, report: (reading: Reading) => void, failed: (error: unknown) => void) {
    this.scroller = scroller;
    this.report = report;
    this.failed = failed;
    // Pages are drawn when they come within one height of the view of the visible part, and
    // released when they leave it; a page's element keeps its size, so nothing moves.
    this.observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        const page = this.sheet?.pages.find((candidate) => candidate.element === entry.target);
        if (page === undefined) continue;
        if (entry.isIntersecting) this.draw(page).catch(this.failed);
        else release(page);
      }
    }, { root: scroller, rootMargin: '100% 0px' });
    scroller.addEventListener('scroll', this.scrolled, { passive: true });
  }

  /**
   * Shows the PDF file `data`: its pages are drawn beside those shown, which stay until the
   * pages at the reading position are drawn. Rejects when pdf.js cannot read the file; the
   * pages shown then stay.
   */
  async show(library: Library, data: Uint8Array): Promise<void> {
    const given = ++this.given;
    this.library = library;
    const file = await library.getDocument({ data, standardFontDataUrl: PDFJS_FONTS }).promise;
    try {
      await this.replace(library, given, file);
    } catch (error) {
      void file.loadingTask.destroy();
      throw error;
    }
  }

  /** Draws the pages of `file`, the file given `given`th, and shows them in place of those shown. */
  private async replace(library: Library, given: number, file: PDFDocumentProxy): Promise<void> {
    if (given !== this.given || this.closed) {
      void file.loadingTask.destroy();
      return;
    }
    const proxies = await Promise.all(Array.from({ length: file.numPages }, (_, index) => file.getPage(index + 1)));
    const element = document.createElement('div');
    element.className = 'flm-pdf-pages';
    const pages = proxies.map((proxy): Page => {
      const { width, height } = proxy.getViewport({ scale: 1 });
      const page: Page = { proxy, element: document.createElement('div'), width, height, drawn: null, drawing: null };
      page.element.className = 'flm-pdf-page';
      page.element.dataset.page = String(proxy.pageNumber);
      element.append(page.element);
      return page;
    });
    const zoom = this.zoom ?? this.fitted(pages[0]);
    for (const page of pages) this.size(page, zoom);
    // The pages at the reading position are drawn before they replace those shown.
    const before = this.position();
    const at = before === null ? 0 : Math.min(before.index, pages.length - 1);
    await Promise.all(pages.slice(Math.max(0, at - 1), at + 2).map((page) => this.drawAt(library, page, zoom)));
    if (given !== this.given || this.closed) {
      void file.loadingTask.destroy();
      return;
    }
    // A zoom chosen while the pages were drawn applies to them.
    const chosen = this.zoom ?? zoom;
    if (chosen !== zoom) for (const page of pages) this.size(page, chosen);
    // The reading position as it is now: the view can have scrolled while the pages were drawn.
    const position = this.position();
    const shown = this.sheet;
    if (shown !== null) {
      for (const page of shown.pages) {
        this.observer.unobserve(page.element);
        giveUp(page);
      }
      shown.element.replaceWith(element);
      void shown.file.loadingTask.destroy();
    } else {
      this.scroller.append(element);
    }
    this.zoom = chosen;
    this.sheet = { file, element, pages };
    if (position !== null) this.restore(position);
    for (const page of pages) this.observer.observe(page.element);
    this.scrolled();
  }

  /** Changes the zoom; the page read stays at the top of the visible part. */
  setZoom(zoom: number): void {
    const sheet = this.sheet;
    if (sheet === null || zoom === this.zoom) return;
    const position = this.position();
    this.zoom = zoom;
    for (const page of sheet.pages) {
      this.size(page, zoom);
      this.observer.unobserve(page.element);
    }
    if (position !== null) this.restore(position);
    // Observed again, the pages near the visible part are drawn at the zoom.
    for (const page of sheet.pages) this.observer.observe(page.element);
    this.scrolled();
  }

  /** Ends the drawing and lets pdf.js free the file. */
  close(): void {
    this.closed = true;
    this.observer.disconnect();
    this.scroller.removeEventListener('scroll', this.scrolled);
    if (this.sheet !== null) void this.sheet.file.loadingTask.destroy();
    this.sheet = null;
  }

  /** The zoom at which `page` fits the width of the view, or 1 when it fits at 1. */
  private fitted(page: Page | undefined): number {
    if (page === undefined) return 1;
    const room = this.scroller.clientWidth - 2 * MARGIN;
    const width = page.width * CSS_UNITS;
    if (room <= 0 || width <= room) return 1;
    return Math.max(LEAST_ZOOM, Math.floor((room / width) * 100) / 100);
  }

  /** Sizes the element of `page` for `zoom`; a drawing at another zoom is stretched until redrawn. */
  private size(page: Page, zoom: number): void {
    const scale = zoom * CSS_UNITS;
    const { style } = page.element;
    style.width = `${Math.floor(page.width * scale)}px`;
    style.height = `${Math.floor(page.height * scale)}px`;
    // The text layer of pdf.js sizes its text from these.
    style.setProperty('--total-scale-factor', String(scale));
    style.setProperty('--scale-round-x', '1px');
    style.setProperty('--scale-round-y', '1px');
  }

  /** Draws `page` at the zoom of the view, unless it shows that zoom or is being drawn at it. */
  private draw(page: Page): Promise<void> {
    if (this.zoom === null || this.library === null) return Promise.resolve();
    return this.drawAt(this.library, page, this.zoom);
  }

  private async drawAt(library: Library, page: Page, zoom: number): Promise<void> {
    if (page.drawn === zoom || page.drawing?.zoom === zoom) return;
    giveUp(page);
    const drawing: NonNullable<Page['drawing']> = { zoom, task: null, text: null };
    page.drawing = drawing;
    const viewport: PageViewport = page.proxy.getViewport({ scale: zoom * CSS_UNITS });
    const ratio = window.devicePixelRatio || 1;
    const canvas = document.createElement('canvas');
    canvas.width = Math.floor(viewport.width * ratio);
    canvas.height = Math.floor(viewport.height * ratio);
    const text = document.createElement('div');
    text.className = 'textLayer';
    try {
      drawing.task = page.proxy.render({ canvas, viewport, transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0] });
      await drawing.task.promise;
      if (page.drawing !== drawing) return;
      drawing.text = new library.TextLayer({ textContentSource: page.proxy.streamTextContent(), container: text, viewport });
      await drawing.text.render();
    } catch (error) {
      // A drawing given up for a later one, or for a closed file, is not a failure.
      if (page.drawing !== drawing || this.closed) return;
      throw error;
    }
    if (page.drawing !== drawing) return;
    page.drawing = null;
    page.drawn = zoom;
    page.element.replaceChildren(canvas, text);
  }

  /** The place at the top of the visible part; `null` when no page is shown. */
  private position(): Position | null {
    const pages = this.sheet?.pages ?? [];
    if (pages.length === 0) return null;
    const top = this.scroller.scrollTop;
    let index = pages.findIndex((page) => page.element.offsetTop + page.element.offsetHeight > top);
    if (index < 0) index = pages.length - 1;
    const { offsetTop, offsetHeight } = pages[index].element;
    const fraction = offsetHeight > 0 ? Math.max(0, (top - offsetTop) / offsetHeight) : 0;
    return { index, fraction, left: this.scroller.scrollLeft };
  }

  private restore(position: Position): void {
    const pages = this.sheet?.pages ?? [];
    const page = pages[Math.min(position.index, pages.length - 1)];
    if (page === undefined) return;
    this.scroller.scrollTop = page.element.offsetTop + position.fraction * page.element.offsetHeight;
    this.scroller.scrollLeft = position.left;
  }

  /** Reports the page read: the last page whose top lies above a third of the visible part. */
  private readonly scrolled = (): void => {
    const sheet = this.sheet;
    if (sheet === null || this.zoom === null) return;
    const line = this.scroller.scrollTop + this.scroller.clientHeight / 3;
    let page = 1;
    for (const candidate of sheet.pages) {
      if (candidate.element.offsetTop <= line) page = candidate.proxy.pageNumber;
      else break;
    }
    this.report({ page, pages: sheet.pages.length, zoom: this.zoom });
  };
}

/** Cancels the drawing under way of `page`, if any; the drawing then ends as given up. */
function giveUp(page: Page): void {
  page.drawing?.task?.cancel();
  page.drawing?.text?.cancel();
  page.drawing = null;
}

/** Gives up the drawing of `page` and removes the one it shows; its element keeps its size. */
function release(page: Page): void {
  giveUp(page);
  if (page.drawn === null) return;
  page.drawn = null;
  page.element.replaceChildren();
}
