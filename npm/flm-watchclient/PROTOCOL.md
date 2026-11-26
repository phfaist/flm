# The protocol of the watch client

The *watch client* keeps a page shown by `flm watch` in step with its document. While
`flm watch` runs, the `flm` program builds the document into an HTML page after each
change of its files, and serves the page to a browser on the same machine. The watch
client has two parts in the browser:

- the *page script*, which runs inside the document's page, with its stylesheet. After
  each build, it updates the parts of the page that changed, or reloads the page. It
  marks the elements in which a problem lies, and opens the place of a clicked element
  in an editor. It adds no visible element to the page;
- the *tools page*, a page of the server's own, which shows the document's page in a
  frame, beside the preview's tools.

The two parts exchange messages when the document's page is shown in the tools page's
frame. The document's page opened alone, at `/`, is a live preview without tools. This
package builds both parts into the files that the `flm` program embeds (see
[README.md](README.md)).

This document specifies what the watch client expects of the page that contains the page
script and of the server that serves the pages, and the messages between the two parts.
Any server that meets these requirements can run the watch client. The server of the
`flm` program is `flm_host::watch::server::Server`, in the crate `flm-host`; the
documentation of its module describes how it meets them.

Contents:

- [Terms used in this document](#terms-used-in-this-document)
- [The addresses of the output and of the
  server](#the-addresses-of-the-output-and-of-the-server)
- [The tools page](#the-tools-page)
- [The page](#the-page)
- [The server and the build that a page
  shows](#the-server-and-the-build-that-a-page-shows)
- [The stream of messages](#the-stream-of-messages)
- [Requests to the server](#requests-to-the-server)
- [The messages between the tools page and the page
  script](#the-messages-between-the-tools-page-and-the-page-script)

## Terms used in this document

- The *server* is the web server that builds the document and serves its output to the
  browser. It also answers the requests of the client at a few addresses of its own, all
  under the prefix `/.flm-preview/`.
- The *client* is the watch client, running in the browser: the page script, or the tools
  page.
- A *build* is one run in which the server produces the document's output. Each build has
  a number, a non-negative integer. The client compares build numbers only for equality,
  so two builds of one server must have different numbers. The `flm` server numbers its
  builds 1, 2, 3, … in the order in which it starts them. A build that the server cancels
  (for a change of a file, a request to build again or a change of the build type) takes a
  number and sends no `update`, so the numbers that a page receives can skip.
- The *output* of a build is a set of files, each with a path, such as `plot.png`. One of
  these files is the *page*, an HTML page written by FLM's HTML backend. The other files
  are those that the page uses, such as images and stylesheets. A build can fail to
  produce an output, for instance when the document's front matter cannot be read. The
  server then keeps the output of the last build that produced one.
- An *output region* is a part of the page that the client can update without reloading
  the page. An output region is either the content of one element of the page, identified
  by the element's `id`, or the page's title. For instance, the element with the `id`
  `flm-region-content` contains the document's content.
- A *problem* is a diagnostic reported during a build: an error, a warning or a note about
  the document, with its text and, when it is known, its position in a source file.

## The addresses of the output and of the server

The client sends each request to an absolute path on the origin of the page (the scheme,
the host and the port of the page's address), such as `/.flm-preview/events`. The page,
the other files of the output and the server's own addresses must therefore be on one
origin.

Each file of the output must be available at `/` followed by the file's path, as in
`/plot.png`, whatever the query of the address. The client relies on these addresses when
it reloads a file that changed in a build ([`update`](#update)): it looks for the elements
of the page whose address, without query and fragment, is the file's address, and it adds
a query to that address. The `flm` server also serves the page at `/`.

The server should send the header `Cache-Control: no-store` with every response. The
browser then caches nothing, and a reloaded page shows the latest output.

## The tools page

The *tools page* is a page of the server's own, at the address `/.flm-preview/`. It shows
the page at `/` in a frame (an `iframe` whose address is `/`), beside the preview's tools.
The files that the tools page uses are at `/.flm-preview/assets/` followed by each file's
path: its script `/.flm-preview/assets/tools.js` and its stylesheet
`/.flm-preview/assets/tools.css`, which the tools page loads by these absolute paths, and
the files of pdf.js that the PDF view loads when it shows a PDF file, by their absolute
paths too: the library `pdfjs/pdf.min.mjs` and its worker `pdfjs/pdf.worker.min.mjs`, two
modules served as `text/javascript`, and the fonts `pdfjs/standard_fonts/FoxitSymbol.pfb`
and `pdfjs/standard_fonts/FoxitDingbats.pfb`, beside their licenses
`pdfjs/standard_fonts/LICENSES.txt`. The tools page itself is also at
`/.flm-preview/assets/index.html`, and at `/.flm-preview/help`, where it shows the help
browser alone ([below](#the-help-browser-alone)). An address under
`/.flm-preview/assets/` that is not the address of one of these files is answered with the
status 404. The `flm` program opens the tools page in the browser, and prints its address.
The page at `/` alone shows the document without the tools.

The server sends a file of the tools page compressed with gzip, with the header
`Content-Encoding: gzip`, when the header `Accept-Encoding` of the request accepts gzip,
as every browser's does; otherwise it sends the file as it is. Each response carries the
header `Vary: Accept-Encoding`.

The tools page opens a connection of its own to the [stream of
messages](#the-stream-of-messages). From `hello` and `update` it shows the output, by the
`media_type` alone, never by the name of the build type: the page at `/` in a frame for
`text/html`; for another `text/` type, the text of `/`, which it reads again after each
`update` whose `page` is `"reload"` or `"regions"` and after each `hello` that names
another `build` than the one it holds; for `application/pdf`, the pages of the PDF file at
`/`, which it reads again at the same messages, keeping the page read and the zoom; a line
that names the media type for any other type; a line that says that the build type gives
no output when `media_type` is `null` and `build` is not (it names the build type when
`earlier_output` is `false`); and the document's name while both are `null`. Its status
bar lists the `profiles` in a selector, which shows the `profile` as the server reports
it and marks the `default_profile`; a choice there sends the request [`POST
/.flm-preview/profile`](#changing-the-build-type). It shows the state of the builds in its
status bar: how long the last build took (`duration_ms`), that a build runs once it has
lasted longer than 0.3 s (from `building` to `update`), with the text of the last
[`progress`](#progress) message of that build when there is one, that the output is from an earlier
build (`earlier_output`), or that the connection is lost. On a `hello` whose `server`
differs from the one of its first `hello`, the tools page loads itself again. It lists the
`problems` in its *Problems* tab ([the problems](#the-problems)); when a problem is
selected there, it sends the page the message `show-place` with the problem's `file` and
`bytes`, and *Open in editor* sends the request [`POST
open`](#opening-a-position-in-an-editor) with the problem's `file` and the `start` of its
`bytes`. It does either only for a problem whose `place` has an `offset`: the `bytes` of a
place in a text that the library parsed under the file's name, such as the document's
title, count from the start of that text and locate nothing in the file's content. A
problem whose `place` has a `file`, a `line` and a `column` but neither an `offset` nor
`bytes` (a problem in the file's front matter) sends no `show-place`, and *Open in editor*
sends `POST open` with its `file`, `line` and `column`; a problem of the title,
which has `bytes` without an `offset`, is not opened. When the server answers the place
instead of opening an editor, the part of the browser that received the gesture copies the
place to the clipboard (the tools page for *Open in editor*, the page script for a click in
the document), and the tools page says so in its status bar with the server's reason: as a
notice when the place was copied, as a failure with the place when it was not.

Its *Metadata* tab shows the root keys of the front matter other than `flm`, from the
[settings state](#reading-the-settings-state), which the tools page reads when the tab
opens, again whenever the `settings` counter of `hello` or `update` changes, and again once
the server has answered each of its changes. Each change of a control is one request
[`POST /.flm-preview/front-matter`](#changing-the-front-matter) against the `revision` of
the state read, which holds every change of the front matter that the control makes (most
controls make one; a revert arrow, for example, may remove several keys): a text field
sends it when the author leaves the field or presses Enter, a switch, a choice and a
checkbox at the change. A row shows a check for 1.5 s once the
server has answered with a success; a refused change shows the server's line of text in
the status bar. The YAML editor of a value starts from the text of [a value as
YAML](#reading-a-value-as-yaml) and sends its text as `yaml`. While `unreadable` is not
`null`, the tab shows that line and its controls are disabled; they are disabled too while
the connection is lost.

Its *Settings* tab shows the keys of `flm` from the same state, read and changed in the
same way. Its common settings and the curated settings of its advanced list are the tools
page's own choice, each at a path of the front matter; a row whose path the `schema` does
not have is left out. Each feature of the `schema` has a row in the advanced list, with the
keys of its entry. A row's control follows from the schema at its path; a schema that
admits `null` beside another type gets the control of that type, after a checkbox that,
unchecked, sends `null`. A row shows the value that `front_matter` writes at its path (a
`null` counts only where the schema admits it), else the value of `in_effect`, else the
default of the `schema`. Its origin is the one that `in_effect` gives for the value at its
path, read down `entries` ([the origins below a key](#reading-the-settings-state)). A row
whose origin is `"workflow"` or `"command_line"` names that source beside its control and
can still be changed: the front matter's value applies under another build type or without
that option. A row is locked only where a write at its path would get `409`: where
`front_matter` holds, at a key of its path but the last, a value that is neither a map nor
`null`, such as the entry `false` of a feature that the front matter switches off. (While
the front matter cannot be read, every control is disabled, as said above.) A row of a
feature that another source switches off can be changed. The tooltip of a row's
origin lists, from [the sources of the value](#the-sources-of-a-value) at its path, which
the tools page reads when the tooltip is first wanted, each source's value, lowest rank
first, the one in effect marked; below, in words, the marks (`x-flm-marks`) of the schema
at the path and of the schema of the object that holds it. When the sources cannot be read,
the tooltip gives the failure's line below its first line. For a row whose value the front
matter writes, the tools page reads the sources of the value when it shows the row, and
again at each new reading of the settings state; when a source of the kind `"package"` is
among them, the row shows the package icon, with the same tooltip, before its revert arrow. The switch of a feature sends `false` at the path of its key
`enabled`; to switch the feature on, it removes that key or the entry `false` where
`front_matter` writes them, and sends `true` otherwise. The options of the templates, the
build types, the math processors and the packages come from `choices`. So do the settings
of the compose template in use: the output is the `composed_as` of the first entry of
`outputs` that has one, the template is the one that `compose.template` gives that output,
its settings are the keys of the `config` of its entry of `templates`, with their
defaults, and a setting is written at the path `flm`, `features`, `compose`,
`template_config`, the output's name, the template's name, the setting's key, where the
`schema` declares it. A row of these shows the value that `front_matter` writes, else the
value of `in_effect`, else the default of the `schema`, else the default of `config`.
Each change sends the changes that the author made, and nothing else: a choice of a
template sends the name at its output's path alone. The values that the server answers
it copied (`copied`) are named in the status bar.

Its *Outline* tab lists the `outline` and the `labels` of the last `hello` or `update` in
one list: each heading followed by the labels whose `heading` is its position, the labels
whose `heading` is `null` first. A click on a heading or on the `designation` of a label
that has an `id` sends the page the message `show-element` with that `id`. Beside each
label, the tools page shows the code that refers to it, which it copies to the clipboard:
`\term{TERM}` for a label `defterm:TERM` (a label that a term definition computes from its
term and its aliases), `\ref{LABEL}` for every other. This rule is the tools page's own; the
server sends no such code. The labels can be hidden by `kind`, a label whose `kind` is
`null` counting as `other`.

Its *Help* tab, shown only when `help` is `true`, shows [the help pages](#the-help-pages):
it reads the index when it first needs it and again when `environment` changes, and the page
shown at each step of its history. Above the page, it shows the page's `path_line`, each
segment but the last a link to the page at its `path`. A click on a link of the page's
content whose `data-flm-kind` is `"flmdoc.page"` shows the page whose path is the link's
`href` without its final `.html`; a link to a place of the same page (an `href` that begins
with `#`) scrolls the page to that place; any other link opens in a new tab of the browser.
A page whose `settings_feature` is not `null` has the link *Settings of this feature*, which
opens the *Settings* tab at that feature. *Open this page in a new tab* opens the help
browser alone on the page shown. In the *Problems* tab, a selected problem whose `place` has
an `offset` shows a line "Related:" with links: the tools page asks for [the constructs at
the place](#the-constructs-at-a-place) given by the problem's `file` and the `start` of its
`bytes`; the innermost command or environment there gives the link to its `help` page and
the link to the page of its `feature` (the page of the index of the kind `feature` whose
`names` hold the feature's name), or, when its `help` is `null`, a search of the help for
its name.

Its status bar's *Inspect*, offered while the frame shows a page (`media_type` `text/html`),
turns [Inspect](#inspect) on and off in the page, and again on in each page that its frame
loads while it is on. When the page reports a selection (`inspected` with a `place`), the
tools page asks for [the constructs at the place](#the-constructs-at-a-place) given by the
`file` and the `start` of `place`, and draws a card over the frame, beside the element's
`rect` and inside the frame's window. Of the constructs whose `file` is that of `place` and
whose range contains the whole of `place`, the innermost is the element's construct (a
construct that starts at `place` but ends inside it, such as a command at the start of a
paragraph, is not), and the others are around it. The card shows the element's construct as
written and its `kind` and `feature`; "Inside:" and the constructs around it, each a link
that sends the page `inspect-place` with that construct's `file`, `start` and `end`; the
place as `FILE:LINE:COLUMN` from `line` and `column`; *Open in editor*, which sends [`POST
open`](#opening-a-position-in-an-editor) with the `file` and the `start` of `place`; *Help*,
when the element's construct has a `help` page; *Copy* and the code that refers to `label`
(as in the *Outline* tab), when `label` is not `null`; and the messages of the `problems`
whose `bytes` overlap `place` (as the elements of a part do), one item for the problems
that *Problems* shows in one row. The card closes on
Escape, on a click elsewhere in the tools page, on a selection of nothing, on `scrolled`,
when the page loads again or its output regions change (the `build` of an `update` changes),
when the frame's size changes, and when Inspect is turned off; closing it sends `inspect`
again, so that no element stays selected.

Its *Export* tab [exports the document](#exporting-the-document). It shows a selector of
the build type, on the one shown until the author chooses another, and asks for the
export's name and command lines when the build type or `environment` changes. A yellow
box above its two buttons states the problems: after an export of the build type chosen
that the server refused with the status `428`, the first line of that answer, the checkbox
*Export despite the errors* and the link *Show log*, which shows the answer's log, whatever
the preview's own `problems`, since the build type exported may have errors that the
preview's has not; otherwise, from the `problems` of the last `hello` or `update`, their
counts with the checkbox and the link *Show problems* when there are errors, the count and
the link when there are warnings alone, and no box when there are neither. The box is
decided by the status of the answer alone, never by its text. While the box holds the
checkbox, *Save as sibling* and *Download* are disabled until it is checked, the request
sends `continue_with_errors` as the checkbox says, and the command line shown is
`command_continuing`; otherwise `command`. A refusal for errors clears the checkbox, and its
box holds until the next `hello` or `update`; from then on, the box is again from the
`problems` of that message. While an export runs, the selector is disabled, and the button
that started it says so. Below the buttons, the tab shows the result of the last export:
the name written, with *Open in default app*
([`POST export/open`](#exporting-the-document)), or the first line of the answer that
refused it, with *Show log* when the answer has a log; a download shows nothing there, and
a refusal for errors shows in the box. A result shows only while the build type it
exported is the one the tab exports, which follows the preview's until the author chooses
one; a choice of another build type in the tab clears the result.

Its *Viewer* tab shows the [viewer settings](#the-viewer-settings) but `panel_side`,
`panel_width` and `panel_height`, and each of its controls sends [`POST
/.flm-preview/viewer`](#changing-the-viewer-settings) with the setting at the change; a
control shows the setting as the last `hello` or `viewer` gives it. While `unreadable` is
not `null`, the tab shows that line and its controls are disabled. The tools page also
sends `POST /.flm-preview/viewer` when the author docks its panel at the other edge
(`panel_side`) and at the end of a drag that resizes it (`panel_width` or
`panel_height`), unless the settings are unreadable or the connection is lost; it then
keeps the panel's place in memory until the next `hello` or `viewer`. It takes its
theme, the wrapping of a text output and whether it opens *Problems* from the same
settings.

Each time its frame has loaded a page, the tools page sends that page the message
[`attach`](#the-messages-between-the-tools-page-and-the-page-script). It reads the page in
its frame only through the messages of the page script.

#### The help browser alone

At `/.flm-preview/help`, the tools page shows the help pages alone, as its *Help* tab shows
them, on the page whose path the address's fragment gives: the path as it is, with `%` and
`#` and the characters that an address cannot hold percent-encoded, such as
`/.flm-preview/help#features/refs/macros/ref`; without a fragment, on the first page of
the index. The fragment follows the page shown, and a fragment changed in the address bar
shows its page. It reads the stream as the tools page does, for `environment`, and asks
for the index whatever `help` says: when the program gives no help pages, it shows the
line of text of the server's answer.

## The page

### The script and the stylesheet

The page must contain the page script and its stylesheet, either inline or by address. The
`flm` server writes both inline into the page.

If the page's `body` exists when the script runs, the page script starts at once.
Otherwise, it starts when the browser has parsed the page (the event `DOMContentLoaded`).

The page script defines no global name and changes no built-in object of the browser. It
adds no element to the page. It sets the following attributes on the page's elements,
which its stylesheet styles:

| Attribute | On | Style |
|---|---|---|
| `data-flm-preview-problem` | an element in which an error or a warning lies ([the problems](#the-problems)); its value is `"error"` or `"warning"`, the worse of the problems that lie in it | a dashed outline, in the color of the severity |
| `data-flm-preview-flash` | an element that the page script shows to the reader, while it flashes once | an animation of the background, at the end of which the page script removes the attribute |
| `data-flm-preview-inspected` | the element that Inspect selected ([Inspect](#inspect)) | a solid outline |

Each color, size and duration of the stylesheet is a custom property declared on `:root`,
such as `--flm-preview-error`, so a page changes it by setting the property.

### The markup that the client reads

The page script reads the following markup of the page:

- The attribute `data-flm-preview-waiting` on the root element (`<html>`) marks a waiting
  page ([below](#the-waiting-page)).
- The attribute `data-flm-source-span` records the part of a source file from which an
  HTML element was written. Its value is a JSON array of three items: the name of the
  source file, the byte offset of the start of the part, and the byte offset just past its
  end, as in `data-flm-source-span='["paper.flm",230,241]'`. FLM's HTML backend writes
  this attribute when its render option `source_spans` is on. The page script uses the
  attribute to find the elements of a part of a source: those in which a problem lies
  ([the problems](#the-problems)), and those that the tools page asks to show
  ([`show-place`](#messages-to-the-page-script)). It also uses it to find the place of a
  clicked element: when the author clicks an element with Ctrl or Cmd and Shift, the page
  script sends a request to open the element's position in an editor ([opening a
  position in an editor](#opening-a-position-in-an-editor)), and while Inspect is on, it
  reports the place of the clicked element to the tools page ([Inspect](#inspect)).
- The `id` of the element of each output region. The page script finds the element of a
  changed output region by this `id` ([`update`](#update)).
- The `id` of each element that a label refers to. The messages of the server map these
  `id`s to the labels (the field `references` of [`hello`](#hello)). The page script
  reports the label of the element that Inspect selected.

The *elements of a part* of a source file are the innermost elements whose
`data-flm-source-span` has the part's file and a range that overlaps the part's range. A
part that covers no bytes counts as covering the byte at its start.

### The waiting page

A *waiting page* is a page that the server serves while no build has produced an output
yet. A waiting page has the attribute `data-flm-preview-waiting` on its root element
(`<html data-flm-preview-waiting>`), and it must contain the page script and its
stylesheet. The element of class `flm-preview-waiting` holds its text, which the
stylesheet styles.

On a waiting page, the page script reloads the page as soon as the server has an output:
when the field `build` of [`hello`](#hello) is not `null`, or when an [`update`](#update)
reports a build that produced an output.

### Updating an output region in place

When a build changes an output region, the page script replaces the content of the
region's element. It keeps every element inside the region that did not change, so that
the reader keeps their position in the page. The page script follows the two conventions
for a page updated in place that the rendering guide of `flm-core` describes (section "A
page updated in place"):

- Once the content of a region's element has been replaced, the page script dispatches the
  event `flm:region-updated` on that element. The event bubbles. Its `detail` contains
  `region`, the `id` of the element, and `artifacts`, the paths of the files of the output
  that changed in the same build (the field `files` of [`update`](#update)).
- A script of the page can transform an element in the browser, as a formula renderer
  replaces the source of a formula by its typeset form. Such a script records the HTML of
  the element before the transformation in the attribute `data-flm-original-html`. If the
  new HTML of the element is equal to this record, the page script leaves the content of
  the element as it is. Otherwise, the page script replaces the content and removes the
  attribute. The other script then transforms the element again when it receives the
  event.

### Scroll to changes

After it has updated the output regions of a build in place, the page script shows the
reader what changed. The *changed elements* are the elements that the update added to the
page, and the elements whose attributes or whose own text (the text directly inside the
element) changed, leaving out the elements inside another changed element and the
attributes that the page script sets itself. An element that a script of the page
transformed and whose new HTML equals its record `data-flm-original-html` is not changed.
The page script scrolls to the first changed element in the order of the page, unless a
part of it is already inside the window, and then flashes each changed element once. A
change of the page's title alone shows nothing. The page script does so only while the
[viewer setting](#the-viewer-settings) `scroll_to_changes` is `true`: it reads the setting
from `hello` and from each message `viewer`, and takes it as `true` until a message gives
it.

## The server and the build that a page shows

The page script needs to determine which server served the page, and which build produced
the output that the page shows. With this information, the page script detects that the
page is out of date, and reloads it, in two cases: when the server has been restarted, and
when a build finished while the page was not connected to the [stream of
messages](#the-stream-of-messages).

The server provides this information in the header `Server-Timing` of the response that
contains the page:

```text
Server-Timing: flm-server;desc="SERVER", flm-build;desc="BUILD"
```

`SERVER` identifies the server, and must be different each time a server starts. `BUILD`
is the number of the build whose output contains the page, written in decimal digits. The
browser makes this header available to the page's scripts as the `serverTiming` of the
page's navigation entry (`performance.getEntriesByType("navigation")`), and the page
script reads both values there. The information is sent in a header so that the page
itself stays exactly as the build wrote it.

If the browser does not make the header available, the page script uses the server and the
build of the first [`hello`](#hello) message that it receives. Browsers make
`serverTiming` available only to a page in a secure context; a page served from
`127.0.0.1` or `localhost` is in a secure context. The page script does not read the
header on a waiting page.

After the page script has applied an [`update`](#update) message of a build that produced
an output, it takes that build as the build that the page shows.

## The stream of messages

The server sends messages to the client as a stream of server-sent events. The page
script and the tools page each open the stream with an `EventSource` on the address
`/.flm-preview/events`. When the
connection is lost, for instance because the server has stopped, the browser reconnects by
itself. The server can set the delay before a reconnection with the field `retry` of the
stream.

Each message is an event with a name, and its data is a JSON object. The server sends
five messages:

| Event | When the server sends it |
|---|---|
| `hello` | first, on every connection to the stream |
| `building` | when a build starts |
| `progress` | while a build runs, when the text of its progress line has changed, at most four times a second |
| `update` | when a build has finished, whether or not it produced an output |
| `viewer` | when the [viewer settings](#the-viewer-settings) have changed; its fields `editors` and `viewer` are those of [`hello`](#hello) |

The page script ignores a message whose data is not a JSON object. It also ignores a
`hello` message whose `server` is not a string or whose `build` is neither a number nor
`null`, and an `update` message whose `build` is not a number or whose `page` is not one
of its four values. For each message that it ignores, the page script writes an error to
the browser's console. The page script also ignores events with other names. A message can
contain more fields than those listed below; the page script ignores them.

### The problems

The messages `hello` and `update` contain the problems of a build, as an array of objects.
Each is the record of the problem that `flm --json-messages` writes, with two fields
more, `bytes` and `excerpt`. A field without a value is left out.

| Field | Value |
|---|---|
| `kind` | `"diagnostic"` for a diagnostic of the document; `"problem"` for a problem that stopped the build before the document was processed |
| `severity` | `"error"`, `"warning"` or `"note"`; a problem of kind `"problem"` is an error |
| `id` | for a diagnostic, the identifier of its condition, such as `"flm.refs.unresolved"` |
| `data` | for a diagnostic, the data of its condition, a JSON value, or `null` when that data has no JSON form |
| `message` | the message for the author, without the severity and without the places |
| `place` | where the problem lies, as the terminal shows it: a [place](#a-place) |
| `frames` | for a diagnostic, the "inside" chain, in the order the terminal lists it, as an array of objects `{"place": PLACE, "title": TITLE}`: each is a group, an argument or another part that the problem lies in, or a place where the content that holds the problem is shown again (the title `"a copy of this content, shown here"`); an empty array when there is none |
| `made_from` | for a diagnostic in content that the library made (the expansion of a macro, a text given in the settings), the place in that content: a [place](#a-place) that gives the content's `label` |
| `bytes` | `{"start": START, "end": END}`, the byte offset of the start of the problem and the byte offset just past its end, counted as in `data-flm-source-span`, in the source named by the `file` of `place` |
| `excerpt` | the line of `place`, when `place` is in the content of a file (it has an `offset`): `{"line": LINE, "before": TEXT, "covered": TEXT, "after": TEXT}`, the line's number counted from 1, and the line's text without its line break, cut in three at the start and the end of what the problem covers; a problem that covers several lines gives its first line, with `covered` running to the end of that line |

#### A place

A place gives the following fields, each left out when it has no value:

| Field | Value |
|---|---|
| `file` | the name of the source file, as in `data-flm-source-span` |
| `label` | the label of the content that the library made, for a place in such content |
| `line`, `column` | the line and the column, each counted from 1 at the top of the file; the column counts bytes |
| `offset` | the byte offset counted from the top of the file, for a place in the file's content (below its front matter) |
| `end` | the end of the part of the file that the place covers, `{"offset": OFFSET, "line": LINE, "column": COLUMN}`, counted as above, for a place in the file's content |

The page script marks the elements in which an error or a warning lies: the [elements of
the part](#the-markup-that-the-client-reads) from the `start` to the `end` of `bytes` in
the `file` of `place` carry the attribute `data-flm-preview-problem`, whose value is the
worse severity of the errors and warnings that lie in it. A note marks no element, nor
does a problem without `bytes`, or whose `place` has no `file` or no `offset`. The page
script takes the marks of the problems of a build off when it marks the problems of the
next.

The tools page lists the problems in its *Problems* tab, by file. It shows one row for the
problems of kind `"diagnostic"` that have the same `id`, the same `data`, the same `place`
and the same `bytes`, with their number; nothing else is merged.

### `hello`

The message `hello` describes the state of the server when the client connects:

| Field | Value |
|---|---|
| `server` | the identifier of the server, the same as `SERVER` in the header `Server-Timing` |
| `document` | the path of the document's file, as the author would write it on a command line; the `flm` server sends the path given to `flm watch` |
| `build` | the number of the build that produced the output that the server has, or `null` while no build has produced an output. A build of a build type that writes no file (such as `check`) produces an output without files, which replaces the output the server had |
| `building` | `true` while a build is running, `false` otherwise |
| `problems` | the problems of the last build ([the problems](#the-problems)), or an empty array before the first build |
| `references` | an object that maps the `id` of an element of the page to a label that refers to the element, as in `{"sec-intro": "sec:intro"}`; where several labels refer to one element, the first in alphabetical order; `null` while no build has produced an output |
| `outline` | the headings of the document that the output shows, in document order ([the outline and the labels](#the-outline-and-the-labels)); `null` while no build has produced an output |
| `labels` | the labels of the document that the output shows, each with what it designates ([the outline and the labels](#the-outline-and-the-labels)); `null` while no build has produced an output |
| `editors` | every editor the server knows, as an array of objects `{"name": NAME, "label": LABEL, "available": AVAILABLE}`: the editors `code`, `cursor`, `codium`, `zed`, `emacsclient`, `subl`, `gvim`, `mvim`, `vim` and `nvim`, in this order, each available (`true`) when it was found on the author's machine, then `{"name": "custom", "label": "Custom command", "available": true}`, always available. A client chooses only an available editor |
| `viewer` | the [viewer settings](#the-viewer-settings), as an object |
| `help` | `true` when the program that runs the server gives [the help pages](#the-help-pages), `false` otherwise; a client that shows the help pages shows them only when it is `true` |
| `media_type` | the media type of the file that the server serves at `/` (the page, or the main file of an output that is not an HTML page), such as `"text/html"` or `"text/plain"`; `null` while no build has produced an output, and for an output that has no main file, such as the output without files of a build type that writes no file (`build` is then not `null`) |
| `earlier_output` | `true` when the output that the server has was produced by an earlier build than the last build that finished, because the last build did not hand over its output (it stopped at the preparation, failed as a program, or its output could not be produced); `false` otherwise, also after a build of a build type that writes no file |
| `duration_ms` | how long the last build that finished took, in whole milliseconds, or `null` before the first build has finished |
| `profile` | the build type in use: the name of the profile that the last build that finished ran, or `null` before the first build has finished. It is the profile asked for, when one was (`flm watch -p NAME`, or the last [`POST /.flm-preview/profile`](#changing-the-build-type) before that build); otherwise the profile of the last successful preparation of the document (the front matter's `default_profile`, else `html`); `null` when neither exists. When the preparation of the profile asked for fails, `profile` stays that name: it can then be absent from `profiles` (`flm watch -p nosuch` gives `"profile": "nosuch"` and `"profiles": []`), and the output served can be that of another profile, with `earlier_output` `true`. A client that shows the build type shows this name as reported, whether `profiles` lists it or not |
| `default_profile` | the default build type: the name of the profile that the builds run when no client has chosen one with [`POST /.flm-preview/profile`](#changing-the-build-type). It is the profile asked for when the server started (`flm watch -p NAME`); otherwise the front matter's `default_profile`, else `html`, as the last successful preparation of the document found them, so that it follows the front matter when the front matter changes. `null` before the first build has finished, and while no profile was asked for at start and no preparation has succeeded. Like `profile`, it can be absent from `profiles` |
| `profiles` | the build types available: the names of the profiles that the document can run, in name order, as the last successful preparation of the document lists them; an empty array before it |
| `environment` | the digest of the document's environment (the packages, the features and their settings, as the last successful preparation of the document assembled them), as 16 hexadecimal digits, or `null` before that preparation. It changes when the packages or the settings that the document uses change |
| `settings` | a counter of the [settings state](#reading-the-settings-state): a number that changes whenever the settings state may have changed (the server increases it at each build that finishes; it is `0` before the first). A page that shows the settings state reads it again when the number differs from the one it read it at |

The page script compares `server` and `build` with the server and the build that the page
shows ([above](#the-server-and-the-build-that-a-page-shows)). If either is different, the
page script reloads the page. On a waiting page, it reloads the page if `build` is not
`null`. Otherwise, it marks the elements in which the `problems` lie, and keeps the
`references`.

### The outline and the labels

The messages `hello` and `update` describe the document that the output shows: the run of
the build that produced it knows its headings and its labels. A build of a build type that
writes no file (such as `check`) also processes the document, and its headings and labels
are given, although no page shows them.

`outline` is an array of the headings, in document order, each an object:

| Field | Value |
|---|---|
| `level` | the heading's level, a number: the smaller the number, the outer the heading (`\section` is 1, `\subsection` 2) |
| `number` | the heading's number as plain text, without a word before it, such as `"2.1"`; `null` for a heading that the document does not number |
| `title` | the title that a table of contents shows for the heading, as plain text |
| `id` | the `id` of the heading's element in the page; `null` when the heading has none |

`labels` is an array of the labels, in the document order of what they refer to, the
labels of one element in alphabetical order, each an object:

| Field | Value |
|---|---|
| `label` | the label, as written, such as `"thm:main"` |
| `designation` | what a reference to the label shows, as plain text: the number with the word before it (`"Thm. 1"`), else the content that the element shows (a heading's title), else the label itself. A tie `~` or a `\ ` is written as one space |
| `id` | the `id` of the element that the label refers to; `null` when it has none |
| `kind` | the kind of what the label refers to, such as `"section"`, `"equation"`, `"theorem"` or `"defterm"` (the kind of its reference point; the kinds are an open set); `null` when the build does not give it |
| `heading` | the heading the label stands under: the position in `outline`, counted from 0, of the last heading at or before what the label refers to in document order, so that a heading's own label stands under that heading; `null` for a label before the first heading |

A field that the build type cannot give is `null`, with no error: a build type whose
build does not number the document gives no `number`, and one whose build gives no element
an identifier gives no `id`.

### The viewer settings

The *viewer settings* say how the author wants the preview shown, whatever the document.
The `flm` server keeps them in the author's file `.flm/watch.json` of the home folder,
which it reads when it starts, and which [`POST
/.flm-preview/viewer`](#changing-the-viewer-settings) changes; a program that runs the
server without a home folder keeps them in memory. The field `viewer` of `hello`, and the
same field of the message `viewer`, which the server sends to every client after each
change, give them as one object; the message `viewer` also gives the `editors` of `hello`
again, since a change reads the file again and its editor command can have changed:

| Field | Value |
|---|---|
| `editor` | the `name` of the editor in which a position opens, one of the available `editors` given with it, or `null` when there is none |
| `theme` | the tools page's theme: `"system"` (light or dark as the system is set), `"light"` or `"dark"` |
| `scroll_to_changes` | `true` when the page scrolls to what a build changed and flashes it ([scroll to changes](#scroll-to-changes)) |
| `open_problems_on_errors` | `true` when the tools page opens its panel on the *Problems* tab when a build has at least one error and the panel is closed |
| `wrap_lines` | `true` when the tools page wraps the long lines of a text output |
| `panel_side` | the edge of the tools page at which its panel is docked: `"right"` or `"bottom"` |
| `panel_width`, `panel_height` | the panel's width at the right and its height at the bottom, in whole pixels, or `null` for the tools page's own |
| `unreadable` | one line that says why the file of the viewer settings cannot be read, or `null`. While it is not `null`, the other fields are the defaults and every change is refused |

The defaults are: the editor that the server chooses, `"system"`, `true`, `false`, `true`,
`"right"`, `null` and `null`. The editor is the one the author chose for the run on the
`flm watch` command line (`--editor`), until a client chooses another; else the one of the
file; else the editor command, when the author gave one; else the first editor found. The
editor command (the key `editor_command` of the file, or `flm watch --editor-command`) is
never sent and never given by a request: a client can only choose `"custom"`. When it does
and the file has no editor command (the key is absent or its value empty), the server writes
the placeholder `"echo {file}:{line}:{column}"` as the editor command, which opens nothing,
for the author to replace.

### `building`

The message `building` reports that a build has started. Its data is
`{"changed": [PATH, …]}`, the paths of the files whose change caused the build. The array
can be empty, for instance for the first build. The page script does not read it; the
tools page reads that a build runs.

### `progress`

The message `progress` reports how far the build under way has gone. Its data is
`{"text": TEXT}`: the text of the build's progress line, the percentage of the build done,
a whole number followed by `%`, then what is being done, as the `flm` program shows it
while it builds (`42% Converting graphics figure-3.pdf`, `60% Fetching citation data
3/12`). The percentage is an estimate; it never decreases during a build, and stays below
100 until the build has ended. The server reads the progress of the build four times a
second, and sends the message when the text differs from the last one sent for that build;
a stream opened while a build runs receives the last text sent for that build right after
`hello`. It sends none between a build's `update` and the next `building`, so a `progress` message
always concerns the build that the last `building` reported. The text is written for a
person and is not a stable interface.

The page script does not read it. The tools page shows the text in its status bar in place
of the words that say that a build runs, until the next `building` or `update`.

### `update`

The message `update` reports that a build has finished, and what changed in the output:

```json
{
  "build": 3,
  "page": "regions",
  "regions": [{"id": "flm-region-content", "name": "content", "kind": "html",
               "text": "<p>…</p>"}],
  "files": ["plot.png"],
  "problems": [{"kind": "diagnostic", "severity": "error", "id": "flm.refs.unresolved",
                "data": {…}, "message": "…",
                "place": {"file": "paper.flm", "offset": 262, "line": 12, "column": 5,
                          "end": {"offset": 273, "line": 12, "column": 16}},
                "frames": [], "bytes": {"start": 230, "end": 241},
                "excerpt": {"line": 12, "before": "See ", "covered": "\\ref{sec:x}",
                            "after": "."}}],
  "references": {"sec-intro": "sec:intro"},
  "outline": [{"level": 1, "number": "1", "title": "Introduction", "id": "sec-intro"}],
  "labels": [{"label": "sec:intro", "designation": "§1", "id": "sec-intro", "kind": "section", "heading": 0}],
  "media_type": "text/html",
  "earlier_output": false,
  "duration_ms": 182,
  "profile": "html",
  "default_profile": "html",
  "profiles": ["html", "pdf-latex", "text"],
  "environment": "5f0c2a9e41d7b3c8",
  "settings": 3
}
```

| Field | Value |
|---|---|
| `build` | the number of the build |
| `page` | how the page must change to show the output of this build: `"same"`, `"regions"`, `"reload"` or `"absent"` (see below) |
| `regions` | when `page` is `"regions"`, the output regions that changed; an empty array otherwise |
| `files` | the paths of the files of the output, other than the page, that changed, or that were added or removed, since the last output |
| `problems` | the problems of this build ([the problems](#the-problems)) |
| `references` | the labels of the elements of the page that the server has after this build, in the form of the field `references` of [`hello`](#hello): when this build did not hand over its output, those of the output that the server kept |
| `outline` | the headings of the document that the output shows, as in [`hello`](#hello), after this build in the same way |
| `labels` | the labels of the document that the output shows, as in [`hello`](#hello), after this build in the same way |
| `media_type` | the media type of the file that the server serves at `/` after this build, as in [`hello`](#hello) |
| `earlier_output` | `true` when that output was produced by an earlier build, because this build did not hand over its output; `false` otherwise, as in [`hello`](#hello) |
| `duration_ms` | how long this build took, in whole milliseconds |
| `profile` | the build type in use, as in [`hello`](#hello) |
| `default_profile` | the default build type, as in [`hello`](#hello) |
| `profiles` | the build types available, as in [`hello`](#hello) |
| `environment` | the digest of the document's environment, as in [`hello`](#hello) |
| `settings` | the counter of the settings state, as in [`hello`](#hello) |

The values of `page` have the following meanings:

- `"same"`: the page is unchanged.
- `"regions"`: the output regions in `regions` changed, and the rest of the page is
  unchanged.
- `"reload"`: the page must be reloaded. The file served at `/` of an output that has no
  output regions, such as a plain text, is reloaded whenever it changed; it is then also
  listed in `files`.
- `"absent"`: the build did not hand over its output (it stopped at the preparation,
  failed as a program, or its output could not be produced), and the page stays as it is.
  A build of a build type that writes no file hands over an output without files: its
  `page` is `"same"` when the output before it was also an output without files,
  `"reload"` otherwise.

Each entry of `regions` is an object with the following fields:

- `id`: the `id` of the region's element.
- `name`: the name of the region, such as `content` or `title`.
- `kind`: `"html"` when `text` is the HTML content of the region's element, or `"words"`
  when `text` is plain text. Only the page's title, the region named `title`, has the kind
  `"words"`.
- `text`: the new content of the region.

When it receives `update`, the page script changes the page as follows:

1. On a waiting page, the page script reloads the page, unless `page` is `"absent"`.
2. If `page` is `"reload"`, the page script reloads the page.
3. If `page` is `"regions"`, the page script updates each region in turn. It updates a
   region of kind `"html"` in place ([updating an output region in
   place](#updating-an-output-region-in-place)), and sets the title of the page to the
   text of the region `title`. If the page has no element with the `id` of a region, or if
   a region of kind `"words"` is not named `title`, the page script reloads the page.
4. Unless `page` is `"absent"`, the page script then reloads each file of `files` that the
   page uses. It looks for the elements whose address is that of the file: the `src` of
   `img`, `source`, `video`, `audio`, `iframe` and `embed` elements, the `href` of `link`
   elements and the `data` of `object` elements. In the address of each such element, it
   replaces the query with `?flm-preview-build=BUILD`, where `BUILD` is the number of the
   build, so that the browser requests the file again.

Then the page script marks the elements in which the problems of the build lie ([the
problems](#the-problems)). If `references` is an object, the page script replaces the
labels that it has with this object. If `references` is `null`, it keeps the labels that
it has. Last, after an update of output regions in place, it shows the changed elements
when the viewer settings ask for it ([scroll to changes](#scroll-to-changes)).

## Requests to the server

The server answers thirteen requests, at addresses under `/.flm-preview/`, beside the
requests for the files of [the tools page](#the-tools-page):

| Request | Purpose |
|---|---|
| `POST /.flm-preview/rebuild` | build the document again |
| `POST /.flm-preview/open` | open a position of a source file in an editor |
| `POST /.flm-preview/viewer` | [change the viewer settings](#changing-the-viewer-settings) |
| `POST /.flm-preview/profile` | change the build type |
| `GET /.flm-preview/at?file=…&start=…` | read [the constructs at a place](#the-constructs-at-a-place) |
| `GET /.flm-preview/help/index` | read the index of [the help pages](#the-help-pages) |
| `GET /.flm-preview/help/page?path=…` | read [one help page](#the-help-pages) |
| `GET /.flm-preview/settings` | read the [settings state](#reading-the-settings-state) |
| `GET /.flm-preview/settings/yaml?path=…` | read [a value as YAML](#reading-a-value-as-yaml) |
| `POST /.flm-preview/front-matter` | [change the front matter](#changing-the-front-matter) |
| `GET /.flm-preview/export?profile=…` | read the name and the command lines of [an export](#exporting-the-document) |
| `POST /.flm-preview/export` | [export the document](#exporting-the-document) |
| `POST /.flm-preview/export/open` | open the [exported](#exporting-the-document) main file outside the browser |

Each `POST` request changes something on the author's machine. With each `POST` request,
the browser sends the header `Origin`, which contains the origin of the page that sent the
request. A server should carry out a `POST` request only when its `Origin` is the server's
own origin, or when the request has no `Origin` header. A page of another site, open in
the same browser, can send a request to any address of the machine, but it cannot set the
header `Origin`. A request without `Origin` comes from a program rather than from a page
in a browser.

**Responses to failed requests.** A status code from 200 to 299 is a success. For any
other status code, the body of the response should be one line of plain text that says
what failed, which the client shows to the author as it is. One answer adds lines after
it: a failed export carries its build's log after a blank line
([exporting the document](#exporting-the-document)). When the page script's request
to open a position fails, it writes that line to the browser's console and sends it to the
tools page ([`open-failed`](#messages-to-the-tools-page)), which shows it in its status
bar, as it shows the line of its own failed requests. If the body is empty, it uses a
sentence with the status code instead, and if the server cannot be reached, a sentence
that says so.

### Building again

`POST /.flm-preview/rebuild`, with no body, asks the server to build the document again.
The server reports this build through the stream of messages, with `building` and
`update`, like any other build. A failed response's line of text is shown as
[responses to failed requests](#requests-to-the-server) says: the tools page shows it
in its status bar.

### Changing the build type

The *build type* is the profile that the builds run, which decides the outputs of the
document: `html`, `text`, `pdf-latex`, and others. `POST /.flm-preview/profile` asks the
server to build the document with another profile from now on. The body of the request is
a JSON object, sent with the media type `application/json`:

```json
{"profile": "text"}
```

`profile` is a name that is not empty, normally one of the `profiles` of
[`hello`](#hello) and [`update`](#update). The server answers `204` and builds the
document again with that profile, which produces every output of the profile's workflow.
It reports this build through the stream of messages, like any other build, and its
`update` carries the new `profile`. The choice lasts while the server runs; it is not
written into the document.

If the document cannot be prepared with that profile (for instance, a profile that no
package declares), the build stops at the preparation. Its `update` reports the problems
of the preparation, `page` is `"absent"`, and the server keeps the output of the last
build that handed one over, with `earlier_output` set to `true`. The `update` of that
build carries the name asked for as `profile`, and the following builds keep running that
profile until another `POST /.flm-preview/profile`: a build succeeds once the cause is
repaired. A body that is not such an object, whose `profile` is missing, empty or not a
text, or that is longer than 4 KiB (4096 bytes), gets the status `400`, with one line of
text that says what the body must be.

### Opening a position in an editor

`POST /.flm-preview/open` asks the server to open a position of a source file in an
editor on the author's machine. The page script sends this request when the author clicks
an element of the page with Ctrl or Cmd and Shift. The position is that of the element's
`data-flm-source-span`. If the element has no such attribute, the page script uses the
attribute of the nearest preceding or enclosing element that has one. The clicked element
flashes once. A click with Ctrl or Cmd alone is left to the browser.

The body of the request is a JSON object, sent with the media type `application/json`, in
one of two forms:

```json
{"file": "paper.flm", "start": 230}
{"file": "paper.flm", "line": 2, "column": 8}
```

- `file` is the name of the source file, and `start` a byte offset into it, as in
  `data-flm-source-span` and in the problems' `bytes`. The server computes the line and the
  column from `start`.
- `line` and `column`, both counted from 1, the column in bytes, are those of the `place`
  of a problem that has no `offset` (a problem in the file's front matter, [the
  problems](#the-problems)).

The body has no other field: the request names no editor. The server opens the `editor` of
the [viewer settings](#the-viewer-settings), on a file that the last build read, and, for
`start`, a source of the build whose output it serves. It answers:

- `204` once the editor has started;
- `200` when the place is one to open but no editor can be opened (there is none, or the
  one chosen cannot be started), with the media type `application/json` and the body
  `{"place": "PATH:LINE:COLUMN", "reason": TEXT}`: the place, with the file's absolute
  path on the author's machine and the column in characters, and one line that says why no
  editor opened. Each part of the browser copies `place` to the clipboard for its own
  gesture: the tools page for *Open in editor*; the page script for a click in the
  document, in the tools page's frame and at `/` alone, and it writes the place and the
  reason to the browser's console and tells the tools page whether it copied the place,
  with the message [`unopened`](#messages-to-the-tools-page). The tools page says so with
  `reason` in its status bar, as a notice when the place was copied, and as a failure that
  also gives the place when it was not;
- `400` for a body that is not one of these objects or that is longer than 64 KiB; `403`
  for a file that the last build did not read or, for `start`, that is not a source of the
  output served; `404` for a `start` past the end of the file; `500` for a file that names
  no file on the machine; each with one line of plain text.

### Changing the viewer settings

`POST /.flm-preview/viewer` changes the [viewer settings](#the-viewer-settings). The body
of the request is a JSON object of the settings to change, each with its new value, or
`null` for its default, sent with the media type `application/json`:

```json
{"panel_side": "bottom", "panel_height": 280}
```

The fields are those of `viewer` but `unreadable`; `editor` is the `name` of one of the
available `editors` that the client last received. The server writes the fields into the file of the
viewer settings as it is at that moment, keeping every other key of the file, answers
`204`, and sends the message `viewer`, with all the settings and the editors, to every
client. A body that names `editor_command`, a field that is not a viewer setting, a value
of the wrong kind or an editor that is not one of the available `editors`, a body that is not a JSON
object, or a body longer than 4 KiB gets the status `400`; while the file cannot be read
(`unreadable`), a change gets `409` with the reason, and the file is left as it is; a file
that cannot be written gets `500`; each with one line of plain text. Nothing is changed in
any of these cases.

### The constructs at a place

A *construct* is a command, an environment or a special character sequence (such as `~`)
written in a file of the document. `GET /.flm-preview/at?file=NAME&start=OFFSET` gives the
constructs at a place of the document that the output shows: `NAME` is the name of a source
file and `OFFSET` a byte offset into it, as in `data-flm-source-span` and in the problems,
each percent-encoded. The response is a JSON object:

```json
{"file": "paper.flm", "start": 230, "line": 12, "column": 5,
 "constructs": [{"kind": "macro", "name": "textbf", "file": "paper.flm",
                 "start": 226, "end": 239, "feature": "textstyles",
                 "help": "features/textstyles/macros/textbf"},
                {"kind": "environment", "name": "theorem", "file": "paper.flm",
                 "start": 180, "end": 300, "feature": "theorems",
                 "help": "features/theorems/environments/theorem"}]}
```

| Field | Value |
|---|---|
| `file`, `start` | the place asked for |
| `line`, `column` | the line and the column of the place, each counted from 1 at the top of the file, as an editor counts them: the column counts characters |
| `constructs` | the constructs that contain the place (each starts at or before it and ends after it), from the innermost outward; an empty array when there is none |

Each construct is an object:

| Field | Value |
|---|---|
| `kind` | `"macro"`, `"environment"` or `"specials"` (a special character sequence) |
| `name` | the name the construct is written with, without the backslash (`"textbf"`, `"theorem"`), or the characters of a special character sequence (`"~"`) |
| `file`, `start`, `end` | the file, and the byte offsets of the start of the construct and just past its end, counted as in `data-flm-source-span` |
| `feature` | the name of the feature that declares the construct, or `null` for a command or an environment that no feature declares |
| `help` | the path of the construct's [help page](#the-help-pages), for the environment of the last successful preparation of the document; `null` when it has none, and when the program gives no help pages |

A use of a user definition is a construct like any other. The content it makes carries the
place of the use in the page, so a place in that content answers with the use; a place in
one of its arguments answers with the constructs written there, then the use.

The response is `400` for a query whose `file` or `start` is missing, given twice, or not
valid, and `404` when no build has produced an output yet, when the document that the
output shows has no source of that name, or when the offset is past the end of the file;
each with one line of text.

### The help pages

The *help pages* are the pages of documentation of the document's environment: its
packages, its features, the commands and environments they declare, their settings, the
workflows, and the other names of the environment. The `flm` program gives the pages of `flm
info -d DOCUMENT`; another program that runs the server may give none, which `hello` tells
(`help`). The pages are those of the environment of the last successful preparation of the
document, the environment whose digest `environment` gives.

Each page has a *path*, such as `features/headings/macros/section`. A path is the
program's: a client reads paths from the index, from a page's links, from a page's
`path_line` and from the constructs at a place, never makes one, and does not read a
path's form, since a name inside a path may itself hold `/`. A path is not
percent-encoded, and may hold `/`, `#` and `\`; a client percent-encodes it in a query.

`GET /.flm-preview/help/index` gives the index of the pages, a JSON object:

```json
{"environment": "3f2a9c0d81b4e675",
 "pages": [{"path": "features/headings/macros/section", "kind": "command",
            "title": "Macro \\section", "summary": "A section heading.",
            "names": ["\\section", "section"]}]}
```

| Field | Value |
|---|---|
| `environment` | the digest of the environment that the pages describe, as `environment` in [`hello`](#hello) |
| `pages` | every page, in the order of the tree of the pages: a page, then the pages below it |

Each page of the index is an object:

| Field | Value |
|---|---|
| `path` | the page's path |
| `kind` | what the page shows (below) |
| `title` | the page's title, as plain text, such as `"Macro \\section"` or `"Feature headings"` |
| `summary` | a short text that says what the page shows, as plain text; `""` when the page has none |
| `names` | the names that the page is looked up by, each once: a macro's name with and without the backslash (`"\\section"`, `"section"`); an environment's name; the characters of a special character sequence; each key of a feature's settings as the feature's name, a dot and the key (`"html.title"`), on the page of that feature's settings; the name of the feature, package, workflow or other name of the environment that a page describes. An empty array for a page that no name finds, such as the list of the features |

The kinds of pages: `root` (`/`, the environment at a glance); `package-index` and
`package`; `feature-index` and `feature`; `command-index` and `command` (a macro, an
environment or a special character sequence); `scoped-package-index` and
`scoped-package` (the commands that a feature defines inside certain environments or
arguments only); `settings-index`, `settings` (the settings of one feature),
`parsing-settings` and `front-matter`; `workflow-index` and `workflow`;
`namespace-index`, `namespace` and `entity` (the families of the environment's internal
names, such as its stages and tables, and one name of such a family); and `program`,
`program-paths` and `program-extensions` (the program, the folders it reads, and the
packages and extensions a document may use).

`GET /.flm-preview/help/page?path=PATH` gives one page, a JSON object:

```json
{"path": "settings/headings", "nearest": [],
 "page": {"path": "settings/headings", "kind": "settings", "title": "Settings of headings",
          "content": "<div …>…</div>", "settings_feature": "headings",
          "path_line": [{"text": "settings", "path": "settings/"},
                        {"text": "headings", "path": "settings/headings"}]}}
```

| Field | Value |
|---|---|
| `path` | the path asked for |
| `page` | the page, or `null` when no page has that path |
| `nearest` | when `page` is `null`, the pages whose paths are nearest to the path asked for, nearest first, at most five, each an object `{"path": PATH, "title": TITLE}`; an empty array otherwise |

The page is an object:

| Field | Value |
|---|---|
| `path`, `kind`, `title` | as in the index |
| `content` | the page's content, as HTML without a document around it, to be placed inside an element of the tools page. A link to another page is an `a` element whose `data-flm-kind` is `"flmdoc.page"` and whose `href` is the other page's path followed by `.html`, such as `href="features/headings.html"` |
| `settings_feature` | the name of the feature, for the page of a feature and for the page of a feature's settings; `null` otherwise. The tools page shows the link *Settings of this feature* for it |
| `path_line` | the page's levels, from its section down to the page itself, each an object `{"text": TEXT, "path": PATH}`: the part that the level adds to the path (a fixed word such as `"features"` or `"macros"`, a whole name, or a command as its page's title writes it, such as `"\\ref"`), and the path of the page at that level. The line of `features/refs/macros/ref` is `features`, `refs`, `macros`, `\ref`; an empty array for `/` |

The response is `400` for a query whose `path` is missing, given twice, or not valid;
`501` when the program gives no help pages; `404` when no preparation of the document has
succeeded yet; and `500` when the index or the page cannot be made; each with one line of
text.

### Reading the settings state

The *settings state* is what the tools page shows of the document's settings: what the
front matter (the YAML block at the top of the document's file) writes, the values in
effect with where each comes from, and the choices that the document's environment
offers. `GET /.flm-preview/settings` reads it. The server makes it from the preparations
of the document (the step of a build that reads the front matter and the packages) and
from the document's file as it is at the request. The response is a JSON object:

| Field | Value |
|---|---|
| `revision` | the revision of the front matter: a digest of its text, as 16 hexadecimal digits, which changes whenever the text changes (a file without a front matter has one too); `null` when the front matter cannot be read |
| `unreadable` | `null`; or, when the front matter cannot be read (it is not valid YAML, for instance while the author edits it), the reason, one line of text that begins with the file's path and the place, such as `"paper.flm:3:1: …"`. The tools page then disables the controls that write the front matter and shows this line |
| `front_matter` | what the front matter writes, as data: the whole front matter, the metadata and `flm` alike; `null` for a file without a front matter, and when the front matter cannot be read |
| `schema` | the JSON Schema of a front matter for the packages that the document uses, in the form that the command `flm schema --of front-matter DOCUMENT` prints (below) |
| `in_effect` | the values in effect, each with its origin (below); `null` before the first successful preparation |
| `choices` | the choices that depend on the document's environment (below); `null` before the first successful preparation, except after a preparation that failed when its packages can be found: the choices then hold `packages`, `packages_in_use` and `math_processors` only |
| `preparation_failed` | `null` when the last preparation succeeded, and before the first; or, when it failed, why: the program's text of each problem that stopped it, one line per problem, after the file's path and the place when they are known |
| `schema_current` | whether `schema` is that of the document's packages as the last build found them; `false` only after a preparation that failed when the packages themselves could not be found |

The state holds nothing about how a page presents the settings.

**After a preparation that failed.** A preparation that succeeded gives the schema, the
values in effect and the choices of the document as it is. After one that failed, the
server looks for the document's packages from the keys of `flm` that decide them (`use`,
`initial_use`, `math`, `default_profile`), as `flm schema --of front-matter` does. When it
finds them, `schema`, and the choices `packages`, `packages_in_use` and
`math_processors`, are those of these packages, and `schema_current` is `true`. The values
in effect, and the choices `templates`, `outputs` and `profiles`, stay those of the last
successful preparation (before the first, the values in effect are `null`, and the choices
hold `packages`, `packages_in_use` and `math_processors` only). When the packages cannot be found either (a package listed under
`use` does not exist, for instance), the schema and every choice stay those of the last
successful preparation, and `schema_current` is `false`. Either way the stream's counter of the settings state
(`settings`) changes.

**The schema.** The features are the properties of
`properties.flm.properties.features.properties`, in their order. The value of each such
property is an object with an array `anyOf`, whose first entry that has no `const` and
whose `type` is not `"null"` is the schema of the feature's entry: either the schema
itself, or a reference `{"$ref": "#/$defs/NAME"}` to an entry of the top-level `$defs` of
the schema. The entry's first property is `enabled`, the switch of the feature. Before the
first successful preparation, the schema is the one the server started with (for `flm
watch`, the schema for the packages compiled into the program).

**An origin** says where a value comes from. It is an object:

| Field | Value |
|---|---|
| `source` | the kind of source: `"defaults"` (the program's defaults), `"package"` (a package of an extension the document uses), `"front_matter"`, `"workflow"` (the settings that a package gives the workflow of the build type), or `"command_line"` (a setting of the command line, or of the preview itself, such as `html.source_spans`) |
| `text` | the program's text for the origin, which a page shows as it is: for instance "the program's defaults", "the option \`-O toc.depth=2\`", a place such as "paper.flm:5:7", or, for a key of `flm` itself that the command line gives, the words of its option, such as "the option \`--use\`" |
| `file`, `line`, `column` | the file where the value is written, and the place there, each counted from 1, the column in bytes; left out when not known |
| `package`, `extension` | for `"package"` and `"workflow"`: the package's name (`IDENTITY#PACKAGE`) and the extension's stem (the name that `use` gives it) |
| `workflow` | for `"workflow"`: the workflow's name |

A source ranks above the sources before it in the order of the table: the value of the
highest source that writes a key is in effect. A source of the kind `"workflow"` or
`"command_line"` ranks above the front matter, so a change of the front matter does not
change the value in effect.

**The values in effect** are an object with two fields:

- `features`: an object that maps the name of each feature that a source writes to an
  object `{"switched_off": ORIGIN, "keys": KEYS}`. `switched_off` is the origin of the
  source that switches the feature off, or `null` when the feature is on. `KEYS` maps each
  key that a source writes to `{"value": VALUE, "origin": ORIGIN}`, and, for a key whose
  value is combined key by key (a key marked `merge-object-key-values`, such as
  `floats.types` or `compose.template_config`), a field `entries` that gives the origin of
  each value below it (below). A key that no source writes is absent: the default of its
  schema applies. A feature switched off has no keys in effect.
- `flm`: the keys of `flm` itself, each `{"value": VALUE, "origin": ORIGIN}`:
  `initial_use` (a list of names), `use` (an array of such objects, one per package: the
  front matter's entries, then those of the command line's `--use`), `math` (a name, or
  `false` for no processor), `default_profile` (a name: the front matter's, else the
  program's default; the build type in use is the stream's `profile`), and `parsing` (an
  object that maps each parsing key that a source writes to such an object).

**The origins below a key.** The field `entries` maps each key of the value that a source
writes to `{"origin": ORIGIN}`, the origin of the write that gives the value of that key.
When the value of that key is itself combined key by key, the object has a field `entries`
of the same form, and so on at every depth. For instance, a package that writes the
setting `accent` of the compose template `base` and a front matter that writes its setting
`note` give, under `compose.template_config`, `{"html": {"origin": …, "entries": {"base":
{"origin": …, "entries": {"accent": {"origin": PACKAGE}, "note": {"origin":
FRONT_MATTER}}}}}}`. The origin of the value at a path below a key is read down `entries`
along the path. A key that a map `entries` lacks is a value of the feature's own default
(such as a float type `figure` that no source writes): it has no origin. Where an object
has no field `entries`, its value is decided whole, and a value inside it has the origin of
that object (the key's `origin` for a key that is not combined key by key). A key that is
left out of the value (`false` under `remove-key-when-value-is-false`, `null` under
`set-to-default-with-null`) keeps its origin here. A key that a compose template does not
declare is absent from `entries`: the cascade leaves it out of the value with a warning,
and it has no origin. [The sources of a value](#the-sources-of-a-value) give every source's value
at one path.

**The choices** are an object:

| Field | Value |
|---|---|
| `templates` | an object that maps the name of an output to the compose templates registered for it, each `{"name": NAME, "package": PACKAGE, "config": CONFIG}`, `CONFIG` being the template's own settings with their defaults |
| `outputs` | the outputs of the build type of the last successful preparation, then the outputs that they are made from, each `{"name": NAME, "made_from": [NAME, …], "composed_as": NAME}`: `composed_as` is the output whose compose templates assemble it (the key of `compose.template` that chooses its template), or `null` for an output that no compose template assembles |
| `packages` | the packages found, in the order in which a name is looked up: the packages compiled into the program, the extensions on the search path, the extensions compiled into the program; each `{"name": NAME}`, with a field `description`, the description of the extension's manifest, for an extension that the document uses |
| `packages_in_use` | the packages that the document uses, each after the packages it uses, each name once: those of `initial_use`, the math processor's, those of `use`, those of the command line's `--use`, the build type's package, and every package that one of these uses. A package compiled into the program is named by its name; a package of an extension by each name through which the extension was found (`notes`, `@acme/notes`, a path), as an entry of `use` names it |
| `math_processors` | the names of the math processors among the packages found |
| `profiles` | the build types, as the stream's `profiles` |

A response other than `200` carries one line of text: `500` when the document's file
cannot be read.

### Reading a value as YAML

`GET /.flm-preview/settings/yaml?path=POINTER` gives the text that an editor of the value
at one path of the front matter starts from. `POINTER` is the path as a JSON Pointer (RFC
6901), percent-encoded: the keys from the root of the front matter, each after a `/`, `~`
written `~0` and `/` written `~1` in a key, and an index for an entry of a list, as in
`/flm/features/floats/types`. The response, of the media type `text/plain`, is the value
as YAML text, ending with a line end: the value that the front matter writes at the path,
else the value in effect, else the default of its schema; empty when there is none. A text
of several lines is written as a literal block when that block reads back as the same
text, and in double quotes, with escapes, otherwise (for example, a text whose first line
begins with a space, or that holds a carriage return).

The response is `400` for a parameter `path` that is missing, given twice, or not a JSON
Pointer (the text says which), `409` when the
front matter cannot be read (with its reason, as `unreadable` gives it), and `500` when
the document's file cannot be read; each with one line of text.

### The sources of a value

`GET /.flm-preview/settings/sources?path=POINTER` gives what the settings give at one path
of the front matter: the value that each source writes there, which one is in effect, what
is in effect as a result, and what a write there copies. `POINTER` is the path as a JSON
Pointer, as for [a value as YAML](#reading-a-value-as-yaml), such as
`/flm/features/compose/template_config/html/base/accent`. The response is a JSON object:

| Field | Value |
|---|---|
| `sources` | the values that the sources write at the path, lowest rank first and, within a rank, in the order written (a package after the packages it uses), each `{"value": VALUE, "origin": ORIGIN, "in_effect": BOOLEAN}`: `VALUE` the value as the source writes it (a path to a file as written, without the root that it receives), `ORIGIN` an [origin](#reading-the-settings-state), and `in_effect` whether this write decides the value at the path. A write decides it also when it leaves the key out (`false` in an object marked `remove-key-when-value-is-false`) or gives it its default (`null` under `set-to-default-with-null`); `value` and `default` below say which. No write is in effect at a place combined key by key, whose value is made of several writes, nor under a feature switched off. `null` for a path that does not lie under a feature's key (`/flm/use`, `/flm/parsing/…`, the metadata), and before the first successful preparation |
| `value`, `origin` | the value in effect at the path, as [the values in effect](#reading-the-settings-state) give it (a path to a file with its root), and its origin; both left out when no value is in effect there. `origin` alone is left out for a value of the feature's own default, which no source writes, and for a value that several keys make (`/flm/use`, `/flm/parsing`, a feature's entry `/flm/features/FEATURE`) |
| `default` | when no value is in effect at the path: the default that applies there. At a setting of a compose template, the compose template's default, as the choice `templates` gives it (a default that a template module computes included); elsewhere, the default of the schema. Left out when there is none, under a feature switched off, and when the write in effect leaves the key out without a default (a `false` that removes the key, a key that a compose template does not declare): no value is in effect then |
| `write_copies` | the values that a write of a value at the path copies into the front matter, each `{"path": [KEY, …], "value": VALUE}`, in the order written; empty when it copies none (see [Changing the front matter](#changing-the-front-matter)). A write that copies any is refused unless the last preparation succeeded. Below `compose.template`, a single name, the front matter's or a lower source's, gives a copy of that name for each other output that has a compose template |

The response is `400` for a parameter `path` that is missing, given twice, or not a JSON
Pointer, `409` when the front matter cannot be read (with its reason, as `unreadable`
gives it), and `500` when the document's file cannot be read; each with one line of text.

### Changing the front matter

`POST /.flm-preview/front-matter` makes a list of changes to the front matter of the
document's file, together. The body of the request is a JSON object, sent with the media
type `application/json`, of at most 4 MiB:

| Field | Value |
|---|---|
| `revision` | the revision of the front matter that the changes were made against, as the field `revision` of the [settings state](#reading-the-settings-state) gives it |
| `changes` | the changes, a list of JSON objects (below), made in the order given |

Each change is a JSON object that holds `path` and exactly one of `value`, `yaml` and
`remove`:

| Field | Value |
|---|---|
| `path` | the path of the value changed: an array of the keys from the root of the front matter, at least one, such as `["flm", "features", "citations", "enabled"]`. Each key but the last names a map |
| `value` | the value to set at the path, as JSON |
| `yaml` | the value to set at the path, as YAML text (such as the text that [a value as YAML](#reading-a-value-as-yaml) gave, edited) |
| `remove` | `true`: the value at the path is removed |

For example, `{"revision": "0123456789abcdef", "changes": [{"path": ["flm", "features",
"citations", "mark"], "value": "bracketed"}, {"path": ["flm", "features", "citations",
"style"], "remove": true}]}`.

- The changes are made in the order given, against the one revision; each change's path is
  read in the front matter as the changes before it left it. The file is written once, with
  all of them or none, and one build follows.
- A value set creates the maps that its path lacks (a key that holds `null` counts as
  lacking).
- A removal also removes each map of the path that it leaves empty: the removal of
  `["flm", "features", "toc", "depth"]` from a front matter whose `toc` holds `depth` alone
  removes `toc`, then `features` and `flm` if they hold nothing else. The removal of a value
  that the front matter does not hold changes nothing.
- The server edits the file in place: only the values that differ are written, and every
  other byte of the file stays as it was (comments, blank lines, quoting, the layout of each
  map and list, the document below the front matter). The file is read back before it is
  written, and the changes are refused unless the file then reads exactly as intended.
- A key that a map lacks is added at the end of the map, except in two maps, where the keys
  already written keep their order and the key added takes its place among them. In the
  root map, a key of the metadata goes after the last key other than `flm`, else before
  `flm`, and `flm` goes at the end. In `flm`, the keys are placed in the order
  `initial_use`, `math`, `use`, `default_profile`, `layout`, `parsing`, `features`,
  `feature_order`: a key added goes after the nearest key written that this order puts
  before it, else before the nearest one it puts after it.
- A feature is switched off and on again by the write and the removal of the key `enabled`
  of its entry (`["flm", "features", FEATURE, "enabled"]`), as any other value.
- **A write keeps every other value.** The settings decide some values whole: a key that is
  not combined key by key, such as `compose.template`, is given whole by the highest source
  that writes it, and an entry of a value combined key by key, such as a float type of
  `floats.types`, is given whole by the highest source that writes that entry, else by the
  feature's own default. A value set at a path inside such a value makes the front matter
  write that value, which then replaces the whole value that a lower source (the feature's
  own default, the program's defaults, a package) gives. When the front matter writes
  nothing there yet, the server copies into the front matter, in the same write, the other
  values of the value in effect along the path, each beside the value set, in the order of
  the keys: setting `["flm", "features", "compose", "template", "html"]` when a package
  writes `template: {html: simple, text: none}` also writes `text: none` into the front
  matter. A copied value is the value in effect, a path to a file with its root (such as
  `ext+mytemplate:logo.svg`), so that it names the same file from the front matter; from
  then on it is the front matter's, and does not follow the package. A copy is left to a
  change of the request when the copy's path is the path that the change sets or removes,
  lies inside that path, or holds it. A value set at a place combined key by key (a setting of a compose template, an
  entry of `floats.types` as a whole) copies nothing: the other values stay those of their
  sources.
- **A single name of `compose.template`.** `compose.template` holds either one name for
  every output or a map of a name per output. A value set at one output's name, when a
  lower source gives the single name in effect and the front matter writes nothing there,
  writes that name out for every other output that has a compose template (the outputs of
  the choice `templates`), as copies. When the front matter itself writes the single name,
  a value set or a removal at one output's name is made whatever source is in effect: the
  server replaces the single name with the map in the
  same write, each other output keeping the name, and the output changed taking the value
  set, or no entry for a removal (no entry left removes the key). The names written are the
  front matter's from then on. An output that gains a compose template later has no entry
  in the map, and takes the default template. A change of `compose.template` itself is
  made as it is given, with no copy.
- **Only after a successful build.** The server adds copies, or the names written out, to
  a request's changes only from the state of a last preparation that succeeded. After a
  preparation that failed, and before the first, the values in effect are not those of the
  document as it is: a request that needs such an addition gets `409`, and nothing is
  written. A request that needs none is made in any state, so that the document can be
  mended from the page.
- [The sources of a value](#the-sources-of-a-value) give, for a path, the copies that a
  write there makes.

The server answers `200` once the file is written, with a JSON object whose field
`copied` lists the values copied, each `{"path": [KEY, …], "value": VALUE}`, in the order
written (empty when the write copies none); changes that leave the file as it is, and an
empty list of changes, write nothing. The change of the file causes a build, which
the server reports through the stream of messages like any other build; a value that a
feature refuses is a problem of that build, at its line of the front matter.

A refused request writes nothing of its changes. The response carries one line of text:

| Status | When |
|---|---|
| `400` | the body is not a JSON object of at most 4 MiB, its `revision` is missing or not a text, its `changes` is missing or not a list, an entry of `changes` is not an object that holds a `path` of at least one key (texts) and exactly one of `value`, `yaml` and `remove: true` (the text names the entry by its position, from 1), or the text of a `yaml` is not valid YAML |
| `412` | `revision` is not the revision of the front matter: the file changed since the page read the settings state. The page reads the settings state again |
| `409` | the front matter cannot be read (with its reason, as `unreadable` gives it); a key of a change's path but the last holds a value that is not a map (such as `false`, the shorthand of a feature switched off); a value set holds a text of several lines, or a map placed as an entry of a list written on several lines (`- …` lines), two kinds of value that are never written from the page; or the changes cannot be written exactly as the front matter is laid out (the text names the paths of the request's changes). The text begins with the file's path and, when known, the place in the file. Also: a change needs values copied, or names written out, and the last preparation did not succeed, or none has yet (the text names the path of that change and the file) |
| `500` | the document's file cannot be read or written |

### Exporting the document

An *export* writes the document as the command `flm render` writes it, for the author to
keep, send or print: the page that the preview shows cannot be saved as it is, since it
carries the places of its elements in the source and the page script. An export is a build
of its own, which the server carries out between two builds of the preview, with the
options that the program running the preview was started with and none of the preview's
own, and with the *build type* chosen for it (a profile, as in [changing the build
type](#changing-the-build-type)), which produces every output of the profile's workflow.
Its files are named as `flm render -o STEM.EXT` names them: `STEM` is the document's file
name without its extension, and `EXT` the extension of the main file of the build type's
*leading output*, the first output of its workflow. `STEM.EXT` is the export's *name*
(`paper.html`, `paper.pdf`); an output of several files writes its other files beside the
main file, with the names `flm render` gives them (`paper.tex` and
`paper-flmx.3fa9c2d1.sty`).

A request about an export waits for its answer while a build of the preview is under way,
and an export can take long (a LaTeX run). The server takes no path from the browser: an
export is always named and placed as above.

**The name and the command lines.** `GET /.flm-preview/export?profile=NAME`, where `NAME`
is a build type, percent-encoded, answers a JSON object:

```json
{"profile": "html", "name": "paper.html",
 "command": "flm render -p html -o paper.html paper.flm --view",
 "command_continuing":
   "flm render -p html --continue-with-errors -o paper.html paper.flm --view"}
```

| Field | Value |
|---|---|
| `profile` | the build type asked for |
| `name` | the export's name, `STEM.EXT` |
| `command` | the command line that writes the same files from a terminal, as the program running the preview composes it from its own options: `flm render -p TYPE [OPTIONS] -o STEM.EXT DOCUMENT --view`, with the document's folder before `STEM.EXT` when it has one; `null` when the program composes none |
| `command_continuing` | the same command line with `--continue-with-errors`, which writes the export even when the document has errors; `null` when the program composes none |

The server prepares the build type to answer, and keeps the preparation for the export
that follows. The answer is `400` for a query whose `profile` is missing, empty, given
twice or not valid, `409` when the build type cannot be prepared or declares no output
(such as `check`), and `503` once the preview has ended, with one line of text.

**The export.** `POST /.flm-preview/export` carries out an export. The body of the request
is a JSON object, sent with the media type `application/json`:

```json
{"profile": "pdf-latex", "action": "save", "continue_with_errors": false}
```

- `profile` is the build type, a name that is not empty.
- `action` is `"save"`, *Save as sibling*: the server writes the files into the document's
  folder, as `flm render -o` writes them, replacing the files of an earlier export of the
  same name; or `"download"`: the server writes nothing and answers the files, for the
  browser to save.
- `continue_with_errors` is `true` to write the export even when the document has errors,
  as `flm render --continue-with-errors` does, and `false` otherwise.

A save answers `200` with a JSON object: `name`, the export's name, and `files`, the path
of each file written, relative to the document's folder (`{"name": "paper.html", "files":
["paper.html"]}`). A download answers `200` with the export's one file as it is, with its
media type, or, when the export has several files, a zip archive of them named `STEM.zip`,
each file at its path, with the media type `application/zip`. Either carries the header
`Content-Disposition: attachment`, with the file's name as `filename*` (UTF-8,
percent-encoded).

**When nothing is written**, the first line of the answer is the reason. The answer is
`428` when the document has errors under the build type, found before any file is
produced or any external program runs, and `continue_with_errors` is `false`. This is the
one refusal that the same request with `continue_with_errors: true` lifts. The answer is
`409` when:

- the build type declares no output;
- a file of a save has the name of a file that the document's build reads (the document's
  own file, for a document named like its export);
- the build fails: the build type cannot be prepared, the build stops, it does not
  produce the leading output's main file, or, when `continue_with_errors` is `false`, a
  stage that writes files or runs an external program reports an error (such as a failed
  run of LaTeX for `pdf-latex`). Such an error leaves nothing written, even when the main
  file was produced.

For a refusal for errors (`428`, or `409` for a build that failed with errors), the lines
that follow the reason, after a blank line, are the *log*: every error of the export's
build, in full, as the command line reports it, with a blank line between two. The error
of a failed run of an external program quotes that program's log (for `pdf-latex`, the
first error that LaTeX's log records). The answer is `400` for a body that is not such an
object or that is longer than 4 KiB (4096 bytes); `500` when files of a save could not be
written or the archive of a download could not be made; and `503` once the preview has
ended. Each of these has one line of text.

**Opening the exported file.** `POST /.flm-preview/export/open`, with no body, asks the
server to open the main file of the last export saved (the main file of its leading
output) with the system's default application, as `flm render --view` does: outside the
browser. The request names no file. The server answers `204` once the application has
started; `404` when no export has been saved since the server started, `501` when the
program running the preview opens no file, and `500` when the file could not be opened,
each with one line of text.

## The messages between the tools page and the page script

When the tools page shows the document's page in its frame, the two parts exchange
messages with `window.postMessage`. Neither reads the other's document. Each message is a
JSON object whose field `channel` is `"flm-preview"` and whose field `type` names the
message; its other fields depend on the type. Each part sends its messages with the
target origin of the server, and accepts a message only when it comes from the other
part's window and from the server's origin: the page script, only from the window around
its own (the frame's parent); the tools page, only from its frame's window. A part
ignores any other message, and any message whose fields it does not understand.

The page script sends nothing until it has received `attach`. A page that is not in the
tools page's frame therefore sends no message.

### Messages to the page script

| `type` | Other fields | Meaning |
|---|---|---|
| `attach` | none | The tools page shows this page in its frame. The tools page sends it each time its frame has loaded a page, and the page script answers `loaded`. |
| `show-place` | `place`: `{"file": NAME, "start": START, "end": END}`, a part of a source file, counted as in `data-flm-source-span` | Scroll to the [elements of the part](#the-markup-that-the-client-reads) and flash each once. |
| `show-element` | `id`: the `id` of an element of the page | Scroll to the element and flash it once. |
| `inspect` | `on`: `true` or `false` | Turn [Inspect](#inspect) on or off. Either way, no element stays selected. |
| `inspect-place` | `place`: a part of a source file, as in `show-place` | While Inspect is on, select the element of the part ([Inspect](#inspect)), and report it with `inspected`. |

To show elements, the page script scrolls the first of them in the order of the page to
the middle of the window. A part or an `id` that designates no element of the page shows
nothing.

### Messages to the tools page

| `type` | Other fields | Meaning |
|---|---|---|
| `loaded` | `waiting`: `true` when the page is a [waiting page](#the-waiting-page) | The answer to `attach`: the page in the frame runs the page script. A page that loads again answers the next `attach` again. |
| `scrolled` | `x`, `y`: the page's scroll position, in CSS pixels | The page scrolled. The page script sends it at most once per frame that the browser draws, and only for a position other than the one at its last `loaded`, at its last `inspected` that carries a `rect`, or at its last `scrolled`: a scroll made before a selection is not reported after it. |
| `inspected` | `place`: the part of a source of the selected element, as in `show-place`; `rect`: `{"x", "y", "width", "height"}`, the element's rectangle in the frame's window, in CSS pixels; `label`: the label that refers to the element, or `null` | A click while Inspect is on, or the answer to `inspect-place` ([Inspect](#inspect)). When the click selected no element, `place` and `rect` are `null` too. For `inspect-place`, `place` is the part asked for, and `rect` and `label` are `null` when no element was selected. |
| `open-failed` | `text`: one line that says what failed | A request to open a position in an editor failed ([opening a position in an editor](#opening-a-position-in-an-editor)). |
| `unopened` | `place`, `reason`: the fields of the answer `200` of `POST open`; `copied`: whether the page script copied `place` to the clipboard | The server answered the place instead of opening an editor; the page script copied `place` to the clipboard, or could not, and the tools page says so with `reason` in its status bar, as a notice when copied and as a failure when not ([opening a position in an editor](#opening-a-position-in-an-editor)). |

### Inspect

While Inspect is on, the page script takes each click on the page before the page's own
handlers, and the browser does not act on it. The click selects the innermost element
around the clicked point, the clicked element included, that has a `data-flm-source-span`.
The selected element carries the attribute `data-flm-preview-inspected`, and the page
script sends `inspected` with its place, its rectangle and its label. The label is the one
of [`references`](#hello) for the element's `id`, else for the `id` of the nearest element
around it that has the same `data-flm-source-span` or none (such as the block of a theorem
around its paragraph); the search stops at the first element around it whose
`data-flm-source-span` differs. A click where no element around the clicked point has a
`data-flm-source-span` selects nothing. A click with Ctrl or Cmd and Shift is not taken: it
still opens the clicked position in an editor.

`inspect-place` selects the element of a part: among the elements whose
`data-flm-source-span` has the part's file and a range that contains the part's whole
range, those whose range is the narrowest, and the outermost of these. The page script
sends `inspected` with the part asked for as `place`, and the element's rectangle and
label; when no element contains the part, it selects nothing and sends `rect` and `label`
as `null`.

Turning Inspect on or off, and an update of the page's output regions, leave no element
selected.
