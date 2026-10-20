//! The data the FLM language attaches to parsing states and to nodes.
//!
//! techy lets a language attach data of its own to parsing states, to nodes,
//! to arguments and to node slots (techy's `ParsedSlot`, the parts of a
//! node's content outside its arguments, such as an environment's body).
//! FLM attaches its own data to parsing states and to nodes (what arguments
//! and slots carry is listed at
//! [`FlmNodeExts`](crate::lang::flm::FlmNodeExts)):
//!
//! - The **state extension**, [`FlmStateExt`], is attached to every parsing
//!   state. It holds what applies at the point where a node is built: the
//!   [structural elements](crate::lang::structural), the restricted-interior
//!   policy ([`RestrictedInterior`]) and the parsing settings of the file being
//!   read ([`FlmParsing`]). A parsing state passes it on to the states derived
//!   from it.
//! - The **node extension**, [`FlmNodeExt`], is attached to every node. It is
//!   built once, when techy adds the node to a tree, and never changes
//!   afterwards. It holds the node's [element](crate::model::element::ElementSpec)
//!   and two optional fields: where the characters of a rewritten text node come
//!   from ([`TextSourceMap`]), and the typed payload of a presentation node, or
//!   of a synthesized or rebuilt node that carries data of its own
//!   ([`PresPayload`]).
//!
//! A [`TextSourceMap`] is a list of text source pieces ([`TextSourcePiece`]),
//! each saying which source range a stretch of the node's text comes from; the
//! pieces cover the text in order.
//!
//! A [`RestrictedInterior`] policy says what the interior of a restricted
//! argument (a characters-only group or a verbatim region) still recognizes. In the parse of a document, it
//! recognizes no command at all. In the parse of a [user
//! definition](crate::lang::rewrites::userdefs::Definition)'s template, the
//! policy keeps the command syntax of a placeholder enabled, so that a
//! placeholder written inside a label, a reference target or raw text is still
//! recognized. The policy lists tokenizer rules, so this module does not depend
//! on how a placeholder is written.

use std::any::Any;
use std::fmt;
use std::sync::{Arc, LazyLock};

use techy::core::token::{
    CommandOverrides, CommandRule, CommentOverrides, GroupOverrides, GroupRule,
    SpecialsOverrides, TokenRulesOverrides,
};
use techy::core::constructs::verbatim_state_delta;
use techy::core::{ParsingState, ParsingStateDelta};
use techy::serialize::{DeserializableValue, SerialValue, SerializableValue};

use crate::model::element::ElementSpec;
use crate::lang::flm::{Flm, FlmGroupType};
use crate::lang::origin::FlmSpan;
use crate::lang::rules::FlmParsing;
use crate::lang::structural::StructuralElements;

// --- the presentation payload -------------------------------------------------------

/// The trait every node payload type implements. A payload is the typed value
/// a node holds beyond its element and its children; presentation nodes carry
/// most payloads.
///
/// Lowering builds a presentation tree from a frozen document tree for one
/// backend, and each presentation node carries a payload of the type its
/// presentation element defines (a styled span's style, a block's role, a
/// link's target). The backend that defines those presentation elements also
/// defines the payload types. This trait is the only interface through which
/// the [projection](crate::dump::project) (the serialized view of a tree that
/// tests compare and that is printed for inspection) and the [serialization of
/// a parse](crate::dump::session) read a payload, so that both can print and
/// store it without knowing its concrete type.
///
/// Two other kinds of node may carry a payload:
///
/// - A node of a [synthesized element](crate::lang::mint::SyntaxDef::Synthesized)
///   (an element no document writes, which a stage builds into a
///   [synthesized value](crate::guide::concepts_overview#synthesized-values))
///   may carry data that none of its arguments holds. For example, an
///   [xnote](crate::stdcore::features::xnotes) is a note set apart from the
///   running text and shown elsewhere or in place (a footnote, an endnote, a
///   side note); the node that the list of xnotes holds for each xnote carries
///   the address of that xnote. Such a payload may refer to a node of the document, which means
///   something only in the run that built it. No reader is registered for it,
///   so a tree holding one can be printed but not read back.
/// - A node of the document that a rewrite pass rebuilt may carry a payload:
///   each part of a cut [block-transparent
///   element](crate::guide::concepts_overview#block-transparent-elements)
///   carries the record its element attached, saying which cut it came from. Such a part is in the
///   document tree itself, so a frozen tree holding it must read back from its
///   serialized form like any other tree. The record is therefore plain data
///   (which cut, which part, how many parts), and the feature that defines the
///   element registers a reader for its payloads under the element's name in
///   the reading session's [`PayloadReaders`](crate::dump::session::PayloadReaders)
///   ([`reader_with`](crate::dump::session::reader_with)), as a backend does for
///   its presentation elements.
///
/// A node's payload is held in [`FlmNodeExt::payload`].
pub trait PresPayload: Any + Send + Sync + fmt::Debug {
    /// Returns the payload as plain serialized data: what the
    /// [projection](crate::dump::project) prints and what the
    /// [serialization](crate::dump::session) writes.
    ///
    /// The reader registered for the node's element receives **exactly this
    /// value** and must rebuild the payload from it alone
    /// ([`PayloadReaders`](crate::dump::session::PayloadReaders)). The value
    /// returned for a presentation node or a rebuilt node must therefore be
    /// self-contained: no offset relative to the node's source, no node
    /// identifier, nothing that only makes sense next to the tree it came from.
    ///
    /// The payload of a synthesized node is the exception, because no reader
    /// rebuilds it (see the [trait documentation](PresPayload)): its value may
    /// refer to the document node it shows, for a person reading the printed
    /// tree, and it is never read back.
    fn project(&self) -> SerialValue;
}

impl dyn PresPayload {
    /// Returns whether this payload is a value of type `T`.
    pub fn is<T: PresPayload>(&self) -> bool {
        (self as &dyn Any).is::<T>()
    }

    /// Returns this payload as a `T`, or `None` when it is a payload of another
    /// type.
    pub fn downcast_ref<T: PresPayload>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }
}

// --- the text source map ------------------------------------------------------------

/// One text source piece of a [`TextSourceMap`]: the next
/// [`out_len`](Self::out_len) bytes of the node's text come from the source
/// bytes that [`span`](Self::span) covers.
///
/// Serialized form: `{len, span}`, the span in techy's own form, which records
/// the source it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, SerializableValue, DeserializableValue)]
#[serial(lang = Flm)]
pub struct TextSourcePiece {
    /// How many bytes of the node's text this piece covers.
    #[serial(name = "len")]
    pub out_len: u32,
    /// The source bytes those text bytes come from. The span carries its own
    /// source, so the pieces of one map may come from different sources.
    #[serial(name = "span")]
    pub span: FlmSpan,
}

/// Where each byte of a text node's text comes from, for a text node whose span
/// alone does not say it.
///
/// Most text nodes are a byte-for-byte copy of one source range, and their span
/// accounts for every byte; such nodes carry no map. The block-structure
/// rewrite pass ([`crate::lang::rewrites::blocks`]) merges adjacent text nodes,
/// collapses whitespace and applies typographic substitutions; a node whose
/// text is then no longer a copy of one source range carries this map, in
/// [`FlmNodeExt::source_map`].
///
/// The [text source pieces](Self::pieces) cover the node's text in order.
/// Each maximal stretch of text that is a byte-for-byte copy of one source
/// range is one piece, and a new piece starts wherever that stops being true:
/// at a collapsed whitespace run, where two text nodes were merged, and at a
/// substitution, even one that keeps the length (`---` becomes `—`, also three
/// bytes).
///
/// Each piece carries its own span, and a span records its source, so one map
/// may refer to several sources: text merged across the boundary of an
/// expanded user definition does.
///
/// Serialized form: `{pieces: [...]}`.
#[derive(Debug, Clone, PartialEq, Eq, Default, SerializableValue, DeserializableValue)]
#[serial(lang = Flm)]
pub struct TextSourceMap {
    #[serial(name = "pieces")]
    pieces: Vec<TextSourcePiece>,
}

impl TextSourceMap {
    /// Returns a map over `pieces`, which must already be in text order.
    pub fn new(pieces: Vec<TextSourcePiece>) -> TextSourceMap {
        TextSourceMap { pieces }
    }

    /// Returns the text source pieces, in the order in which they cover the node's text.
    pub fn pieces(&self) -> &[TextSourcePiece] {
        &self.pieces
    }

    /// Returns how many bytes of the node's text the text source pieces cover,
    /// in total.
    ///
    /// # Panics
    ///
    /// In debug builds, if the piece lengths sum to more than a `u32` holds. A
    /// map is built from one node's text, so reaching that would take a text
    /// node of more than four gigabytes.
    pub fn total_len(&self) -> u32 {
        self.pieces.iter().map(|piece| piece.out_len).sum()
    }

    /// Returns the text source piece covering the byte at `offset` in the
    /// node's text, together with the byte's offset within that piece; `None`
    /// for an offset at or past [`total_len`](Self::total_len).
    ///
    /// The piece's span says which source range that stretch of text comes
    /// from. The byte at `offset` is not necessarily the byte at the same
    /// distance into the span: a piece whose text was substituted or collapsed
    /// covers a source range of a different length.
    ///
    /// # Panics
    ///
    /// In debug builds, under the same overflow condition as
    /// [`total_len`](Self::total_len).
    pub fn piece_at(&self, offset: u32) -> Option<(&TextSourcePiece, u32)> {
        let mut start = 0u32;
        for piece in &self.pieces {
            let end = start + piece.out_len;
            if offset < end {
                return Some((piece, offset - start));
            }
            start = end;
        }
        None
    }
}

// --- the node extension -----------------------------------------------------------

/// The data FLM attaches to every node: its element, and two optional fields.
///
/// [`Flm::make_node_ext`](crate::techy::core::Lang::make_node_ext) builds it
/// once, when techy adds the node to a tree. Every node has an
/// [element](Self::element). A text node whose characters were rewritten also
/// has a [source map](Self::source_map). A presentation node, and a synthesized
/// or rebuilt node that carries data of its own, also has a
/// [payload](Self::payload). A node copied into another tree keeps its
/// extension unchanged; a node the library creates gets a new extension when it
/// is created.
#[derive(Clone)]
pub struct FlmNodeExt {
    /// The node's element: what the node means.
    pub element: Arc<ElementSpec>,
    /// Where each byte of a text node's text comes from; `None` means the text
    /// is a byte-for-byte copy of the node's span.
    pub source_map: Option<Arc<TextSourceMap>>,
    /// The typed payload ([`PresPayload`]) of a presentation node, of a
    /// synthesized node that carries data of its own, or of a node a rewrite
    /// pass rebuilt with data of its own (a part of a cut [block-transparent
    /// element](crate::guide::concepts_overview#block-transparent-elements),
    /// carrying its cut
    /// record); `None` on every other node.
    pub payload: Option<Arc<dyn PresPayload>>,
}

impl FlmNodeExt {
    /// Returns the extension of a node carrying `element`, with no source map
    /// and no payload. A parse builds this for every node.
    pub fn new(element: Arc<ElementSpec>) -> FlmNodeExt {
        FlmNodeExt { element, source_map: None, payload: None }
    }

    /// Builder: sets `map` as the node's source map
    /// ([`source_map`](Self::source_map)).
    pub fn with_source_map(mut self, map: Arc<TextSourceMap>) -> FlmNodeExt {
        self.source_map = Some(map);
        self
    }

    /// Builder: sets `payload` as the node's payload
    /// ([`payload`](Self::payload)).
    pub fn with_payload(mut self, payload: Arc<dyn PresPayload>) -> FlmNodeExt {
        self.payload = Some(payload);
        self
    }
}

impl fmt::Debug for FlmNodeExt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlmNodeExt")
            .field("element", &self.element.name)
            .field("source_map", &self.source_map.as_ref().map(|map| map.pieces().len()))
            .field("payload", &self.payload)
            .finish()
    }
}

// --- the state extension ------------------------------------------------------------

/// One command syntax that stays recognized inside restricted arguments and
/// verbatim regions during the parse of a template: an escape character, and
/// the characters that may follow it to form a command name.
///
/// This is techy's [`CommandRule`] as plain data, so that a policy can be
/// declared and serialized without holding a parser type.
///
/// Serialized form: `{escape_char, name_chars}`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize,
    SerializableValue, DeserializableValue,
)]
pub struct TemplateCommandRule {
    /// The escape character that starts the command.
    #[serial(name = "escape_char")]
    pub escape_char: char,
    /// The characters that form a multi-character name after the escape
    /// character; empty when only single-character names exist.
    #[serial(name = "name_chars")]
    pub name_chars: String,
}

/// The tokenizer rules that stay enabled inside a restricted argument or a
/// verbatim region while the template of a [user
/// definition](crate::lang::rewrites::userdefs::Definition) is parsed.
///
/// A user definition may pass one of its arguments into a label, a reference
/// target or raw text, and the placeholder that marks where the argument goes
/// must still be recognized there. The parse of a template therefore keeps the
/// rules listed here enabled inside those regions; the template language keeps
/// only the command syntax that starts a placeholder (`#`). Everything else is
/// read as in a document.
///
/// Serialized form: `{commands: [...], specials}`.
#[derive(
    Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
    SerializableValue, DeserializableValue,
)]
#[serde(default)]
pub struct TemplateInteriorRules {
    /// The command syntaxes that stay recognized; an empty list leaves commands
    /// off.
    #[serial(name = "commands")]
    pub commands: Vec<TemplateCommandRule>,
    /// Whether special character sequences are still recognized.
    #[serial(name = "specials")]
    pub specials: bool,
}

/// The restricted-interior policy: what the interior of a **restricted
/// argument** recognizes. The parsing state carries it
/// ([`FlmStateExt::restricted_interior`]).
///
/// The language has two restricted argument shapes, each read by a parser of
/// its own: the characters-only group
/// ([`FlmCharsGroupArgumentParser`](crate::lang::chars_group::FlmCharsGroupArgumentParser),
/// as in `\label{…}` and `\ref{…}`) and the verbatim argument or environment
/// body ([`crate::lang::verbatim`], as in `\verbcode{…}`). Both parsers read
/// their interior under the parsing state this policy derives
/// ([`chars_group_delta`](Self::chars_group_delta),
/// [`verbatim_delta`](Self::verbatim_delta)), so one setting on the state
/// decides for both whether a placeholder is recognized inside them. A
/// [terminated argument](crate::lang::mint::ArgStdKind::Terminated) read as
/// raw text is verbatim content too, and is read under the same policy
/// ([`raw_text_delta`](Self::raw_text_delta)). The
/// [module documentation of
/// `lang::verbatim`](crate::lang::verbatim#the-two-restricted-argument-shapes)
/// states how the two shapes differ.
///
/// Serialized form: `"document"`, or `{template: {commands, specials}}`.
#[derive(
    Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
    SerializableValue, DeserializableValue,
)]
#[serde(rename_all = "kebab-case")]
pub enum RestrictedInterior {
    /// The policy of a document parse: no command and no special character
    /// sequence is recognized inside a restricted argument, so `#` and `\` are
    /// ordinary characters there.
    #[default]
    #[serial(name = "document")]
    Document,
    /// The policy of a template parse: the listed rules stay enabled inside a
    /// restricted argument, so a placeholder written there is recognized.
    #[serial(name = "template")]
    Template(TemplateInteriorRules),
}

impl RestrictedInterior {
    /// Returns the change of parsing state under which a verbatim region's
    /// interior is read: techy's verbatim change ([`verbatim_state_delta`]:
    /// every tokenizer rule off, with `terminator` as the only recognized
    /// closing delimiter) and, under [`Template`](Self::Template), the listed
    /// rules enabled again.
    pub fn verbatim_delta(&self, terminator: Arc<GroupRule<Flm>>) -> ParsingStateDelta<Flm> {
        let mut delta = verbatim_state_delta(terminator);
        self.keep_template_rules(&mut delta.rules);
        delta
    }

    /// Returns the change of parsing state under which raw text is read by a
    /// parser that installs its terminator itself: the change of
    /// [`verbatim_delta`](Self::verbatim_delta) (every tokenizer rule off,
    /// and under [`Template`](Self::Template) the listed rules enabled
    /// again), with the expected closing delimiter left as it is.
    ///
    /// A [terminated argument](crate::lang::mint::ArgStdKind::Terminated)
    /// read as raw text uses it: the terminator of such an argument depends
    /// on the run of characters that opens it at the use site, so the
    /// terminator is known only once the parser has read that run, and the
    /// parser then installs it as the expected closing delimiter of the
    /// content.
    pub fn raw_text_delta(&self) -> ParsingStateDelta<Flm> {
        let mut rules = TokenRulesOverrides::disable_all();
        self.keep_template_rules(&mut rules);
        ParsingStateDelta::new().rules(rules)
    }

    /// Returns the change of parsing state under which a characters-only
    /// group's interior is read. `base` is the state the argument itself is
    /// parsed under, and `group_type` the group class of the argument's group.
    ///
    /// The change switches commands and special character sequences off and
    /// comments on, and keeps only the group rules of `base` whose class is
    /// `group_type`, so that the delimiters of mathematics, a different group
    /// class, are read as characters. Under [`Template`](Self::Template), the
    /// listed rules are enabled again on top of that.
    pub fn chars_group_delta(
        &self,
        base: &ParsingState<Flm>,
        group_type: FlmGroupType,
    ) -> ParsingStateDelta<Flm> {
        let mut rules = TokenRulesOverrides {
            commands: CommandOverrides::disable(),
            specials: SpecialsOverrides::disable(),
            comments: CommentOverrides { enabled: Some(true), ..CommentOverrides::default() },
            groups: GroupOverrides {
                rules: Some(
                    base.rules()
                        .group_rules()
                        .iter()
                        .filter(|rule| rule.group_type == group_type)
                        .cloned()
                        .collect(),
                ),
                ..GroupOverrides::default()
            },
            ..TokenRulesOverrides::default()
        };
        self.keep_template_rules(&mut rules);
        ParsingStateDelta::new().rules(rules)
    }

    /// Under [`Template`](Self::Template), turns the listed rules back on in
    /// `rules`; under [`Document`](Self::Document), leaves `rules` as it is.
    fn keep_template_rules(&self, rules: &mut TokenRulesOverrides<Flm>) {
        let RestrictedInterior::Template(template) = self else {
            return;
        };
        if !template.commands.is_empty() {
            let commands = template
                .commands
                .iter()
                .map(|rule| {
                    Arc::new(CommandRule {
                        escape_char: rule.escape_char,
                        name_chars: rule.name_chars.clone(),
                    })
                })
                .collect();
            rules.commands = CommandOverrides { enabled: Some(true), rules: Some(commands) };
        }
        if template.specials {
            rules.specials = SpecialsOverrides { enabled: Some(true) };
        }
    }

    /// Returns whether this is the [`Template`](Self::Template) policy.
    pub fn is_template(&self) -> bool {
        matches!(self, RestrictedInterior::Template(_))
    }
}

/// The shared default structural elements: the set a state carries until a
/// built language installs the registry's own. techy requires a language's
/// state extension to implement `Default`, and that default refers to this set.
static DEFAULT_STRUCTURAL: LazyLock<Arc<StructuralElements>> =
    LazyLock::new(|| Arc::new(StructuralElements::default()));

/// The state extension: the data FLM attaches to every parsing state, which
/// applies at the point where a node is built.
///
/// It holds the [structural elements](Self::structural), from which every node
/// that is not a declared callable takes its element; the
/// [restricted-interior policy](Self::restricted_interior); and the [parsing
/// settings](Self::parsing) of the file being read.
///
/// The value never changes in place. A different extension is installed by
/// deriving a new parsing state with it (`ParsingStateDelta::new().ext(…)`):
/// [`crate::lang::build`] installs the registry's structural elements that way,
/// and the parse of a template switches to the
/// [`Template`](RestrictedInterior::Template) policy that way.
#[derive(Debug, Clone)]
pub struct FlmStateExt {
    /// The structural elements that apply: the elements of the nodes that are
    /// not declared callables.
    pub structural: Arc<StructuralElements>,
    /// What the interior of a restricted argument (a characters-only group or
    /// a verbatim region) recognizes.
    pub restricted_interior: RestrictedInterior,
    /// The parsing settings under which the current file is read.
    ///
    /// In the document's primary file, these are the document's settings, which
    /// the initial state of a built language carries. In a file included with
    /// `\input`, they are the settings that apply where the command is written,
    /// with the included file's own settings on top ([`crate::lang::input`]).
    /// The language's initial parsing state, from which no document is parsed,
    /// carries the defaults.
    ///
    /// The tokenizer rules of the state are derived from these settings; this
    /// field records which settings they came from, for the code that needs
    /// them during the parse: `\input` passes them to the embedding program
    /// ([`InputResolver::resolve`](crate::lang::InputResolver::resolve)), and
    /// the diagnostic for a lone `%`
    /// ([`PercentForbidden`](crate::lang::PercentForbidden)) states from them how
    /// to start a comment.
    pub parsing: Arc<FlmParsing>,
}

impl FlmStateExt {
    /// Returns an extension carrying `structural`, with the
    /// [`Document`](RestrictedInterior::Document) restricted-interior policy and
    /// the default parsing settings.
    pub fn new(structural: Arc<StructuralElements>) -> FlmStateExt {
        FlmStateExt {
            structural,
            restricted_interior: RestrictedInterior::Document,
            parsing: Arc::new(FlmParsing::default()),
        }
    }

    /// Builder: sets the parsing settings of the file being read
    /// ([`parsing`](Self::parsing)). It only records them and changes no
    /// tokenizer rule: the caller applies the matching rules
    /// ([`FlmParsing::state_delta`] or [`FlmParsing::token_rules_overrides`]) in
    /// the same derivation of the parsing state.
    pub fn with_parsing(mut self, parsing: FlmParsing) -> FlmStateExt {
        self.parsing = Arc::new(parsing);
        self
    }

    /// Builder: sets the restricted-interior policy
    /// ([`restricted_interior`](Self::restricted_interior)).
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::lang::{FlmStateExt, RestrictedInterior};
    ///
    /// let ext = FlmStateExt::default()
    ///     .with_restricted_interior(RestrictedInterior::Template(Default::default()));
    /// assert!(ext.restricted_interior.is_template());
    /// ```
    pub fn with_restricted_interior(mut self, policy: RestrictedInterior) -> FlmStateExt {
        self.restricted_interior = policy;
        self
    }
}

impl Default for FlmStateExt {
    fn default() -> FlmStateExt {
        FlmStateExt::new(Arc::clone(&DEFAULT_STRUCTURAL))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use techy::source::{Source, SourceSpan};

    use crate::lang::origin::FlmSource;

    #[test]
    fn a_source_map_locates_the_piece_of_a_text_offset() {
        let source: Arc<FlmSource> = Arc::new(Source::new("ab  cd"));
        let map = TextSourceMap::new(vec![
            TextSourcePiece { out_len: 2, span: SourceSpan::new(&source, 0..2) },
            TextSourcePiece { out_len: 1, span: SourceSpan::new(&source, 2..4) },
            TextSourcePiece { out_len: 2, span: SourceSpan::new(&source, 4..6) },
        ]);
        assert_eq!(map.total_len(), 5);
        let (piece, within) = map.piece_at(3).unwrap();
        assert_eq!(piece.span.range(), 4..6);
        assert_eq!(within, 0);
        assert!(map.piece_at(5).is_none());
    }

    #[test]
    fn the_document_policy_is_the_default() {
        assert_eq!(FlmStateExt::default().restricted_interior, RestrictedInterior::Document);
        assert!(!RestrictedInterior::Document.is_template());
    }
}
