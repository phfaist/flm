//! The HTML sink: the only interface through which this library writes HTML
//! bytes.
//!
//! An [`HtmlSink`] holds the HTML being written. An emitter never holds the
//! output string: it writes through the sink, which has one entry point per
//! *kind of content*, and each entry point escapes what that kind of content
//! needs escaped. This is the whole of the HTML backend's escaping:
//!
//! - [`text`](HtmlSink::text) — document text, escaped
//!   ([`super::escape::escape_text_into`]);
//! - a **tag**: [`open_block`](HtmlSink::open_block) or
//!   [`open_inline`](HtmlSink::open_inline) hands back a [`TagBuilder`] whose
//!   [`attr`](TagBuilder::attr) escapes every value and always writes it
//!   inside double quotation marks, whose
//!   [`attr_json`](TagBuilder::attr_json) does the same inside single
//!   quotation marks for a JSON value, and
//!   [`close`](HtmlSink::close) closes the element the sink last opened;
//!   [`void_block`](HtmlSink::void_block) and
//!   [`void_inline`](HtmlSink::void_inline) write an element that has no
//!   content and is never closed, such as `<img>`;
//! - [`markup`](HtmlSink::markup) — a [`Markup`] value, HTML written through
//!   unchanged (that type's documentation states what its creator must
//!   ensure), and
//!   [`markup_block`](HtmlSink::markup_block), which writes such a value where
//!   a block-level element could be placed, on a line of its own;
//! - [`raw_text_element`](HtmlSink::raw_text_element) — a `<style>` or a
//!   `<script>`, the two elements whose content a browser reads as raw text
//!   and which therefore cannot be escaped at all; the entry point writes
//!   those two tags and no other, and refuses content that holds the
//!   element's own closing tag, since that and nothing else would end the
//!   element early.
//!
//! These last two are the only entry points that write bytes unescaped: a
//! [`Markup`] value, and the content of a `<style>` or a `<script>`.
//!
//! Text a document wrote reaches a page through `text`, `attr` and
//! `attr_json`, and through no other entry point. Each of the three escapes
//! what its place needs escaped, so text that reaches one of them cannot leave
//! it. No entry point takes an already-escaped string, and
//! nothing reads the buffer back: the sink offers no accessor to the bytes
//! written before [`finish`](HtmlSink::finish) takes them — its `Debug` form
//! shows how much has been written and which elements are open, never the
//! bytes — so no decision can depend on output and no escaping can run over
//! output. Escaping twice, or escaping inside an element whose content must not
//! be escaped, therefore cannot be written at all.
//!
//! The sink checks nothing about document input: it escapes what it is
//! given.
//! What must be checked before it is written — a tag or attribute name from a
//! construct node ([`is_valid_tag_name`]), an address
//! ([`check_address`](super::address::check_address)) — is checked by the
//! emitter, which can report the node it refused. Checking a name is the
//! caller's responsibility: every entry point taking one states that as its
//! precondition and checks it again in a debug build, where a name that fails
//! is a programming error in the calling code and not document input
//! ([panics](crate::guide::panics) lists these checks).
//!
//! # How the bytes are laid out
//!
//! The sink introduces a line break and two spaces of indentation per level
//! between **block-level** elements, and introduces nothing anywhere else.
//! Inline content is written exactly as it comes, because whitespace between
//! inline elements is text a reader sees; and there is no pass over the
//! finished output, which would mean reading it back.
//!
//! Whether an element is block-level is decided by the call that opened it —
//! [`open_block`](HtmlSink::open_block) against
//! [`open_inline`](HtmlSink::open_inline) — and never by its tag name. A
//! construct node's tag is chosen by its rule's author, and its payload states
//! which of the two it is (the `block` field of
//! [`HtmlTagPayload`](super::HtmlTagPayload)).
//!
//! A **preformatted region** — the content of `<pre>`, where every space and
//! line break is text the reader sees — is marked when the element is opened
//! ([`TagBuilder::preformatted`]) and nothing is introduced inside it.
//!
//! Only the innermost open element decides whether a position is in inline
//! flow, so a block-level element opened inside an inline one writes no line
//! break before itself, and its own children are broken and indented again.
//! The HTML emitter never produces that case, since a block-level element
//! inside an inline one is invalid HTML. A preformatted region, by contrast,
//! applies to everything inside it, however deeply nested: it is how a caller
//! asks for no whitespace anywhere inside an element.
//!
//! # Examples
//!
//! ```
//! use flm_core::stdcore::backends::html::HtmlSink;
//!
//! let mut sink = HtmlSink::new();
//! sink.open_block("div").class(["flm-pres-block"]).finish();
//! sink.open_block("p").class(["flm-pres-par"]).finish();
//! sink.text("Hello ");
//! sink.open_inline("em").finish();
//! sink.text("there");
//! sink.close();
//! sink.close();
//! sink.close();
//! assert_eq!(
//!     sink.finish(),
//!     "<div class=\"flm-pres-block\">\n  <p class=\"flm-pres-par\">Hello <em>there</em></p>\n</div>",
//! );
//! ```

use std::fmt;

use super::escape::{escape_attr_into, escape_attr_single_quoted_into, escape_text_into};
use super::markup::Markup;
use super::names::{is_valid_attribute_name, is_valid_tag_name};

/// What one level of block-level nesting is indented by.
const INDENT: &str = "  ";

/// The two elements whose content a browser reads as raw text: the only tags
/// [`HtmlSink::raw_text_element`] accepts.
const RAW_TEXT_TAGS: [&str; 2] = ["style", "script"];

/// An element the sink has opened and not yet closed.
#[derive(Debug, Clone)]
struct OpenElement {
    /// The tag name, for the closing tag.
    tag: String,
    /// Whether the element was opened as block-level.
    block: bool,
    /// Whether a block-level child was written inside it; if so, its closing
    /// tag goes on a line of its own.
    block_children: bool,
    /// Whether the element's content is a preformatted region.
    preformatted: bool,
}

/// The error [`HtmlSink::raw_text_element`] returns for content that holds the
/// closing tag of the element it would be written into.
///
/// The content of a `<style>` or a `<script>` is **raw text**: a browser reads
/// it character by character up to the element's closing tag and reads no
/// character reference on the way, so nothing in it can be escaped, and the
/// only sequence that must not appear in it is that closing tag (`</style` or
/// `</script`, in any letter case, followed by anything). Content holding one
/// would end the element early and leave the rest of it in the page as markup.
///
/// It reports a mistake in the definition or in the configuration that
/// supplied the content, never anything a document can write. The HTML
/// backend reports it as
/// [`flm.html.definition-not-inlinable`](super::defs::DefinitionNotInlinable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTextRefused {
    /// The element's tag name, as the call gave it.
    pub tag: String,
    /// The byte offset, in the content, of the `</` that begins the closing
    /// tag found there.
    pub at: usize,
}

impl fmt::Display for RawTextRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the content of a `<{}>` element holds its own closing tag at byte {}, which would \
             end the element there",
            self.tag, self.at
        )
    }
}

/// The byte offset of the first `</<tag>` in `content`, comparing the tag name
/// without regard to letter case; `None` when the content holds none.
///
/// What follows the name does not matter: a browser ends a raw text element at
/// `</` plus the name plus any character that can follow a tag name, and
/// refusing every `</name` keeps the rule one sentence long.
fn closing_tag_at(content: &str, tag: &str) -> Option<usize> {
    let needle_len = 2 + tag.len();
    content.match_indices("</").find_map(|(at, _)| {
        let name = content.get(at + 2..at + needle_len)?;
        name.eq_ignore_ascii_case(tag).then_some(at)
    })
}

/// The HTML sink: the only interface through which an HTML emitter writes
/// bytes (see [the module documentation](self)).
///
/// A sink is created empty, written into through its entry points, and handed
/// over as a string by [`finish`](HtmlSink::finish).
///
/// The bytes leave the sink through `finish` and by no other way: the `Debug`
/// form of a sink shows how many bytes have been written, how deep the
/// block-level nesting is and which elements are open, and never the bytes
/// themselves, so that no output can be read back through it — see [the module
/// documentation](self) for why that matters.
#[derive(Clone, Default)]
pub struct HtmlSink {
    out: String,
    open: Vec<OpenElement>,
    /// How many open elements were marked as preformatted regions.
    preformatted: usize,
    /// How many open elements are block-level, which is the indentation level
    /// of the next block-level element written.
    depth: usize,
}

impl fmt::Debug for HtmlSink {
    /// The sink's state — the bytes written so far counted, the block-level
    /// depth, the open elements by tag name and how many of them are
    /// preformatted regions — and never the bytes written.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HtmlSink")
            .field("bytes_written", &self.out.len())
            .field("depth", &self.depth)
            .field("open", &self.open.iter().map(|element| &element.tag).collect::<Vec<_>>())
            .field("preformatted", &self.preformatted)
            .finish()
    }
}

impl HtmlSink {
    /// An empty sink.
    pub fn new() -> HtmlSink {
        HtmlSink::default()
    }

    /// Writes document text, escaped
    /// ([`super::escape::escape_text_into`]).
    ///
    /// Text is escaped the same way everywhere, a preformatted region
    /// included: a preformatted region changes how a browser lays text out,
    /// not how it reads markup.
    pub fn text(&mut self, text: &str) {
        escape_text_into(&mut self.out, text);
    }

    /// Writes a [`Markup`] value: HTML that is written unchanged, without
    /// escaping.
    ///
    /// It is one of the two entry points that write bytes unescaped; the other
    /// is [`raw_text_element`](HtmlSink::raw_text_element), for the content of
    /// a `<style>` or a `<script>`. See [`Markup`] for what the creator of the
    /// value must ensure.
    pub fn markup(&mut self, markup: &Markup) {
        self.out.push_str(markup.as_str());
    }

    /// Writes a [`Markup`] value as **block-level** content: on a line of its
    /// own, at the indentation of the place where it is written, and with the
    /// closing tag of the enclosing element written on a line of its own too.
    ///
    /// This is how HTML that this library already wrote is placed inside other
    /// HTML: the page assembly writes each document tree's HTML into
    /// the page through it, so that the line break between two block-level
    /// tags is there at the join as it is everywhere else: after the opening
    /// tag of the element the trees are written into, between two trees, and
    /// before that element's closing tag.
    ///
    /// The bytes themselves are written unchanged. The
    /// indentation *inside* them is not touched: changing it would mean
    /// reading finished output back and rewriting it, which this sink offers
    /// no way to do. The line break and the indentation *before* them are
    /// written as the bytes are placed, as for every block-level element
    /// written through this sink.
    ///
    /// This is not a second path for unescaped bytes. What it writes is a
    /// [`Markup`] value, created under the requirement that type's
    /// documentation states, and this entry point changes nothing about that
    /// requirement: it decides where the bytes are placed and nothing about
    /// what they are. Everything [`markup`](HtmlSink::markup) says therefore
    /// holds here too.
    ///
    /// Nothing is introduced inside a preformatted region or in inline flow,
    /// exactly as when a block-level element is opened; the caller places the
    /// markup where a block-level element could be placed.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::html::{HtmlSink, Markup};
    ///
    /// let mut sink = HtmlSink::new();
    /// sink.open_block("main").finish();
    /// sink.markup_block(&Markup::trusted("<p>one</p>\n<p>two</p>"));
    /// sink.close();
    /// // The markup begins on a line of its own, at the indentation of the
    /// // place where it is written, and keeps its own line breaks inside.
    /// assert_eq!(sink.finish(), "<main>\n  <p>one</p>\n<p>two</p>\n</main>");
    /// ```
    pub fn markup_block(&mut self, markup: &Markup) {
        self.begin_block_position();
        self.markup(markup);
    }

    /// Opens a **block-level** element with the tag name `tag` and hands back
    /// the [`TagBuilder`] that writes its attributes.
    ///
    /// Block-level is what the line breaks and the indentation are written
    /// between (see [the module documentation](self)). The element stays open
    /// until [`close`](HtmlSink::close).
    ///
    /// # Preconditions
    ///
    /// The caller has validated `tag` with [`is_valid_tag_name`]; the tag
    /// name is then written unchanged. A name that fails that check is a
    /// programming error in the calling code, never document input, since a
    /// name that came from a [construct node](super::construct) is checked by
    /// the emitter, which reports the node it refused.
    ///
    /// # Panics
    ///
    /// *Debug builds only*: `tag` fails that check. A release build writes the
    /// name unchanged, which may produce markup the caller did not intend.
    pub fn open_block(&mut self, tag: &str) -> TagBuilder<'_> {
        self.open_element(tag, true)
    }

    /// Opens an **inline** element with the tag name `tag` and hands back the
    /// [`TagBuilder`] that writes its attributes.
    ///
    /// No whitespace is ever introduced around or inside an inline element:
    /// whitespace there is text a reader sees. The element stays open until
    /// [`close`](HtmlSink::close).
    ///
    /// # Preconditions
    ///
    /// The caller has validated `tag`, as for
    /// [`open_block`](HtmlSink::open_block).
    ///
    /// # Panics
    ///
    /// *Debug builds only*: `tag` fails that check.
    pub fn open_inline(&mut self, tag: &str) -> TagBuilder<'_> {
        self.open_element(tag, false)
    }

    /// Writes a block-level element that has no content and is never closed —
    /// `<hr>`, `<meta>` — and hands back the [`TagBuilder`] that writes its
    /// attributes.
    ///
    /// # Preconditions
    ///
    /// The caller has validated `tag`, as for
    /// [`open_block`](HtmlSink::open_block).
    ///
    /// # Panics
    ///
    /// *Debug builds only*: `tag` fails that check.
    pub fn void_block(&mut self, tag: &str) -> TagBuilder<'_> {
        self.void_element(tag, true)
    }

    /// Writes an inline element that has no content and is never closed —
    /// `<img>`, `<br>` — and hands back the [`TagBuilder`] that writes its
    /// attributes.
    ///
    /// # Preconditions
    ///
    /// The caller has validated `tag`, as for
    /// [`open_block`](HtmlSink::open_block).
    ///
    /// # Panics
    ///
    /// *Debug builds only*: `tag` fails that check.
    pub fn void_inline(&mut self, tag: &str) -> TagBuilder<'_> {
        self.void_element(tag, false)
    }

    /// Closes the element the sink opened last.
    ///
    /// Elements therefore close in the order they were opened, by
    /// construction: there is no way to name another one.
    ///
    /// # Panics
    ///
    /// When no element is open. That is a mistake in the emitter — it closed
    /// something it did not open — and never anything a document can cause.
    pub fn close(&mut self) {
        let element =
            self.open.pop().expect("the emitter closed an element the sink had not opened");
        if element.preformatted {
            self.preformatted -= 1;
        }
        if element.block {
            self.depth -= 1;
            if element.block_children {
                self.break_line();
            }
        }
        self.out.push_str("</");
        self.out.push_str(&element.tag);
        self.out.push('>');
    }

    /// Writes a **raw text element** — a `<style>` or a `<script>` — with the
    /// attributes given and `content` as its content, written byte for byte.
    ///
    /// A browser reads the content of such an element as raw text up to the
    /// element's closing tag, so nothing in it can be escaped: this is where
    /// the text of a style or script definition, and what a host put in its
    /// place, is written.
    ///
    /// `style` and `script` are the two elements HTML reads that way, and this
    /// entry point writes those two tags and no other: an element whose
    /// content a browser does read as markup — `<textarea>`, `<title>` — is
    /// written through [`open_block`](HtmlSink::open_block) or
    /// [`open_inline`](HtmlSink::open_inline) and [`text`](HtmlSink::text),
    /// which escape what they write.
    ///
    /// The attributes are written in the order given, each value escaped as
    /// any other attribute value.
    ///
    /// This is one of the two entry points that write bytes unescaped; the
    /// other is [`markup`](HtmlSink::markup).
    ///
    /// # Errors
    ///
    /// [`RawTextRefused`] when `content` holds the element's own closing tag,
    /// in any letter case, which would end the element there; nothing is
    /// written then.
    ///
    /// # Preconditions
    ///
    /// The content is written without escaping, so the caller must ensure that
    /// it does not come from a document or from any other input the caller does
    /// not control (this library passes only the text of style and script
    /// definitions here).
    ///
    /// The caller has validated every attribute name with
    /// [`is_valid_attribute_name`]; a name that fails that check is a
    /// programming error in the calling code, never document input.
    ///
    /// # Panics
    ///
    /// When `tag` is neither `style` nor `script`. The content of this element
    /// is written unescaped, so writing it into an element a browser parses
    /// some other way would put unescaped bytes where they are read as markup;
    /// that is a programming error in the calling code, and the method panics
    /// in every build rather than only in a debug build.
    ///
    /// *Debug builds only*: an attribute name fails the check above.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::html::HtmlSink;
    ///
    /// let mut sink = HtmlSink::new();
    /// sink.raw_text_element("style", &[("media", "screen")], ".flm-pres-par { margin: 0 }\n")
    ///     .expect("the stylesheet holds no closing tag");
    /// assert_eq!(
    ///     sink.finish(),
    ///     "<style media=\"screen\">.flm-pres-par { margin: 0 }\n</style>",
    /// );
    /// ```
    pub fn raw_text_element(
        &mut self,
        tag: &str,
        attributes: &[(&str, &str)],
        content: &str,
    ) -> Result<(), RawTextRefused> {
        assert!(
            RAW_TEXT_TAGS.contains(&tag),
            "a raw text element is a <style> or a <script>, not a <{tag}>: the content of any \
             other element is escaped where it is written"
        );
        if let Some(at) = closing_tag_at(content, tag) {
            return Err(RawTextRefused { tag: tag.to_string(), at });
        }
        self.begin_block_position();
        self.out.push('<');
        self.out.push_str(tag);
        for (name, value) in attributes {
            self.write_attribute(name, value);
        }
        self.out.push('>');
        // The content enters the page unescaped, as this element demands, and
        // it does so through the one door for unescaped bytes.
        self.markup(&Markup::from_library(content));
        self.out.push_str("</");
        self.out.push_str(tag);
        self.out.push('>');
        Ok(())
    }

    /// How many bytes have been written so far.
    ///
    /// This is a count, not content: the sink still offers no way to read what
    /// it wrote, so no decision can depend on the bytes and no escaping can run
    /// over them a second time. The count exists because an emitter has to be
    /// able to say *where* in its output something was written — the HTML
    /// emitter
    /// records the offset at which a document tree placed in another tree
    /// belongs, so that the page assembly can write that tree's HTML there.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::html::HtmlSink;
    ///
    /// let mut sink = HtmlSink::new();
    /// assert_eq!(sink.len(), 0);
    /// sink.text("a<b");
    /// assert_eq!(sink.len(), "a&lt;b".len());
    /// ```
    pub fn len(&self) -> usize {
        self.out.len()
    }

    /// Whether nothing has been written yet.
    pub fn is_empty(&self) -> bool {
        self.out.is_empty()
    }

    /// The tag of the element the sink would close next — the one it opened
    /// last — or `None` when no element is open.
    ///
    /// Like [`len`](HtmlSink::len), this returns a fact about the sink's state
    /// and never the bytes written. It lets an emitter that keeps its own
    /// record of the elements it has opened (the HTML emitter keeps one for the
    /// elements inside a formula, which it may have to write a second time)
    /// check, in a debug build, that its record and the sink's agree.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::html::HtmlSink;
    ///
    /// let mut sink = HtmlSink::new();
    /// assert_eq!(sink.open_tag(), None);
    /// sink.open_inline("em").finish();
    /// assert_eq!(sink.open_tag(), Some("em"));
    /// sink.close();
    /// assert_eq!(sink.open_tag(), None);
    /// ```
    pub fn open_tag(&self) -> Option<&str> {
        self.open.last().map(|element| element.tag.as_str())
    }

    /// Returns the HTML written, ending the sink.
    ///
    /// # Panics
    ///
    /// When an element is still open. That is a mistake in the emitter — it
    /// would hand over a page with an unclosed element — and never anything a
    /// document can cause.
    pub fn finish(self) -> String {
        assert!(
            self.open.is_empty(),
            "the emitter finished the sink with {} element(s) still open: {}",
            self.open.len(),
            self.open.iter().map(|e| e.tag.as_str()).collect::<Vec<_>>().join(", ")
        );
        self.out
    }

    // --- the writing the entry points share ------------------------------------------

    /// Opens an element and pushes it on the stack of open elements.
    ///
    /// The tag name is the caller's to validate; the check is repeated here in
    /// a debug build.
    fn open_element(&mut self, tag: &str, block: bool) -> TagBuilder<'_> {
        debug_assert!(
            is_valid_tag_name(tag),
            "the caller opened an element whose tag name is not writable: {tag:?}"
        );
        if block {
            self.begin_block_position();
        }
        self.out.push('<');
        self.out.push_str(tag);
        self.open.push(OpenElement {
            tag: tag.to_string(),
            block,
            block_children: false,
            preformatted: false,
        });
        if block {
            self.depth += 1;
        }
        TagBuilder { sink: self, opened: true }
    }

    /// Writes an element that has no content and closes nothing.
    ///
    /// The tag name is the caller's to validate; the check is repeated here in
    /// a debug build.
    fn void_element(&mut self, tag: &str, block: bool) -> TagBuilder<'_> {
        debug_assert!(
            is_valid_tag_name(tag),
            "the caller wrote an element whose tag name is not writable: {tag:?}"
        );
        if block {
            self.begin_block_position();
        }
        self.out.push('<');
        self.out.push_str(tag);
        TagBuilder { sink: self, opened: false }
    }

    /// Prepares the place a block-level element is about to be written at: the
    /// line break and the indentation before it, and the record on the
    /// enclosing element that its closing tag goes on a line of its own.
    ///
    /// Nothing is introduced inside a preformatted region, nor in inline flow
    /// — inside an element opened as inline — where introduced whitespace
    /// would be text.
    fn begin_block_position(&mut self) {
        if self.preformatted > 0 || self.open.last().is_some_and(|element| !element.block) {
            return;
        }
        self.break_line();
        if let Some(parent) = self.open.last_mut() {
            parent.block_children = true;
        }
    }

    /// Writes a line break and the indentation of the current level, unless
    /// nothing has been written yet.
    fn break_line(&mut self) {
        if self.out.is_empty() {
            return;
        }
        self.out.push('\n');
        for _ in 0..self.depth {
            self.out.push_str(INDENT);
        }
    }

    /// Writes ` name="value"`, the value escaped.
    ///
    /// The name is the caller's to validate, at every entry point that takes
    /// one; the check is repeated here in a debug build.
    fn write_attribute(&mut self, name: &str, value: &str) {
        debug_assert!(
            is_valid_attribute_name(name),
            "the caller wrote an attribute whose name is not writable: {name:?}"
        );
        self.out.push(' ');
        self.out.push_str(name);
        self.out.push_str("=\"");
        escape_attr_into(&mut self.out, value);
        self.out.push('"');
    }

    /// Writes ` name='value'`, the value escaped for a single-quoted attribute.
    ///
    /// The name is the caller's to validate, as
    /// [`write_attribute`](HtmlSink::write_attribute) states.
    fn write_json_attribute(&mut self, name: &str, value: &str) {
        debug_assert!(
            is_valid_attribute_name(name),
            "the caller wrote an attribute whose name is not writable: {name:?}"
        );
        self.out.push(' ');
        self.out.push_str(name);
        self.out.push_str("='");
        escape_attr_single_quoted_into(&mut self.out, value);
        self.out.push('\'');
    }
}

/// The attributes of the element being opened, written one by one, and the
/// [`finish`](TagBuilder::finish) that ends the opening tag.
///
/// A builder is obtained from [`HtmlSink::open_block`],
/// [`HtmlSink::open_inline`], [`HtmlSink::void_block`] or
/// [`HtmlSink::void_inline`], and its calls chain:
///
/// ```
/// use flm_core::stdcore::backends::html::HtmlSink;
///
/// let mut sink = HtmlSink::new();
/// sink.void_inline("img")
///     .class(["flm-pres-image"])
///     .attr("src", "plot.png")
///     .attr("alt", "a \"plot\"")
///     .finish();
/// assert_eq!(sink.finish(), "<img class=\"flm-pres-image\" src=\"plot.png\" alt=\"a &quot;plot&quot;\">");
/// ```
///
/// Attributes are written in the order the calls are made, and the sink
/// neither reorders them nor checks that a name occurs once: the order is the
/// emitter's, which fixes it so that output can be compared byte for byte.
///
/// # Preconditions
///
/// Attribute *names* are written unchanged, so the caller has validated
/// every one of them with [`is_valid_attribute_name`]; a name that comes from
/// a [construct node](super::construct) is checked by the
/// emitter, which reports the node it refused. A name that fails the check is
/// a programming error in the calling code, never document input, and the
/// builder's entry points check it again in a debug build.
#[must_use = "the opening tag is unfinished until finish() is called"]
pub struct TagBuilder<'s> {
    sink: &'s mut HtmlSink,
    /// Whether the element was pushed on the sink's stack of open elements,
    /// which a void element is not.
    opened: bool,
}

impl<'s> TagBuilder<'s> {
    /// Writes the `class` attribute holding `classes`, separated by single
    /// spaces.
    ///
    /// Call it once per element, before the other attributes: the class comes
    /// first in every element this backend writes. It exists beside
    /// [`attr`](TagBuilder::attr) because the class attribute is the only one
    /// whose value this backend builds from several names (the element's, the
    /// role's), and joining them here keeps every caller from writing the
    /// separator.
    pub fn class<I, S>(self, classes: I) -> TagBuilder<'s>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut value = String::new();
        for class in classes {
            if !value.is_empty() {
                value.push(' ');
            }
            value.push_str(class.as_ref());
        }
        self.attr("class", &value)
    }

    /// Writes ` name="value"`, the value escaped
    /// ([`super::escape::escape_attr_into`]) and always inside double
    /// quotation marks.
    ///
    /// This, [`attr_json`](TagBuilder::attr_json) and [`HtmlSink::text`] are
    /// the three entry points document-derived text reaches a page through.
    ///
    /// # Preconditions
    ///
    /// The caller has validated `name`, as [the builder](TagBuilder) states.
    ///
    /// # Panics
    ///
    /// *Debug builds only*: `name` fails that check. A release build writes
    /// the name unchanged, which may produce markup the caller did not
    /// intend.
    pub fn attr(self, name: &str, value: &str) -> TagBuilder<'s> {
        self.sink.write_attribute(name, value);
        self
    }

    /// Writes ` name='value'`, the value escaped
    /// ([`super::escape::escape_attr_single_quoted_into`]) and always inside
    /// single quotation marks.
    ///
    /// This is how an attribute whose value is **JSON** is written.
    /// HTML allows either quotation mark around an attribute value; inside
    /// single ones a double quotation mark is written as itself, so the raw page
    /// shows the JSON as JSON —
    /// `data-flm-source-span='["book/ch1.flm",0,8]'` — instead of a run of
    /// `&quot;`. Only `&` and `'` are rewritten, and unconditionally: like
    /// [`attr`](TagBuilder::attr), this is a place document-derived text
    /// reaches a page through, and text that reaches it cannot leave the
    /// attribute.
    ///
    /// The sink neither builds the JSON nor checks that the value is JSON:
    /// the caller is responsible for that, and this backend's only such
    /// attribute is built by its emitter.
    ///
    /// # Preconditions
    ///
    /// The caller has validated `name`, as [the builder](TagBuilder) states.
    ///
    /// # Panics
    ///
    /// *Debug builds only*: `name` fails that check. A release build writes
    /// the name unchanged, which may produce markup the caller did not
    /// intend.
    pub fn attr_json(self, name: &str, value: &str) -> TagBuilder<'s> {
        self.sink.write_json_attribute(name, value);
        self
    }

    /// Writes ` name`, an attribute whose presence is the whole of what it
    /// says — `hidden`, `defer` — and which therefore has no value.
    ///
    /// # Preconditions
    ///
    /// The caller has validated `name`, as [the builder](TagBuilder) states.
    ///
    /// # Panics
    ///
    /// *Debug builds only*: `name` fails that check.
    pub fn flag(self, name: &str) -> TagBuilder<'s> {
        debug_assert!(
            is_valid_attribute_name(name),
            "the caller wrote an attribute whose name is not writable: {name:?}"
        );
        self.sink.out.push(' ');
        self.sink.out.push_str(name);
        self
    }

    /// Marks the element's content as a **preformatted region**: every space
    /// and line break inside it is text a reader sees, so the sink introduces
    /// no line break and no indentation anywhere inside it, down to the
    /// element's closing tag.
    ///
    /// This is what `<pre>` is opened with. A void element has no content, so
    /// there is no region to mark and the call does nothing.
    pub fn preformatted(self) -> TagBuilder<'s> {
        if !self.opened {
            // A void element has no content, so there is no region to mark.
            return self;
        }
        let Some(element) = self.sink.open.last_mut() else { return self };
        if !element.preformatted {
            element.preformatted = true;
            self.sink.preformatted += 1;
        }
        self
    }

    /// Ends the opening tag, writing `>`.
    pub fn finish(self) {
        self.sink.out.push('>');
    }
}

impl fmt::Debug for TagBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TagBuilder").field("opened", &self.opened).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_are_broken_and_indented_and_inline_content_is_not() {
        let mut sink = HtmlSink::new();
        sink.open_block("div").class(["flm-pres-block", "flm-heading"]).finish();
        sink.open_block("p").finish();
        sink.text("a ");
        sink.open_inline("em").finish();
        sink.text("b");
        sink.close();
        sink.text(" c");
        sink.close();
        sink.open_block("div").finish();
        sink.open_block("p").finish();
        sink.text("d");
        sink.close();
        sink.close();
        sink.close();
        assert_eq!(
            sink.finish(),
            "<div class=\"flm-pres-block flm-heading\">\n  <p>a <em>b</em> c</p>\n  <div>\n    \
             <p>d</p>\n  </div>\n</div>"
        );
    }

    #[test]
    fn a_block_with_only_inline_content_closes_on_its_own_line_start() {
        let mut sink = HtmlSink::new();
        sink.open_block("p").finish();
        sink.text("only text");
        sink.close();
        assert_eq!(sink.finish(), "<p>only text</p>");
    }

    #[test]
    fn a_void_element_is_never_closed_and_takes_its_position_from_its_call() {
        let mut sink = HtmlSink::new();
        sink.open_block("p").finish();
        sink.text("see ");
        sink.void_inline("img").attr("src", "a.png").finish();
        sink.close();
        sink.void_block("hr").finish();
        assert_eq!(sink.finish(), "<p>see <img src=\"a.png\"></p>\n<hr>");
    }

    #[test]
    fn nothing_is_introduced_inside_a_preformatted_region() {
        let mut sink = HtmlSink::new();
        sink.open_block("pre").class(["flm-pres-verbatim"]).preformatted().finish();
        sink.open_block("code").finish();
        sink.text("if a < b:\n    return");
        sink.close();
        sink.close();
        sink.open_block("p").finish();
        sink.text("after");
        sink.close();
        assert_eq!(
            sink.finish(),
            "<pre class=\"flm-pres-verbatim\"><code>if a &lt; b:\n    return</code></pre>\n\
             <p>after</p>"
        );
    }

    #[test]
    fn an_inline_root_is_written_as_it_comes() {
        let mut sink = HtmlSink::new();
        sink.text("a ");
        sink.open_inline("strong").attr("data-flm-style", "bold").finish();
        sink.text("b");
        sink.close();
        assert_eq!(sink.finish(), "a <strong data-flm-style=\"bold\">b</strong>");
    }

    #[test]
    fn every_value_is_escaped_and_quoted_and_a_flag_has_none() {
        let mut sink = HtmlSink::new();
        sink.void_block("meta").attr("content", "a\"b&c<d>").flag("hidden").finish();
        assert_eq!(sink.finish(), "<meta content=\"a&quot;b&amp;c&lt;d&gt;\" hidden>");
    }

    #[test]
    fn a_json_value_stands_between_single_quotation_marks_with_its_own_escaping() {
        let mut sink = HtmlSink::new();
        sink.open_block("p")
            .attr("title", "a'b")
            .attr_json("data-flm-source-span", "[\"book/ch1.flm\",0,8]")
            .finish();
        sink.close();
        assert_eq!(
            sink.finish(),
            "<p title=\"a'b\" data-flm-source-span='[\"book/ch1.flm\",0,8]'></p>"
        );
    }

    #[test]
    fn a_raw_text_element_refuses_its_own_closing_tag_in_any_letter_case() {
        let mut sink = HtmlSink::new();
        let refused = sink
            .raw_text_element("script", &[], "var a = \"</ScRiPt>\";")
            .expect_err("the content ends the element");
        assert_eq!(refused, RawTextRefused { tag: String::from("script"), at: 9 });
        // Nothing was written for the refused element.
        assert_eq!(sink.clone().finish(), "");
        // Another element's closing tag is ordinary content.
        sink.raw_text_element("script", &[], "var a = \"</style>\";").expect("not its own");
        assert_eq!(sink.finish(), "<script>var a = \"</style>\";</script>");
    }

    #[test]
    fn a_raw_text_element_stands_between_blocks() {
        let mut sink = HtmlSink::new();
        sink.open_block("head").finish();
        sink.raw_text_element("style", &[], "p { margin: 0 }\n").expect("no closing tag");
        sink.close();
        assert_eq!(sink.finish(), "<head>\n  <style>p { margin: 0 }\n</style>\n</head>");
    }

    #[test]
    fn a_markup_block_stands_on_its_own_line_and_keeps_its_own_indentation() {
        let mut sink = HtmlSink::new();
        sink.open_block("main").finish();
        sink.markup_block(&Markup::trusted("<p>one</p>\n<p>two</p>"));
        sink.markup_block(&Markup::trusted("<p>three</p>"));
        sink.close();
        // Each piece begins on a line of its own at the current indentation,
        // the closing tag of the element they stand in goes on a line of its
        // own, and the line breaks inside a piece are the piece's own.
        assert_eq!(
            sink.finish(),
            "<main>\n  <p>one</p>\n<p>two</p>\n  <p>three</p>\n</main>"
        );
    }

    #[test]
    fn a_markup_block_introduces_nothing_in_inline_flow_or_inside_a_preformatted_region() {
        let mut sink = HtmlSink::new();
        sink.open_block("p").finish();
        sink.open_inline("span").finish();
        sink.markup_block(&Markup::trusted("<b>x</b>"));
        sink.close();
        sink.close();
        assert_eq!(sink.finish(), "<p><span><b>x</b></span></p>");

        let mut sink = HtmlSink::new();
        sink.open_block("pre").preformatted().finish();
        sink.markup_block(&Markup::trusted("<code>x</code>"));
        sink.close();
        assert_eq!(sink.finish(), "<pre><code>x</code></pre>");
    }

    #[test]
    fn the_debug_form_shows_the_state_and_never_the_bytes() {
        let mut sink = HtmlSink::new();
        sink.open_block("p").finish();
        sink.text("secret");
        sink.open_inline("em").finish();
        let shown = format!("{sink:?}");
        assert!(!shown.contains("secret"), "{shown}");
        assert!(!shown.contains("<p>"), "{shown}");
        assert_eq!(
            shown,
            "HtmlSink { bytes_written: 13, depth: 1, open: [\"p\", \"em\"], preformatted: 0 }"
        );
    }

    #[test]
    #[should_panic(expected = "not a <textarea>")]
    fn a_raw_text_element_is_a_style_or_a_script_and_no_other_tag() {
        let mut sink = HtmlSink::new();
        // `<textarea>`'s content is read as markup, so writing it unescaped
        // would put markup where the caller meant text.
        let _ = sink.raw_text_element("textarea", &[], "x");
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "attribute whose name is not writable")]
    fn a_raw_text_element_checks_its_attribute_names_while_testing() {
        // A debug assertion: the test suite is built in debug mode, so the
        // check runs here. A release build writes the name as it stands.
        let mut sink = HtmlSink::new();
        let _ = sink.raw_text_element("style", &[("x\">", "y")], "p{}");
    }

    #[test]
    #[should_panic(expected = "closed an element the sink had not opened")]
    fn closing_what_was_never_opened_is_an_emitter_mistake() {
        let mut sink = HtmlSink::new();
        sink.open_block("p").finish();
        sink.close();
        sink.close();
    }

    #[test]
    #[should_panic(expected = "still open")]
    fn finishing_with_an_element_open_is_an_emitter_mistake() {
        let mut sink = HtmlSink::new();
        sink.open_block("p").finish();
        let _ = sink.finish();
    }
}
