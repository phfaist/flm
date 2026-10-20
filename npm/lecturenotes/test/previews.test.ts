// The previews: the citation ranges, the content a preview copies, the placement.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  EDGE_PX, MAX_ENTRIES, POPUP_GAP_PX,
  type MarkPart,
  buildPreview, citationEntries, citedRanges, entriesBetween, joinsRange, linkTarget, markParts,
  type MathJaxLike, type Placeable,
  placePopup, positionPopup, previewElements, typesetPopup,
} from '../src/previews';
import { FakeDocument, type FakeElement, h, outline } from './dom';

// ── The citation ranges ────────────────────────────────────────────────────

/** The parts of a mark written as in the page: `[`, links by number, the text between, `]`. */
function mark(...items: (number | string)[]): MarkPart[] {
  return items.map((item) => (typeof item === 'number' ? { link: `citation-${item}` } : { text: item }));
}
const one = (n: number) => ({ first: `citation-${n}`, last: `citation-${n}` });
const range = (a: number, b: number) => ({ first: `citation-${a}`, last: `citation-${b}` });

test('a dash between two citation links joins a range; any other text separates', () => {
  assert.deepEqual(citedRanges(mark('[', 1, ']')), [one(1)], '[1]');
  assert.deepEqual(citedRanges(mark('[', 1, ',', 3, ']')), [one(1), one(3)], '[1,3]');
  assert.deepEqual(citedRanges(mark('[', 3, '–', 5, ']')), [range(3, 5)], '[3–5]');
  assert.deepEqual(citedRanges(mark('[', 1, '–', 3, ', ', 56, ']')), [range(1, 3), one(56)], '[1–3, 56]');
  assert.deepEqual(citedRanges(mark('[', 2, '; Thm 4]')), [one(2)], '[2; Thm 4]');
  assert.deepEqual(citedRanges(mark('[', 2, '; Thm 4–', 5, ']')), [one(2), one(5)],
    'the text after a semicolon separates, even when it ends with a dash');
  assert.deepEqual(citedRanges(mark(1, ' - ', 4)), [range(1, 4)], 'a hyphen with spaces joins');
  assert.deepEqual(citedRanges(mark('[', 1, '–', 3, '–', 6, ']')), [range(1, 6)]);
  assert.deepEqual(citedRanges(mark('[', ']')), []);
  assert.equal(joinsRange('–'), true);
  assert.equal(joinsRange(' - '), true);
  assert.equal(joinsRange(','), false);
  assert.equal(joinsRange(''), false, 'two links with nothing between are two works');
});

// ── A page ─────────────────────────────────────────────────────────────────

const display = { 'data-flm-display': 'display' };
const cite = (n: number) => h('a.flm-pres-link', { href: `#citation-${n}`, 'data-flm-kind': 'cite' }, String(n));

/** A page shaped as the rendered sample: headings, paragraphs, displayed formulas, a theorem, a footnote, a bibliography. */
function page(): { doc: FakeDocument; byId: (id: string) => FakeElement } {
  const content = h('article.flm-content#flm-content', {},
    h('h2.flm-pres-block--heading#sec-a', { 'data-flm-level': '1' }, 'A', h('a.ln-anchor', { href: '#sec-a' }, '§')),
    h('p.flm-pres-par#p1'), h('p.flm-pres-par#p2'), h('p.flm-pres-par#p3'), h('p.flm-pres-par#p4'),
    h('h3.flm-pres-block--heading#sec-b', { 'data-flm-level': '2' }, 'B'),
    h('p.flm-pres-par#p5'),
    h('h3.flm-pres-block--heading#sec-c', { 'data-flm-level': '2' }, 'C'),
    h('span.flm-pres-math--source#f1', display, '\\begin{align}', h('tex-html', {}, h('span.flm-pres-math--text#eq-x')), 'x\\end{align}'),
    h('p.flm-pres-par#p6'),
    h('p.flm-pres-par#p7', {}, 'The channel'),
    h('span.flm-pres-math--source#f2', display, h('tex-html', {}, h('span.flm-pres-math--text#eq-tp'))),
    h('p.flm-pres-par#p8', {}, 'Condition ', h('a.flm-pres-link', { href: '#eq-tp', 'data-flm-kind': 'ref' }, 'Eq. (2)')),
    h('div.flm-pres-block.flm-theorems-theorem#theorem-1', { 'data-flm-kind': 'theoremlike theorem' },
      h('p.flm-pres-par#p10', {}, 'if and only if'),
      h('span.flm-pres-math--source#f3', display,
        h('mjx-container', { display: 'true' }, h('mjx-html', {}, h('span.flm-pres-math--text#eq-kl')))),
      h('p.flm-pres-par#p11', {}, 'for some matrix')),
    h('p.flm-pres-par#p12'),
    h('mjx-container#f4', { display: 'true' }, h('span#eq-y')),
    h('p.flm-pres-par#p13'),
    h('p.flm-pres-par#p9', {}, 'Text.',
      h('sup.flm-pres-span.flm-xnotes-xnote-mark#fnref-1', {},
        h('a.flm-pres-link', { href: '#fn-1', 'data-flm-kind': 'xnote' }, 'a')),
      ' The surveys ',
      h('span.flm-pres-span.flm-citations-mark#m1', { 'data-flm-kind': 'attached' }, '[', cite(3), '–', cite(5), ']'),
      h('span.flm-pres-span.flm-citations-mark#m2', { 'data-flm-kind': 'attached' }, '[', cite(1), '–', cite(10), ', ', cite(12), ']'),
      h('span.flm-pres-span.flm-citations-mark#m3', { 'data-flm-kind': 'attached' }, '[', cite(7), '–', cite(2), ']'),
      h('a.flm-pres-link#url', { href: 'https://example.org/', 'data-flm-kind': 'url' }, 'a site'),
      h('a.flm-pres-link#missing', { href: '#nowhere', 'data-flm-kind': 'ref' }, 'nothing')),
  );
  const notes = h('ol.flm-pres-list#notes', { 'data-flm-kind': 'numbered xnotes labeled' },
    h('li#fn-1', {}, h('span.flm-pres-list-label', {}, 'a'),
      h('p.flm-pres-par', {}, 'A note. ', h('a.flm-pres-link', { href: '#fnref-1', 'data-flm-kind': 'xnote-back' }, '↩'))));
  const entries = Array.from({ length: 12 }, (_, i) =>
    h(`li#citation-${i + 1}`, {}, h('span.flm-pres-list-label', {}, `[${i + 1}]`), h('p.flm-pres-par', {}, `Work ${i + 1}`)));
  const bibliography = h('ol.flm-pres-list#bib', { 'data-flm-kind': 'numbered citations labeled' }, ...entries);
  const doc = new FakeDocument([content, h('section.ln-backmatter', {}, notes, bibliography)]);
  const byId = (id: string): FakeElement => {
    const element = doc.getElementById(id);
    assert.ok(element, `#${id} is on the page`);
    return element;
  };
  return { doc, byId };
}

const ids = (elements: Element[]): string[] => elements.map((e) => e.id);
const asElement = (e: FakeElement): Element => e as unknown as Element;
const asDocument = (d: FakeDocument): Document => d as unknown as Document;

// ── The content ────────────────────────────────────────────────────────────

test('a heading previews with up to three following siblings, until the next heading', () => {
  const { byId } = page();
  assert.deepEqual(ids(previewElements(asElement(byId('sec-a')))), ['sec-a', 'p1', 'p2', 'p3']);
  assert.deepEqual(ids(previewElements(asElement(byId('sec-b')))), ['sec-b', 'p5']);
});

test('a list entry, a block, an inline element preview as the rules say', () => {
  const { byId } = page();
  assert.deepEqual(ids(previewElements(asElement(byId('fn-1')))), ['fn-1'], 'a footnote: the entry');
  assert.deepEqual(ids(previewElements(asElement(byId('theorem-1')))), ['theorem-1'], 'a block: itself');
  assert.deepEqual(ids(previewElements(asElement(byId('fnref-1')))), ['p9'], 'an inline target: its paragraph');
  assert.deepEqual(ids(previewElements(asElement(byId('eq-kl')))), ['theorem-1'],
    'a formula inside a theorem: the theorem, the nearest enclosing block');
});

test('a displayed formula between paragraphs previews with the paragraph before and the paragraph after it', () => {
  const { byId } = page();
  assert.deepEqual(ids(previewElements(asElement(byId('eq-tp')))), ['p7', 'f2', 'p8']);
  assert.deepEqual(ids(previewElements(asElement(byId('eq-x')))), ['f1', 'p6'], 'a heading before: no paragraph before');
  assert.deepEqual(ids(previewElements(asElement(byId('eq-y')))), ['p12', 'f4', 'p13'], 'a typeset formula');
});

test('the preview copies the content without ids, anchors or back-links', () => {
  const { doc, byId } = page();
  const footnote = buildPreview(asElement(byId('fnref-1').children[0]), asDocument(doc)) as unknown as FakeElement;
  assert.equal(outline(footnote),
    'div.ln-preview-content(ol.flm-pres-list(li(span.flm-pres-list-label p.flm-pres-par)))');
  assert.equal(footnote.textContent.trim(), 'aA note.');
  assert.equal(byId('fn-1').querySelectorAll('a').length, 1, 'the page keeps its back-link');

  const link = h('a.flm-pres-link', { href: '#sec-a', 'data-flm-kind': 'ref' });
  const heading = buildPreview(asElement(link), asDocument(doc)) as unknown as FakeElement;
  assert.equal(outline(heading),
    'div.ln-preview-content(h2.flm-pres-block--heading p.flm-pres-par p.flm-pres-par p.flm-pres-par)');
  assert.equal(byId('sec-a').querySelectorAll('.ln-anchor').length, 1, 'the page keeps its anchor');

  const formula = buildPreview(asElement(byId('p8').children[0]), asDocument(doc)) as unknown as FakeElement;
  assert.equal(outline(formula),
    'div.ln-preview-content(p.flm-pres-par span.flm-pres-math--source(tex-html(span.flm-pres-math--text)) p.flm-pres-par(a.flm-pres-link))');
});

test('a citation previews the entries its mark covers, ranges expanded, at most eight', () => {
  const { doc, byId } = page();
  const d = asDocument(doc);
  const [three, five] = byId('m1').querySelectorAll('a');
  assert.deepEqual(markParts(asElement(byId('m1'))), mark('[', 3, '–', 5, ']'));
  assert.deepEqual(ids(citationEntries(asElement(three), d)), ['citation-3', 'citation-4', 'citation-5']);
  assert.deepEqual(ids(citationEntries(asElement(five), d)), ['citation-3', 'citation-4', 'citation-5'],
    'every link of one mark gives the same entries');
  const preview = buildPreview(asElement(five), d) as unknown as FakeElement;
  assert.equal(outline(preview), 'div.ln-preview-content(ol.flm-pres-list(li(span.flm-pres-list-label p.flm-pres-par) '
    + 'li(span.flm-pres-list-label p.flm-pres-par) li(span.flm-pres-list-label p.flm-pres-par)))');

  const many = byId('m2').querySelectorAll('a')[0];
  assert.equal(citationEntries(asElement(many), d).length, 11, '[1–10, 12]');
  const long = buildPreview(asElement(many), d) as unknown as FakeElement;
  assert.equal(long.children[0].children.length, MAX_ENTRIES);
  assert.equal(outline(long.children[1]), 'p.ln-preview-more');
  assert.equal(long.children[1].textContent, 'and 3 more');

  const backwards = byId('m3').querySelectorAll('a')[0];
  assert.deepEqual(ids(citationEntries(asElement(backwards), d)), ['citation-7', 'citation-2'],
    'a range whose end comes first: its two ends');
  assert.deepEqual(ids(entriesBetween(asElement(byId('citation-2')), asElement(byId('citation-2')))), ['citation-2']);
});

test('only links to this page preview', () => {
  const { doc, byId } = page();
  assert.equal(linkTarget(asElement(byId('url'))), null);
  assert.equal(buildPreview(asElement(byId('url')), asDocument(doc)), null);
  assert.equal(buildPreview(asElement(byId('missing')), asDocument(doc)), null, 'a link to nothing');
  assert.equal(linkTarget(asElement(h('a', { 'data-mjx-href': '#eq-x' }))), 'eq-x', 'a link inside a typeset formula');
  assert.equal(linkTarget(asElement(h('a', { href: '#sec%20a' }))), 'sec a');
});

// ── The placement ──────────────────────────────────────────────────────────

test('the popup goes below the link, above it when there is no room below, and stays inside the window both ways', () => {
  const link = { left: 100, right: 140, top: 200, bottom: 220 };
  assert.deepEqual(placePopup(link, 200, 100, 1000, 800), { left: 20, top: 220 + POPUP_GAP_PX });
  const low = { left: 100, right: 140, top: 700, bottom: 720 };
  assert.deepEqual(placePopup(low, 200, 100, 1000, 800), { left: 20, top: 700 - POPUP_GAP_PX - 100 });
  const tall = placePopup({ left: 100, right: 140, top: 50, bottom: 70 }, 200, 760, 1000, 800);
  assert.equal(tall.top, 800 - EDGE_PX - 760, 'no room on either side: clamped to the bottom margin');
  const middle = placePopup({ left: 100, right: 140, top: 300, bottom: 320 }, 200, 500, 1000, 800);
  assert.equal(middle.top, 800 - EDGE_PX - 500, 'no room on either side: moved up to stay inside the window');
  const huge = placePopup({ left: 100, right: 140, top: 300, bottom: 320 }, 200, 900, 1000, 800);
  assert.equal(huge.top, EDGE_PX, 'taller than the window: the top edge margin');
  assert.equal(placePopup({ left: 0, right: 10, top: 0, bottom: 10 }, 200, 50, 1000, 800).left, EDGE_PX);
  assert.equal(placePopup({ left: 990, right: 1000, top: 0, bottom: 10 }, 200, 50, 1000, 800).left, 1000 - EDGE_PX - 200);
});

test('the popup is measured at the top left corner, not at the place of the preview shown before', () => {
  // A popup whose width shrinks to the room between its left and the window's right edge.
  const style = { left: '929px', top: '0px' };
  const popup: Placeable = {
    style,
    get offsetWidth() { return Math.min(512, 1000 - parseFloat(style.left)); },
    get offsetHeight() { return 1000 - parseFloat(style.left) < 512 ? 320 : 120; },
  };
  positionPopup(popup, { left: 960, right: 990, top: 100, bottom: 120 }, 1000, 800, 0, 50);
  assert.equal(style.left, `${1000 - EDGE_PX - 512}px`, 'measured at the full width');
  assert.equal(style.top, `${120 + POPUP_GAP_PX + 50}px`, 'the page\'s scroll added');
});

test('the popup is typeset after MathJax\'s pending work, and a failure ends the chain quietly', async () => {
  const order: string[] = [];
  let release!: () => void;
  const pending = new Promise<void>((resolve) => { release = resolve; });
  const popup = {} as Element;
  const mathJax: MathJaxLike = {
    startup: { promise: pending.then(() => { order.push('page'); }) },
    typesetPromise: async (elements) => {
      assert.deepEqual(elements, [popup]);
      order.push('popup');
      throw new Error('typesetting failed');
    },
  };
  const before = mathJax.startup!.promise;
  const done = typesetPopup(mathJax, popup);
  assert.notEqual(mathJax.startup!.promise, before, 'the chain is stored back');
  await Promise.resolve();
  assert.deepEqual(order, [], 'nothing runs before the pending work ends');
  release();
  await done;
  await mathJax.startup!.promise;
  assert.deepEqual(order, ['page', 'popup']);

  // Without a startup promise, the call runs at once.
  const calls: Element[][] = [];
  await typesetPopup({ typesetPromise: async (elements) => { calls.push(elements); } }, popup);
  assert.deepEqual(calls, [[popup]]);
  await typesetPopup({}, popup);
});
