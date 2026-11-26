# The lecture-notes page (`lecturenotes`)

`lecturenotes` is an FLM extension that renders a book-length document, such as a set of
lecture notes, as one HTML page with a sidebar. It provides one compose template, also
named `lecturenotes`, for the output `html`. A *compose template* is the text that
arranges the parts FLM produces for a document (the content, the title, the table of
contents, the notes) into the finished page.

The extension is self-contained: `flm render` and `flm watch` give the finished page with
nothing else installed. The page's stylesheet and script are two committed files of the
compose template's folder, `templates/html/lecturenotes/dist/`, compiled ahead of time
from TypeScript and CSS sources kept outside the extension, in the repository's npm
workspace (`npm/lecturenotes/`); Node is needed only to change those sources.

Contents of this file:

- [Using the template](#using-the-template)
- [What the page looks like and does](#what-the-page-looks-like-and-does)
- [The settings](#the-settings)
- [A document with parts](#a-document-with-parts)
- [Fonts and further stylesheets](#fonts-and-further-stylesheets)
- [The custom properties a site overrides](#the-custom-properties-a-site-overrides)
- [Placing the document inside a site](#placing-the-document-inside-a-site)
- [The stylesheet and script on their own](#the-stylesheet-and-script-on-their-own)
- [Changing the sources and rebuilding `dist/`](#changing-the-sources-and-rebuilding-dist)
- [Differences from the template of the Python FLM](#differences-from-the-template-of-the-python-flm)

## Using the template

A document lists the extension under `use` in its front matter:

```yaml
---
title: "Quantum Error Correction: Lecture Notes"
author: [A. Lecturer]
date: Autumn 2026
flm:
  use: [lecturenotes]
---
```

The extension is compiled into the program, so nothing needs to be installed:

```text
flm render -o out/notes.html notes.flm
flm watch  notes.flm
```

To try a modified copy, put the folder `lecturenotes/` on the search path: give
`--extensions-dir extensions/data` from the repository's root, or copy or link it into the
folder `_flmextensions` beside the document, into a folder listed in the environment
variable `FLMPATH`, or into `~/.flm/extensions`; a copy found there wins over the one
compiled into the program (see `extensions/README.md`).

Using the extension sets three settings, which the document may override like any setting
an extension writes:

- the compose template of the output `html` is `lecturenotes` (`compose: {template: {html:
  lecturenotes}}`). The template exists for the output `html` only; the other outputs keep
  their default compose template, so the same document also renders, for instance, with
  `-p text`;
- the table of contents is placed (`pieces: {include: [flm.toc]}`); how deep it goes is the
  document's setting `toc.depth`;
- the library's page stylesheet `flm.html.style.page` is left out (`html: {definitions:
  {flm.html.style.page: provided-by-host}}`): the template's own stylesheet sets the page's
  frame instead.

**The files a render writes.** Beside the page, the template ships three files, each named
after the page's stem (the file name without its extension) and the file's name:

| file, under `-o out/notes.html` | under `-o out/` | what it holds |
|---|---|---|
| `out/notes.html` | `out/index.html` | the page |
| `out/notes-style.css` | `out/index-style.css` | the page's stylesheet: the setting `font_family`, then the compiled stylesheet `dist/lecturenotes.css`, then the template's rules by group (see *The headings and the entries, by group* below) |
| `out/notes-script.js` | `out/index-script.js` | the page's script: the compiled script `dist/lecturenotes.js`, then one call of its setup function with the setting `navigation_heading_level` |
| `out/notes-fragment.html` | `out/index-fragment.html` | the page's parts without the frame, for placing the document inside a site's own page (see [Placing the document inside a site](#placing-the-document-inside-a-site)) |

The setting `css` can ship further files beside the page, such as a font or an image of
the document's folder (see [Fonts and further stylesheets](#fonts-and-further-stylesheets)):
each is named after the page's stem, the file's own stem and a digest of its content. The
program also writes its record of the files it wrote, `out/.notes.html.flm-written`, from
which the next render removes the files it no longer writes.

## What the page looks like and does

The page has three areas:

- **The bar**, at the top, stays in view while the page scrolls. It holds the document's
  title (a link to the top of the page), the title of the navigation unit being read
  (see *The navigation level* below), an arrow to the previous unit and one to the next,
  a button that hides or shows the sidebar, and the links of the setting `links`.
- **The sidebar**, on the right, holds the table of contents under the label "Contents".
  It scrolls on its own, under the bar. By default it is *folded*: it shows the entries
  down to the navigation level, and the entries below it only for the unit being read. A
  button beside the label, two chevrons pointing apart, opens every entry; its chevrons
  then point together, and it folds the entries again. The entry of the heading being read is
  marked, and the sidebar scrolls to keep that entry in view, except for four seconds
  after the reader scrolled the sidebar by hand.
- **The main column**, 52rem wide, holds the title block (the document's title, authors
  and date), the content, and after the content the lists of notes and the bibliography.

On a narrow window (56rem wide or less) the sidebar is hidden and the bar's button opens
it as a drawer from the right; a click on an entry, or anywhere outside the drawer, closes
it. The bar then shows no document title. When the page is printed, the bar, the sidebar
and the heading anchors are left out and the content takes the page's width.

**The navigation level.** The author chooses which heading level is the document's unit
of reading and navigation, the *navigation level*, with the setting
`navigation_heading_level` (1 by default, the level of `\section` among the standard
heading kinds). It is the level at which the document would be cut if it were split into
separate pages: the chapters of a book, whatever heading kind they are written with. A *navigation unit* is a heading at the
navigation level and what follows it, up to the next heading at that level or above it.
The navigation level decides the title in the bar (that of the unit being read), where the
arrows and the keys `[` and `]` lead, and which entries of the sidebar fold. A heading
above the navigation level, such as a part above chapters, ends the unit before it and
belongs to no unit.

**The headings and the entries, by group.** The template puts each heading of the
content, and each entry of the table of contents, into groups by the heading's level,
compared with the navigation level and with the *top heading level*, the setting
`top_heading_level` (1 by default; 0 in a document with parts, beside the render option
`html.root_heading_level`, see [A document with parts](#a-document-with-parts)), so that the
looks do not depend on how the document's levels are numbered. With T the top heading
level and N the navigation heading level:

| group | the headings | in a document with parts (level 0, the top heading level), chapters at the navigation level (1) and sections (2) | default look of a heading | default look of an entry |
|---|---|---|---|---|
| `top` | at the level T | the parts | none of its own | none of its own |
| `major` | at the level N or above it (a level of at most N) | the parts and the chapters | a band | bold, with a small space above |
| `navigation` | at the level N | the chapters | none of its own | none of its own; the folding hides the entries below a chapter's entry |
| `minor` | below the level N (a level greater than N) | the sections, and their subsections | bold, with the spaces of a section | the lighter color |

A heading of the content carries its level in the library's attribute `data-flm-level`,
and so does each entry (`li`) of the table of contents. The template computes from its two settings one
CSS condition per group, in its file `css_selectors.jinja`: the value `css_selectors`,
whose entry `headings` holds the entries `top`, `major`, `navigation` and `minor`. Each value begins with `[` or `:`; written directly after a
selector, with no space, it narrows the selector to the elements of the group, and it is
also a valid selector by itself. For T = 0 and N = 1 the values are:

```text
top         [data-flm-level="0"]
major       :is([data-flm-level="0"], [data-flm-level="1"])
navigation  [data-flm-level="1"]
minor       [data-flm-level]:not([data-flm-level="0"], [data-flm-level="1"])
```

The template's own rules by group, in its file `groups.css`, are written with these
values, as in `.flm-content .flm-pres-block--heading{{ css_selectors.headings.major }}`
and `.flm-toc li{{ css_selectors.headings.major }}`; the page's stylesheet holds them after
the compiled stylesheet `dist/lecturenotes.css`. Beyond the group, the size of a heading
is the library's, by its HTML level (`<h1>`, `<h2>`, ...): a part is larger than a
chapter. A document changes these looks with the setting `css`, which reads the same
values (see [The settings](#the-settings)): for instance, `.flm-content
.flm-pres-block--heading{{ css_selectors.headings.top }} { text-transform: uppercase; }`
sets the headings of the group `top` in capitals, and `.flm-toc li[data-flm-level="2"] >
.flm-pres-link` selects the entries of the headings at the level 2.

The groups go by level rather than by place in the table. The table nests each entry
under the nearest entry before it whose level is lower, and indents each list nested in
another by the library's property `--flm-toc-indent` (1em in this template): a unit that
comes before the first part, such as a preface, therefore stands at the top of the table,
as far left as the parts, but is not in the group `top`. A document that wants the
indentation to follow the groups instead turns the nesting's indentation off and indents
by group, in the setting `css`:

```css
:root { --flm-toc-indent: 0; }
.flm-toc li{{ css_selectors.headings.minor }} > .flm-pres-link { margin-left: 1.2em; }
```

**Navigating.** The arrows, and the keys `[` (previous) and `]` (next), scroll to the
previous or the next navigation unit and put that unit's address (`#id`) into the
browser's address bar. Every heading that has an identifier gets a discreet `§` link after its
title, whose address is the heading's. Jumps are instant.

**Previews.** Hovering a link to another place of the page (a reference, a defined term, a
citation, a footnote mark) shows that place in a popup, after a short delay; moving the
pointer onto the popup keeps it open. What the popup shows:

- for a heading: the heading and up to three blocks that follow it, up to the next heading;
- for an entry of a list (a footnote, an entry of the bibliography): the entry;
- for a place inside a paragraph or another block: that block, which gives a formula or
  a term its context; a displayed formula stands inside the paragraph it is written in,
  so a reference to it, or to one of its lines, shows that paragraph. A long block is cut
  around the place: at most about 300 characters before it and 200 after, cut at a word,
  with an ellipsis where text is left out; a formula is never cut. When no words stand
  before the place in its block, the popup also shows the end of the paragraph before the
  block (about 160 characters at most), and when no words stand after it, the beginning
  of the paragraph after (about 100), provided that paragraph has words outside its
  formulas;
- for a citation: the bibliography entries of every work the citation mark covers, a range
  such as `[3–5]` expanded to the works 3, 4 and 5; every number of one mark shows the
  same list, and more than eight works show the first eight and a line saying how many
  more.

The popup is between 24rem and 32rem wide (90% of the window at most), whatever it
shows, so that a displayed formula's number stands apart from the formula and a narrow
figure's caption has room.

On a touch screen, a long press (about half a second) on the link shows the popup and a
plain tap follows the link. A link that receives the keyboard's focus shows its popup
too. Escape, or a click or tap elsewhere, closes the popup. Links to other pages, the
entries of the table of contents and the links back from a footnote are not previewed.

**The reading position.** The script records the reader's position (the nearest heading
above the top of the window, and the distance from it) each time scrolling stops. Back
and forward in the browser's history return to the recorded position, corrected once the
page has settled, so that content that changed height (typeset formulas, loaded fonts)
does not shift it. The position is also kept in the browser's storage, per page address,
and restored when the page is opened again without a `#id` in its address.

**Mathematics.** Delivering the mathematics (MathJax, or MathML) is the library's and the
math extension's job; the template delivers none. The template spaces displayed formulas,
lets a wide formula scroll sideways on a narrow window, and names a font list for formulas
written as MathML (`--ln-math-family`). The popups know a formula by FLM's mark of a
formula, the class `flm-math-formula`, which the outermost element of every formula
carries whichever math processor wrote it: a reference to a formula, or to one of its
lines, shows the paragraph the formula stands in, and a popup that cuts a long paragraph
keeps each formula whole. The popup typesets
nothing itself: once its copy is in the page, it dispatches FLM's event for copies of page
content (below), on which the math processor's script typesets the formulas of the copy
that are not typeset yet (those still in TeX, and those that MathJax's lazy mode has not
typeset yet), and the popup places itself again when its size changes.

**The live preview.** Under `flm watch`, the preview updates the page in place as the
document changes; the script then reads the headings again, closes an open popup and
records the position again.

## The settings

The template has five settings, which its description declares under `settings`, each
with its type, its description and its default. A document writes them in
`template_config`, under the output (`html`) and then the compose template's name, each
key replacing the default it names:

```yaml
---
flm:
  use: [lecturenotes]
  features:
    compose:
      template_config:
        html:
          lecturenotes:
            navigation_heading_level: 1
            top_heading_level: 1
            font_family: '"Source Serif 4", Georgia, serif'
            css: |
              @import url("https://fonts.googleapis.com/css2?family=Source+Serif+4:ital,opsz,wght@0,8..60,400..700;1,8..60,400..700&display=swap");
              :root { --ln-accent: #a71d5d; }
              .flm-toc li{{ css_selectors.headings.navigation }} > .flm-pres-link { text-transform: uppercase; }
            links:
              - {text: Course page, href: "https://example.org/course/"}
              - {text: Exercises, href: exercises.html}
---
```

On the command line, one setting is written `-O 'compose.template_config={html: {lecturenotes:
{navigation_heading_level: 2}}}'`. Each source of settings replaces only the settings it
writes: the others keep the values that the front matter or a package gives them. A key
that the template does not declare is left out, with a warning.

| setting | shape | default | what it does |
|---|---|---|---|
| `navigation_heading_level` | a whole number from 0 | `1` | the navigation level: the level of the headings that begin the navigation units, the document's units of reading (see *The navigation level* in [What the page looks like and does](#what-the-page-looks-like-and-does)). It decides the bar's title, the arrows, the keys `[` and `]`, the folding of the sidebar, and the groups `major`, `navigation` and `minor` of the headings and of the sidebar's entries |
| `top_heading_level` | a whole number from 0 | `1` | the top heading level: the level of the document's outermost headings, the group `top` of the headings and of the sidebar's entries (see *The headings and the entries, by group* in [What the page looks like and does](#what-the-page-looks-like-and-does)); 0 in a document with parts |
| `font_family` | a CSS font-family list | `Georgia, "Iowan Old Style", Charter, "Bitstream Charter", "Palatino Linotype", Palatino, "Book Antiqua", "Times New Roman", serif` | the family of the page's text, written as given into the page's stylesheet (`:root { --ln-family: …; }`) |
| `css` | a text of CSS, applied as a template | `''` | the document's own stylesheet, written in a `<style>` element of the page's head after the link to the template's stylesheet, so that it overrides the template's rules: values of the custom properties (see [The custom properties a site overrides](#the-custom-properties-a-site-overrides)), rules of the document's own, and the rules `@import` of web fonts and other stylesheets at its beginning (see [Fonts and further stylesheets](#fonts-and-further-stylesheets)). An empty text writes no element |
| `links` | a list of maps, each with both `text` and `href` | `[]` | links shown in the bar, after the buttons: `text` is the link's text, `href` its address |

**The setting `css` is a template.** The text is applied as a template of the template's
engine, `minijinja`, with the delimiters `{{ }}`, `{% %}` and `{# #}`, before it is
written into the page. It reads the template's values, among them the selectors of the
groups, `css_selectors.headings.top`, `.major`, `.navigation` and `.minor` (see *The
headings and the entries, by group* in [What the page looks like and
does](#what-the-page-looks-like-and-does)), and the settings, `config`. A value is written
as it is, not escaped for HTML; the text that results is encoded for the `<style>` element,
every `</` written `<\/`, which CSS reads as `</` and which cannot end the element. In the
text, `{#` begins a comment of the template language, which ends at `#}`: a rule whose
brace is followed by `#` is written with a space between them (`{ #`). The text also has
the engine's functions, by which it reads and ships files of the document's folder (see
[Fonts and further stylesheets](#fonts-and-further-stylesheets)). A text that is not a
valid template, or a file it reads that is not there, stops the render with the engine's
message.

The program does not check that a setting has its shape: a value of another shape (a text
where a list is expected, a link without `href`) gives a page that is not the one meant,
or stops the render with the template engine's own message. The script checks
`navigation_heading_level` and reports a value that is not a whole number from 0 on the
browser's console.

## A document with parts

A part is a heading at level 0, one level above the sections. The standard heading kinds
start at level 1, so a document with parts declares a heading kind at level 0 and a
command for it. The HTML backend's root heading level (the level written as `<h1>`) is 1
by default, and a heading above it is an error; the extension does not change it, so a
document with parts also sets the render option `html.root_heading_level` to 0, and the
template's setting `top_heading_level` to 0 so that the parts are the group `top`:

```yaml
---
flm:
  use: [lecturenotes]
  features:
    html: {root_heading_level: 0}
    compose:
      template_config:
        html:
          lecturenotes: {top_heading_level: 0}
    headings:
      headings:
        part: {level: 0, counter: null, style: null}
      commands:
        part: {heading: part}
---
\part{Fundamental theory}

\section{Protecting information against noise}
```

Without `root_heading_level: 0`, the render stops with the error "a heading's level must
be at least the root heading level 1 (the render option `root_heading_level`); this
heading's level is 0", placed at the part's heading. The text output has a root heading
level of its own: a text render (`-p text`) of a document with parts sets
`text: {root_heading_level: 0}` in the same way.

## Fonts and further stylesheets

The template ships no font. Its default family is a list of serif fonts commonly installed
on computers and phones, ending with the generic family `serif`. To use another font:

1. give the family in the setting `font_family`, as a CSS font-family list;
2. when the font is a web font, provide its faces in the setting `css`, in one of two ways.

A web font of a web-font service (such as Google Fonts) is imported by its address, with a
rule `@import` at the beginning of the text of `css`, which is the beginning of the
`<style>` element's stylesheet, the one place where CSS reads that rule:

```css
@import url("https://fonts.googleapis.com/css2?family=Source+Serif+4&display=swap");
```

A font file of the document's folder is shipped beside the page by the function
`include_artifact_url`, which reads the file, writes it beside the page and gives its
address there. The path begins with the root `doc:`, which names the document's folder;
after it, the path is read as a path written in the document is (relative to the
document's folder):

```css
@font-face {
  font-family: "My Serif";
  src: url("{{ include_artifact_url("doc:fonts/my-serif.woff2") }}") format("woff2");
}
```

An image of the document's folder is shipped the same way, as in `.ln-titleblock {
background: url("{{ include_artifact_url("doc:banner.png") }}") }`. The
setting `css` also brings in a stylesheet of the document's folder, written into the
`<style>` element:

- as it is, with `{{ include_raw("doc:my.css") }}`;
- applied as a template, with `{{ include("doc:my.css") }}`: the file reads
  the same values as the text of `css`, `css_selectors` among them, and its values are
  written as they are;
- downloaded from an address and written as it is, with `{{
  include_raw("https://example.org/my.css") }}`. The program keeps the downloaded file in
  its cache for seven days.

The text of `css` is written in the front matter, not read from a file, so it has no
folder of its own: every path written in it begins with a root, such as `doc:` for the
document's folder or `ext+NAME:` for a file of an extension `NAME` that the document uses.

Such a stylesheet can set the template's custom properties (next section), or provide a
math font for formulas written as MathML, together with `--ln-math-family`. Under `flm
watch`, the preview builds again when a file the setting reads changes. A site that
places the document inside its own pages adds its stylesheets itself (see [Placing the
document inside a site](#placing-the-document-inside-a-site)).

## The custom properties a site overrides

The template's stylesheet writes every color, size, space and font as a CSS custom
property under the prefix `--ln-`, declared on `:root` with no weight (`:where(:root)`),
so that a declaration on `:root` in any other stylesheet replaces it wherever that
stylesheet stands. The template has no setting for each look: a document changes them by
setting these properties in the setting `css`, and a site in a stylesheet of its own. The complete list, with a comment per group, is at the top of the stylesheet
`templates/html/lecturenotes/dist/lecturenotes.css`. The groups:

| group | properties |
|---|---|
| colors | `--ln-text`, `--ln-muted`, `--ln-accent`, `--ln-accent-hover`, `--ln-accent-bg`, `--ln-panel-bg`, `--ln-entry-bg`, `--ln-border`, `--ln-band-color`, `--ln-bg`, `--ln-selection`, `--ln-rule`, `--ln-shadow` |
| the text | `--ln-family`, `--ln-font-features`, `--ln-size`, `--ln-line-height` |
| the frame | `--ln-measure` (the column's width, 52rem), `--ln-sidebar-width`, `--ln-drawer-max-width`, `--ln-bar-height`, `--ln-gutter`, `--ln-gutter-narrow`, `--ln-space-below`, `--ln-space-below-narrow`, `--ln-scroll-gap` |
| the bar | `--ln-bar-*`, `--ln-unit-lead`, `--ln-unit-lead-gap`, `--ln-button-*`, `--ln-disabled-opacity` |
| the sidebar | `--ln-sidebar-size`, `--ln-sidebar-line-height`, `--ln-sidebar-padding`, `--ln-label-*`, `--ln-toggle-*`, `--ln-folded-icon`, `--ln-open-icon`, and its entries: `--ln-entry-*`, `--ln-current-weight`, `--ln-major-entry-*`, `--ln-current-major-entry-weight` (the entries of the group `major`) |
| the title block and the notes after the content | `--ln-title-*`, `--ln-subtitle-*`, `--ln-backmatter-*` |
| the headings of the content | `--ln-heading-number-gap`, `--ln-band-rule`, `--ln-major-heading-*` (the headings of the group `major`), `--ln-minor-heading-*` (the headings of the group `minor`), `--ln-anchor-gap`, `--ln-anchor-padding`, `--ln-anchor-size` |
| the text's elements | `--ln-term-decoration`, `--ln-defterm-weight`, `--ln-theorem-style`, `--ln-proof-*`, `--ln-float-padding`, `--ln-caption-*`, `--ln-quote-style` |
| the previews | `--ln-preview-*` (the popup's widths, spaces, look, and its layer above the bar and the drawer) |
| mathematics | `--ln-math-space`, `--ln-math-family` |
| the motions | `--ln-fast`, `--ln-drawer-motion`, `--ln-proof-motion` |

The stylesheet also sets some of the library's own properties on the same rule
(`--flm-link-color`, `--flm-heading-*`, `--flm-margin-note-*`, `--flm-toc-indent`,
`--flm-floats-*`, `--flm-math-display-space-*`); the library's guide to rendering lists
what each does. The template's stylesheet stands outside the library's cascade layer
`flm`, so its rules override the library's.

The popup of the previews is a `div` of class `ln-preview`, added at the end of the page's
`body` the first time it is shown (class `ln-preview-shown` while shown); its content is a
`div` of class `ln-preview-content`; the ellipsis where it leaves text out is a `span` of
class `ln-preview-cut`. The `--ln-preview-*` properties set its look.

## Placing the document inside a site

There are two ways to publish the page as part of a site.

**The page as it is.** The page `STEM.html` is an ordinary HTML page. It reaches its files
by relative addresses: `STEM-style.css` and `STEM-script.js` beside it, the files the
setting `css` ships, the addresses its rules `@import` give, and the files the document
itself refers to. A site copies the
page with the files beside it; a site whose pages go through a bundler gives the bundler
the page as input, and the bundler follows those addresses as for any other page. The
template does not bundle anything itself.

**The fragment.** `STEM-fragment.html` holds the page's parts without the frame: no
`<html>`, `<head>` or `<body>`, no bar and no sidebar label. In order, each after an HTML
comment naming it:

1. the library's styles region: the stylesheets the document's content needs;
2. the document's styles: the setting `css` in a `<style>` element, as in the page, and
   nothing when the setting is empty;
3. the title block, `<header class="ln-titleblock">`;
4. the table of contents, inside `<aside class="ln-sidebar">`;
5. the content, `<article class="flm-content" id="flm-content">`;
6. the lists of notes and the bibliography, `<section class="ln-backmatter">`;
7. the library's scripts region: the scripts the document's content needs.

A site places these parts inside a page of its own, with the same elements around them as
the page gives them, and links `STEM-style.css` and `STEM-script.js` (or the two files of
`dist/` with its own call of the setup function; next section), after the library's
styles and before the document's, so that the document's rules override the template's.
The addresses of the files the setting `css` ships are relative to the page's folder, where
the site keeps them. The script works
with the parts the site's page has: only the content (`id="flm-content"`) is required, and
what concerns a part the page does not have (the bar's buttons, the sidebar) is left out.

## The stylesheet and script on their own

The page's stylesheet and script are an npm package in their own right,
`flm-lecturenotes` (not published), usable on any page that follows its markup, with or
without FLM. Its sources are the member `npm/lecturenotes` of the repository's npm
workspace (`npm/README.md` describes the workspace). It builds two files, written into
the compose template's folder `templates/html/lecturenotes/dist/`:

- `dist/lecturenotes.css`, one stylesheet;
- `dist/lecturenotes.js`, one script, which defines one global name, `flmLecturenotes`,
  holding one function, `setup(settings)`.

A page includes both and calls the setup function once:

```html
<link rel="stylesheet" href="lecturenotes.css">
<script src="lecturenotes.js"></script>
<script>flmLecturenotes.setup({ navigation_heading_level: 1 });</script>
```

`settings` is an object; the script reads `navigation_heading_level` (1 when absent) and
ignores every other field. The setup runs once the document is parsed, wherever the call
stands. The font family is the custom property `--ln-family`, set on `:root` by the page.

The stylesheet `dist/lecturenotes.css` has no rule by group: the looks of the headings and
of the entries of the groups `major` and `minor`, and the folding of the table of
contents, are the rules of the template's file `templates/html/lecturenotes/groups.css`,
which the template writes with its selectors of the groups (see *The headings and the
entries, by group* in [What the page looks like and
does](#what-the-page-looks-like-and-does)). A page that uses the stylesheet on its own
writes those rules itself, with conditions on the attribute `data-flm-level` of its
headings and entries for its own levels: without them, the headings have no band, the
entries are alike, and the table of contents does not fold.

**The page's elements.** The script finds the page's parts by these classes; the classes of
the page's own elements begin with `ln-`:

```text
.ln-app                      the frame
  header.ln-topbar           the bar
    a.ln-book-title          the title, a link to the top
    .ln-current-unit         the title of the unit being read, written by the script
    .ln-prev, .ln-next       the previous-unit and next-unit buttons
    .ln-sidebar-toggle       the button that hides or shows the sidebar
    .ln-topbar-links         the bar's links (a.ln-topbar-link)
  aside.ln-sidebar           the sidebar, holding the table of contents (.flm-toc)
    .ln-toc-toggle           the unfold button: folded or all open
  main.ln-main               the main column
    .ln-column
      header.ln-titleblock   the title block
      #flm-content           the content
      section.ln-backmatter  the lists of notes and the bibliography
```

Only `#flm-content` is required; a part the page does not have is left out of the
behavior. The content and the table of contents are FLM's HTML output: the script reads
the headings (`.flm-pres-block--heading` elements with an `id` and a `data-flm-level`,
outside the regions of the pieces), the entries of the table of contents (the links
inside `.flm-toc` whose address is `#id`), the links it previews (`a.flm-pres-link` whose
`data-flm-kind` is `ref`, `term`, `cite` or `xnote` and whose address is `#id`), and the
citation marks (`.flm-citations-mark`).

**What the script changes on the page.** It adds no element around a document element and
moves none. It sets:

- on the root element `<html>`, the classes `ln-sidebar-collapsed` (the sidebar hidden on a
  wide window), `ln-drawer-open` (the drawer open on a narrow one) and `ln-toc-all` (every
  entry of the sidebar open);
- on each heading of the content, an `a.ln-anchor` (`§`) at its end;
- on the entry (`li`) of the table of contents of the unit being read, the class
  `ln-open`, and on the link of the heading being read, `aria-current`;
- the popup `div.ln-preview` at the end of `body`.

It keeps, in the browser's storage, the folding (`flm-lecturenotes:toc-folding`), whether
the sidebar is hidden (`flm-lecturenotes:sidebar-hidden`) and the reading position per
page address (`flm-lecturenotes:position:` followed by the address without its `#id`), and
the reading position in each history entry's state (the field
`flm-lecturenotes:position`).

**The live preview's event.** When the event `flm:region-updated` reaches `document` (FLM's
live preview dispatches it after it updated part of the page in place), the script reads
the headings again, re-adds the anchors, closes the popup and records the position.

**The popup's event.** Each time the popup shows a copy, the script dispatches the event
`flm:content-copied` on the copy (the `div.ln-preview-content`), which bubbles to
`document`. This is FLM's convention for a copy of part of the page (the rendering guide's
"A page updated in place"): the copy holds no `id` and keeps each element's
`data-flm-original-html`, and a script that transforms elements, such as the formula
renderer's configuration, completes on the event what the copy needs.

## Changing the sources and rebuilding `dist/`

The sources are in the repository, under `npm/lecturenotes/src/`: the script in TypeScript
(`index.ts`, `layout.ts`, `sidebar.ts`, `previews.ts`, `position.ts`) and the stylesheet as
plain CSS, one file per concern (`style/*.css`, joined in the order of `STYLE_FILES` in
`npm/lecturenotes/build.mjs`). The build needs Node; its only dependencies are `esbuild`
and `typescript`, for development, installed by the repository's npm workspace `npm/`:

```text
cd npm
npm install                     # once: esbuild and typescript, from npm/package-lock.json
npm test -w lecturenotes        # the unit tests, with Node's test runner
npm run build -w lecturenotes   # type check, then write dist/lecturenotes.js and dist/lecturenotes.css
```

In the folder `npm/lecturenotes/`, `node build.mjs test` and `node build.mjs` do the same
as the last two commands. The build writes the two files into this extension's
`templates/html/lecturenotes/dist/`, each beginning with the line `Generated by
npm/lecturenotes (node build.mjs); do not edit.` The script's source map stays in
`npm/lecturenotes/target/staging/`, outside the extension, and the script holds no link to
it. `node build.mjs --out DIR` writes the two files into `DIR`. The bundle is not
minified, so that it can be read and compared in version control. The build fails when
either file holds one of the delimiters of the template that includes it (`{%`, `%}`,
`{{`, `}}`, `{#`, `#}`, the list `DELIMITERS` of `build.mjs`); the test
`the_description_s_delimiters_are_those_the_bundle_s_build_checks` of
`flm/tests/lecturenotes.rs` checks that this list equals the `delimiters` of
`templates/html/lecturenotes/flmtemplate.yaml`.

`dist/` is committed, since the extension ships it. After a change of the sources, rebuild
and commit `dist/` with them: the test `the_committed_bundle_matches_a_fresh_build` of
`flm/tests/lecturenotes.rs` builds the bundle into a temporary folder and compares the
result with `dist/` byte for byte (it is skipped, with a message, when `node` is not on the
`PATH` or the workspace's dependencies are not installed).

**The template's files.** Under `templates/html/lecturenotes/`:

| file | what it is |
|---|---|
| `flmtemplate.yaml` | the template's description: its name, output, engine, text (`main`), delimiters, settings and shipped files |
| `lecturenotes.html` | the page: the frame around the library's parts, the template's text; it writes the setting `css`, applied as a template, in a `<style>` element of its head |
| `css_selectors.jinja` | the template's code that computes the value `css_selectors` from the settings `top_heading_level` and `navigation_heading_level`; it writes nothing. The page, `style.css` and `fragment.html` bring the value in with the statement `{% from "css_selectors.jinja" import css_selectors %}` |
| `groups.css` | the template's rules by group, written with `css_selectors`; `style.css` includes it (the statement `include`) |
| `style.css` | the page's stylesheet, applied as a template: the `font_family` line, then `dist/lecturenotes.css` included as it is (`include_raw`), then `groups.css` |
| `script.js` | the page's script, applied as a template: `dist/lecturenotes.js` included as it is, then `window.flmLecturenotes.setup(...)` with the setting `navigation_heading_level` as JSON |
| `fragment.html` | the fragment, applied as a template; it writes the setting `css` as the page does |
| `dist/lecturenotes.css`, `dist/lecturenotes.js` | the stylesheet and the script built by `npm/lecturenotes`, committed and never edited by hand; `style.css` and `script.js` include them |

**The browser check.** In the repository, `npm/lecturenotes/check/check.py` renders the
sample document (`flm/tests/documents/lecturenotes/notes.flm`) and the documents
`plain.flm` and `levels.flm` beside the script, opens them in Google Chrome with no window,
and checks the looks of the groups, the sidebar, the navigation and the previews through
the page itself; the file's opening
comment says how to run it. It is run by hand.

## Missing, to be implemented from the template of the Python FLM

This template is an adaptation of a similar internal template I had earlier.  In the
conversion to the new template format, Claude made some choices that were
misguided. A short TODO list of things we need to fix:

**Changed. NEEDS TO BE RESET**

- The old template lowered the opacity of figures and set a smaller text size on
  phones; Claude failed to carry both over.
- No/insufficient text context in popups when hovering an equation number
  reference.

**Planned enhancements:**

- a banner image above the title
- a companion for the Astro site builder, which would read the fragment
- a display of one chapter at a time, and a split of the document into one page
  per chapter
