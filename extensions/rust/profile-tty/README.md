# profile-tty

The extension `profile-tty` is the profile `tty` of the `flm` program: it produces a
document as text for a terminal. The text is the one the output `text` prints, laid out the
same way; bold, italic and underlined runs and links to addresses outside the document are
shown with the escape sequences a terminal understands, and the headings, the frames of the
tables and the links are colored. The images shown as blocks are drawn with colored
half-block characters. The extension is written in Rust and is compiled into the `flm`
program.

## Producing text for a terminal

The command gives the profile `tty`, which loads the extension `profile-tty` and runs its
workflow `tty`; the document's front matter needs nothing for it:

```text
flm render -p tty -o - paper.flm | less -R
```

`-o out/` writes the file `out/index.ansi`.

The workflow `tty` is the workflow `text` with the extension's output `tty` in place of
`text`: it runs the same checks, and its layout is the layout of `text`, with two settings
of the text backend that a terminal shows well: the tables are framed with box-drawing
characters (`text.table_frame=box`) and the rules under the headings are drawn with `═` and
`─` (`text.heading_rules=unicode`); it also sets the graphics settings and selects the
rule that draws the images (see [Images](#images)). `-O text.table_frame=none` and
`-O text.heading_rules=ascii` give the plain look back. The render options of the text
backend change it in the same way, for instance `-O text.width=100` or
`-O text.link_addresses=false`. The program does not measure the terminal: the width is the
text backend's, 80 columns unless an option sets it.

## What the escape sequences show

| in the document | shown as |
|---|---|
| a heading's title, bold text | bold |
| italic and emphasized text | italic |
| underlined text | underlined |
| a link to an address outside the document | underlined, and a hyperlink to the address |

A link to a place inside the document, monospaced text, small capitals and verbatim content
are written plain. Unless `tty.colors` (below) is `none`, some parts also get
one of the sixteen standard colors of the terminal, whose values the terminal's theme
chooses:

- the title of a first-level heading: bright cyan;
- the title of a deeper heading, the rules under the headings and the `#` characters in
  front of a deep heading: cyan;
- the frame of a table, and the rule under its header rows: bright black;
- a link to an address outside the document: bright blue.

The hyperlinks follow the OSC 8 convention, which most terminals and `less` 566 or later
honor; a terminal that does not shows the text without them.

The setting `tty.colors` chooses which colors are written: `none` writes none, keeping
bold, italics, underlining and hyperlinks; `standard`, the default, writes the standard
colors; `rich` also writes a color given by its red, green and blue values as it is, for a
terminal that shows 24-bit colors (with `standard`, such a color is written as the nearest
standard color). Any other value is an error, and the document is then written without
colors. For instance, to write no color:

```text
flm render -p tty -O tty.colors=none -o - paper.flm
```

`flm info` follows the convention of [no-color.org](https://no-color.org): when it paints
its pages for a terminal and the environment variable `NO_COLOR` is set and not empty,
whatever its value, it renders them with `tty.colors=none`.

Every line is self-contained: the attributes and the hyperlink are closed before each
newline and set again on the next line, after its indentation, which is never styled. A
control character in the document's text (verbatim content can hold one) is written as the
replacement character U+FFFD, so the document cannot send its own escape sequences to the
terminal.

## Images

An image shown as a block (the content of a figure) prints its label,
`[image: <alternative text or address>]`, as under the output `text`, and under it the image
drawn with the half-block characters `▀`, `▄`, `█` and the space: one character per column,
two pixel rows per line. The characters show the image in two tones, light and dark, so the
drawing means something without colors. An image shown inline prints its label alone.

The extension reads no file itself: the graphics feature of the FLM library finds, reads and
converts the images, and the workflow `tty` sets two of its settings for the text output. The
strategy `content` (`graphics.strategy.text`) keeps each image's content for the extension,
and a conversion rule (`graphics.convert.text`) turns a vector image (`svg`, `pdf`) into a
PNG image, by a converter that offers the pair: `magick`, which needs ImageMagick installed.
A PNG, JPEG, GIF or WebP image needs no conversion. Like any setting of the graphics
feature, the rules can be changed for a render, for instance to convert SVG drawings only:
`-O 'graphics.convert.text=[{from: svg, to: png}]'`. `-O graphics.strategy.text=reference`
keeps no content, so only the labels are printed.

The drawing reads PNG, JPEG, GIF and WebP images. An image of another format (an SVG
drawing that could not be converted, because ImageMagick is not installed, for instance), or
a file that is not a valid image, prints its label alone, with the error
`tty.graphics-not-shown`; a failed conversion is also reported by the graphics feature.

The drawing has the size the image is shown at, the width and the height the library
decides for the output `text` (the author's `width=` and `height=`, else the image's own
size; with one of the two, the other follows the image's proportions; the setting
`graphics.magnification.text` multiplies both), divided by the size of a terminal cell. An
image whose size the library could not decide (the warning `flm.graphics.unknown-size`) is
drawn at its size in pixels taken at 96 dots per inch. A `width=` and a `height=` whose
proportions differ from the image's stretch the drawing, as they stretch the image on the
other outputs. Two settings give the size of a terminal cell in points:
`tty.graphics_cell_width_pt`, 6 by default (the text output gives a column the width of 6
points, so an image `1in` wide takes 12 columns), and `tty.graphics_cell_height_pt`, 12 by
default (a terminal line is about twice as tall as a column is wide). Like `tty.colors`,
they are properties of the terminal the output is shown on: they are set on the command line
(`-O tty.graphics_cell_height_pt=14`) or by a workflow's settings. A value that is not a
positive number is the error `tty.invalid-cell-size`, and the default applies. A transparent
pixel shows black.

The rule `tty.graphics.image` of the text backend, which only the workflow `tty` prefers,
makes the drawing. It lowers the image node the graphics feature has prepared. An image shown
as a block becomes a block of the role `tty.graphics`, the block that stands for the image: it
holds the label and, when the image is drawn, a verbatim block of the rows of characters.

Under `tty.colors` other than `none`, the stage `tty.paint` colors each character of the
drawing with the colors of the two pixels it covers: under `rich` as their red, green and
blue values, under `standard` as the nearest of the sixteen standard colors. The printed text
carries the characters, not their colors, so the stage finds them again: the drawing is the
verbatim block whose node, named by the text output's position mark, sits in a block of the
role `tty.graphics`; the stage draws each image of the document at that grid's size, and the
first whose characters are the printed ones gives the colors. A verbatim block an author
wrote is never colored, and the label line keeps its ordinary look.

## Why an output over the text output

The extension makes no layout of its own. The text backend of the FLM library prints the
document and records, beside the text, which ranges of it are a heading's title, bold or
emphasized text, a link, the frame of a table and so on: its style marks. Only the printer
knows where each piece of content lands once the lines are broken and indented, so the marks
are the library's. The escape sequences are a terminal's business, so the painting is this
extension's: the stage `tty.paint` reads the text output's table `flm.output.text` (the
printed text and its style marks) and writes each marked range with the sequences that show
it. The output `tty` is therefore an output made over the text output, not a backend of its
own: a second backend would lay the text out a second time, and the two layouts could drift
apart. Removing the escape sequences from the output `tty` gives the text the text backend
printed for the workflow `tty`, with its control characters replaced. That text is the output
`text` under the workflow's settings, plus the drawings of the images (the rows under each
label, and the blank line before them), which the output `text` does not print: only the
workflow `tty` selects the rule that draws them.

For the same reason, the frame of the tables and the characters of the rules under the
headings are settings of the text backend (`text.table_frame` and `text.heading_rules`),
not of this extension. A frame changes where every character of a table lands and how much
width its columns get, which only the printer decides; a painter that drew it would have to
lay the text out again. The rules under the headings are characters of the printed text in
the same way: the painter colors them and does not choose them. As settings of `text`, the
frame and the rules also serve a plain text file, and the workflow `tty` only chooses them.

## The text output it needs

The workflow `tty` extends the workflow `text`, and the stage `tty.paint` needs the table
`flm.output.text`, which the text backend makes. A document that switches the feature `text`
off therefore cannot be built with the profile `tty`: the command stops with status 2. A
document that lists `profile-tty` under `use` and switches `text` off stops the same way
under every profile, since the extension's workflow is declared whatever the profile.

## The WebAssembly form

The same code builds as a WebAssembly module, which a program without the extension
compiled in loads from its search path:

```text
extensions/rust/build-wasm.sh profile-tty
flm render --extensions-dir target/wasm-extensions -p tty -o - paper.flm | less -R
```

The script writes `target/wasm-extensions/profile-tty/`, holding this file, the manifest
`flmextension.yaml` and the module `profile_tty.wasm`. A copy of the extension on the
search path is used in place of the one compiled into the program. The two forms give the
same bytes.
