//! The HTML contract as data: one row per presentation element saying which
//! tag, classes and attributes this backend writes for it, which payload
//! fields it cannot express, and which style definition gives it its default
//! look.
//!
//! The **HTML contract** is the set of class names and attributes a
//! stylesheet or a script may rely on in the HTML backend's output. It is
//! written down once, here, as [`CONVENTIONS`]: the emitter reads a row before
//! it writes a node, [`markdown_table`] prints the rows for the [rendering
//! guide](crate::guide::rendering#the-html-contract), and tests compare the three
//! against each other, so that the output, this table and the documentation
//! cannot drift apart.
//!
//! What the rows do **not** hold is a class name. A class is derived from the
//! name the element already has, by [`class_of_element`] and
//! [`class_of_role`](super::names::class_of_role), and an attribute that is
//! not one of HTML's own is
//! derived from the payload field's name by [`data_attribute`], as the custom
//! property of a length field is by [`style_property`]. Storing them
//! would let a class drift from the element it represents.
//!
//! # A length on the page
//!
//! A payload field holding a [`Length`] — a line's indentation — is written
//! as a **custom property** (a CSS variable) in the element's `style`
//! attribute, named after the field by [`style_property`] and valued with the
//! CSS length [`css_length`] spells, a tab being a multiple of the custom
//! property `--flm-tab`, which the core stylesheet declares on the page's
//! root:
//!
//! ```html
//! <div class="flm-pres-line" style="--flm-element-indent-left: calc(2 * var(--flm-tab))" data-flm-align="left">…</div>
//! ```
//!
//! The property sets nothing by itself: the stylesheet applies it
//! (`padding-left: var(--flm-element-indent-left, 0)`), so a page's own rules
//! decide what an indentation looks like and win over the shipped ones, and
//! a host changes the width of a tab with one property. A length is written
//! once, as the property, never also as a `data-flm-…` attribute, and a zero
//! length writes nothing, the stylesheet's fallback being zero, so an
//! ordinary line carries no `style` attribute. **Nothing else ever enters
//! `style`**: the attribute holds the custom properties the HTML contract lists —
//! a payload's length fields, and the ratio of two of them (below) — and
//! never a CSS property such as `padding-left`, which would outrank every
//! stylesheet rule ([`Written::Style`], [`Written::Ratio`]).
//!
//! One further kind of custom property is written by the same writer: the
//! **ratio** of two of a payload's length fields, where a stylesheet needs
//! the proportions of a box and CSS cannot divide one length by another. An
//! image shown at a width and a height is written with its two lengths and
//! their ratio, which the core stylesheet reads so that the image keeps its
//! proportions where the page is narrower than it
//! ([`Written::Ratio`], [`css_ratio`]):
//!
//! ```html
//! <img class="flm-pres-image" src="plot.png" style="--flm-element-width: 5cm; --flm-element-height: 2in; --flm-element-aspect-ratio: 141.73228 / 144">
//! ```
//!
//! Three things are outside the HTML contract, because a page must stay free to
//! change them: the whitespace between tags, the order the attributes are
//! written in, and the choice of tag. A stylesheet that selects
//! `.flm-pres-span[data-flm-style="bold"]` keeps working regardless of the tag the
//! emitter picks for it; one that selects `strong` does not, and is written
//! against something this backend never promised.
//!
//! # How to read a row
//!
//! [`Convention::tag`] says how the tag is chosen ([`TagRule`]).
//! [`Convention::fields`] lists every field of the element's payload, in the
//! order the attributes derived from them are written, each with what becomes
//! of it ([`Written`]): an attribute HTML itself defines, a `data-flm-…`
//! attribute, a custom property in the `style` attribute, the node's second
//! class, the class of a block before the node's own, an attribute on each
//! entry of a list, or nothing.
//! [`Convention::style`] gives the style definition the emitter requires when
//! it writes the element. A requirement is only an identifier; how the
//! definition reaches the page is decided by the stage that resolves the
//! definitions ([`super::defs`]).

use super::names::{class_of_element, data_attribute, style_property};
use crate::stdcore::backends::pres::{self, DocRole, I48F16, Length, LengthUnit};

/// The attribute every length field of a payload is written in: `style`,
/// each field as one custom property ([`Written::Style`]).
pub const STYLE_ATTRIBUTE: &str = "style";

/// The custom property holding the width of one **tab**, the unit of
/// indentation each backend sets through one setting of its own: `--flm-tab`.
/// The core stylesheet declares it on the page's root, `2em` by default;
/// [`css_length`] spells a length in tabs as a multiple of it.
pub const TAB_PROPERTY: &str = "--flm-tab";

/// Returns `length` spelled as a CSS length: a length in tabs as
/// `calc(<amount> * var(--flm-tab))` ([`TAB_PROPERTY`]), a length of any
/// other unit as written, amount then unit (`1.5em`, `10pt`, `3mm`,
/// `0.25in`, `2cm`).
///
/// The vocabulary's units other than the tab are CSS's own, spelled as CSS
/// spells them, and its point is CSS's point, 1/72 inch. The amount is
/// written in the decimal the length projects as — the shortest that reads
/// back to the same exact amount — so the output is byte-identical wherever
/// it is produced. This is the only way a length is written on the page; the
/// emitter writes it as the value of a custom property ([`Written::Style`]).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::conventions::css_length;
/// use flm_core::stdcore::backends::pres::Length;
///
/// assert_eq!(css_length(&Length::tabs(2)), "calc(2 * var(--flm-tab))");
/// assert_eq!(css_length(&Length::parse("1.5em")?), "1.5em");
/// assert_eq!(css_length(&Length::parse("10pt")?), "10pt");
/// # Ok::<(), flm_core::stdcore::backends::pres::LengthError>(())
/// ```
pub fn css_length(length: &Length) -> String {
    match length.unit {
        LengthUnit::Tabs => format!("calc({} * var({TAB_PROPERTY}))", length.amount_decimal()),
        LengthUnit::Ems
        | LengthUnit::Points
        | LengthUnit::Millimeters
        | LengthUnit::Centimeters
        | LengthUnit::Inches => length.to_string(),
    }
}

/// Returns the ratio `first ÷ second` of two lengths spelled as CSS's `aspect-ratio`
/// reads a ratio, `<first> / <second>` with both written in points; `None`
/// unless both are greater than zero and in physical units (`pt`, `mm`, `cm`,
/// `in`), since a tab or an em has no fixed size to compare.
///
/// The two amounts are the lengths converted to points
/// ([`Length::convert_to`]), each rounded once to the nearest 1/65536 pt, and
/// written in the shortest decimal that reads back; CSS divides them. This is
/// the value of the custom property a [`Written::Ratio`] field declares.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::conventions::css_ratio;
/// use flm_core::stdcore::backends::pres::Length;
///
/// let ratio = css_ratio(&Length::parse("2in")?, &Length::parse("36pt")?);
/// assert_eq!(ratio.as_deref(), Some("144 / 36"));
/// assert_eq!(css_ratio(&Length::parse("2em")?, &Length::parse("1em")?), None);
/// # Ok::<(), flm_core::stdcore::backends::pres::LengthError>(())
/// ```
pub fn css_ratio(first: &Length, second: &Length) -> Option<String> {
    let points = |length: &Length| {
        length
            .convert_to(LengthUnit::Points)
            .ok()
            .filter(|converted| converted.amount > I48F16::ZERO)
    };
    let (first, second) = (points(first)?, points(second)?);
    Some(format!("{} / {}", first.amount_decimal(), second.amount_decimal()))
}

/// The class the page's content container carries, and the identifier it is
/// found by: `flm-content`.
///
/// It is not derived from any element, because the container is the page's own
/// and represents no node of a document. See [`page`](super::page) for the
/// page layout it is part of.
pub const CONTENT_CLASS: &str = "flm-content";

/// The class of the element holding a list entry's label where that label is
/// rich content rather than a marker a browser draws: `flm-pres-list-label`.
pub const LIST_LABEL_CLASS: &str = "flm-pres-list-label";

/// The class of the two marker lines of an unattributed environment — the
/// `\begin{…}` and `\end{…}` lines the output shows as the author wrote them:
/// `flm-pres-unknown-marker`.
pub const UNKNOWN_MARKER_CLASS: &str = "flm-pres-unknown-marker";

/// The three classes of the HTML contract that are not derived from an element
/// or a role, in the order this module declares them.
///
/// They are spelled by hand, but they are spelled to the same rule: read
/// backwards through the mapping [`class_of_element`] applies — a single hyphen
/// was a dot, a double hyphen was a hyphen — they are `flm.content`,
/// `flm.pres.list.label` and `flm.pres.unknown.marker`. Each is an element's
/// name followed by the name of a part of it, which is how a role is spelled,
/// so none of them can be read as the class of a hyphenated element name.
pub const AUXILIARY_CLASSES: [&str; 3] =
    [CONTENT_CLASS, LIST_LABEL_CLASS, UNKNOWN_MARKER_CLASS];

// --- the style definitions the rows name ------------------------------------------------

/// The style definition of the elements every document has: paragraphs,
/// blocks, spans, links, images and decorations.
///
/// The mark on an unresolved reference is here rather than with the other
/// error looks ([`STYLE_ERRORS`]), because it is a display style of a span and
/// the row of a span gives this definition: a document whose only error is an
/// unresolved reference requires no other definition.
pub const STYLE_CORE: &str = "flm.html.style.core";
/// The style definition of the six heading sizes.
///
/// A run-in heading is not styled here: it reaches the page as a run-in
/// decoration, whose row gives [`STYLE_CORE`].
pub const STYLE_HEADINGS: &str = "flm.html.style.headings";
/// The style definition of verbatim content, inline and as a block.
pub const STYLE_VERBATIM: &str = "flm.html.style.verbatim";
/// The style definition of passthrough mathematics (mathematics written into
/// the page as source, for the MathJax script to typeset): displayed formulas,
/// and content inside a formula.
pub const STYLE_MATH_PASSTHROUGH: &str = "flm.html.style.math-passthrough";
/// The style definition of lists, and of a run of lines with its alignment.
pub const STYLE_LISTS: &str = "flm.html.style.lists";
/// The style definition of what the output shows as an error: an unattributed
/// callable, an element this backend cannot write, and the mark a rule leaves
/// where it dropped something.
///
/// An unresolved reference is marked by [`STYLE_CORE`] instead, for the reason
/// given there.
pub const STYLE_ERRORS: &str = "flm.html.style.errors";

/// The **script** definition that turns the LaTeX of a formula into
/// mathematics in the reader's browser: `flm.html.mathjax`.
///
/// It is the only definition the emitter requires that no row of the table
/// below gives. A row's `style` column holds the definition that sets an
/// element's default *look*, and this one is not that: it is a script, and a
/// page needs it only because passthrough mathematics is written as LaTeX for a
/// formula renderer to read (see [the emitter](super::emit#mathematics)). The
/// emitter requires it at the first passthrough formula it writes in a tree, beside the style
/// definition the mathematics rows give.
///
/// A requirement is only an identifier: how the definition reaches the page
/// (a script element in the page, a file a host bundles, nothing at all
/// because the host loads its own) is decided where the definitions are
/// resolved ([`super::defs`]), not here.
pub const SCRIPT_MATHJAX: &str = "flm.html.mathjax";

// --- the row types ----------------------------------------------------------------------

/// How the tag of one presentation element is chosen.
///
/// The choice is **not** part of the HTML contract (see [the module
/// documentation](self)); it is recorded so that the table a reader sees says
/// what the output looks like today, and so that a test can compare the table
/// with the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagRule {
    /// Always this tag.
    Fixed(&'static str),
    /// The tag the render options map the heading kind's name to
    /// ([`HtmlRenderOptions::heading_tags`](super::HtmlRenderOptions::heading_tags)),
    /// else by the heading's level counted from the root heading level
    /// ([`HtmlRenderOptions::root_heading_level`](super::HtmlRenderOptions::root_heading_level)):
    /// `<h1>` at the root's level, `<h2>` one level deeper, and so on to
    /// `<h6>`, which every heading five levels below the root and deeper gets;
    /// `data-flm-level` keeps the true level. A heading above the root is
    /// reported and written `<h1>`.
    HeadingLevel,
    /// By the span's display style: `<em>`, `<strong>`, `<i>`, `<code>`,
    /// `<u>`, `<sup>`, `<sub>`, and `<span>` for every other style and for a
    /// span with none.
    SpanStyle,
    /// The first tag when the payload's `display` field is inline, the second
    /// when it is a block.
    Display {
        /// The tag written for inline display.
        inline: &'static str,
        /// The tag written for block display.
        block: &'static str,
    },
    /// `<code>` inline; `<pre><code>` as a block, with the class and the
    /// attributes on the `<pre>`.
    VerbatimDisplay,
    /// `<ol>`, `<ul>` or `<dl>` by the list's structural kind token —
    /// [`LIST_NUMBERED`](pres::LIST_NUMBERED),
    /// [`LIST_UNNUMBERED`](pres::LIST_UNNUMBERED) or
    /// [`LIST_TERMS`](pres::LIST_TERMS), the first of them among the kinds;
    /// `<ul>` when none of them is.
    ListKind,
    /// The tag [`BLOCK_TAGS`] maps the block's role to ([`block_tag`]):
    /// `<blockquote>` for a quotation, `<figure>` for a float,
    /// `<figcaption>` for the line under its content and `<nav>` for a table
    /// of contents; `<div>` for every role
    /// the table does not name, and for a block with no role.
    BlockRole,
    /// No element of its own: the node stands for something else, which is
    /// what the emitter writes in its place.
    None,
}

impl TagRule {
    /// Returns the rule as one line of prose, for the printed table
    /// ([`markdown_table`]).
    pub fn describe(self) -> String {
        match self {
            TagRule::Fixed(tag) => format!("`<{tag}>`"),
            TagRule::HeadingLevel => String::from(
                "`<h1>`–`<h6>` by level counted from `root_heading_level`, `<h6>` five levels \
                 below it and deeper; a render option maps a heading kind to a tag of its own",
            ),
            TagRule::SpanStyle => String::from("by style; `<span>` otherwise"),
            TagRule::Display { inline, block } => {
                format!("`<{inline}>` inline, `<{block}>` as a block")
            }
            TagRule::VerbatimDisplay => {
                String::from("`<code>` inline, `<pre><code>` as a block")
            }
            TagRule::ListKind => String::from(
                "`<ol>`, `<ul>` or `<dl>` by the kind `numbered`, `unnumbered` or `terms`; `<ul>` \
                 otherwise",
            ),
            TagRule::BlockRole => {
                let mapped: Vec<String> = BLOCK_TAGS
                    .iter()
                    .map(|entry| format!("`<{}>` for the role `{}`", entry.tag, entry.role))
                    .collect();
                format!("by role: {}; `<{DEFAULT_BLOCK_TAG}>` otherwise", mapped.join(", "))
            }
            TagRule::None => String::from("none of its own"),
        }
    }
}

/// One row of [`BLOCK_TAGS`]: the tag a block of one role is written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockTag {
    /// The block's role, as its payload spells it (`flm.quote`).
    pub role: &'static str,
    /// The tag written for a block of that role, without angle brackets
    /// (`blockquote`).
    pub tag: &'static str,
}

/// The tag of a `flm.pres.block` whose role [`BLOCK_TAGS`] does not name, and
/// of a block with no role: `div`.
pub const DEFAULT_BLOCK_TAG: &str = "div";

/// The **tag table** of blocks: the tag a `flm.pres.block` is written with,
/// keyed by the block's role ([`TagRule::BlockRole`]).
///
/// A block of a role the table lists is written with that tag — a quotation,
/// role `flm.quote`, as a `<blockquote>`, regardless of its environment, since the
/// environment's name is a kind a stylesheet selects on
/// (`.flm-quote[data-flm-kind~="address"]`); a float, role
/// `flm.floats.float`, as a `<figure>`, regardless of its float type, and the line under
/// its content, role `flm.floats.float.caption`, as a `<figcaption>`, which
/// the float's rule places last in the float, where HTML wants it; a table of
/// contents, role `flm.toc`, as a `<nav>` — and
/// every other block as a [`DEFAULT_BLOCK_TAG`] (`<div>`), a float's content
/// among them. The table is this backend's: no feature,
/// no configuration value and no lowering rule specifies a tag, so a feature that
/// wants a tag for the blocks of its role gets a row here. A host wanting
/// another tag for one block writes a lowering rule that stages (builds into
/// the lowered tree; see [`Lowerer`](crate::core::lowering::Lowerer)) an HTML
/// tag construct node, as for any other element. As the [module
/// documentation](self) says of every tag, the choice is not part of the
/// HTML contract: a stylesheet selects a quotation by `.flm-quote`, never by
/// `blockquote`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::conventions::block_tag;
///
/// assert_eq!(block_tag(Some("flm.quote")), "blockquote");
/// assert_eq!(block_tag(Some("flm.floats.float")), "figure");
/// assert_eq!(block_tag(Some("flm.floats.float.caption")), "figcaption");
/// assert_eq!(block_tag(Some("flm.toc")), "nav");
/// assert_eq!(block_tag(Some("flm.floats.float.content")), "div");
/// assert_eq!(block_tag(Some("flm.theorems.theorem")), "div");
/// assert_eq!(block_tag(None), "div");
/// ```
pub static BLOCK_TAGS: &[BlockTag] = &[
    BlockTag { role: "flm.quote", tag: "blockquote" },
    BlockTag { role: "flm.floats.float", tag: "figure" },
    BlockTag { role: "flm.floats.float.caption", tag: "figcaption" },
    BlockTag { role: "flm.toc", tag: "nav" },
];

/// Returns the tag a `flm.pres.block` of the role `role` is written with: the
/// tag of the row of [`BLOCK_TAGS`] for that role, else [`DEFAULT_BLOCK_TAG`].
pub fn block_tag(role: Option<&str>) -> &'static str {
    role.and_then(|role| BLOCK_TAGS.iter().find(|entry| entry.role == role))
        .map_or(DEFAULT_BLOCK_TAG, |entry| entry.tag)
}

/// What the emitter does with one field of a presentation payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// An attribute HTML itself defines, written under this name: `id`,
    /// `href`, `src`, `alt`, `width`, `srcset`, and the two attributes of
    /// accessibility, `role` (a [document role](DocRole), written as
    /// [`doc_role_value`] spells it) and `aria-label`.
    Own(&'static str),
    /// An attribute under the prefix `data-flm-` ([`data_attribute`]), named
    /// after the token given here — the field's own name, except in two
    /// cases. Where one attribute carries what two fields spell differently:
    /// a `kinds` list and a single `kind` are both written as `data-flm-kind`.
    /// And where the attribute is named after what an author writes: a
    /// heading's `name` is `data-flm-heading`, and its `in_toc` is
    /// `data-flm-toc`, after the key `toc` of a heading's options.
    Data(&'static str),
    /// A custom property in the element's `style` attribute
    /// ([`STYLE_ATTRIBUTE`]), named after the field by [`style_property`]
    /// (`indent_left` is `--flm-element-indent-left`) and valued with the
    /// field's [`Length`] as [`css_length`] spells it. For a length field
    /// alone; a zero length writes nothing.
    ///
    /// This is the only thing the `style` attribute ever holds, with the
    /// ratios of [`Written::Ratio`]: the custom properties of a payload's
    /// length fields, which a stylesheet applies, and never a CSS property,
    /// which would outrank every stylesheet rule. A length field is written
    /// this way and in no other: never also as a `data-flm-…` attribute,
    /// whose text (`2tab`) a stylesheet can do nothing with.
    Style,
    /// A custom property in the element's `style` attribute holding the
    /// ratio of the two length fields named here, first over second, as
    /// [`css_ratio`] spells it — written only where both are set, greater
    /// than zero and in physical units. The entry's own name is not a field
    /// of the payload but the name of the property, derived as a length
    /// field's is ([`style_property`]): `aspect_ratio` is
    /// `--flm-element-aspect-ratio`. It lets a stylesheet keep a box's
    /// proportions, which CSS cannot compute from two lengths.
    Ratio(&'static str, &'static str),
    /// The node's second class, derived from the role's name
    /// ([`class_of_role`](super::names::class_of_role)).
    RoleClass,
    /// The block class, `flm-pres-block`: the class of a block container,
    /// [`pres::BLOCK`], which the core stylesheet spaces, written before the
    /// element's own class where the field says the node is a block, and
    /// nothing where it says the node is inline: an image that is a direct
    /// child of a float's body is spaced and stacked as a block, and one
    /// inside a line of text is not.
    BlockClass,
    /// An attribute written under this name on the element of **each
    /// entry** of a list — its `<li>`, or a description list's `<dt>` — or
    /// of each cell of a table of cells — its `<td>` or `<th>` — from that
    /// entry's value: the field holds one value per entry, in the order of
    /// the entries, and an entry with no value gets no attribute. It is never
    /// written on the element of the list or the table itself. The name is an
    /// attribute HTML itself defines (`id`, `colspan`, `scope`), or one under
    /// the prefix `data-flm-`; a name ending in a hyphen is a family, one
    /// attribute per value, the value's name appended (a cell's custom style
    /// `orange` is `data-flm-style-orange`, with an empty value).
    Entry(&'static str),
    /// Nothing is written for it, for the reason given: the output cannot
    /// express it, or it reaches the page as content rather than as an
    /// attribute.
    Dropped(&'static str),
}

/// One field of a presentation payload, and what becomes of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// The field's name, spelled as the payload's projection spells it.
    pub field: &'static str,
    /// What the emitter does with it.
    pub written: Written,
}

impl Field {
    /// The attribute this field is written under on the element's own tag,
    /// when it is written as one there. A field written on each entry of a
    /// list ([`Written::Entry`]) returns `None` here and its attribute from
    /// [`entry_attribute`](Self::entry_attribute); a length field
    /// ([`Written::Style`]) returns `None` and its custom property from
    /// [`style_property`](Self::style_property), all of a row's length
    /// fields sharing the single [`STYLE_ATTRIBUTE`].
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::html::conventions::{Field, Written};
    ///
    /// let level = Field { field: "level", written: Written::Data("level") };
    /// assert_eq!(level.attribute().as_deref(), Some("data-flm-level"));
    ///
    /// let role = Field { field: "role", written: Written::RoleClass };
    /// assert_eq!(role.attribute(), None);
    /// ```
    pub fn attribute(&self) -> Option<String> {
        match self.written {
            Written::Own(name) => Some(String::from(name)),
            Written::Data(stem) => Some(data_attribute(stem)),
            Written::Style
            | Written::Ratio(..)
            | Written::RoleClass
            | Written::BlockClass
            | Written::Entry(_)
            | Written::Dropped(_) => None,
        }
    }

    /// The custom property this field is written as in the element's `style`
    /// attribute, when it is a length field written there
    /// ([`Written::Style`]) or a ratio of two ([`Written::Ratio`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::html::conventions::row;
    /// use flm_core::stdcore::backends::pres;
    ///
    /// let line = row(pres::LINE).expect("a row");
    /// let indent = line.field("indent_left").expect("the field");
    /// assert_eq!(indent.style_property().as_deref(), Some("--flm-element-indent-left"));
    /// assert_eq!(indent.attribute(), None);
    /// ```
    pub fn style_property(&self) -> Option<String> {
        match self.written {
            Written::Style | Written::Ratio(..) => Some(style_property(self.field)),
            _ => None,
        }
    }

    /// The attribute this field is written under on the element of each
    /// entry of a list or each cell of a table of cells, when it is written
    /// there ([`Written::Entry`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::html::conventions::row;
    /// use flm_core::stdcore::backends::pres;
    ///
    /// let list = row(pres::LIST).expect("a row");
    /// let anchors = list.field("entry_anchors").expect("the field");
    /// assert_eq!(anchors.entry_attribute(), Some("id"));
    /// assert_eq!(anchors.attribute(), None);
    /// ```
    pub fn entry_attribute(&self) -> Option<&'static str> {
        match self.written {
            Written::Entry(name) => Some(name),
            _ => None,
        }
    }
}

/// One row of the HTML contract: everything this backend writes for one
/// presentation element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Convention {
    /// The presentation element's name (`flm.pres.par`).
    pub element: &'static str,
    /// How the tag is chosen.
    pub tag: TagRule,
    /// Every field of the element's payload, in the order the attributes
    /// derived from them are written.
    pub fields: &'static [Field],
    /// The style definition the emitter requires when it writes the element;
    /// `None` for a node that writes no element of its own.
    pub style: Option<&'static str>,
}

impl Convention {
    /// The class every node of this element carries, derived from the
    /// element's name.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::html::conventions::row;
    /// use flm_core::stdcore::backends::pres;
    ///
    /// assert_eq!(row(pres::PAR).expect("a row").class(), "flm-pres-par");
    /// ```
    pub fn class(&self) -> String {
        class_of_element(self.element)
    }

    /// The field named `field`, when the row has one.
    pub fn field(&self, field: &str) -> Option<&Field> {
        self.fields.iter().find(|entry| entry.field == field)
    }

    /// Whether the element's payload carries a role that becomes a second
    /// class.
    pub fn has_role(&self) -> bool {
        self.fields.iter().any(|entry| entry.written == Written::RoleClass)
    }
}

// --- the table --------------------------------------------------------------------------

/// A field written as an attribute HTML itself defines.
const fn own(field: &'static str, attribute: &'static str) -> Field {
    Field { field, written: Written::Own(attribute) }
}

/// A field written as the `data-flm-…` attribute named after it.
const fn data(field: &'static str) -> Field {
    Field { field, written: Written::Data(field) }
}

/// A field written as the `data-flm-…` attribute named after `stem` rather
/// than after the field: what a `kinds` list and a single `kind` share, so
/// that a stylesheet selects a kind token the same way whichever field carried
/// it; and a heading's `name` and `in_toc`, written as `data-flm-heading` and
/// `data-flm-toc`, after what an author writes.
// See decision rationale at [§flm-dd-dr:rend-html-role].
const fn data_as(field: &'static str, stem: &'static str) -> Field {
    Field { field, written: Written::Data(stem) }
}

/// A field written as an attribute HTML itself defines on each entry of a
/// list.
const fn entry(field: &'static str, attribute: &'static str) -> Field {
    Field { field, written: Written::Entry(attribute) }
}

/// A length field written as a custom property in the `style` attribute.
const fn style(field: &'static str) -> Field {
    Field { field, written: Written::Style }
}

/// The ratio of two length fields, written as the custom property named after
/// `name` in the `style` attribute.
const fn ratio(name: &'static str, first: &'static str, second: &'static str) -> Field {
    Field { field: name, written: Written::Ratio(first, second) }
}

/// A field that becomes the node's second class.
const fn role(field: &'static str) -> Field {
    Field { field, written: Written::RoleClass }
}

/// A field that adds the class of a block before the node's own where it
/// says the node is a block.
const fn block_class(field: &'static str) -> Field {
    Field { field, written: Written::BlockClass }
}

/// A field nothing is written for, with the reason.
const fn dropped(field: &'static str, why: &'static str) -> Field {
    Field { field, written: Written::Dropped(why) }
}

/// The HTML contract, one row per element of the [presentation
/// vocabulary](crate::stdcore::backends::pres), in the vocabulary's own order.
///
/// Every name of [`pres::ALL`] has exactly
/// one row, which a test holds; [`row`] is the lookup the emitter uses.
pub static CONVENTIONS: &[Convention] = &[
    Convention {
        element: pres::PAR,
        tag: TagRule::Fixed("p"),
        fields: &[own("anchor", "id")],
        style: Some(STYLE_CORE),
    },
    Convention {
        element: pres::BLOCK,
        tag: TagRule::BlockRole,
        fields: &[
            own("anchor", "id"),
            role("role"),
            own("doc_role", "role"),
            data_as("kinds", "kind"),
        ],
        style: Some(STYLE_CORE),
    },
    Convention {
        element: pres::BLOCK_HEADING,
        tag: TagRule::HeadingLevel,
        fields: &[
            own("anchor", "id"),
            data_as("name", "heading"),
            data("level"),
            data_as("in_toc", "toc"),
            dropped("number", "the number stands in the content"),
        ],
        style: Some(STYLE_HEADINGS),
    },
    Convention {
        element: pres::RUNIN_HEADING,
        tag: TagRule::Fixed("span"),
        fields: &[
            own("anchor", "id"),
            data_as("name", "heading"),
            data("level"),
            data_as("in_toc", "toc"),
            dropped("number", "the number stands in the content"),
        ],
        style: Some(STYLE_CORE),
    },
    Convention {
        element: pres::SPAN,
        tag: TagRule::SpanStyle,
        fields: &[
            own("anchor", "id"),
            role("role"),
            own("doc_role", "role"),
            data("style"),
            data_as("kinds", "kind"),
        ],
        style: Some(STYLE_CORE),
    },
    Convention {
        element: pres::LIST,
        tag: TagRule::ListKind,
        fields: &[own("anchor", "id"), data_as("kinds", "kind"), entry("entry_anchors", "id")],
        style: Some(STYLE_LISTS),
    },
    Convention {
        element: pres::CELLS,
        tag: TagRule::Fixed("table"),
        fields: &[
            dropped(
                "rows",
                "the grid's size is its shape: one `<tr>` per row, and in it one element per \
                 position a span does not cover, an empty `<td>` where there is no cell",
            ),
            dropped("cols", "the grid's size is its shape, as for `rows`"),
            dropped(
                "cells",
                "each entry is a `<td>`, or a `<th>` for a header, at its top-left position, \
                 holding its `cell` slot, its fields written as the attributes of each entry",
            ),
            entry("row_span", "rowspan"),
            entry("col_span", "colspan"),
            entry("header", "scope"),
            entry("align", "data-flm-align"),
            entry("rules", "data-flm-rules"),
            entry("styles", "data-flm-style-"),
        ],
        style: Some(crate::stdcore::features::cells::STYLE),
    },
    Convention {
        element: pres::LINES,
        tag: TagRule::Fixed("div"),
        fields: &[],
        style: Some(STYLE_LISTS),
    },
    Convention {
        element: pres::LINE,
        tag: TagRule::Fixed("div"),
        fields: &[style("indent_left"), style("indent_right"), data("align")],
        style: Some(STYLE_LISTS),
    },
    Convention {
        element: pres::VERBATIM,
        tag: TagRule::VerbatimDisplay,
        fields: &[data("kind"), data("display"), data("lang")],
        style: Some(STYLE_VERBATIM),
    },
    Convention {
        element: pres::MATH_PASSTHROUGH,
        // One tag for both displays: a displayed formula can stand inside a
        // paragraph, where HTML allows no block element. See decision
        // rationale at [§flm-dd-dr:rend-html-math-embedding].
        tag: TagRule::Fixed("span"),
        fields: &[data("display"), data("env")],
        style: Some(STYLE_MATH_PASSTHROUGH),
    },
    Convention {
        element: pres::MATH_TEXT,
        tag: TagRule::Fixed("span"),
        fields: &[
            own("anchor", "id"),
            data("command"),
            dropped(
                "at_tag",
                "the content is written at the position of an equation line's tag; no attribute: a \
                 pair of dollar signs is written around the element instead, inside the \
                 formula",
            ),
        ],
        style: Some(STYLE_MATH_PASSTHROUGH),
    },
    Convention {
        element: pres::LINK,
        tag: TagRule::Fixed("a"),
        fields: &[
            own("href", "href"),
            own("doc_role", "role"),
            own("label", "aria-label"),
            data("kind"),
        ],
        style: Some(STYLE_CORE),
    },
    Convention {
        element: pres::IMAGE,
        tag: TagRule::Fixed("img"),
        fields: &[
            own("anchor", "id"),
            own("src", "src"),
            block_class("display"),
            own("alt", "alt"),
            own("srcset", "srcset"),
            style("width"),
            style("height"),
            // See decision rationale at [§flm-dd-dr:rend-html-image-size].
            ratio("aspect_ratio", "width", "height"),
        ],
        style: Some(STYLE_CORE),
    },
    Convention {
        element: pres::UNSUPPORTED,
        tag: TagRule::Fixed("span"),
        fields: &[
            data("element"),
            dropped("message", "the reason is reported as a diagnostic, not shown in the page"),
        ],
        style: Some(STYLE_ERRORS),
    },
    Convention {
        element: pres::DROPPED,
        tag: TagRule::Fixed("span"),
        fields: &[
            data("condition"),
            dropped(
                "detail",
                "written as the element's content, after `dropped: `, so that it shows without a \
                 stylesheet",
            ),
        ],
        style: Some(STYLE_ERRORS),
    },
    Convention {
        element: pres::UNKNOWN,
        tag: TagRule::Display { inline: "span", block: "div" },
        fields: &[
            data("kind"),
            data("name"),
            dropped(
                "spelling",
                "what the author typed, written as the element's content: a macro's spelling, an \
                 environment's `\\begin` and `\\end` lines",
            ),
        ],
        style: Some(STYLE_ERRORS),
    },
    Convention {
        element: pres::DECO_RUNIN,
        tag: TagRule::Fixed("span"),
        fields: &[],
        style: Some(STYLE_CORE),
    },
    Convention {
        element: pres::DECO_FINAL,
        tag: TagRule::Fixed("span"),
        fields: &[],
        style: Some(STYLE_CORE),
    },
    Convention {
        element: pres::DECO_MARGIN,
        tag: TagRule::Fixed("span"),
        fields: &[],
        style: Some(STYLE_CORE),
    },
    Convention {
        element: pres::DECO_MARK,
        tag: TagRule::None,
        fields: &[dropped("slot", "identifies the decoration written in the mark's place")],
        style: None,
    },
    Convention {
        element: pres::SUBTREE,
        tag: TagRule::None,
        fields: &[dropped("tree", "identifies the document tree written in the node's place")],
        style: None,
    },
];

/// Returns the value of the `role` attribute written for the document role `role`:
/// `doc-` followed by the role's stable name ([`DocRole::as_str`]) —
/// `doc-endnotes`, `doc-footnote`, `doc-noteref`, `doc-backlink`,
/// `doc-bibliography`, `doc-biblioref` and `doc-toc`, the roles the
/// digital-publishing accessibility vocabulary defines for notes, cited works
/// and a table of contents.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::conventions::doc_role_value;
/// use flm_core::stdcore::backends::pres::DocRole;
///
/// assert_eq!(doc_role_value(DocRole::NoteRef), "doc-noteref");
/// assert_eq!(doc_role_value(DocRole::Toc), "doc-toc");
/// ```
pub fn doc_role_value(role: DocRole) -> String {
    format!("doc-{}", role.as_str())
}

/// Returns the row of the presentation element `element`, or `None` when the
/// name is not one of the vocabulary's. The emitter looks each node's row up
/// with it.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::conventions::row;
/// use flm_core::stdcore::backends::pres;
///
/// let heading = row(pres::BLOCK_HEADING).expect("a row for every element");
/// assert_eq!(heading.field("level").and_then(|f| f.attribute()).as_deref(),
///            Some("data-flm-level"));
/// assert!(row("flm.pres.nothing").is_none());
/// ```
pub fn row(element: &str) -> Option<&'static Convention> {
    CONVENTIONS.iter().find(|convention| convention.element == element)
}

/// Returns the HTML contract as a Markdown table: one line per element, with
/// the element's name, the tag, the classes, the attributes, what is dropped
/// and the style definition.
///
/// The [rendering guide](crate::guide::rendering#the-html-contract) holds this
/// table, and a test compares it with this function's result, so that the
/// guide cannot fall behind the code.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::conventions::markdown_table;
///
/// let table = markdown_table();
/// assert!(table.starts_with("| element | tag |"));
/// assert!(table.contains("`flm.pres.par`"));
/// ```
pub fn markdown_table() -> String {
    let mut out = String::from(
        "| element | tag | classes | attributes | dropped | style definition |\n\
         |---|---|---|---|---|---|\n",
    );
    for convention in CONVENTIONS {
        let writes_an_element = convention.tag != TagRule::None;
        let mut classes = if writes_an_element {
            vec![format!("`{}`", convention.class())]
        } else {
            Vec::new()
        };
        let mut attributes: Vec<String> = Vec::new();
        let mut dropped: Vec<String> = Vec::new();
        let properties: Vec<String> = convention
            .fields
            .iter()
            .filter_map(|field| {
                field.style_property().map(|property| match field.written {
                    Written::Ratio(first, second) => {
                        format!("`{first}` ÷ `{second}` as `{property}`")
                    }
                    _ => format!("`{}` as `{property}`", field.field),
                })
            })
            .collect();
        let mut style_written = false;
        for field in convention.fields {
            match field.written {
                Written::Style | Written::Ratio(..) => {
                    // The row's length fields share one attribute, printed
                    // where the first of them stands.
                    if !style_written {
                        attributes.push(format!(
                            "`{STYLE_ATTRIBUTE}` ← {} (custom properties, none for a zero length)",
                            properties.join(", ")
                        ));
                        style_written = true;
                    }
                }
                Written::Own(name) => {
                    attributes.push(format!("`{name}` ← `{}`", field.field));
                }
                Written::Data(stem) => {
                    attributes.push(format!("`{}` ← `{}`", data_attribute(stem), field.field));
                }
                // A family is spelled with the place of the value's name.
                Written::Entry(name) if name.ends_with('-') => {
                    attributes.push(format!("each entry's `{name}<name>` ← `{}`", field.field));
                }
                Written::Entry(name) => {
                    attributes.push(format!("each entry's `{name}` ← `{}`", field.field));
                }
                Written::RoleClass => classes.push(String::from("the role's own class")),
                Written::BlockClass => classes.push(format!(
                    "`{}` before it where `{}` is `block`",
                    class_of_element(pres::BLOCK),
                    field.field
                )),
                Written::Dropped(why) => dropped.push(format!("`{}` ({why})", field.field)),
            }
        }
        let join = |parts: Vec<String>| {
            if parts.is_empty() { String::from("—") } else { parts.join(", ") }
        };
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} |\n",
            convention.element,
            convention.tag.describe(),
            join(classes),
            join(attributes),
            join(dropped),
            convention.style.map_or_else(|| String::from("—"), |style| format!("`{style}`")),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_element_of_the_vocabulary_has_exactly_one_row() {
        assert_eq!(CONVENTIONS.len(), pres::ALL.len());
        for element in pres::ALL {
            assert!(row(element).is_some(), "no row for `{element}`");
        }
        for convention in CONVENTIONS {
            assert!(
                pres::ALL.contains(&convention.element),
                "`{}` is not an element of the vocabulary",
                convention.element
            );
        }
    }

    #[test]
    fn a_rows_attribute_names_are_writable_and_occur_once() {
        for convention in CONVENTIONS {
            let mut names: Vec<String> =
                convention.fields.iter().filter_map(|field| field.attribute()).collect();
            let count = names.len();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), count, "`{}` writes one attribute twice", convention.element);
            // What an entry carries stands on another element than the
            // row's own, so it is held to the same checks apart.
            let mut entry: Vec<String> = convention
                .fields
                .iter()
                .filter_map(|field| field.entry_attribute().map(String::from))
                .collect();
            let count = entry.len();
            entry.sort();
            entry.dedup();
            assert_eq!(entry.len(), count, "`{}` writes one attribute twice", convention.element);
            for name in names.into_iter().chain(entry) {
                assert!(
                    super::super::names::is_valid_attribute_name(&name),
                    "`{name}` is not writable"
                );
            }
        }
    }

    #[test]
    fn a_length_is_spelled_as_a_css_length_in_every_unit() {
        let spelled = |text: &str| css_length(&Length::parse(text).expect("a length"));
        assert_eq!(css_length(&Length::tabs(1)), "calc(1 * var(--flm-tab))");
        assert_eq!(css_length(&Length::tabs(2)), "calc(2 * var(--flm-tab))");
        assert_eq!(spelled("1.5tab"), "calc(1.5 * var(--flm-tab))");
        assert_eq!(spelled("-1tab"), "calc(-1 * var(--flm-tab))");
        assert_eq!(spelled("1.5em"), "1.5em");
        assert_eq!(spelled("10pt"), "10pt");
        assert_eq!(spelled("3mm"), "3mm");
        assert_eq!(spelled("2cm"), "2cm");
        assert_eq!(spelled("0.25in"), "0.25in");
        assert_eq!(spelled(".5in"), "0.5in");
        // Every unit has its spelling, the tab alone going through the
        // custom property.
        for unit in LengthUnit::ALL {
            let css = css_length(&Length::new(Length::tabs(3).amount, unit));
            if unit == LengthUnit::Tabs {
                assert_eq!(css, format!("calc(3 * var({TAB_PROPERTY}))"));
            } else {
                assert_eq!(css, format!("3{}", unit.as_str()));
            }
        }
    }

    #[test]
    fn a_length_field_is_a_custom_property_of_the_style_attribute_and_nothing_else() {
        let line = row(pres::LINE).expect("a row");
        let properties: Vec<String> =
            line.fields.iter().filter_map(|field| field.style_property()).collect();
        assert_eq!(properties, ["--flm-element-indent-left", "--flm-element-indent-right"]);
        // Written once: no `data-flm-…` attribute beside the property.
        let attributes: Vec<String> = line.fields.iter().filter_map(|field| field.attribute()).collect();
        assert_eq!(attributes, ["data-flm-align"]);
        // No row writes a length field any other way, and no two fields of a
        // row share a property.
        for convention in CONVENTIONS {
            let mut names: Vec<String> =
                convention.fields.iter().filter_map(|field| field.style_property()).collect();
            let count = names.len();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), count, "`{}` writes one property twice", convention.element);
        }
    }

    #[test]
    fn the_printed_row_of_a_line_reads_the_two_properties() {
        let table = markdown_table();
        let line = table
            .lines()
            .find(|line| line.starts_with("| `flm.pres.line` |"))
            .expect("the row");
        assert!(
            line.contains(
                "`style` ← `indent_left` as `--flm-element-indent-left`, `indent_right` as \
                 `--flm-element-indent-right`"
            ),
            "{line}"
        );
        assert!(!line.contains("data-flm-indent"), "{line}");
    }

    #[test]
    fn every_document_role_is_written_as_a_digital_publishing_role() {
        let values: Vec<String> = DocRole::ALL.into_iter().map(doc_role_value).collect();
        assert_eq!(
            values,
            [
                "doc-endnotes",
                "doc-footnote",
                "doc-noteref",
                "doc-backlink",
                "doc-bibliography",
                "doc-biblioref",
                "doc-toc"
            ]
        );
    }

    #[test]
    fn the_printed_table_holds_one_line_per_element() {
        let table = markdown_table();
        // The header and its separator, then one line per element.
        assert_eq!(table.lines().count(), pres::ALL.len() + 2);
        for element in pres::ALL {
            assert!(table.contains(&format!("| `{element}` |")), "`{element}` is not in the table");
        }
    }
}
