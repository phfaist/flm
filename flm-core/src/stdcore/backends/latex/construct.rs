//! The four LaTeX construct elements (a macro call, an environment, raw LaTeX
//! text and a definition) with their typed payloads, and the placeholder rule
//! of the recomposed LaTeX.
//!
//! A construct element is a presentation element that mirrors one construct
//! of an output format, so that format-specific output is structured data in
//! the lowered tree rather than a string ([construct
//! element](crate::guide::concepts_overview#backends-lowering-and-outputs)). A
//! lowering rule stages one of these four elements to write LaTeX that no
//! shared presentation element ([`crate::stdcore::backends::pres`]) can
//! express: the shared elements include no catch-all element and no element
//! that carries raw bytes for a named format. The four are named
//! `flm.pres.latex.<name>`, and every LaTeX backend writes them natively.
//! Like every presentation element, each node carries exactly one typed
//! payload on the node's extension ([`FlmNodeExt`]),
//! and holds its content only in its content slots (the named groups of a
//! presentation node's children;
//! [`read`](crate::stdcore::backends::pres::read)). A content slot of these
//! nodes is a node slot in techy's sense (techy's `ParsedSlot`): a part of a
//! node's content outside its arguments, such as an environment's body.
//!
//! | element | payload | content slots |
//! |---|---|---|
//! | [`MACRO`] (`flm.pres.latex.macro`) | [`LatexMacroPayload`]: the command name, the shape of each argument written, and whether the arguments are handed over in one group | one [`ARG_SLOT`] per argument that has content (a star has none), in order |
//! | [`ENVIRONMENT`] (`flm.pres.latex.environment`) | [`LatexEnvironmentPayload`]: the environment name, the argument shapes, whether the body is verbatim, whether the arguments are handed over in one group, whether the environment takes its body as an argument | the argument slots, then the `body` slot ([`BODY_SLOT`](crate::stdcore::backends::pres::BODY_SLOT)) |
//! | [`RAW`] (`flm.pres.latex.raw`) | [`LatexRawPayload`]: LaTeX text written byte for byte | none |
//! | [`DEFINITION`] (`flm.pres.latex.definition`) | [`LatexDefinitionPayload`]: a macro or environment definition's name, how many arguments it takes, and how it is written; or a feature's declaration and the name it is guarded on | `body` for a macro and a declaration, [`BEGIN_SLOT`] and [`END_SLOT`] for an environment |
//!
//! An emitter writes every construct from its payload and normalizes the
//! call syntax: a mandatory argument is always braced, no space follows a
//! command name, and `\begin{name}` is followed directly by its arguments and
//! body. The content of each content slot is the document's own and is written through
//! the emitter's sink ([`super::sink`]). A definition node is never written
//! where it occurs in the tree: the emitter records the definition for the
//! style file ([`super::style`]).
//!
//! # Projections
//!
//! Every payload implements [`PresPayload`] with a self-contained projection
//! (the value written when a lowered tree is serialized,
//! [`crate::dump::session`], and printed by the projection,
//! [`crate::dump::project`]) and has
//! a registered reader that rebuilds the payload from that projection
//! ([`payload_readers`]). The projections are: `macro`:
//! `{name, args?, packed?}`; `environment`: `{name, args?, verbatim, packed?,
//! body?}`; `raw`: `{text}`; `definition`: `{kind, name, source?, args?,
//! packed?, robust?, body?, feature?}`. The optional keys are written only when
//! they hold something:
//!
//! - `args`: on a call, the list of shape names ([`LatexArgShape::as_str`]),
//!   one per argument the call writes, present when the list is not empty; on
//!   a definition, how many arguments the definition takes, present when it
//!   takes at least one.
//! - `packed`: the arguments are handed over in one brace group, one pair of
//!   braces around each ([`LatexMacroPayload::packed`],
//!   [`LatexDefinitionPayload::packed`]); a call and the definition it calls
//!   agree on it.
//! - `robust`: a macro definition is written as a robust command rather than
//!   as an expandable macro ([`LatexDefinitionPayload::robust`]); only a
//!   macro definition carries it.
//! - `body`: the environment takes what is written between `\begin` and
//!   `\end` as its last argument rather than letting LaTeX set it between the
//!   begin code and the end code
//!   ([`LatexDefinitionPayload::body_argument`],
//!   [`LatexEnvironmentPayload::body_argument`]); only an environment carries
//!   it, its definition and its calls alike, with the same value.
//! - `source`: the name the document declared, where it differs from `name`.
//! - `feature`: the feature that wrote the definition from its configuration
//!   ([`LatexDefinitionPayload::feature`]).
//!
//! # Reading a node's content slots
//!
//! Two readers at the end of the module ([`arg_slots`], [`slot_named`]) pair
//! a node's content slots with its payload for an emitter.
//!
//! # The placeholder rule
//!
//! When a lowering rule of the `latex-recomposed` backend copies a template
//! (the template of a kept user definition,
//! [`kept`](crate::stdcore::features::userdefs::kept), or a list label
//! template), it records on the copy of each placeholder a [`LatexPoint`]
//! value: the LaTeX text written at that placeholder. The placeholder rule
//! ([`placeholder_rule`], `flm.placeholder.lower.latex-recomposed`), which
//! the `latex` feature registers, writes that text.

use std::fmt;
use std::sync::Arc;

use techy::core::node::{
    BuildId, CallableData, NodeKind, NodeRef, NodeSlice, ParsedArguments, ParsedSlots,
};
use techy::serialize::{DeserializeContext, DeserializeError, SerialValue};
use techy::transform::RestageContext;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::dump::session::PayloadReaders;
use crate::lang::structural::PLACEHOLDER;
use crate::lang::{Flm, FlmNodeExt, PresPayload};
use crate::model::element::{element_of, element_value};
use crate::model::names::ElementName;
use crate::stdcore::backends::latex_recomposed::BACKEND as RECOMPOSED_BACKEND;
use crate::stdcore::backends::pres::payload::{bad, entries, field, map, text};

/// `flm.pres.latex.macro` — a macro call: `\name` followed by its arguments,
/// one content slot per argument that has content.
pub const MACRO: &str = "flm.pres.latex.macro";
/// `flm.pres.latex.environment` — an environment: `\begin{name}`, its
/// arguments, its body, `\end{name}`.
pub const ENVIRONMENT: &str = "flm.pres.latex.environment";
/// `flm.pres.latex.raw` — raw LaTeX text written byte for byte; the node has
/// no children.
pub const RAW: &str = "flm.pres.latex.raw";
/// `flm.pres.latex.definition` — a macro or environment definition, or a
/// feature's declaration, written into the style file and never where the node
/// occurs in the tree.
pub const DEFINITION: &str = "flm.pres.latex.definition";

/// The four construct elements, in registration order.
pub const ALL: [&str; 4] = [MACRO, ENVIRONMENT, RAW, DEFINITION];

/// The name of every argument's content slot on a macro or environment node.
/// The name repeats once per argument, so these content slots are read by
/// index ([`arg_slots`]).
pub const ARG_SLOT: &str = "arg";
/// The name of the content slot holding an environment definition's begin
/// code.
pub const BEGIN_SLOT: &str = "begin";
/// The name of the content slot holding an environment definition's end
/// code.
pub const END_SLOT: &str = "end";

// --- projection helpers ---------------------------------------------------------------
//
// `text`, `map`, `bad`, `entries` and `field` are the presentation payloads'
// own helpers, shared from `crate::stdcore::backends::pres::payload`; the
// ones below know something about these payloads in particular.

/// A required string field.
fn string(
    element: &str,
    entries: &[(String, SerialValue)],
    key: &str,
) -> Result<String, DeserializeError> {
    match field(entries, key) {
        Some(SerialValue::Str(s)) => Ok(s.clone()),
        Some(other) => Err(bad(element, format!("`{key}` must be a string, found {other:?}"))),
        None => Err(bad(element, format!("`{key}` is missing"))),
    }
}

/// A required boolean field.
fn boolean(
    element: &str,
    entries: &[(String, SerialValue)],
    key: &str,
) -> Result<bool, DeserializeError> {
    match field(entries, key) {
        Some(SerialValue::Bool(b)) => Ok(*b),
        Some(other) => Err(bad(element, format!("`{key}` must be a boolean, found {other:?}"))),
        None => Err(bad(element, format!("`{key}` is missing"))),
    }
}

/// The list of argument shapes under `key`; an absent or null key reads as no
/// arguments.
fn shapes(
    element: &str,
    entries: &[(String, SerialValue)],
    key: &str,
) -> Result<Vec<LatexArgShape>, DeserializeError> {
    match field(entries, key) {
        None | Some(SerialValue::Null) => Ok(Vec::new()),
        Some(SerialValue::List(items)) => items
            .iter()
            .map(|item| match item {
                SerialValue::Str(s) => LatexArgShape::parse(s)
                    .ok_or_else(|| bad(element, format!("unknown argument shape `{s}`"))),
                other => Err(bad(element, format!("`{key}` holds a non-string {other:?}"))),
            })
            .collect(),
        Some(other) => Err(bad(element, format!("`{key}` must be a list, found {other:?}"))),
    }
}

/// The argument shapes as serial data under `key`, pushed only when there is
/// at least one.
fn push_shapes(
    entries: &mut Vec<(&'static str, SerialValue)>,
    key: &'static str,
    args: &[LatexArgShape],
) {
    if !args.is_empty() {
        entries.push((key, SerialValue::List(args.iter().map(|a| text(a.as_str())).collect())));
    }
}

// --- the argument shapes --------------------------------------------------------------

/// One optional boolean key of a projection, read by name — `packed`, `body`:
/// an absent or null key reads as `false`, and anything but a boolean is an
/// error that identifies `element` and the key.
fn optional_flag(
    element: &str,
    entries: &[(String, SerialValue)],
    key: &str,
) -> Result<bool, DeserializeError> {
    match field(entries, key) {
        None | Some(SerialValue::Null) => Ok(false),
        Some(SerialValue::Bool(b)) => Ok(*b),
        Some(other) => Err(bad(element, format!("`{key}` must be a boolean, found {other:?}"))),
    }
}

/// One optional boolean key as serial data, pushed only when it is set.
fn push_flag(entries: &mut Vec<(&'static str, SerialValue)>, key: &'static str, set: bool) {
    if set {
        entries.push((key, SerialValue::Bool(true)));
    }
}

/// The shape of one argument of a macro or environment construct: how the
/// emitter writes the argument, and whether the argument has a content slot.
///
/// The shapes are the syntactic forms LaTeX has for an argument. A `*` and a
/// `[…]` are what an author writes for a starred variant and an optional
/// argument; a `{…}` is a mandatory argument. The fourth shape, [`EscVerb`],
/// is a mandatory argument whose content is verbatim text written in the
/// encoding that the `\EscVerb` command of the `fvextra` package reads (a
/// backslash before every printable ASCII character that is not a letter or a
/// digit), so that verbatim content survives inside a moving argument such as
/// a section title. With this shape a lowering rule only declares the shape,
/// and the sink does the encoding ([`super::sink::LatexSink::escverb`]).
///
/// [`EscVerb`]: LatexArgShape::EscVerb
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LatexArgShape {
    /// A star: written as `*`; no content slot.
    Star,
    /// An optional argument: written as `[…]`, or `[{…}]` when the content
    /// could be misread — the emitter decides from the slot's nodes.
    Optional,
    /// A mandatory argument: always written as `{…}`.
    Mandatory,
    /// A mandatory argument holding verbatim content, written as `{…}` in the
    /// `\EscVerb` encoding; its slot holds the verbatim text as text nodes.
    EscVerb,
}

impl LatexArgShape {
    /// Every shape, in declaration order.
    pub const ALL: [LatexArgShape; 4] =
        [LatexArgShape::Star, LatexArgShape::Optional, LatexArgShape::Mandatory, LatexArgShape::EscVerb];

    /// The shape's stable name, the projected spelling: `star`, `optional`,
    /// `mandatory`, `escverb`.
    pub fn as_str(self) -> &'static str {
        match self {
            LatexArgShape::Star => "star",
            LatexArgShape::Optional => "optional",
            LatexArgShape::Mandatory => "mandatory",
            LatexArgShape::EscVerb => "escverb",
        }
    }

    /// The shape of a stable name, if any.
    pub fn parse(name: &str) -> Option<LatexArgShape> {
        LatexArgShape::ALL.into_iter().find(|shape| shape.as_str() == name)
    }

    /// Whether an argument of this shape has a content slot on the node: every
    /// shape but [`Star`](LatexArgShape::Star).
    pub fn has_slot(self) -> bool {
        !matches!(self, LatexArgShape::Star)
    }
}

impl fmt::Display for LatexArgShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How many of the given shapes have a content slot.
fn slot_bearing(args: &[LatexArgShape]) -> usize {
    args.iter().filter(|shape| shape.has_slot()).count()
}

// --- the payloads ---------------------------------------------------------------------

/// The payload of `flm.pres.latex.macro`: the command's name and the shape of
/// each of its arguments, in the order they are written.
///
/// The node carries one content slot named [`ARG_SLOT`] per argument whose
/// shape [has a slot](LatexArgShape::has_slot), in argument order; a
/// [`Star`](LatexArgShape::Star) contributes no slot. The emitter writes
/// `\<name>` and then each argument in its shape — or, under
/// [`packed`](Self::packed), every argument inside one brace group.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LatexMacroPayload {
    /// The command's name without the backslash (`emph`, `section`).
    pub name: String,
    /// The shape of each argument, in order.
    pub args: Vec<LatexArgShape>,
    /// Whether the call hands every argument over in **one** brace group,
    /// `\<name>{{first}{second}…}`, instead of writing each argument in its
    /// own shape.
    ///
    /// This is how a call of a definition of more than
    /// [`PLAIN_ARGUMENT_LIMIT`] arguments is written, since a LaTeX macro body
    /// can refer to only nine parameters
    /// ([`LatexDefinitionPayload::packed`], which the definition carries).
    /// Every argument then has a brace group of its own inside the one group,
    /// in order, regardless of its shape, so every argument of a packed call
    /// has a shape with a content slot.
    pub packed: bool,
}

impl LatexMacroPayload {
    /// A macro call with the given name and argument shapes, writing every
    /// argument the command takes, each in its own shape.
    pub fn new(name: impl Into<String>, args: impl IntoIterator<Item = LatexArgShape>) -> Self {
        LatexMacroPayload { name: name.into(), args: args.into_iter().collect(), packed: false }
    }

    /// The same call with every argument handed over in one brace group
    /// ([`packed`](Self::packed)).
    pub fn packed(mut self) -> Self {
        self.packed = true;
        self
    }

    /// How many content slots the node carries: the arguments whose shape has
    /// one.
    pub fn slot_count(&self) -> usize {
        slot_bearing(&self.args)
    }

    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] that identifies `flm.pres.latex.macro`
    /// when the projection is not a map, when `name` is missing or is not a
    /// string,
    /// when `args` is present and is not a list of shape names, or when
    /// `packed` is present and is not a boolean.
    pub fn from_projection(value: &SerialValue) -> Result<LatexMacroPayload, DeserializeError> {
        let e = entries(MACRO, value)?;
        Ok(LatexMacroPayload {
            name: string(MACRO, e, "name")?,
            args: shapes(MACRO, e, "args")?,
            packed: optional_flag(MACRO, e, "packed")?,
        })
    }
}

impl PresPayload for LatexMacroPayload {
    fn project(&self) -> SerialValue {
        let mut entries = vec![("name", text(&self.name))];
        push_shapes(&mut entries, "args", &self.args);
        push_flag(&mut entries, "packed", self.packed);
        map(entries)
    }
}

/// The payload of `flm.pres.latex.environment`: the environment's name, the
/// shape of each of its arguments, whether its body is verbatim, and whether
/// the environment takes its body as an argument of its own.
///
/// The node carries one [`ARG_SLOT`] per argument whose shape has one, in
/// order, then the `body` slot ([`BODY_SLOT`](crate::stdcore::backends::pres::BODY_SLOT)) holding the lowered
/// body. The emitter writes `\begin{<name>}`, the arguments, the body and
/// `\end{<name>}`. When [`verbatim`](Self::verbatim) is set the body is
/// verbatim text: the emitter writes it byte for byte, on lines of its own —
/// a newline after `\begin{…}` and its arguments, and the `\end{…}` on a line
/// of its own — because a verbatim environment discards the rest of the line
/// that holds its `\begin`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LatexEnvironmentPayload {
    /// The environment's name (`align`, `flmVerbatimCode`).
    pub name: String,
    /// The shape of each argument written after `\begin{<name>}`, in order.
    pub args: Vec<LatexArgShape>,
    /// Whether the body is verbatim text written byte for byte with the
    /// newline discipline described on the type.
    pub verbatim: bool,
    /// Whether the arguments are handed over in **one** brace group after
    /// `\begin{<name>}`, one pair of braces around each, as
    /// [`LatexMacroPayload::packed`] describes for a macro call.
    pub packed: bool,
    /// Whether the environment **takes its body as an argument** — it was
    /// declared with `+b` ([`LatexDefinitionPayload::body_argument`]) — so
    /// that LaTeX hands what is written between `\begin` and `\end` to the
    /// definition as a trailing argument instead of setting it between the
    /// begin code and the end code.
    ///
    /// The body then reaches LaTeX as a **token group**: its characters are
    /// read, once, as LaTeX reads that argument, long before the definition
    /// can say anything about what they mean. Content whose meaning depends
    /// on being read out of the file, such as a verbatim block, must therefore
    /// be written in a form that survives being read as tokens; the emitter
    /// reads this field to choose that form.
    ///
    /// A call is staged with this field set from the same value the
    /// definition itself is written from, so a call and its definition cannot
    /// disagree about it.
    pub body_argument: bool,
}

impl LatexEnvironmentPayload {
    /// An environment with the given name and argument shapes, its body
    /// ordinary lowered content, writing every argument the environment
    /// takes.
    pub fn new(name: impl Into<String>, args: impl IntoIterator<Item = LatexArgShape>) -> Self {
        LatexEnvironmentPayload {
            name: name.into(),
            args: args.into_iter().collect(),
            verbatim: false,
            packed: false,
            body_argument: false,
        }
    }

    /// The same environment with a verbatim body.
    pub fn verbatim(mut self) -> Self {
        self.verbatim = true;
        self
    }

    /// The same environment with every argument handed over in one brace
    /// group ([`packed`](Self::packed)).
    pub fn packed(mut self) -> Self {
        self.packed = true;
        self
    }

    /// The same environment taking its body as an argument
    /// ([`body_argument`](Self::body_argument)).
    pub fn body_argument(mut self) -> Self {
        self.body_argument = true;
        self
    }

    /// How many argument slots precede the body slot.
    pub fn arg_slot_count(&self) -> usize {
        slot_bearing(&self.args)
    }

    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] that identifies
    /// `flm.pres.latex.environment` when the projection is not a map, when
    /// `name` is missing or is not a string,
    /// when `args` is present and is not a list of shape names, when
    /// `verbatim` is missing or is not a boolean, or when `packed` or `body`
    /// is present and is not a boolean.
    pub fn from_projection(
        value: &SerialValue,
    ) -> Result<LatexEnvironmentPayload, DeserializeError> {
        let e = entries(ENVIRONMENT, value)?;
        Ok(LatexEnvironmentPayload {
            name: string(ENVIRONMENT, e, "name")?,
            args: shapes(ENVIRONMENT, e, "args")?,
            verbatim: boolean(ENVIRONMENT, e, "verbatim")?,
            packed: optional_flag(ENVIRONMENT, e, "packed")?,
            body_argument: optional_flag(ENVIRONMENT, e, "body")?,
        })
    }
}

impl PresPayload for LatexEnvironmentPayload {
    fn project(&self) -> SerialValue {
        let mut entries = vec![("name", text(&self.name))];
        push_shapes(&mut entries, "args", &self.args);
        entries.push(("verbatim", SerialValue::Bool(self.verbatim)));
        push_flag(&mut entries, "packed", self.packed);
        push_flag(&mut entries, "body", self.body_argument);
        map(entries)
    }
}

/// The LaTeX text that a copy of a template writes at one of the template's
/// placeholders: for a kept user definition, the parameter (`#1`) that a
/// placeholder of one of its arguments becomes, or a part of the test that a
/// conditional becomes; for a list label template, the `enumitem`
/// command that a number format becomes.
///
/// It is not a construct payload. It is the element value carried by a copy
/// of a template node, made when a lowering rule copies the template, and it
/// says what LaTeX the copy writes without the node becoming a construct at
/// that point. Two readers act on it, which is why it belongs to the LaTeX
/// family rather than to one of the features that create it:
///
/// - the placeholder rule ([`placeholder_rule`]), which stages a raw LaTeX
///   node ([`LatexRawPayload`]) holding the text;
/// - the emitter, for a copy inside **verbatim content**, which a backend
///   copies character for character rather than lowering it, so that no
///   rule ever reaches the copy. Inside verbatim content the text is written
///   as LaTeX, not as verbatim characters, as a template requires when it
///   refers to one of its arguments inside a verbatim block or an inline
///   verbatim command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatexPoint {
    /// The LaTeX to write at the point.
    pub text: String,
}

impl LatexPoint {
    /// The value that writes `text`.
    pub fn new(text: impl Into<String>) -> LatexPoint {
        LatexPoint { text: text.into() }
    }
}

/// Stages one copy of a template node that carries LaTeX text: a childless
/// copy of `at` (a placeholder node of a compiled template, or a node that
/// represents a part of the LaTeX that a placeholder or a part of a
/// conditional of the template becomes) carrying `text`
/// as a [`LatexPoint`].
///
/// The node's own argument nodes (the braces of `#{name}`, a conditional's
/// branches) are not copied: the rule that lowers the copy reads the carried
/// text and nothing else. The node extension is built directly rather than
/// through the language's usual constructor (`Flm::make_node_ext`), because
/// that constructor resolves the element from the node's specification and
/// would return the element every compiled template shares, without the
/// `LatexPoint` value.
pub(crate) fn stamp_point(
    at: &NodeRef<'_, Flm>,
    text: impl Into<String>,
    cx: &mut RestageContext<'_, Flm, (), ()>,
) -> Result<BuildId, LowerError> {
    let Some(data) = at.callable() else {
        return Err(LowerError::structure(
            "a template node that is to carry LaTeX text must be a command, an environment or a \
             special character sequence such as `~`, and this one is none of these",
        ));
    };
    let kind = NodeKind::callable(CallableData {
        callable_type: data.callable_type,
        name: data.name.clone(),
        spec: Arc::clone(&data.spec),
        arguments: ParsedArguments::empty(),
        slots: ParsedSlots::empty(),
        invocation_syntax: data.invocation_syntax.clone(),
    });
    let mut element = (**element_of(at)).clone();
    element.value = Some(Arc::new(LatexPoint::new(text)));
    let ext = FlmNodeExt::new(Arc::new(element));
    cx.builder()
        .add(kind, at.span().clone(), Arc::clone(at.parsing_state()), Vec::new(), ext, ())
        .map_err(|error| LowerError::Build { detail: error.to_string() })
}

/// The name of the placeholder rule, `flm.placeholder.lower.latex-recomposed`.
pub const PLACEHOLDER_LATEX_RULE: &str = "flm.placeholder.lower.latex-recomposed";

/// The implementation of [`PLACEHOLDER_LATEX_RULE`]: it lowers a copy of a
/// template node that carries a [`LatexPoint`] to a raw LaTeX node
/// ([`LatexRawPayload`]) holding the carried text: the parameter that a
/// placeholder of a kept user definition becomes, a part of the test that a
/// conditional becomes, or the `enumitem` command that a number format of a
/// list's own label template becomes.
///
/// A placeholder node that carries no `LatexPoint` means that the rule which
/// copied the template broke its invariant; the rule then fails with a
/// structure error ([`LowerError`]).
#[derive(Debug, Default, Clone, Copy)]
pub struct PlaceholderLatexRule;

impl Lower for PlaceholderLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let point = element_value::<LatexPoint>(&node).ok_or_else(|| {
            LowerError::structure(
                "a placeholder lowered for the latex-recomposed backend must be a copy of a \
                 template node that carries its LaTeX text",
            )
        })?;
        let written = lw.pres_with(
            cx,
            &ElementName::from(RAW),
            LatexRawPayload::new(point.text.clone()),
            Vec::new(),
        )?;
        Ok(Lowering::Emit(vec![written]))
    }
}

/// The placeholder rule ([`PlaceholderLatexRule`]), ready to be registered by
/// the `latex` feature ([`super::LatexFamily`]). It is written for the
/// language's placeholder element, produces `flm.pres.latex.raw`, and is the
/// default on the `latex-recomposed` backend only, the only backend for which
/// templates are copied with `LatexPoint` values.
pub fn placeholder_rule() -> LoweringRule {
    LoweringRule::new(
        PLACEHOLDER_LATEX_RULE,
        PLACEHOLDER,
        LoweringDecl::new().produces(RAW),
        Arc::new(PlaceholderLatexRule),
        DefaultMark::OnBackends(vec![RECOMPOSED_BACKEND.into()]),
    )
    .docs(
        "a placeholder of a template copy, or a part of one of its conditionals, lowers to the \
         LaTeX text the copy carries for it",
    )
}

/// The payload of `flm.pres.latex.raw`: LaTeX text the emitter writes byte
/// for byte. The node is childless.
///
/// A rule uses a raw node only where the bytes are LaTeX syntax that no other
/// construct expresses and that need no escaping: a blank line that writes a
/// paragraph break, a parameter (`#1`) inside a recomposed definition, a line
/// of a display-math environment rebuilt from its equation lines, a literal
/// character's spelling as the author wrote it (`\%`, `\textbackslash`).
/// Document text never goes here: it goes through the emitter's escaping as a
/// text node. Neither does any structure that a macro or environment construct
/// can carry, because a raw node carries no structure, no source position for
/// its parts and no escaping, so the lowered output loses everything that
/// makes it inspectable.
///
/// The text must not hold a `%` that starts a LaTeX comment: the sink refuses
/// such a text ([`super::sink::LatexSink::raw`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LatexRawPayload {
    /// The LaTeX text, written as it is.
    pub text: String,
}

impl LatexRawPayload {
    /// A raw node's payload holding `text`.
    pub fn new(text: impl Into<String>) -> LatexRawPayload {
        LatexRawPayload { text: text.into() }
    }

    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] that identifies `flm.pres.latex.raw`
    /// when the projection is not a map or `text` is missing or is not a
    /// string.
    pub fn from_projection(value: &SerialValue) -> Result<LatexRawPayload, DeserializeError> {
        let e = entries(RAW, value)?;
        Ok(LatexRawPayload { text: string(RAW, e, "text")? })
    }
}

impl PresPayload for LatexRawPayload {
    fn project(&self) -> SerialValue {
        map(vec![("text", text(&self.text))])
    }
}

/// The largest number of arguments a definition written into the style file
/// takes one by one: nine, because a LaTeX macro body can refer to nine
/// parameters (`#1` to `#9`).
///
/// A definition of more arguments takes them all in one group instead, one
/// pair of braces around each ([`LatexDefinitionPayload::packed`]), and its
/// body reads them out of that group. The number is a threshold, not a cap:
/// a definition of any number of arguments can be written.
pub const PLAIN_ARGUMENT_LIMIT: usize = 9;

/// What a `flm.pres.latex.definition` node defines: a macro, an
/// environment, or a feature's declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LatexDefinitionKind {
    /// A macro: `\providecommand\<name>[<n>]{<body>}`, or `\long\def` where
    /// the definition is written under the document's own name
    /// ([`is_bare`](LatexDefinitionPayload::is_bare)), or
    /// `\ProvideDocumentCommand\<name>{+m …}` — `\DeclareDocumentCommand`
    /// under the document's own name — where the definition is asked to be
    /// robust ([`robust`](field@LatexDefinitionPayload::robust)).
    Macro,
    /// An environment:
    /// `\ProvideDocumentEnvironment{<name>}{+m …}{<begin>}{<end>}`, or
    /// `\DeclareDocumentEnvironment` where the definition is written under the
    /// document's own name.
    Environment,
    /// A **declaration**: LaTeX that makes something other than a command
    /// definition — `\newtheorem{lemma}[theorem]{Lemma}`, `\newcounter{…}` —
    /// written as `\ifcsname <name>\endcsname\else <body>\fi`, so that
    /// it runs only where the control sequence `\<name>` is not defined yet.
    ///
    /// A declaration defines no command of its own that a guard such as
    /// `\providecommand` could test, so the definition gives the control
    /// sequence it is guarded on ([`name`](field@LatexDefinitionPayload::name)):
    /// the environment it declares (`lemma` tests `\lemma`), or the
    /// counter's register (`c@theorem`). Something of that name defined
    /// before the style file is loaded (by the document class, a package or
    /// the author's own preamble) is kept, and the declaration does nothing.
    /// It takes no argument, its whole text is its `body` slot, and it is
    /// always written by a feature from its own configuration
    /// ([`feature`](field@LatexDefinitionPayload::feature)).
    Declaration,
}

impl LatexDefinitionKind {
    /// The kind's stable name: `macro`, `environment` or `declaration`.
    pub fn as_str(self) -> &'static str {
        match self {
            LatexDefinitionKind::Macro => "macro",
            LatexDefinitionKind::Environment => "environment",
            LatexDefinitionKind::Declaration => "declaration",
        }
    }

    /// The kind of a stable name, if any.
    pub fn parse(name: &str) -> Option<LatexDefinitionKind> {
        match name {
            "macro" => Some(LatexDefinitionKind::Macro),
            "environment" => Some(LatexDefinitionKind::Environment),
            "declaration" => Some(LatexDefinitionKind::Declaration),
            _ => None,
        }
    }
}

impl fmt::Display for LatexDefinitionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The payload of `flm.pres.latex.definition`: a macro or environment
/// definition, or a feature's declaration
/// ([`LatexDefinitionKind::Declaration`]), that the style file must hold for
/// the recomposed LaTeX to compile.
///
/// The content slots hold the definition's LaTeX as lowered content: `body`
/// ([`BODY_SLOT`](crate::stdcore::backends::pres::BODY_SLOT)) for a macro and
/// a declaration, or [`BEGIN_SLOT`] and [`END_SLOT`] for an environment. The
/// definition takes [`args`](Self::args) arguments, all of them mandatory, and
/// the body refers to them as `#1` to `#<args>`; every call writes exactly that
/// many groups, in that order, and nothing else, so LaTeX reads no argument
/// syntax of its own at a call. An environment may take one more argument, its
/// own body ([`body_argument`](Self::body_argument)); a call writes no group
/// for that one, since what a call writes between `\begin` and `\end` is
/// already the body.
///
/// The lowering rule that recomposes a kept user definition
/// ([`kept`](crate::stdcore::features::userdefs::kept)) stages a definition
/// node at every use site of the definition, since rules keep no state. The
/// emitter writes nothing in the node's place: it records the definition for
/// the style file once per name, so that the style file holds it exactly
/// once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatexDefinitionPayload {
    /// What the node defines: a macro, an environment or a feature's
    /// declaration ([`LatexDefinitionKind`]).
    pub kind: LatexDefinitionKind,
    /// The name defined, without a backslash (`flmUDhello`, `flmUEnote`);
    /// for a declaration, the name of the control sequence it is guarded on
    /// (`lemma`, `c@theorem`).
    pub name: String,
    /// The name the document declared, without a backslash (`hello`,
    /// `note`), where it differs from [`name`](Self::name) — a recomposed
    /// user definition is renamed under a prefix of its own
    /// ([`recomposed_name`](super::names::recomposed_name)). The style
    /// file writes it in the comment line above the definition, so that an
    /// author reading the file recognizes their own definition. Equal to
    /// [`name`](Self::name) unless
    /// [`with_source_name`](Self::with_source_name) said otherwise, which is
    /// how a definition written under the document's own name is told from one
    /// written under a prefix ([`is_bare`](Self::is_bare)).
    pub source_name: String,
    /// How many arguments the definition takes. Any number: past
    /// [`PLAIN_ARGUMENT_LIMIT`] the call hands them over in one group
    /// ([`packed`](field@Self::packed)) rather than one group each.
    pub args: usize,
    /// Whether the call hands the arguments over in **one** group instead of
    /// one group each: `\<name>{{first}{second}…}`, one pair of braces around
    /// each argument, in argument order.
    ///
    /// This is how a definition of more than [`PLAIN_ARGUMENT_LIMIT`]
    /// arguments is written, since a LaTeX macro body can refer to only nine
    /// parameters. The definition then takes one LaTeX parameter regardless of
    /// [`args`](field@Self::args), and its body reads argument *n* out of
    /// it with the style file's accessor
    /// ([`ARG_COMMAND`](super::preamble::ARG_COMMAND), `\flmArg{<n>}{#1}`).
    /// A definition of nine effective arguments or fewer keeps the plain
    /// form, where the body refers to `#1` to `#<args>` directly.
    pub packed: bool,
    /// Whether the definition is written as a **robust** command rather than
    /// as a plain expandable macro: the render option
    /// [`robust_names`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::robust_names)
    /// lists the name the document declared.
    ///
    /// A *robust* command is one LaTeX writes into a file of its own — the
    /// table of contents, the auxiliary file — as the call itself rather than
    /// as what the call stands for, and runs when that file is read back. The
    /// macro is then written with the document command interface,
    /// `\ProvideDocumentCommand\<name>{+m …}` with one `+m` per argument
    /// (`\DeclareDocumentCommand` where the definition is written under the
    /// document's own name, [`is_bare`](Self::is_bare)), where an expandable
    /// one is `\providecommand\<name>[<n>]`. The `+` makes each argument
    /// long, so it may hold a paragraph break, as `\providecommand` does of
    /// itself.
    ///
    /// Only a macro carries this. An environment is robust however it is
    /// written — `\begin` opens a group, runs the begin code, sets the body
    /// and runs the end code, none of which expansion alone could produce — so
    /// the option changes nothing for one, and the projection refuses a payload
    /// that claims it ([`from_projection`](Self::from_projection)).
    pub robust: bool,
    /// Whether the definition takes what is written between `\begin` and `\end`
    /// as an **argument** of its own rather than letting LaTeX set it between
    /// the begin and the end code: the environment's argument specification
    /// ends with `+b`, after one `+m` per argument, and the body is the last
    /// parameter — `#<args + 1>`, or `#2` where the arguments are
    /// [`packed`](field@Self::packed) into one group.
    ///
    /// An environment written this way carries its whole definition in the
    /// [`BEGIN_SLOT`], with the body's parameter written where the definition
    /// wants the body, and its [`END_SLOT`] is empty. This is how a definition
    /// is written that wants the body somewhere a begin code and an end code
    /// cannot put it — inside a command's argument, or in two places.
    ///
    /// Only an environment can take one: a macro's `body` slot is its whole
    /// definition already, so [`LatexDefinitionKind::Macro`] leaves this
    /// `false`.
    pub body_argument: bool,
    /// The feature that writes this definition from its own configuration,
    /// where the definition is not one the document declared: `defterm` for
    /// the environment of a defined term, whose begin code is the configured
    /// lead-in. `None` for a recomposed user definition.
    ///
    /// Such a definition is written under the name it is given and always
    /// **guarded** — `\providecommand`, `\ProvideDocumentEnvironment`, the
    /// test `\ifcsname` around a declaration — like
    /// a definition piece, so that a definition of that name the user made
    /// before the style file is loaded wins; the style file writes it before
    /// the document's own definitions, under a comment line that gives the
    /// feature's name ([`RecomposedDefinition::feature`](super::RecomposedDefinition::feature)).
    /// A feature stages one where the definition's text must be lowered — a
    /// template of its configuration, written through the ordinary rules —
    /// which a definition piece, fixed text, cannot hold.
    pub feature: Option<String>,
}

impl LatexDefinitionPayload {
    /// A definition of `kind` named `name` taking `args` arguments, declared
    /// in the document under that same name and calling them one group each.
    pub fn new(kind: LatexDefinitionKind, name: impl Into<String>, args: usize) -> Self {
        let name = name.into();
        LatexDefinitionPayload {
            kind,
            source_name: name.clone(),
            name,
            args,
            packed: false,
            robust: false,
            body_argument: false,
            feature: None,
        }
    }

    /// The same definition written by the feature `feature` from its own
    /// configuration ([`feature`](field@Self::feature)): guarded, under the
    /// name it is given.
    pub fn of_feature(mut self, feature: impl Into<String>) -> Self {
        self.feature = Some(feature.into());
        self
    }

    /// The same definition declared in the document under `source_name`, the
    /// author's own spelling, which the style file writes in the comment line
    /// above the definition ([`source_name`](Self::source_name)).
    pub fn with_source_name(mut self, source_name: impl Into<String>) -> Self {
        self.source_name = source_name.into();
        self
    }

    /// The same definition with its arguments handed over in one group
    /// ([`packed`](Self::packed)).
    pub fn packed(mut self) -> Self {
        self.packed = true;
        self
    }

    /// The same definition written as a robust command
    /// ([`robust`](field@Self::robust)). A macro only: an environment is robust
    /// however it is written, and the projection refuses one that claims it.
    pub fn robust(mut self) -> Self {
        self.robust = true;
        self
    }

    /// The same definition taking its body as its last argument
    /// ([`body_argument`](Self::body_argument)). An environment only: a macro
    /// has no body of its own to take, and the projection refuses one that
    /// claims it ([`from_projection`](Self::from_projection)).
    pub fn body_argument(mut self) -> Self {
        self.body_argument = true;
        self
    }

    /// Whether the definition is written under the name the document spelled
    /// rather than under a prefix of its own. The render option
    /// [`bare_names`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::bare_names)
    /// asked for it, so the definition is meant to replace any definition that
    /// LaTeX or a package makes under that name, and the emitter writes it
    /// unconditionally rather than guarded. A feature's definition
    /// ([`feature`](field@Self::feature)) is never bare.
    pub fn is_bare(&self) -> bool {
        self.feature.is_none() && self.name == self.source_name
    }

    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] that identifies
    /// `flm.pres.latex.definition` when the projection is not a map, when
    /// `kind` is missing or is not the name of a kind
    /// ([`LatexDefinitionKind::parse`]), when `name` is missing or is not a
    /// string, when `source` or `feature` is present and is not a string, when
    /// a declaration gives no `feature`, takes arguments or is `packed`, when
    /// `args` is present and is not a whole number of arguments, when `packed`,
    /// `robust` or `body` is present and is not a boolean, when `body` is set
    /// on anything but an environment (a macro's `body` slot is already its
    /// whole definition), or when `robust` is set on anything but a macro (an
    /// environment is robust however it is written). Any number of arguments
    /// is accepted: past [`PLAIN_ARGUMENT_LIMIT`] the call hands them over in
    /// one group.
    pub fn from_projection(
        value: &SerialValue,
    ) -> Result<LatexDefinitionPayload, DeserializeError> {
        let e = entries(DEFINITION, value)?;
        let kind = string(DEFINITION, e, "kind")?;
        let kind = LatexDefinitionKind::parse(&kind)
            .ok_or_else(|| bad(DEFINITION, format!("unknown kind `{kind}`")))?;
        let args = definition_arity(e, "args")?;
        let packed = optional_flag(DEFINITION, e, "packed")?;
        let robust = optional_flag(DEFINITION, e, "robust")?;
        let body_argument = optional_flag(DEFINITION, e, "body")?;
        if body_argument && kind != LatexDefinitionKind::Environment {
            return Err(bad(DEFINITION, String::from("`body` is only for an environment")));
        }
        if robust && kind != LatexDefinitionKind::Macro {
            return Err(bad(DEFINITION, String::from("`robust` is only for a macro")));
        }
        let name = string(DEFINITION, e, "name")?;
        let source_name = match field(e, "source") {
            None => name.clone(),
            Some(SerialValue::Str(s)) => s.clone(),
            Some(other) => {
                return Err(bad(DEFINITION, format!("`source` must be a string, found {other:?}")));
            }
        };
        let feature = match field(e, "feature") {
            None | Some(SerialValue::Null) => None,
            Some(SerialValue::Str(s)) => Some(s.clone()),
            Some(other) => {
                return Err(bad(DEFINITION, format!("`feature` must be a string, found {other:?}")));
            }
        };
        if kind == LatexDefinitionKind::Declaration && (feature.is_none() || args != 0 || packed) {
            return Err(bad(
                DEFINITION,
                String::from(
                    "a declaration must give `feature`, take no argument and not be packed",
                ),
            ));
        }
        Ok(LatexDefinitionPayload {
            kind,
            name,
            source_name,
            args,
            packed,
            robust,
            body_argument,
            feature,
        })
    }
}

impl PresPayload for LatexDefinitionPayload {
    fn project(&self) -> SerialValue {
        let mut entries = vec![("kind", text(self.kind.as_str())), ("name", text(&self.name))];
        if self.source_name != self.name {
            entries.push(("source", text(&self.source_name)));
        }
        if self.args != 0 {
            entries.push(("args", SerialValue::Int(self.args as i64)));
        }
        push_flag(&mut entries, "packed", self.packed);
        push_flag(&mut entries, "robust", self.robust);
        push_flag(&mut entries, "body", self.body_argument);
        if let Some(feature) = &self.feature {
            entries.push(("feature", text(feature)));
        }
        map(entries)
    }
}

/// How many arguments a definition takes, read from the key `name`, `args`;
/// an absent or null key reads as none.
fn definition_arity(
    entries: &[(String, SerialValue)],
    name: &str,
) -> Result<usize, DeserializeError> {
    match field(entries, name) {
        None | Some(SerialValue::Null) => Ok(0),
        Some(SerialValue::Int(count)) => usize::try_from(*count)
            .map_err(|_| bad(DEFINITION, format!("`{name}` must not be negative, found {count}"))),
        Some(other) => {
            Err(bad(DEFINITION, format!("`{name}` must be a whole number, found {other:?}")))
        }
    }
}

// --- readers and registration ---------------------------------------------------------

/// Registers `parse` as the reader of `element`'s payload.
fn reader<P: PresPayload>(
    readers: &mut PayloadReaders,
    element: &'static str,
    parse: fn(&SerialValue) -> Result<P, DeserializeError>,
) {
    readers.register(element, move |value: &SerialValue, _cx: &mut DeserializeContext<'_, Flm>| {
        parse(value).map(|payload| Arc::new(payload) as Arc<dyn PresPayload>)
    });
}

/// The payload readers of the four construct elements: what a reading session
/// ([`reader_with`](crate::dump::session::reader_with)) needs, beside the
/// shared vocabulary's own ([`payload_readers`](crate::stdcore::backends::pres::payload_readers)), to read a stored
/// lowered tree of a LaTeX backend back into typed payloads.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::construct::{ALL, payload_readers};
///
/// assert_eq!(payload_readers().len(), ALL.len());
/// ```
pub fn payload_readers() -> PayloadReaders {
    let mut readers = PayloadReaders::new();
    reader(&mut readers, MACRO, LatexMacroPayload::from_projection);
    reader(&mut readers, ENVIRONMENT, LatexEnvironmentPayload::from_projection);
    reader(&mut readers, RAW, LatexRawPayload::from_projection);
    reader(&mut readers, DEFINITION, LatexDefinitionPayload::from_projection);
    readers
}

/// Registers the four construct elements with their payload types. The
/// `latex` feature ([`super::LatexFamily`]) calls this from its own
/// registration; a second registration in one registry declares the elements
/// twice and fails the build.
///
/// # Errors
///
/// Returns the [`BuildError`] the feature builder raises for a presentation
/// element already declared in this registry.
pub fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.presentation_element::<LatexMacroPayload>(
        MACRO,
        "a LaTeX macro call: the command name and one content slot per argument",
    )?;
    b.presentation_element::<LatexEnvironmentPayload>(
        ENVIRONMENT,
        "a LaTeX environment: the content slots of its arguments, then the body",
    )?;
    b.presentation_element::<LatexRawPayload>(RAW, "LaTeX text written byte for byte")?;
    b.presentation_element::<LatexDefinitionPayload>(
        DEFINITION,
        "a macro or environment definition, or a feature's declaration, written to the style \
         file and never where the node occurs",
    )?;
    Ok(())
}

/// The content of the content slot named `name` on `node`, the first one of
/// that name when the name repeats; `None` when the node has no such slot.
pub fn slot_named<'t>(node: &NodeRef<'t, Flm>, name: &str) -> Option<NodeSlice<'t, Flm>> {
    let slots = node.slots()?;
    let index = slots.iter().position(|slot| slot.name() == Some(name))?;
    node.slot_content_nodes(index)
}

/// The arguments of a macro or environment node paired with their content
/// slots: for each shape in `args`, the shape and the content of its content
/// slot — `None` for a [`Star`](LatexArgShape::Star), which has no content
/// slot, and for an argument the node carries no content slot for (a
/// hand-built node; a node a rule built always has them).
///
/// The content slots named [`ARG_SLOT`] are taken in order and matched to the
/// shapes that have a content slot, in order; any other content slot (an
/// environment's `body`) is skipped.
pub fn arg_slots<'t>(
    node: &NodeRef<'t, Flm>,
    args: &[LatexArgShape],
) -> Vec<(LatexArgShape, Option<NodeSlice<'t, Flm>>)> {
    let mut arg_indexes = node
        .slots()
        .map(|slots| {
            slots
                .iter()
                .enumerate()
                .filter(|(_, slot)| slot.name() == Some(ARG_SLOT))
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter();
    args.iter()
        .map(|shape| {
            let content = if shape.has_slot() {
                arg_indexes.next().and_then(|index| node.slot_content_nodes(index))
            } else {
                None
            };
            (*shape, content)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_placeholder_rule_requires_nothing_and_is_marked_for_the_recomposed_backend() {
        // The placeholder rule is marked for the recomposed backend, so the
        // plan's ladder offers it on no other; it requires nothing, so its
        // selection demands nothing of a plan.
        let placeholder = placeholder_rule();
        assert_eq!(placeholder.element, PLACEHOLDER);
        assert!(placeholder.decl.requires.is_empty());
        assert_eq!(placeholder.decl.produces, [ElementName::from(RAW)]);
        assert!(placeholder.default.is_specific_to(&RECOMPOSED_BACKEND.into()));
    }

    #[test]
    fn a_macro_projection_omits_an_empty_argument_list() {
        let payload = LatexMacroPayload::new("relax", []);
        assert_eq!(payload.project(), map(vec![("name", text("relax"))]));
        let payload = LatexMacroPayload::new(
            "section",
            [LatexArgShape::Star, LatexArgShape::Optional, LatexArgShape::Mandatory],
        );
        assert_eq!(payload.slot_count(), 2);
        assert_eq!(
            payload.project(),
            map(vec![
                ("name", text("section")),
                ("args", SerialValue::List(vec![text("star"), text("optional"), text("mandatory")])),
            ])
        );
    }

    #[test]
    fn a_definition_projects_its_arity_and_omits_an_empty_one() {
        let payload = LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDplain", 0);
        assert!(payload.is_bare());
        assert_eq!(
            payload.project(),
            map(vec![("kind", text("macro")), ("name", text("flmUDplain"))])
        );
        // Three arguments, under a prefix: the name the document declared is
        // projected beside the name defined, and the definition is not bare.
        let payload = LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDaround", 3)
            .with_source_name("around");
        assert!(!payload.is_bare());
        assert_eq!(
            payload.project(),
            map(vec![
                ("kind", text("macro")),
                ("name", text("flmUDaround")),
                ("source", text("around")),
                ("args", SerialValue::Int(3)),
            ])
        );
    }

    #[test]
    fn every_payload_reads_back_from_its_projection() {
        let macro_ = LatexMacroPayload::new("verbcode", [LatexArgShape::EscVerb]);
        assert_eq!(LatexMacroPayload::from_projection(&macro_.project()).unwrap(), macro_);
        let env = LatexEnvironmentPayload::new("flmVerbatimCode", [LatexArgShape::Optional])
            .verbatim();
        assert_eq!(LatexEnvironmentPayload::from_projection(&env.project()).unwrap(), env);
        let raw = LatexRawPayload::new("\n\n");
        assert_eq!(LatexRawPayload::from_projection(&raw.project()).unwrap(), raw);
        let def = LatexDefinitionPayload::new(LatexDefinitionKind::Environment, "flmUEnote", 4)
            .with_source_name("note");
        assert_eq!(LatexDefinitionPayload::from_projection(&def.project()).unwrap(), def);
        // A definition of no arguments, and one whose call packs them.
        let none = LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDplain", 0);
        assert_eq!(LatexDefinitionPayload::from_projection(&none.project()).unwrap(), none);
        let packed = LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDbig", 12).packed();
        assert_eq!(LatexDefinitionPayload::from_projection(&packed.project()).unwrap(), packed);
        // A packed call carries the same flag, so the call and the definition
        // agree about the one group.
        let call = LatexMacroPayload::new("flmUDbig", [LatexArgShape::Mandatory; 12]).packed();
        assert_eq!(LatexMacroPayload::from_projection(&call.project()).unwrap(), call);
        let env = LatexEnvironmentPayload::new("flmUEbig", [LatexArgShape::Mandatory; 12]).packed();
        assert_eq!(LatexEnvironmentPayload::from_projection(&env.project()).unwrap(), env);
        // An environment call that says its definition takes the body as an
        // argument: the same `body` key as the definition's, so a call and its
        // definition cannot disagree about what the body reaches LaTeX as.
        let grabbed = LatexEnvironmentPayload::new("flmUEwrap", [LatexArgShape::Mandatory])
            .body_argument();
        assert_eq!(LatexEnvironmentPayload::from_projection(&grabbed.project()).unwrap(), grabbed);
        assert_eq!(
            grabbed.project(),
            map(vec![
                ("name", text("flmUEwrap")),
                ("args", SerialValue::List(vec![text("mandatory")])),
                ("verbatim", SerialValue::Bool(false)),
                ("body", SerialValue::Bool(true)),
            ])
        );
        // An environment that takes its body as its last argument, plain and
        // packed: the flag belongs to the definition alone, a call writing the
        // body where it always did.
        let body = LatexDefinitionPayload::new(LatexDefinitionKind::Environment, "flmUEwrap", 1)
            .with_source_name("wrap")
            .body_argument();
        assert_eq!(LatexDefinitionPayload::from_projection(&body.project()).unwrap(), body);
        let both = LatexDefinitionPayload::new(LatexDefinitionKind::Environment, "flmUEbig", 12)
            .packed()
            .body_argument();
        assert_eq!(LatexDefinitionPayload::from_projection(&both.project()).unwrap(), both);
        // A macro asked to be robust, alone and packed: the flag says the
        // definition is written through the document command interface, which
        // is a property of the definition's head and of nothing a call writes.
        let declaration = LatexDefinitionPayload::new(LatexDefinitionKind::Declaration, "lemma", 0)
            .of_feature("theorems");
        assert_eq!(LatexDefinitionPayload::from_projection(&declaration.project()).unwrap(), declaration);
        // A declaration is a feature's and takes no argument.
        let orphan = LatexDefinitionPayload::new(LatexDefinitionKind::Declaration, "lemma", 0);
        assert!(LatexDefinitionPayload::from_projection(&orphan.project()).is_err());
        let with_args = LatexDefinitionPayload::new(LatexDefinitionKind::Declaration, "lemma", 1)
            .of_feature("theorems");
        assert!(LatexDefinitionPayload::from_projection(&with_args.project()).is_err());
        let robust = LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDhello", 1)
            .with_source_name("hello")
            .robust();
        assert_eq!(LatexDefinitionPayload::from_projection(&robust.project()).unwrap(), robust);
        let robust_packed = LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDbig", 12)
            .packed()
            .robust();
        assert_eq!(
            LatexDefinitionPayload::from_projection(&robust_packed.project()).unwrap(),
            robust_packed
        );
    }

    #[test]
    fn a_macro_definition_that_claims_a_body_argument_is_refused() {
        // Only an environment has a body to take as an argument: a macro's own
        // `body` slot is its whole definition already.
        let error = LatexDefinitionPayload::from_projection(&map(vec![
            ("kind", text("macro")),
            ("name", text("flmUDwrap")),
            ("body", SerialValue::Bool(true)),
        ]))
        .unwrap_err();
        assert!(error.to_string().contains("`body` is only for an environment"), "{error}");
    }

    #[test]
    fn an_environment_definition_that_claims_robustness_is_refused() {
        // An environment is robust however it is written — `\begin` opens a
        // group and runs code, which expansion alone could not do — so there
        // is nothing for the flag to ask for, and a payload carrying it is a
        // structure error rather than an author's mistake.
        let error = LatexDefinitionPayload::from_projection(&map(vec![
            ("kind", text("environment")),
            ("name", text("flmUEnote")),
            ("robust", SerialValue::Bool(true)),
        ]))
        .unwrap_err();
        assert!(error.to_string().contains("`robust` is only for a macro"), "{error}");
    }

    #[test]
    fn a_malformed_projection_is_refused_naming_the_element() {
        let error = LatexMacroPayload::from_projection(&map(vec![("name", SerialValue::Int(1))]))
            .unwrap_err();
        assert!(error.to_string().contains(MACRO));
        let error = LatexMacroPayload::from_projection(&map(vec![
            ("name", text("x")),
            ("args", SerialValue::List(vec![text("braced")])),
        ]))
        .unwrap_err();
        assert!(error.to_string().contains("braced"));
        // Any number of arguments is accepted: past nine the call packs
        // ([`PLAIN_ARGUMENT_LIMIT`]), so there is no cap left to refuse.
        let many = LatexDefinitionPayload::from_projection(&map(vec![
            ("kind", text("macro")),
            ("name", text("x")),
            ("args", SerialValue::Int(20)),
            ("packed", SerialValue::Bool(true)),
        ]))
        .unwrap();
        assert_eq!(many.args, 20);
        assert!(many.packed);
        let error = LatexDefinitionPayload::from_projection(&map(vec![
            ("kind", text("macro")),
            ("name", text("x")),
            ("args", SerialValue::Int(3)),
            ("packed", SerialValue::Int(1)),
        ]))
        .unwrap_err();
        assert!(error.to_string().contains("`packed` must be a boolean"));
        let error = LatexDefinitionPayload::from_projection(&map(vec![
            ("kind", text("macro")),
            ("name", text("x")),
            ("args", SerialValue::Str(String::from("three"))),
        ]))
        .unwrap_err();
        assert!(error.to_string().contains("whole number"));
    }

    #[test]
    fn the_shape_names_round_trip_and_only_the_star_has_no_slot() {
        for shape in LatexArgShape::ALL {
            assert_eq!(LatexArgShape::parse(shape.as_str()), Some(shape));
            assert_eq!(shape.has_slot(), shape != LatexArgShape::Star);
        }
        assert_eq!(LatexArgShape::parse("braced"), None);
    }
}
