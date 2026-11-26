# Letter

The `letter` extension formats a single letter as HTML or as LaTeX for
LuaLaTeX. It provides European and US layouts, optional letter fields, and
letterheads supplied as separate image files. Loading the extension selects
the `letter` template for both outputs.

The standard options cover paper, margins, body typography, logo placement,
and the principal spacing choices. Custom CSS and a LaTeX preamble provide
control over institutional designs, fonts, headers, and other typography.

## Writing a letter

Add `letter` under `flm.use` and put the letter inside one top-level `letter`
environment:

```flm
---
title: An invitation
flm:
  use: [letter]
---
\begin{letter}

\begin{fromaddress}
Alex Morgan\\
Example Research Institute\\
Example Street 14\\
10115 Berlin, Germany\\
\email{alex@example.org}
\end{fromaddress}

\begin{toaddress}
Dr. Taylor Smith\\
Example University\\
University Avenue 20\\
1000 Brussels, Belgium
\end{toaddress}

\letterdate{Berlin, 16 November 2026}
\lettersubject{An invitation to collaborate}
\salutation{Dear Dr. Smith,}

Thank you for your interest in our research. We would be pleased to discuss a
joint workshop with your group.

The enclosed outline describes a possible program. Further information is
available on \href{https://example.org/workshop}{our workshop page}.

\closing{Yours sincerely,}
\begin{signature}
Alex Morgan\\
Research Coordinator
\end{signature}
\enclosed{Enclosed: Workshop outline}

\end{letter}
```

The commands `\lettersubject`, `\letterdate`, `\salutation`, `\closing`, and
`\enclosed` accept inline FLM. Multiline fields use the environments
`fromaddress`, `toaddress`, `signature`, and `enclosed`, with `\\` between
lines. For example:

```flm
\begin{enclosed}
Enclosed: Workshop outline\\
Registration form
\end{enclosed}
```

Fields must appear directly inside `letter`, outside paragraphs, groups, and
other fields. Each field may appear once; the command and environment forms
of `enclosed` count as the same field. Duplicate fields are errors, including
duplicate empty fields. The document must contain exactly one `letter`
environment.

All fields are optional. Omitted fields and fields containing only whitespace,
comments, or line breaks disappear with their associated spacing. Commands
count as content even when they have no textual argument: a reference, note,
or image can produce visible content. A signature can appear without a
closing; handwriting space is reserved only when both fields are present.

Fields may be written in any order. Their placement follows the selected
layout. Numbering and references follow FLM's source order, so write numbered
content in its intended reading order. Write punctuation, dates, labels such
as `Enclosed:` or `Anlagen:`, and personal details explicitly. A final `\\`
in an address requests another line.

Ordinary FLM content forms the letter body, including paragraphs, lists,
mathematics, links, references, and notes. Body content can span several
pages. Addresses, the letterhead, and the closing/signature block are kept
together where possible. An individual block taller than a page must be
shortened or restyled. The letterhead appears once, at the start of the letter.

## Layout and rendering

With `style: europe`, the sender appears at the upper left, a column logo at
the upper right, the recipient below, and the date aligned right. The closing
and signature align with the body. The sender uses a 9pt font, reduced to the
body font size when the latter is smaller. Body paragraphs have no first-line
indent by default.

With `style: us`, a column logo appears at the upper left and the sender and
date occupy the right column. The closing and signature begin at that column.
Body paragraphs have a `2em` first-line indent, including the first paragraph.
The sender uses the body font size. Both layouts use an 8mm column gap.

Paper size is independent of layout: either style can use A4 or US Letter.
These layouts provide general letter formatting; envelope window positioning,
postal compliance, mail merge, and repeating page headers or footers are
outside the standard option set. Use the customization interfaces below for
additional layout requirements.

Both outputs set the text flush left, with a ragged right edge. In rendered
LaTeX, the default value of `latex_preamble`, `\raggedright`, does this (see
[Custom LaTeX preamble](#custom-latex-preamble)).

By default, HTML uses Times New Roman, TeX Gyre Termes, Times, or the browser's
serif fallback. Rendered LaTeX defaults to TeX Gyre Termes and requires LuaLaTeX
with the template's LaTeX packages installed. Set `font_family` to choose a
different text font (see [Fonts](#fonts)). External links use italic text in
the body font and retain clickable targets. Browser and LaTeX fonts and line
breaking can differ, so page breaks need not match.

```sh
flm render myletter.flm -o out/myletter.html
flm render -p latex-rendered myletter.flm -o out/myletter.tex
flm render -p pdf-latex myletter.flm -o out/myletter.pdf
flm render -p text myletter.flm -o out/myletter.txt
```

The `pdf-latex` profile compiles the rendered LaTeX using `latexmk` and
LuaLaTeX. Plain text contains the letter's fields and body without the logo
or visual layout. HTML adapts to narrow screens. When printing HTML, use the
document's paper size, disable browser-added headers and footers, and check
the print preview. Printing to the paper edge also requires suitable printer
support.

To see on screen where the printed pages end, set `show_page_breaks: true` for
`html`. A thin, light line then marks the end of the text of each printed page,
computed from the paper size and the margins. The lines are an approximation:
the browser can move a block to the next page to keep it together, so a
printed page can end before its line, and the later lines then come too late.
They are not printed, and narrow screens do not show them.

## Logos and full-width letterheads

The default is `logo: null`. Set `logo: placeholder` to demonstrate the layout
with the supplied generic artwork. Supply an organization's actual logo as a
separate file. A path is relative to the file in which it is written: the
document's folder for the front matter, and the package's folder for the
manifest of a package that uses `letter`, so that such a package ships its own
letterhead. A package written as one file (`mypackage.yaml`) has no folder of
files, so it names a file of the document's folder with `doc:`. The same rule
holds for `custom_css_file` and `latex_preamble_file`. A path may also name its
folder with a root (`doc:logo.svg`, `ext+mypackage:logo.svg`):

```yaml
flm:
  use: [letter]
  features:
    compose:
      template_config:
        html:
          letter:
            logo: './myletterhead.svg'
            logo_alt: Example Research Institute
            letterhead: page-width
            logo_max_height: 30mm
        latex-rendered:
          letter:
            logo: './myletterhead.pdf'
            letterhead: page-width
            logo_max_height: 30mm
```

Configure a logo file separately for each output. HTML accepts SVG, PNG, JPEG,
GIF, and WebP; rendered LaTeX (including `pdf-latex`) accepts PDF, PNG, and
JPEG. Supply both versions when using SVG for HTML and PDF for LaTeX, as above.
A PNG or JPEG can be used in both settings. Only the file for the requested
output is required. The placeholder includes both an SVG and a PDF.

The template copies logo files unchanged and performs no graphics conversion.
The `graphics.strategy` and `graphics.convert` settings apply to images in the
document body, not the letterhead. Keep the generated companion files with the
HTML or LaTeX file. A missing file or an unsupported image format is an error.
`logo_alt` supplies alternative text for HTML; use an empty string for
decorative artwork that repeats the sender's written name.

Choose the available logo area with `letterhead`:

| Value | Placement |
| --- | --- |
| `columns` | Beside the sender, using `logo_width`; right in Europe, left in US |
| `text-width` | Above the sender and date, across the text area |
| `page-width` | Above the sender and date, extending across both side margins |

The wide modes put the logo on its own row and preserve the body's margins.
`page-width` extends horizontally to the paper edges. On narrow screens it
extends to the viewport edges.

The letterhead stands at the top of the first page, below that page's top
margin, `letterhead_margin_top`. By default it equals `margin_top`. Set it to
`0mm` for a letterhead at the paper's top edge, or to a larger length for paper
with a printed letterhead. The other pages keep `margin_top`.

Images retain their proportions and fit within the available width and
`logo_max_height`. A tall image can therefore render narrower than the offered
width. Increase `logo_max_height` or supply wider artwork to fill a wide
letterhead. Wide artwork aligns left. `logo_width` applies only to `columns`;
it is ignored in the wide modes. With `logo: null`, no logo space is reserved,
and the logo-specific fit checks do not apply.

## Template options

Configure options under
`flm.features.compose.template_config.OUTPUT.letter`. The outputs are
`html` and `latex-rendered`; the latter also controls `pdf-latex`. Options
are independent between outputs. The CSS options and `logo_alt` exist only for
`html`, and the LaTeX preamble options only for `latex-rendered`. For example,
the US layout on US Letter paper with slightly more line spacing is:

```yaml
flm:
  use: [letter]
  features:
    compose:
      template_config:
        html:
          letter:
            style: us
            paper: letter
            line_spacing: '1.3'
        latex-rendered:
          letter:
            style: us
            paper: letter
            line_spacing: '1.3'
```

| Option | Default | Meaning |
| --- | --- | --- |
| `style` | `europe` | `europe` or `us` placement and default indentation |
| `paper` | `a4` | `a4` or `letter`, independently of style |
| `logo` | `null` | No logo, `placeholder`, or an image path |
| `letterhead` | `columns` | `columns`, `text-width`, or `page-width` |
| `logo_width` | `45mm` | Maximum column-logo width |
| `logo_max_height` | `45mm` | Maximum logo height in every mode |
| `font_family` | `null` | Default fonts; otherwise an HTML CSS font list or a LaTeX font name (see [Fonts](#fonts)) |
| `font_size` | `11pt` | Body font size, from 6pt through 24pt |
| `line_spacing` | `'1.25'` | Body baseline distance divided by font size |
| `margin_top` | `23mm` | Top paper margin |
| `margin_bottom` | `25mm` | Bottom paper margin |
| `margin_left` | `25mm` | Left paper margin |
| `margin_right` | `25mm` | Right paper margin |
| `letterhead_margin_top` | `null` | Top margin of the first page, above the letterhead; `null` for `margin_top` |
| `paragraph_spacing` | `0.65em` | Space between body paragraphs |
| `paragraph_indent` | `null` | Automatic: `0pt` in Europe, `2em` in US; a length overrides it |
| `header_gap` | `8mm` | Space between the principal letterhead/address sections |
| `signature_gap` | `18mm` | Handwriting space between closing and signature |

The options of `html` alone:

| Option | Default | Meaning |
| --- | --- | --- |
| `logo_alt` | `''` | Alternative text for the logo |
| `show_page_breaks` | `false` | On screen, a line where each printed page approximately ends |
| `custom_css` | `''` | Additional CSS, after defaults and `custom_css_file` |
| `custom_css_file` | `null` | CSS file path |

The options of `latex-rendered` alone:

| Option | Default | Meaning |
| --- | --- | --- |
| `latex_preamble` | `'\raggedright'` | Additional LaTeX preamble, after defaults and `latex_preamble_file` |
| `latex_preamble_file` | `null` | LaTeX preamble file path |

Lengths accept nonnegative decimal values with `mm`, `cm`, `pt`, or `em`,
such as `8mm`, `0pt`, and `0.5em`. An `em` is resolved against the requested
body font, including in margins and the smaller sender address. `font_size`
accepts positive `pt`, `mm`, or `cm` lengths within its 6–24pt range.
`line_spacing` accepts a multiplier from 1 through 3. Quote fractional
multipliers: FLM's JavaScript template interface accepts integer JSON numbers,
so write `'1.3'`, rather than a fractional YAML number.

An option name that the template does not declare, such as an option of the
other output, is a warning, and the option is ignored. Invalid types and
invalid values are errors. Margins must leave at least 40mm of text width and
height, also on the first page with `letterhead_margin_top` as its top margin.
Paragraph indentation must
leave at least 10mm of line width. Paragraph spacing, header gap, and
signature gap must each be smaller than the text height. A visible logo needs
a positive maximum height, and that height plus the header gap must be smaller
than the text height of the first page. In column mode its width must be positive and fit beside
the sender: the US layout allows half the text width less half the column
gap; the European layout leaves at least 20mm for the sender in addition to
the column gap. All length values must be at most 16000pt.

These checks validate the configured geometry. The height of addresses and
other content still affects the final layout. Very large fonts, unusually tall
signatures, and unbreakable content can require custom styling or shorter text.

## Fonts

Set `font_family` separately for each output:

```yaml
flm:
  use: [letter]
  features:
    compose:
      template_config:
        html:
          letter:
            font_family: '"TeX Gyre Heros", Arial, sans-serif'
        latex-rendered:
          letter:
            font_family: 'TeX Gyre Heros'
```

For HTML, the value is a CSS `font-family` list. The browser uses the first
available family. Fonts must be installed on the reader's system or supplied
through `@font-face` in custom CSS; the template does not bundle fonts.

For rendered LaTeX and `pdf-latex`, the value is one font name understood by
`fontspec`. The font must be available to LuaLaTeX on the machine producing
the PDF; a missing font is a compilation error. Use a plain font name without
TeX special characters. Advanced font features and separate choices for math
or monospaced text belong in `latex_preamble`.

The option changes the letter's main text family, including addresses,
subject, closing, and signature. It does not select math or monospaced fonts.
Omit the option or set it to `null` to keep the defaults. An empty value is an
error. Custom CSS and LaTeX preamble settings are applied afterward and can
override the selected family.

## Custom CSS

`custom_css_file` and `custom_css` are combined, file first, in a stylesheet
loaded after the default styles. This preserves normal CSS syntax, including
strings and comments. For example:

```yaml
html:
  letter:
    custom_css_file: './letter.css'
    custom_css: |
      .flm-letter-lettersubject { color: #23465c; }
      .flm-letter-signoff { margin-top: 10mm; }
```

The following selectors are the letter's customization interface:

| Selector | Content |
| --- | --- |
| `.flm-letter-sheet` | Paper container; its padding holds the margins on screen and the side margins in print |
| `.flm-letter` | The letter |
| `.flm-letter-header` | Logo, sender, recipient, and date |
| `.flm-letter-logo` | Logo image box |
| `.flm-letter-fromaddress`, `.flm-letter-toaddress` | Sender and recipient |
| `.flm-letter-letterdate`, `.flm-letter-lettersubject` | Date and subject |
| `.flm-letter-salutation`, `.flm-letter-body` | Salutation and body |
| `.flm-letter-signoff` | Closing/signature group |
| `.flm-letter-closing`, `.flm-letter-signature` | Closing and signature |
| `.flm-letter-enclosed` | Enclosures |

The `body` element also has a style class (`flm-letter-europe` or
`flm-letter-us`), a mode class (`flm-letter-columns`,
`flm-letter-text-width`, or `flm-letter-page-width`),
`flm-letter-no-logo` when the logo is absent, and `flm-letter-page-breaks` with
`show_page_breaks`. Combine these selectors to target
one layout. Use FLM's normal presentation selectors for formatting inside a
field or paragraph. For example:

```css
body { font-family: Georgia, serif; }
.flm-letter-europe .flm-letter-fromaddress { font-size: 10pt; }
.flm-letter .flm-pres-link[data-flm-kind~="url"] { font-style: normal; }
@media print {
  .flm-letter-lettersubject { color: black; }
}
```

Relative URLs in custom CSS resolve from the generated stylesheet's output
folder. Arrange additional fonts and image files there separately; reading
a CSS file does not copy the files it references. Custom CSS applies only to
HTML. Keep matching LaTeX adjustments in the preamble when both outputs need
the same design.

## Custom LaTeX preamble

`latex_preamble_file` and `latex_preamble` are inserted after all default
definitions, including the logo, and before `\begin{document}`. The file is
inserted first. Use these options to load packages, select fonts, and replace
the document's formatting commands.

The default value of `latex_preamble` is `\raggedright`, for a ragged right
edge. A value replaces it: keep `\raggedright` in the value for a ragged right
edge, or leave it out for justified text. The default also follows a
`latex_preamble_file`, so a file that sets the alignment needs
`latex_preamble: ''`. For example:

```yaml
latex-rendered:
  letter:
    latex_preamble_file: './letter-preamble.tex'
    latex_preamble: |
      \raggedright
      \setmainfont{TeX Gyre Pagella}
      \RenewDocumentCommand{\flmxLetterSubject}{+m}{%
        \par\noindent{\large\bfseries #1}\par\vspace{7mm}%
      }
```

The public formatting commands receive already-rendered FLM content:

| Command or environment | Arguments |
| --- | --- |
| `\flmxLetterHeader` | Sender, recipient, date; each may be empty |
| `\flmxLetterSubject` | Subject |
| `\flmxLetterSalutation` | Salutation |
| `flmxLetterBody` | Environment containing the body |
| `\flmxLetterSignoff` | Closing, signature; either may be empty |
| `\flmxLetterEnclosed` | Enclosures |
| `\flmxLetterLogo` | No arguments; logo content |
| `\flmxLetterLinkFont` | No arguments; font declarations for external links |

Subject, salutation, enclosures, and signoff commands are omitted when their
fields are absent. The header and body environment are always present. A
replacement header or signoff must handle its empty arguments. Preserve the
arguments as rendered content; they can contain links, notes, and FLM's line
formatting commands.

For a header customization that retains the standard field layout, save and
wrap the existing command:

```latex
\NewCommandCopy{\organizationLetterHeader}{\flmxLetterHeader}
\RenewDocumentCommand{\flmxLetterHeader}{+m +m +m}{%
  \organizationLetterHeader{#1}{#2}{#3}%
  \noindent\rule{\textwidth}{0.4pt}\par\vspace{4mm}%
}
```

For a LaTeX-drawn logo with `logo: null`, enable its header slot explicitly
and replace the logo command:

```latex
\flmxLetterHasLogotrue
\renewcommand{\flmxLetterLogo}{%
  \fbox{\parbox[c][12mm][c]{35mm}{\centering Example Organization}}%
}
```

The `\flmxLetterHasLogotrue` switch enables the logo slot for this use.
Other helper commands and lengths in the shipped preamble are implementation
details; use the public formatting commands above for reusable customizations.
Packages and files referenced by a custom preamble must be available to the
LaTeX compiler. The preamble options provide LaTeX source directly; they are
not parsed as FLM.
