//! The configuration of the `xnotes` feature: the xnote kinds an author
//! writes ([`XnoteKind`], listed by [`XnotesConfig`]), each with its
//! commands, the words a reference writes, its label type, whether it is
//! numbered, its mark, where its xnotes are shown ([`XnotePlacement`]) and how
//! its content is laid out.
//!
//! A feature's configuration is the plain Rust value the feature is
//! constructed from; it is fixed when the registry is built and cannot vary
//! per document or per render (see [settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//! The same values can be written as settings, one entry per xnote kind under
//! the settings key `kinds` ([`crate::core::settings`]).

use std::hash::{Hash, Hasher};

use crate::core::settings::named::{self, NamedEntry};
use crate::lang::mint::ArgLayout;
use crate::model::names::Name;
use crate::stdcore::features::lists::holds_placeholder;
use crate::stdcore::features::numbering::{NumberTemplate, PluralWords, PrefixWords, Within};
use crate::stdcore::features::refs::CAPITAL_VARIANT;

/// Where the xnotes of an xnote kind are shown: in a list after the
/// document's content, or where each one is written.
///
/// The placement is part of the xnote kind's configuration, fixed when the
/// registry is built; no render request can change it. The reason is that an
/// in-place xnote holding paragraphs is a *block* of the document: it
/// interrupts the paragraph it is written in, as a displayed formula does.
/// The [block-structure pass](crate::lang::rewrites::blocks) decides whether
/// a node is a block before the document's trees are frozen (no longer
/// changed; see [frozen
/// trees](crate::guide::concepts_overview#frozen-trees-and-computed-data)),
/// from the element value and the node alone, so
/// the answer is a fact of the document, the same for every output; a listed
/// xnote is always inline content. A document whose footnotes are listed on
/// one output and shown in place on another is therefore rendered with two
/// configurations, that is, with two registries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum XnotePlacement {
    /// In a list after the document's content: the xnote's mark is placed
    /// where the xnote was written, linked to the xnote's entry in the list.
    /// The default.
    #[default]
    List,
    /// Where the xnote is written: one inline span holding the mark and the
    /// content, or, for content holding paragraphs, a block between the two
    /// parts of the paragraph it interrupts.
    InPlace,
}

/// One xnote kind: one way of being an xnote, a piece of text set apart from
/// the running text (the footnotes written `\footnote{…}`, the side notes a
/// configuration adds, the notes another feature's elements carry; see the
/// [module documentation of the feature](super)).
///
/// An xnote kind states, field by field:
///
/// - [`name`](XnoteKind::name): what the kind is called, `footnote`. It is
///   made of ASCII letters only, because other names are derived from it: the
///   counter `flm.xnotes.<name>`, the list element, the LaTeX
///   command `\flmXN<name>`.
/// - [`commands`](XnoteKind::commands): the commands with which an author
///   writes an xnote of the kind, without the backslash: `footnote`. Empty for
///   a kind that no author writes, which another feature contributes through
///   the service slot of xnote kinds
///   ([`contribute_kind`](super::contribute_kind)).
/// - [`words`](XnoteKind::words): the words a reference writes before an
///   xnote's number, `Footnote~` before one number and `Footnotes~` before
///   several. `\ref{fn:x}` prints them (`Footnote~1`); the default title of
///   the list is made from them; and the accessible name of the link from a
///   mark to its xnote begins with them.
/// - [`label_type`](XnoteKind::label_type): the only label type that an
///   xnote of the kind accepts, `fn` in `\label{fn:x}`; also the prefix of
///   its anchors in the output (`fn-1`, `fnref--1kzmq`). It is made of ASCII
///   letters only, like the name, so that it can be written as the label type
///   of a `\label` and begin an anchor.
/// - [`numbered`](XnoteKind::numbered): whether the kind's xnotes are
///   counted, in document order, in a counter of the kind's own. A reference
///   to a numbered xnote shows its number; a reference to an unnumbered xnote
///   shows its content where that content is inline, and otherwise its label.
/// - [`number_format`](XnoteKind::number_format): how a reference writes the
///   number of a numbered kind's xnote, `#{arabic}`.
/// - [`within`](XnoteKind::within): the counter within which a numbered kind
///   is numbered, `None` for numbering through the whole document.
/// - [`mark`](XnoteKind::mark): what is shown in the running text where an
///   xnote was written. For a numbered kind it is a number template,
///   `#{arabic}` by default, and the mark is the xnote's number written
///   through it; for an unnumbered kind it is a fixed mark, a template with no
///   number placeholder (`*`, `\dag`), or empty for no mark. In both cases
///   the template is FLM text, compiled when the registry is built.
/// - [`placement`](XnoteKind::placement): listed after the content, or shown
///   in place ([`XnotePlacement`]).
/// - [`layout`](XnoteKind::layout): how the content argument is laid out:
///   inferred from what the author wrote by default, or fixed.
/// - [`title`](XnoteKind::title) and [`title_level`](XnoteKind::title_level):
///   the heading placed above the kind's list, and its level.
/// - [`backlink`](XnoteKind::backlink): the text of the link from the entry
///   of a listed xnote back to its mark, `↩`.
/// - [`latex_command`](XnoteKind::latex_command): the LaTeX command to which
///   the style file's default definition of `\flmXN<name>` passes the xnote
///   (`footnote`), a command of the author's LaTeX that the author's own
///   preamble may redefine.
///
/// # Numbered and unnumbered xnote kinds
///
/// Whether a kind is numbered is stated by its flag
/// [`numbered`](XnoteKind::numbered) and is never inferred from its
/// templates. The [`mark`](XnoteKind::mark) template is read the same way for
/// both: as FLM text in the language of a counter style's number template,
/// compiled when the registry is built, a mistake in it failing the build
/// (see [the template
/// language](crate::guide::concepts_overview#the-template-language-shared-by-features)).
/// For a numbered kind it is a number template; for an unnumbered kind it is
/// a fixed mark, and an unnumbered kind whose mark holds a number placeholder
/// such as `#{arabic}` is refused when the registry is built, since it has no
/// number to write ([`XnoteKind::validate`]). The
/// [`number_format`](XnoteKind::number_format) and [`within`](XnoteKind::within)
/// of an unnumbered kind are ignored, neither compiled nor checked, so that
/// switching a kind between numbered and unnumbered needs no other change.
///
/// An xnote kind does not depend on any particular output: the mark is one
/// text, shown as a superscript, which each output writes in its own way. For
/// a numbered kind, a host that wants the mark written differently on one
/// output replaces the kind's mark style, `flm.xnotes.style.<name>.mark`
/// ([`mark_style_name`](super::mark_style_name)), through the numbering
/// feature's configuration in that output's registry, as it replaces any
/// declared counter style. An unnumbered kind's fixed mark is compiled by the
/// `xnotes` feature from [`mark`](XnoteKind::mark) and is not a declared
/// counter style: a different fixed mark on one output needs a configuration
/// with a different `mark` in that output's registry.
///
/// # Examples
///
/// ```
/// use flm_core::lang::mint::ArgLayout;
/// use flm_core::stdcore::features::numbering::{NumberTemplate, PrefixWords};
/// use flm_core::stdcore::features::xnotes::{XnoteKind, XnotePlacement};
///
/// // The default kind.
/// let footnote = XnoteKind::footnote();
/// assert_eq!(footnote.commands, ["footnote"]);
/// assert_eq!(footnote.title.as_deref(), Some("Footnotes"));
///
/// // A second numbered kind, listed after the footnotes.
/// let endnote = XnoteKind::numbered("endnote", PrefixWords::new("Endnote~", "Endnotes~"))
///     .commands(["endnote"])
///     .label_type("en");
/// assert_eq!(endnote.title.as_deref(), Some("Endnotes"));
///
/// // An unnumbered kind shown where it is written, with no mark.
/// let sidenote = XnoteKind::unnumbered("sidenote", PrefixWords::new("Side note~", "Side notes~"))
///     .commands(["sidenote"])
///     .label_type("sn")
///     .placement(XnotePlacement::InPlace);
/// assert!(!sidenote.numbered);
/// assert_eq!(sidenote.mark, NumberTemplate::formats(""));
///
/// // A kind whose content is always paragraphs.
/// let long = XnoteKind::footnote().layout(ArgLayout::Blocks);
/// assert_eq!(long.layout, ArgLayout::Blocks);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(transform = xnote_kind_schema)]
pub struct XnoteKind {
    /// The kind's name, ASCII letters only: `footnote`.
    /// In settings, it is written as the entry's key, not as a field.
    #[schemars(skip)]
    pub name: Name,
    /// The commands with which an xnote of the kind is written, without the
    /// backslash; empty for a kind that only another feature creates.
    pub commands: Vec<String>,
    /// The words a reference writes before one number and before several,
    /// with their variants: `Footnote~`, `Footnotes~`.
    pub words: PrefixWords,
    /// The only label type that an xnote of the kind accepts, and the prefix
    /// of its anchors in the output: `fn`. ASCII letters only.
    pub label_type: String,
    /// Whether the kind's xnotes are numbered.
    pub numbered: bool,
    /// How a reference writes a numbered xnote's number; ignored for an
    /// unnumbered kind. `#{arabic}` by default.
    pub number_format: NumberTemplate,
    /// The counter within which a numbered kind is numbered, or `None` for
    /// numbering through the whole document; ignored for an unnumbered kind.
    pub within: Option<Within>,
    /// The text of the mark, as FLM text: a number template for a numbered
    /// kind (`#{arabic}`), a fixed mark with no number placeholder for an
    /// unnumbered one (empty for no mark).
    pub mark: NumberTemplate,
    /// Where the kind's xnotes are shown.
    pub placement: XnotePlacement,
    /// How the content argument is laid out:
    /// [`InlineOrBlocks`](ArgLayout::InlineOrBlocks) by default, which makes
    /// a note written on one run inline content and one holding a blank line
    /// or a block paragraphs. [`Inline`](ArgLayout::Inline) refuses a blank
    /// line in the note, as every inline argument does.
    /// [`Blocks`](ArgLayout::Blocks) makes every note paragraphs, even an
    /// empty one: with it, an in-place xnote is always a block of its own,
    /// `\footnote{}` included.
    pub layout: ArgLayout,
    /// The heading placed above the kind's list, as FLM text; `None` for no
    /// heading. The constructors set the plural word, capitalized and without
    /// the non-breaking space (`Footnotes`).
    ///
    /// The title is compiled when the registry is built, and a title that is
    /// not valid FLM fails the build. It is read in the language of a counter
    /// style's number template, with no placeholder available: `#` starts a
    /// placeholder there, so a literal `#` is written `\#` or `##`, and a
    /// placeholder such as `#{n}` fails the build.
    pub title: Option<String>,
    /// The level of the heading above the list, counted from zero like every
    /// heading level; 2 by default.
    pub title_level: u8,
    /// The text of the link from a listed xnote back to its mark: `↩`.
    pub backlink: String,
    /// The LaTeX command to which the style file's default definition of
    /// `\flmXN<name>` passes the xnote, without the backslash (`footnote`): a
    /// command of the author's LaTeX, which the author's own preamble may
    /// redefine. ASCII letters only.
    pub latex_command: String,
}


/// The schema of an xnote kind in settings: only `words` is required, because
/// the entry is read over what [`XnoteKind::numbered`] makes from the name and
/// the `words` written.
fn xnote_kind_schema(schema: &mut schemars::Schema) {
    crate::core::settings::named::require_only(schema, &["words"]);
}
impl XnoteKind {
    /// A numbered xnote kind named `name` with the reference words `words`:
    /// no command, the label type `name`, numbered through the document, the
    /// number written `#{arabic}` in a reference and in the mark, listed, the
    /// layout inferred from the content, the title made of the plural word
    /// capitalized, at level 2, the back-link `↩`, the LaTeX command
    /// `footnote`. The builder methods change each of these.
    ///
    /// The title is computed from `words` here, once: a kind whose words are
    /// changed afterwards keeps its title until [`title`](XnoteKind::title())
    /// sets another.
    pub fn numbered(name: impl Into<Name>, words: PrefixWords) -> XnoteKind {
        let name = name.into();
        XnoteKind {
            label_type: name.as_str().to_string(),
            name,
            commands: Vec::new(),
            title: Some(plural_title(&words)),
            words,
            numbered: true,
            number_format: NumberTemplate::formats("#{arabic}"),
            within: None,
            mark: NumberTemplate::formats("#{arabic}"),
            placement: XnotePlacement::List,
            layout: ArgLayout::InlineOrBlocks,
            title_level: 2,
            backlink: String::from("\u{21a9}"),
            latex_command: String::from("footnote"),
        }
    }

    /// An unnumbered xnote kind named `name` with the reference words
    /// `words`: as [`XnoteKind::numbered`], but its xnotes are counted in no
    /// counter and its mark is empty, so that no mark is written.
    pub fn unnumbered(name: impl Into<Name>, words: PrefixWords) -> XnoteKind {
        XnoteKind { numbered: false, mark: NumberTemplate::formats(""), ..XnoteKind::numbered(name, words) }
    }

    /// The default xnote kind, `footnote`: written `\footnote`, the words
    /// `Footnote~` and `Footnotes~` (the capital variant the same), the label
    /// type `fn`, numbered through the document, `#{arabic}` in a reference and
    /// in the mark, listed, its layout inferred, the title `Footnotes` at
    /// level 2, the back-link `↩`, the LaTeX command `footnote`.
    pub fn footnote() -> XnoteKind {
        let words = PrefixWords::new("Footnote~", "Footnotes~")
            .with_variant(CAPITAL_VARIANT, PluralWords::new("Footnote~", "Footnotes~"));
        XnoteKind::numbered("footnote", words).commands(["footnote"]).label_type("fn")
    }

    /// Builder: the commands with which an xnote of the kind is written,
    /// without the backslash.
    pub fn commands(mut self, commands: impl IntoIterator<Item = impl Into<String>>) -> XnoteKind {
        self.commands = commands.into_iter().map(Into::into).collect();
        self
    }

    /// Builder: the label type the kind's xnotes accept, made of ASCII
    /// letters only.
    pub fn label_type(mut self, label_type: impl Into<String>) -> XnoteKind {
        self.label_type = label_type.into();
        self
    }

    /// Builder: how a reference writes a numbered xnote's number.
    pub fn number_format(mut self, number_format: NumberTemplate) -> XnoteKind {
        self.number_format = number_format;
        self
    }

    /// Builder: the counter within which a numbered kind is numbered.
    pub fn within(mut self, within: Within) -> XnoteKind {
        self.within = Some(within);
        self
    }

    /// Builder: the text of the mark ([`mark`](XnoteKind::mark)).
    pub fn mark(mut self, mark: NumberTemplate) -> XnoteKind {
        self.mark = mark;
        self
    }

    /// Builder: where the kind's xnotes are shown.
    pub fn placement(mut self, placement: XnotePlacement) -> XnoteKind {
        self.placement = placement;
        self
    }

    /// Builder: how the content argument is laid out.
    pub fn layout(mut self, layout: ArgLayout) -> XnoteKind {
        self.layout = layout;
        self
    }

    /// Builder: the heading placed above the kind's list, as FLM text,
    /// compiled when the registry is built (a literal `#` is written `\#` or
    /// `##`; see [`title`](XnoteKind::title)).
    pub fn title(mut self, title: impl Into<String>) -> XnoteKind {
        self.title = Some(title.into());
        self
    }

    /// Builder: no heading above the kind's list.
    pub fn without_title(mut self) -> XnoteKind {
        self.title = None;
        self
    }

    /// Builder: the level of the heading above the list, counted from zero.
    pub fn title_level(mut self, level: u8) -> XnoteKind {
        self.title_level = level;
        self
    }

    /// Builder: the text of the link back from a listed xnote to its mark.
    pub fn backlink(mut self, backlink: impl Into<String>) -> XnoteKind {
        self.backlink = backlink.into();
        self
    }

    /// Builder: the LaTeX command to which the style file's default
    /// definition of `\flmXN<name>` passes the xnote, without the backslash
    /// and made of ASCII letters: a command of the author's LaTeX, which the
    /// author's own preamble may redefine.
    pub fn latex_command(mut self, command: impl Into<String>) -> XnoteKind {
        self.latex_command = command.into();
        self
    }

    /// Checks what can be checked of one xnote kind on its own;
    /// [`contribute_kind`](super::contribute_kind) calls it before it declares
    /// anything.
    ///
    /// # Errors
    ///
    /// A sentence describing the first problem found: a name, a label type or
    /// a LaTeX command that is empty or holds anything but ASCII letters; an
    /// unnumbered kind whose mark holds a number placeholder. The number and
    /// mark templates of a numbered kind are checked by the numbering feature,
    /// when the registry compiles the kind's counter styles; the title and an
    /// unnumbered kind's fixed mark are compiled, and checked, when the
    /// registry is built.
    pub fn validate(&self) -> Result<(), String> {
        let name = self.name.as_str();
        if name.is_empty() || !name.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Err(format!(
                "the xnote kind name `{name}` must consist of ASCII letters only"
            ));
        }
        let label_type = self.label_type.as_str();
        if label_type.is_empty() || !label_type.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Err(format!(
                "the label type `{label_type}` of the xnote kind `{name}` must consist of ASCII \
                 letters only"
            ));
        }
        let latex_command = self.latex_command.as_str();
        if latex_command.is_empty() || !latex_command.chars().all(|ch| ch.is_ascii_alphabetic()) {
            // The command is written into the style file as it stands.
            // See decision rationale at [§flm-dd-dr:std-xnote-latex-command].
            return Err(format!(
                "the LaTeX command `{latex_command}` of the xnote kind `{name}` must consist of \
                 ASCII letters only, written without the backslash"
            ));
        }
        if !self.numbered && names_number(&self.mark) {
            return Err(format!(
                "the mark `{}` of the unnumbered xnote kind `{name}` must not write a number, \
                 since the kind is not numbered",
                self.mark.text()
            ));
        }
        Ok(())
    }

    /// Writes every field of the kind into `out`, for a registry digest: each
    /// text preceded by its length, so that two different kinds never write
    /// the same bytes.
    ///
    /// The fields are taken apart by name, so a field added to the kind and
    /// not written here fails to compile.
    pub(crate) fn digest_into(&self, out: &mut Vec<u8>) {
        let XnoteKind {
            name,
            commands,
            words,
            label_type,
            numbered,
            number_format,
            within,
            mark,
            placement,
            layout,
            title,
            title_level,
            backlink,
            latex_command,
        } = self;
        push_text(out, name.as_str());
        push_len(out, commands.len());
        for command in commands {
            push_text(out, command);
        }
        push_plural(out, &words.base);
        push_len(out, words.variants.len());
        for (variant, plural) in &words.variants {
            push_text(out, variant);
            push_plural(out, plural);
        }
        push_text(out, label_type);
        out.push(u8::from(*numbered));
        push_template(out, number_format);
        match within {
            Some(within) => {
                out.push(1);
                within.digest_into(out);
            }
            None => out.push(0),
        }
        push_template(out, mark);
        out.push(match placement {
            XnotePlacement::List => 0,
            XnotePlacement::InPlace => 1,
        });
        out.push(match layout {
            ArgLayout::Inline => 0,
            ArgLayout::Blocks => 1,
            ArgLayout::InlineOrBlocks => 2,
        });
        match title {
            Some(title) => {
                out.push(1);
                push_text(out, title);
            }
            None => out.push(0),
        }
        out.push(*title_level);
        push_text(out, backlink);
        push_text(out, latex_command);
    }
}

/// Whether the mark template `mark` holds a number placeholder: a `#{…}` in a
/// [`Formats`](NumberTemplate::Formats) template, or one of the five
/// characters that represent the number in an
/// [`Initial`](NumberTemplate::Initial) one.
// See decision rationale at [§flm-dd-dr:std-xnote-unnumbered-mark].
fn names_number(mark: &NumberTemplate) -> bool {
    match mark {
        NumberTemplate::Formats(text) => holds_placeholder(text),
        NumberTemplate::Initial(text) => text.chars().any(|ch| matches!(ch, 'a' | 'A' | 'i' | 'I' | '1')),
    }
}

/// The default title of an xnote kind with the reference words `words`: the
/// plural word (the capital variant's, where one is declared) without its
/// trailing non-breaking space, its first letter capitalized: `Footnotes~`
/// gives `Footnotes`.
fn plural_title(words: &PrefixWords) -> String {
    let plural = words.variants.get(CAPITAL_VARIANT).unwrap_or(&words.base);
    let word = plural.other.trim_end_matches(|ch: char| ch == '~' || ch.is_whitespace());
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The configuration of the [`XnotesFeature`](super::XnotesFeature): the xnote
/// kinds an author writes, in the order in which their lists are placed after
/// the content.
///
/// The default ([`XnotesConfig::new`]) declares one xnote kind,
/// [`XnoteKind::footnote`]: `\footnote{…}`, numbered, listed after the
/// content under the heading `Footnotes`. A second xnote kind is added by
/// configuration, with no new syntax:
///
/// ```
/// use flm_core::stdcore::features::numbering::PrefixWords;
/// use flm_core::stdcore::features::xnotes::{XnoteKind, XnotesConfig};
///
/// let config = XnotesConfig::new().kind(
///     XnoteKind::numbered("endnote", PrefixWords::new("Endnote~", "Endnotes~"))
///         .commands(["endnote"])
///         .label_type("en"),
/// );
/// assert_eq!(config.kinds.len(), 2);
/// assert_eq!(config.validate(), Ok(()));
/// ```
///
/// [`XnotesFeature::new`](super::XnotesFeature::new) takes this value; the
/// standard package carries it as
/// [`StdcoreConfig::xnotes`](crate::stdcore::StdcoreConfig::xnotes). It is a
/// feature's configuration: fixed when the registry is built, before any
/// document is parsed (see [settings, options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)),
/// because the commands are part of the language, whether an xnote is a block
/// is decided before a document's trees are frozen (no longer changed; see
/// [frozen
/// trees](crate::guide::concepts_overview#frozen-trees-and-computed-data)),
/// and the counters and
/// counter styles the feature declares depend on it. The feature therefore
/// adds [`digest_bytes`](XnotesConfig::digest_bytes) to the registry digest,
/// the fingerprint that decides whether a cached result may be reused.
///
/// The configuration is checked when the registry is built
/// ([`XnotesConfig::validate`]): every xnote kind on its own
/// ([`XnoteKind::validate`]), then two kinds of one name, two kinds sharing a
/// command, and two kinds sharing a label type are refused. A kind with no
/// command is accepted; no author can write its xnotes. A kind that another
/// feature contributes ([`contribute_kind`](super::contribute_kind)) is
/// checked on its own only: nothing refuses that it shares a label type with
/// a configured kind.
// See decision rationale at [§flm-dd-dr:std-xnote-label-type].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct XnotesConfig {
    /// The xnote kinds, in the order in which their lists are placed.
    #[serde(deserialize_with = "crate::core::settings::named::deserialize")]
    #[schemars(
        transform = crate::core::settings::named::map_schema,
        with = "std::collections::BTreeMap<String, XnoteKind>"
    )]
    pub kinds: Vec<XnoteKind>,
}

impl Default for XnotesConfig {
    fn default() -> XnotesConfig {
        XnotesConfig::new()
    }
}

impl XnotesConfig {
    /// The default configuration: a single xnote kind, [`XnoteKind::footnote`].
    pub fn new() -> XnotesConfig {
        XnotesConfig { kinds: vec![XnoteKind::footnote()] }
    }

    /// A configuration declaring no xnote kind at all.
    pub fn empty() -> XnotesConfig {
        XnotesConfig { kinds: Vec::new() }
    }

    /// Builder: adds the xnote kind `kind` after the kinds already declared.
    pub fn kind(mut self, kind: XnoteKind) -> XnotesConfig {
        self.kinds.push(kind);
        self
    }

    /// The declared xnote kind named `name`, if there is one.
    pub fn kind_named(&self, name: &str) -> Option<&XnoteKind> {
        self.kinds.iter().find(|kind| kind.name.as_str() == name)
    }

    /// Checks the configuration; the feature calls it when it registers.
    ///
    /// # Errors
    ///
    /// A sentence describing the first problem found: one of
    /// [`XnoteKind::validate`]'s, or two xnote kinds of one name, sharing a
    /// command, or sharing a label type.
    pub fn validate(&self) -> Result<(), String> {
        for (at, kind) in self.kinds.iter().enumerate() {
            kind.validate()?;
            for earlier in &self.kinds[..at] {
                if earlier.name == kind.name {
                    return Err(format!(
                        "two xnote kinds have the name `{}`; each xnote kind needs a name of its \
                         own",
                        kind.name
                    ));
                }
                if let Some(command) = kind.commands.iter().find(|c| earlier.commands.contains(c)) {
                    return Err(format!(
                        "the xnote kinds `{}` and `{}` are both written `\\{command}`; each xnote \
                         kind needs commands of its own",
                        earlier.name, kind.name
                    ));
                }
                if earlier.label_type == kind.label_type {
                    return Err(format!(
                        "the xnote kinds `{}` and `{}` both accept the label type `{}`; each \
                         xnote kind needs a label type of its own",
                        earlier.name, kind.name, kind.label_type
                    ));
                }
            }
        }
        Ok(())
    }

    /// The canonical rendering of the configuration: what enters the registry
    /// digest, and what `Hash` hashes.
    ///
    /// Every xnote kind is written out in order, every field of it, each text
    /// preceded by its length, so two configurations give the same bytes
    /// exactly when they are equal. Fields that have no effect (an unnumbered
    /// kind's number format) are written too: a digest that differs where
    /// nothing else does only prevents the reuse of a cached result, and never
    /// produces a wrong one.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        push_len(&mut out, self.kinds.len());
        for kind in &self.kinds {
            kind.digest_into(&mut out);
        }
        out
    }
}

impl Hash for XnotesConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

/// Writes a count as eight little-endian bytes.
fn push_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&(len as u64).to_le_bytes());
}

/// Writes a text as its length and its bytes.
fn push_text(out: &mut Vec<u8>, text: &str) {
    push_len(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// Writes the two words of `plural`.
fn push_plural(out: &mut Vec<u8>, plural: &PluralWords) {
    push_text(out, &plural.one);
    push_text(out, &plural.other);
}

/// Writes which language `template` is written in, then its text.
fn push_template(out: &mut Vec<u8>, template: &NumberTemplate) {
    out.push(match template {
        NumberTemplate::Formats(_) => 0,
        NumberTemplate::Initial(_) => 1,
    });
    push_text(out, template.text());
}

/// An xnote kind is written in settings under its name, in the map of
/// [`XnotesConfig::kinds`], and read over the kind that
/// [`XnoteKind::numbered`] makes for that name and the words written; the
/// words must be written.
impl NamedEntry for XnoteKind {
    fn read_entry(name: &str, data: serde_json::Value) -> Result<XnoteKind, String> {
        let words: PrefixWords = named::required(name, &data, "words")?;
        named::read_over(&XnoteKind::numbered(name, words), "name", name, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_kind_is_the_footnote() {
        let config = XnotesConfig::new();
        assert_eq!(config.kinds.len(), 1);
        let kind = &config.kinds[0];
        assert_eq!(kind.name.as_str(), "footnote");
        assert_eq!(kind.commands, ["footnote"]);
        assert_eq!(kind.words.base, PluralWords::new("Footnote~", "Footnotes~"));
        assert_eq!(kind.words.variants.get(CAPITAL_VARIANT), Some(&kind.words.base));
        assert_eq!(kind.label_type, "fn");
        assert!(kind.numbered);
        assert_eq!(kind.number_format, NumberTemplate::formats("#{arabic}"));
        assert_eq!(kind.mark, NumberTemplate::formats("#{arabic}"));
        assert!(kind.within.is_none());
        assert_eq!(kind.placement, XnotePlacement::List);
        assert_eq!(kind.layout, ArgLayout::InlineOrBlocks);
        assert_eq!(kind.title.as_deref(), Some("Footnotes"));
        assert_eq!(kind.title_level, 2);
        assert_eq!(kind.backlink, "\u{21a9}");
        assert_eq!(kind.latex_command, "footnote");
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn the_default_title_is_the_plural_word_capitalized() {
        let kind = XnoteKind::numbered("endnote", PrefixWords::new("endnote~", "endnotes~"));
        assert_eq!(kind.title.as_deref(), Some("Endnotes"));
        let words = PrefixWords::new("n.~", "nn.~").with_variant(CAPITAL_VARIANT, PluralWords::new("Note~", "Notes~"));
        assert_eq!(XnoteKind::unnumbered("aside", words).title.as_deref(), Some("Notes"));
        assert_eq!(XnoteKind::numbered("x", PrefixWords::default()).title.as_deref(), Some(""));
    }

    #[test]
    fn the_refusals_name_what_is_wrong() {
        let refused = |config: XnotesConfig| config.validate().expect_err("refused");
        let words = || PrefixWords::new("Note~", "Notes~");
        assert!(refused(XnotesConfig::empty().kind(XnoteKind::numbered("side-note", words())))
            .contains("`side-note`"));
        assert!(refused(XnotesConfig::empty().kind(XnoteKind::numbered("", words()))).contains("ASCII"));
        for label_type in ["", "fn:x", "side-note", "n1"] {
            let kind = XnoteKind::numbered("other", words()).label_type(label_type);
            assert!(
                refused(XnotesConfig::empty().kind(kind)).contains("ASCII letters only"),
                "{label_type:?}"
            );
        }
        assert!(refused(XnotesConfig::new().kind(XnoteKind::footnote().label_type("x")))
            .contains("two xnote kinds have the name"));
        assert!(refused(
            XnotesConfig::new().kind(XnoteKind::numbered("other", words()).commands(["footnote"]))
        )
        .contains("\\footnote"));
        assert!(refused(XnotesConfig::new().kind(XnoteKind::numbered("other", words()).label_type("fn")))
            .contains("label type `fn`"));
        let level_zero = XnoteKind::numbered("other", words()).title_level(0);
        assert_eq!(level_zero.validate(), Ok(()), "a title level of 0 is accepted");
        let marked = XnoteKind::unnumbered("aside", words()).mark(NumberTemplate::formats("(#{alph})"));
        assert!(refused(XnotesConfig::empty().kind(marked)).contains("must not write a number"));
        let marked = XnoteKind::unnumbered("aside", words()).mark(NumberTemplate::initial("(a)"));
        assert!(refused(XnotesConfig::empty().kind(marked)).contains("must not write a number"));
    }

    #[test]
    fn what_is_accepted() {
        let words = || PrefixWords::new("Note~", "Notes~");
        let config = XnotesConfig::new()
            .kind(XnoteKind::numbered("addendum", words()).label_type("add"))
            .kind(XnoteKind::unnumbered("aside", words()).mark(NumberTemplate::formats("\u{2020}")))
            .kind(XnoteKind::unnumbered("star", words()).mark(NumberTemplate::formats("\\#")))
            // An unnumbered kind's number format is ignored, not checked.
            .kind(
                XnoteKind::unnumbered("plain", words())
                    .label_type("pl")
                    .number_format(NumberTemplate::formats("#{nosuchformat}")),
            );
        // Two kinds with no command share none.
        let config = config.kind(XnoteKind::numbered("other", words()));
        assert_eq!(config.validate(), Ok(()), "{config:?}");
    }

    #[test]
    fn the_digest_bytes_change_with_every_field() {
        let base = XnotesConfig::new();
        let kind = XnoteKind::footnote;
        let one = |kind: XnoteKind| XnotesConfig { kinds: vec![kind] };
        let changed = [
            one(kind().commands(["fn", "footnote"])),
            one(XnoteKind { words: PrefixWords::new("Note~", "Notes~"), ..kind() }),
            one(kind().label_type("note")),
            one(XnoteKind { numbered: false, ..kind() }),
            one(kind().number_format(NumberTemplate::formats("#{roman}"))),
            one(kind().number_format(NumberTemplate::initial("#{arabic}"))),
            one(kind().within(Within::parent("flm.headings.section"))),
            one(kind().mark(NumberTemplate::formats("[#{arabic}]"))),
            one(kind().placement(XnotePlacement::InPlace)),
            one(kind().layout(ArgLayout::Blocks)),
            one(kind().title("Notes")),
            one(kind().without_title()),
            one(kind().title_level(3)),
            one(kind().backlink("back")),
            one(kind().latex_command("endnote")),
            XnotesConfig::empty(),
            base.clone().kind(XnoteKind::numbered("endnote", PrefixWords::new("E~", "Es~"))),
        ];
        for other in &changed {
            assert_ne!(other.digest_bytes(), base.digest_bytes(), "{other:?}");
        }
        assert_eq!(XnotesConfig::new().digest_bytes(), base.digest_bytes());
    }
}
