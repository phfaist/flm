//! The `math` feature: mathematics written in LaTeX notation, its
//! display-math environments, its equation lines and their numbers.
//!
//! This feature introduces mathematics into the language and nothing more. It
//! declares the few things FLM reads inside mathematics: the display-math
//! environments and the lines they are made of, the commands that put document
//! text inside a formula, and references to equations. It checks no formula
//! and processes none: a *processing feature* does, a feature that reads the
//! mathematical content and gives every formula its lowering rules (*Processing
//! mathematics*, below). Its one rule on a formula is the rule of last resort
//! of the display-math environments, which reports an environment that no
//! processing feature lowers ([`unprocessed`]). The guide to the language
//! describes mathematics for authors
//! ([mathematics](crate::guide::language_syntax#mathematics)).
//!
//! # What an author writes
//!
//! - **Inline and displayed mathematics**, `\(…\)` and `\[…\]` (and `$…$`,
//!   `$$…$$` when the parsing option
//!   [`FlmParsing::dollar_math`](crate::lang::FlmParsing::dollar_math) is on). The language
//!   layer reads these as math groups, nodes of the element `flm.math`
//!   ([`MATH`](crate::lang::structural::MATH)).
//! - **Six display-math environments**: `equation`, `align`, `gather` and their
//!   starred forms `equation*`, `align*`, `gather*`. The unstarred forms number
//!   their lines.
//! - **Inside a display-math environment's body**, the four commands `\\`,
//!   `\label{eq:…}`, `\tag{…}` (or `\tag*{…}`) and `\nonumber`, which divide the
//!   body into equation lines and say how each line is labeled and numbered
//!   ([`lines`]).
//! - **Text inside a formula**: `\text{…}`, `\textbf{…}`, `\mbox{…}` and the other
//!   text commands of math mode, whose argument is document content ([`text`]).
//! - **`\ensuremath{…}`**, in both modes ([`ensuremath`]): in text, an inline
//!   formula, read as `\(…\)` is (its braces delimit a math group, a node of
//!   `flm.math`); inside a formula, its argument, as if written in its place.
//! - **References to equations**: `\eqref{eq:…}`, which shows an equation line's
//!   number without a word before it ([`eqref`]).
//!
//! Every other command of mathematics (`\alpha`, `\frac`, a `cases`
//! environment) is the processing feature's to declare or accept.
//!
//! A processing feature needs to know nothing of `\ensuremath` written in text:
//! the formula is a math group like any other, the content of the command's
//! argument, which the feature's own rule ([`ensuremath::RULE`]) lowers. On the
//! recomposed LaTeX, [`latex::group_opening`] and [`latex::group_closing`],
//! which the feature's rule for `flm.math` calls, write that formula
//! `\ensuremath{…}` as written. Inside a formula, a writer of the formula's TeX
//! for a renderer writes the argument's content in place of the command
//! ([`ensuremath`], *In mathematics*).
//!
//! # Processing mathematics
//!
//! A processing feature does three things, which this feature leaves to it:
//!
//! - it says what a command of mathematics is, for instance with a *fallback*
//!   ([`FallbackDef`](crate::lang::mint::FallbackDef)) for macros and
//!   environments in math mode, or with declared elements in math mode;
//! - it provides the capability [`CONTENT`] (`flm.math.content`) with a check
//!   stage of its own, and adds its check to the collection
//!   [`flm.validated`](crate::stdcore::VALIDATED): [`CONTENT`] itself, or, for
//!   an extension, which adds to a collection only a capability named under
//!   its own stem, a capability of its own that the stage provides beside
//!   [`CONTENT`];
//! - it registers the lowering rules of `flm.math` and of
//!   [`ENVIRONMENT`] (`flm.math.environment`) on the backends it serves.
//!
//! The `stdcore` package holds no processing feature: a registry adds one from
//! another package, for example the feature of the package
//! [`math-passthrough`](crate::math_passthrough), which accepts every formula
//! and carries its source to the output as written, or an extension that
//! typesets formulas. Every standard workflow demands the collection
//! [`flm.validated`](crate::stdcore::VALIDATED), which holds the check of the
//! mathematics exactly when a processing feature is registered. A registry that holds
//! this feature and no processing feature therefore builds, and so does the
//! plan of every standard workflow: a document without formulas renders
//! without error, and each formula is an Error
//! ([`MathUnprocessed`](crate::stdcore::backends::lowering::MathUnprocessed),
//! `flm.math.unprocessed`), reported where a backend lowers it through a rule
//! of last resort: the `lowering` feature's for a math group
//! ([`MathGroupRule`](crate::stdcore::backends::lowering::MathGroupRule)),
//! this feature's for a display-math environment ([`unprocessed::RULE`]).
//!
//! # Equation lines, reference points and numbers
//!
//! The body of a display-math environment is an [opaque
//! region](crate::guide::concepts_overview#opaque-regions) in math mode: the
//! rewrite passes leave it untouched. The feature reads it once as a list of
//! [equation lines](crate::guide::concepts_overview#equation-lines), the parts of
//! the body between two `\\`, with their labels, tags and no-number marks
//! ([`lines::equation_lines`]).
//!
//! Each equation line is a reference point
//! ([`RefPoint`]; see [reference points, labels
//! and anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)):
//! a place in the document that can be referred to, numbered and given an anchor.
//! The environment declares one point per line ([`MathEnvironment`] implements
//! [`Referenceable`]), of the reference kind [`KIND`] (`equation`).
//!
//! - A line **can be numbered** when its environment is unstarred and the line
//!   carries no `\nonumber`. It keeps the labels written on it and accepts labels
//!   of the type [`LABEL_TYPES`] (`eq`), so `\ref{eq:name}` refers to the line
//!   that carries `\label{eq:name}`.
//! - A line that can be numbered and carries no `\tag` is numbered, unless the
//!   feature's configuration numbers no equations
//!   ([`MathConfig::numbered_equations`]). It counts in the counter [`COUNTER`]
//!   (`flm.math.equation`) and its number is shown in the counter style
//!   [`COUNTER_STYLE`] (`flm.math.style.equation`): `(1)`, and `Eq.~(1)` in a
//!   reference ([counters and counter
//!   styles](crate::guide::concepts_overview#counters-and-counter-styles)).
//! - A line with a `\tag` counts in nothing: a reference to it shows the tag
//!   instead of a number.
//! - A line that **cannot be numbered** (every line of a starred environment,
//!   and a line marked `\nonumber`) keeps no label and accepts no label type, so
//!   nothing can refer to it. A `\label` or `\tag` written on such a line is an
//!   Error ([`lines::LabelWithoutNumber`]).
//! - **Every line has an anchor**, the identifier a link points at: for a
//!   labeled line, the anchor of its first label (`eq-euler` for
//!   `\label{eq:euler}`), and otherwise [`ANCHOR_PREFIX`] (`eq`), a
//!   double hyphen, the line's position among the document's equation lines and
//!   four letters, as in `eq--8kqzt`
//!   ([`auto_anchor`](crate::stdcore::features::refs::auto_anchor)).
//!
//! The counter and the counter style belong to the numbering feature
//! ([`numbering`](crate::stdcore::features::numbering)): this feature contributes
//! their declarations ([`counter_decl`], [`counter_style`]) to that feature's
//! service slots as peer contributions, contributions to a service slot that
//! another feature owns, which the registry build drops with one Note when that
//! feature is not registered ([`services`](crate::core::services)). The numbers
//! are then not computed, and no line shows one.
//!
//! The table references [`ANCHORS`](crate::stdcore::features::refs::ANCHORS) (the anchors table of the references
//! feature) and [`COUNTERS`] (the counter table of the numbering feature) are
//! where a processing feature's rules read each line's anchor and number; the
//! library of [`frame`] reads them for it ([`frame::line_frames`]).
//!
//! # What the feature's own elements become on each backend
//!
//! The feature's rules lower the commands it declares outside the formulas
//! themselves: a text command, `\ensuremath` and `\eqref`. Each stages (builds
//! into the lowered tree; see [`Lowerer`](crate::core::lowering::Lowerer))
//! presentation nodes, except the rule of `\ensuremath`, which lowers its
//! argument through the rules and stages nothing of its own.
//!
//! | element | `text` | `html` | `latex-recomposed` |
//! |---|---|---|---|
//! | `flm.math.text` | [`text::RULE`] | [`text::RULE`] | [`latex::TEXT_LATEX_RULE`] |
//! | `flm.math.ensuremath` | [`ensuremath::RULE`] | [`ensuremath::RULE`] | [`ensuremath::RULE`] |
//! | `flm.math.eqref` | [`eqref::RULE`] | [`eqref::RULE`] | [`eqref::LATEX_RULE`] |
//!
//! The feature also registers, on every backend, the rule of last resort of
//! the display-math environments, [`unprocessed::RULE`], which is chosen only
//! where no processing feature has a rule for [`ENVIRONMENT`] on the backend
//! ([`unprocessed`]).
//!
//! **A text command becomes content**: its argument is document text, so
//! [`text::RULE`] lowers it like inline content anywhere else, into a
//! [`flm.pres.math-text`](crate::stdcore::backends::pres::MATH_TEXT) node; a
//! reference in it is a link, and a character the output format must escape is
//! escaped. A processing feature decides whether a text command inside a
//! formula is lowered through these rules (`math-passthrough` does).
//!
//! The rendering guide describes the result under [mathematics in
//! HTML](crate::guide::rendering#mathematics-in-html) and [mathematics in the
//! recomposed LaTeX](crate::guide::rendering#mathematics-in-the-recomposed-latex).
//!
//! # The capabilities
//!
//! A capability is a registered dotted name for a state of processing that a
//! stage brings about ([`CapabilityDef`]). The feature registers two:
//!
//! - [`CONTENT`] (`flm.math.content`): the mathematics of the document has been
//!   checked. No stage of this feature provides it: a processing feature
//!   provides it with a check of its own (the feature of the package
//!   `math-passthrough` accepts every formula) and adds its check to the
//!   collection [`flm.validated`](crate::stdcore::VALIDATED) (*Processing
//!   mathematics*, above), so that every workflow that validates a document,
//!   the [`check` workflow](crate::stdcore::CHECK_WORKFLOW) among them, runs
//!   that check exactly when a processing feature is registered. Where two
//!   registered features provide it, a plan request prefers one of their two
//!   stages; otherwise the plan build fails
//!   ([`PlanError::Contested`](crate::core::plan::PlanError::Contested)), so a
//!   registry usually holds one processing feature.
//! - [`RENDERED`] (`flm.math.rendered`): some rendered form of the mathematics
//!   exists. No standard feature provides it. An extension that renders
//!   mathematics provides a refinement of it, named in the extension's own
//!   namespace (the `math-mathjax` extension's table `math-mathjax.rendered`,
//!   the MathML of each formula, in its `mathml` mode), and a rule that needs
//!   rendered mathematics of any kind requires `flm.math.rendered`. A plan that
//!   demands it cannot be built until such an extension is registered.
//!
//! # Settings
//!
//! - [`MathConfig`], the feature's configuration (fixed when the registry is
//!   built; see [settings, options and
//!   configuration](crate::guide::concepts_overview#settings-options-and-configuration)):
//!   whether the lines of the unstarred environments are numbered.
//! - [`MathRecomposeOptions`], an option type set per render request: what the
//!   recomposed LaTeX writes around a display-math environment, and whether it
//!   writes the environment through the LaTeX hook `\flmMathEnvironment`. The
//!   rules of a processing feature on the `latex-recomposed` backend read it.
//!
//! Each field of the two types is a settings key under its own name
//! ([`crate::core::settings`]).
//!
//! # Map of the module
//!
//! - [`Math`] — the feature, configured by [`MathConfig`]; [`feature`] returns it
//!   with the default configuration.
//! - [`ENVIRONMENT`], [`MathEnvironment`], [`ENVIRONMENTS`] — the element and the
//!   data of an environment's element values.
//! - [`KIND`], [`LABEL_TYPES`], [`ANCHOR_PREFIX`], [`COUNTER`],
//!   [`COUNTER_STYLE`], [`counter_decl`], [`counter_style`] — the reference points
//!   of the equation lines and their numbering.
//! - [`CONTENT`], [`RENDERED`] — the capabilities.
//! - [`lines`] — the four commands of an environment's body and the reader of
//!   equation lines.
//! - [`text`] — the text commands of math mode.
//! - [`ensuremath`] — `\ensuremath`, an inline formula in text and its
//!   argument inside a formula.
//! - [`eqref`] — `\eqref`.
//! - [`unprocessed`] — the rule of last resort of the display-math
//!   environments.
//! - [`frame`] — the library a processing feature computes a formula's frame
//!   with: whether a math group is displayed, and each equation line's anchor
//!   and number or tag.
//! - [`latex`] — the recomposition rule of the text commands, the LaTeX names
//!   of the environments, [`MathRecomposeOptions`], the library a processing
//!   feature writes a formula's frame and an environment's lines as LaTeX
//!   with, and the newline rule that every LaTeX writer of the lines applies.

// The provider-less parent capability: [§flm-dd-dr:std-math-rendered-parent].

pub mod ensuremath;
pub mod eqref;
pub mod frame;
pub mod latex;
pub mod lines;
pub mod text;
pub mod unprocessed;

pub use latex::MathRecomposeOptions;


use std::sync::Arc;

use techy::core::node::NodeRef;
use techy::latexlike::Mode;

use crate::core::settings::SettingsDecl;
use crate::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, BlockRole, BlockRoleAspect, FixedBlockRole, NumberSource,
    RefPoint, Referenceable,
};
use crate::model::element::ElementSpec;
use crate::model::names::{FeatureName, Name, RefKind};
use crate::core::registry::{BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::lang::Flm;
use crate::lang::mint::{BodyDef, ElementDef, SyntaxDef};
use crate::model::options::Bind;
use crate::stdcore::features::numbering::{
    COUNTERS, COUNTERS_SLOT, CounterDecl, CounterStyleDecl, Counters, Delimiters, JoinDecl,
    NumberTemplate, PluralWords, PrefixWords, STYLES_SLOT,
};
use crate::stdcore::features::refs;

/// The feature's registered name, `math`.
pub const FEATURE: &str = "math";

/// The declaration of the feature's settings: the fields of [`MathConfig`] and
/// of [`MathRecomposeOptions`] are its settings keys. See
/// [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<MathConfig>(
            "Whether the lines of `equation`, `align` and `gather` are numbered (`true` by \
            default). Fixed when the registry is built: it cannot vary per document or per \
            render.",
        )
        .options::<MathRecomposeOptions>(
            Bind::Render,
            "How the recomposed LaTeX writes a display-math environment: the LaTeX written \
            before and after it, and whether the environment is written through the command \
            `\\flmMathEnvironment`. Set per render request.",
        )
}


/// The name of the element type of the six display-math environments,
/// `flm.math.environment`. The data of each element value is a
/// [`MathEnvironment`].
pub const ENVIRONMENT: &str = "flm.math.environment";

/// The name of the capability `flm.math.content`: the mathematics of the
/// document has been checked. No stage of this feature provides it: a feature
/// that processes mathematics provides it with a check of its own and adds that
/// check to the collection [`flm.validated`](crate::stdcore::VALIDATED), so that
/// a workflow that validates a document runs it exactly when such a feature is
/// registered (the feature of the package
/// [`math-passthrough`](crate::math_passthrough) accepts every formula). See
/// the module documentation, *The capabilities*.
pub const CONTENT: &str = "flm.math.content";

/// The name of the capability `flm.math.rendered`: some rendered form of the
/// mathematics exists.
///
/// No standard feature provides it or a refinement of it. An extension that
/// renders mathematics provides a refinement named in its own namespace, and a
/// rule that needs rendered mathematics of any kind requires
/// `flm.math.rendered`. A plan that demands it fails to build when no
/// registered feature provides a refinement.
pub const RENDERED: &str = "flm.math.rendered";


/// The reference kind of every equation line's reference point, `equation`:
/// the name that says what sort of place a reference point is
/// ([`RefPoint`]).
pub const KIND: &str = "equation";

/// The label types that an equation line that can be numbered accepts: `eq`.
///
/// A label is written `\label{type:label}`, and its type, the part before the
/// colon, says what sort of place it marks ([reference points, labels and
/// anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)).
/// An equation line takes `\label{eq:euler}`; a label of any other type on
/// that line is reported by the label check of the references feature
/// ([`CHECK_LABELS`](crate::stdcore::features::refs::CHECK_LABELS)) as
/// `flm.refs.label-type-not-accepted`.
pub const LABEL_TYPES: [&str; 1] = ["eq"];

/// The name of the counter that a numbered equation line counts in,
/// `flm.math.equation`: its counter value at the line is the line's number
/// ([`CounterDecl`]).
///
/// This feature declares the counter ([`counter_decl`]). Every line that can
/// be numbered and carries no `\tag` counts in it, unless the feature's
/// configuration numbers no equations ([`MathConfig`]). The counter is not
/// numbered within another counter; a document that wants equations numbered
/// within sections, `(1.1)`, `(1.2)`, sets that in its numbering options
/// ([`NumberingOptions`](crate::stdcore::features::numbering::NumberingOptions)).
pub const COUNTER: &str = "flm.math.equation";

/// The name of the counter style that an equation line's number is shown in,
/// `flm.math.style.equation` ([`CounterStyleDecl`]): the number format, the
/// words a reference writes before the number and the parentheses around it.
///
/// This feature declares the counter style; [`counter_style`] says what it
/// writes. See [`COUNTER`].
pub const COUNTER_STYLE: &str = "flm.math.style.equation";

/// The text that the anchor of an equation line without a label begins with,
/// `eq`: the first of the [`LABEL_TYPES`].
///
/// Every equation line has an anchor, the identifier a link points at
/// ([`ANCHORS`](crate::stdcore::features::refs::ANCHORS)), so that a link from
/// a table of contents, another page or a script can lead to any line. A line
/// labeled `eq:euler` has the anchor `eq-euler`; a line without a label has
/// the anchor `eq--`, its position among the document's equation lines and
/// four letters, as in `eq--8kqzt`
/// ([`auto_anchor`](crate::stdcore::features::refs::auto_anchor)).
pub const ANCHOR_PREFIX: &str = LABEL_TYPES[0];


/// The display-math environments the feature declares, as `(name, numbered)`,
/// where `numbered` is the value of [`MathEnvironment::numbered`].
pub const ENVIRONMENTS: [(&str, bool); 6] = [
    ("equation", true),
    ("equation*", false),
    ("align", true),
    ("align*", false),
    ("gather", true),
    ("gather*", false),
];

/// The data of one element value of `flm.math.environment`: which display-math
/// environment was written, and whether its lines are numbered.
///
/// The value is also the element's [`Referenceable`] aspect: it reads the
/// environment's body as equation lines and declares one reference point per
/// line (see the module documentation, *Equation lines, reference points and
/// numbers*).
///
/// A lowering rule or a stage reads it from a node with
/// [`element_value`](crate::model::element::element_value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathEnvironment {
    /// The environment's name as written (`align*`).
    pub name: Name,
    /// Whether the environment is an unstarred form, whose lines can be
    /// numbered. Such a line states a [number
    /// source](crate::model::aspects::NumberSource) unless `\nonumber` or a
    /// `\tag` is written on it, or the configuration numbers no equations.
    pub numbered: bool,
    /// Whether the registry numbers equations at all
    /// ([`MathConfig::numbered_equations`]), as the configuration was when the
    /// registry was built. A line states a number source only when this and
    /// [`numbered`](MathEnvironment::numbered) are both `true`; a line of an
    /// unstarred environment keeps its labels in either case.
    pub numbered_equations: bool,
}

/// The configuration of the [`math`](self) feature: whether the lines of the
/// unstarred display-math environments are numbered.
///
/// A feature's configuration is the plain Rust value the feature is
/// constructed from; it is fixed when the registry is built ([settings,
/// options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
/// This setting must be fixed when the registry is built, because the
/// reference points that an environment declares depend on it, and the code
/// that declares them (the element's [`Referenceable`] aspect; see [elements
/// and aspects](crate::guide::concepts_overview#elements-and-aspects)) cannot
/// read an option set per document or per render. The feature adds
/// [`digest_bytes`](MathConfig::digest_bytes) to the registry digest, so that
/// two registries built with different configurations have different
/// digests.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::math::MathConfig;
///
/// assert!(MathConfig::new().numbered_equations);
/// let unnumbered = MathConfig::new().numbered_equations(false);
/// assert!(!unnumbered.numbered_equations);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct MathConfig {
    /// Whether the lines of `equation`, `align` and `gather` that carry no
    /// `\nonumber` and no `\tag` are numbered. `true` by default. The lines
    /// of `equation*`, `align*` and `gather*` are never numbered.
    pub numbered_equations: bool,
}

impl Default for MathConfig {
    fn default() -> MathConfig {
        MathConfig { numbered_equations: true }
    }
}

impl MathConfig {
    /// The default configuration: the lines of the unstarred environments are
    /// numbered.
    pub fn new() -> MathConfig {
        MathConfig::default()
    }

    /// Builder: sets [`numbered_equations`](MathConfig::numbered_equations).
    pub fn numbered_equations(mut self, numbered: bool) -> MathConfig {
        self.numbered_equations = numbered;
        self
    }

    /// The bytes this configuration adds to the [registry
    /// digest](crate::model::digest), one per field.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let MathConfig { numbered_equations } = self;
        vec![u8::from(*numbered_equations)]
    }
}

impl std::hash::Hash for MathConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

/// The reference points of a display-math environment: one per equation line,
/// in the order the lines are written ([`lines::equation_lines`]), including
/// the lines that cannot be numbered.
///
/// **A line that can be numbered** (a line of an unstarred environment with no
/// `\nonumber`) keeps the labels written on it and accepts the
/// [`LABEL_TYPES`]. It states:
///
/// - a [number source](NumberSource) when it carries no `\tag` and the
///   feature's configuration numbers equations
///   ([`MathConfig::numbered_equations`]): the [`COUNTER`], the
///   [`COUNTER_STYLE`] and one level to advance, so that a reference to the
///   line shows its number;
/// - the content of its `\tag` when it carries one, and no number source: a
///   reference to the line shows the tag as the author wrote it, formatting
///   included, and the line counts in nothing.
///
/// A line with a `\tag` therefore states no number source and still keeps its
/// labels, because a reference to it shows the tag.
///
/// **A line that cannot be numbered** (every line of `equation*`, `align*` and
/// `gather*`, and a line marked `\nonumber`) keeps no label and accepts no
/// label type, so no reference resolves to it and no safe label is assigned
/// for a `\label` written on it. Such a `\label` or `\tag` is reported once,
/// when the body is parsed, as `flm.math.label-without-number`
/// ([`lines::LabelWithoutNumber`]); a `\ref` to that label is then
/// unresolved. The line is still a reference point, so that a tool can walk
/// every line of every environment, and it still has an anchor.
///
/// Every line's anchor follows [`AnchorPolicy::Auto`] with the prefix
/// [`ANCHOR_PREFIX`]: the anchor of its first label, or one derived from its
/// position. A line's mathematics is never stated as content, because a
/// reference shows a number or a tag, never a formula.
impl Referenceable for MathEnvironment {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        lines::equation_lines(node)
            .iter()
            .map(|line| {
                let can_be_numbered = self.numbered && !line.nonumber;
                let (labels, malformed) = if can_be_numbered {
                    refs::point_labels(line.labels.iter())
                } else {
                    (Vec::new(), Vec::new())
                };
                RefPoint {
                    kind: RefKind::from(KIND),
                    labels,
                    malformed,
                    computed: 0,
                    // A line with a label of its own is anchored on it; one
                    // without takes its position among the equation lines.
                    anchor: AnchorPolicy::Auto { prefix: ANCHOR_PREFIX.to_string() },
                    number: (can_be_numbered && self.numbered_equations && line.tag.is_none())
                        .then(|| NumberSource {
                        counter: Name::from(COUNTER),
                        instances: Vec::new(),
                        style: Name::from(COUNTER_STYLE),
                        advance: vec![0],
                        number_format: None,
                    }),
                    content: line.tag.as_ref().and_then(|tag| tag.content_ref()),
                    accepts: if can_be_numbered {
                        AcceptedRefTypes::these(LABEL_TYPES)
                    } else {
                        // Nothing may be written for a line with no number.
                        AcceptedRefTypes::These(Vec::new())
                    },
                }
            })
            .collect()
    }
}

/// The element value of one display-math environment, `name`: a block whose
/// [`MathEnvironment`] value records the name and declares the reference
/// points of its lines, numbered where `numbered` and `numbered_equations` are
/// both `true`.
fn environment_element(name: &str, numbered: bool, numbered_equations: bool) -> Arc<ElementSpec> {
    let value = Arc::new(MathEnvironment { name: Name::from(name), numbered, numbered_equations });
    Arc::new(
        ElementSpec::new(ENVIRONMENT)
            .docs(format!("The `{name}` display-math environment."))
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block)))
            .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
            .with_value(value),
    )
}

/// The declaration of the counter [`COUNTER`] (`flm.math.equation`), shown by
/// default in the counter style [`COUNTER_STYLE`] and numbered within no other
/// counter.
///
/// Numbered within no other counter means that the counter never restarts and
/// that no other counter's number is written before its own. A document that
/// wants equations numbered within sections, `(1.1)`, `(1.2)`, `(2.1)`, sets
/// that in its numbering options
/// ([`NumberingOptions`](crate::stdcore::features::numbering::NumberingOptions)).
///
/// The [`Math`] feature contributes this declaration to the numbering
/// feature's service slot of counters as a peer contribution, a contribution to
/// a service slot that another feature owns ([`services`](crate::core::services)):
/// where the numbering feature is not registered, the contribution is dropped
/// with one Note and no equation is numbered.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::math;
///
/// let decl = math::counter_decl();
/// assert_eq!(decl.name, math::COUNTER);
/// assert_eq!(decl.default_style, math::COUNTER_STYLE);
/// assert!(decl.within.is_none());
/// ```
pub fn counter_decl() -> CounterDecl {
    CounterDecl::new(COUNTER, COUNTER_STYLE).docs(
        "The equation counter: every numbered line of a display-math environment counts in \
         it, in the order of the document.",
    )
}

/// The declaration of the counter style [`COUNTER_STYLE`]
/// (`flm.math.style.equation`): the number in decimal digits between
/// parentheses, `(3)`; the prefix words `Eq.~` before one number and `Eqs.~`
/// before several, and `Equation~` and `Equations~` in the variant `capital`;
/// several numbers joined compactly (`(1,2)`, `(1–3)`).
///
/// A counter style says how a counter value is shown ([`CounterStyleDecl`];
/// see [counters and counter
/// styles](crate::guide::concepts_overview#counters-and-counter-styles)). A
/// `\ref` to an equation writes the prefix words before the number, and
/// `\ref[noprefix]` and `\eqref` leave them out; the parentheses are written in
/// every case. The option `S` of a `\ref` selects the variant `capital`, for a
/// reference at the start of a sentence.
///
/// Every text of the counter style is written in the template language and
/// compiled once when the registry is built: the `~` of `Eq.~` is FLM's
/// non-breaking space.
///
/// The [`Math`] feature contributes this declaration to the numbering
/// feature's service slot of counter styles as a peer contribution, like
/// [`counter_decl`].
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::math;
///
/// let style = math::counter_style();
/// assert_eq!(style.name, math::COUNTER_STYLE);
/// assert_eq!(style.delimiters.open, "(");
/// assert_eq!(style.prefix_words.base.one, "Eq.~");
/// assert_eq!(style.prefix_words.variants["capital"].other, "Equations~");
/// ```
pub fn counter_style() -> CounterStyleDecl {
    let mut style = CounterStyleDecl::new(COUNTER_STYLE);
    style.number_format = NumberTemplate::formats("#{arabic}");
    style.delimiters = Delimiters::new("(", ")");
    style.prefix_words = PrefixWords::new("Eq.~", "Eqs.~")
        .with_variant(refs::CAPITAL_VARIANT, PluralWords::new("Equation~", "Equations~"));
    style.join = JoinDecl::compact();
    style
}

/// The `math` feature: it registers the six display-math environments and the
/// commands of their bodies, the text commands of math mode, `\ensuremath`,
/// `\eqref`, the counter and counter style of equations, and the two
/// capabilities. It
/// registers no fallback, no check and no rule on a formula: a feature that
/// processes mathematics does (the module documentation, *Processing
/// mathematics*).
#[derive(Debug, Default, Clone, Copy)]
pub struct Math {
    config: MathConfig,
}

impl Math {
    /// The feature with the configuration `config`.
    pub fn new(config: MathConfig) -> Math {
        Math { config }
    }
}

impl FlmFeature for Math {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Math").docs(
            "Mathematics: the display-math environments and their equation lines, the text \
             commands of math mode, `\\ensuremath`, `\\eqref`, equation numbering, and the capabilities \
             `flm.math.content` and `flm.math.rendered`. A feature that processes mathematics \
             (`math-passthrough`) checks and lowers the formulas.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner: FeatureName = b.feature().clone();
        for (name, numbered) in ENVIRONMENTS {
            b.element(
                ElementDef::new(
                    owner.clone(),
                    [name],
                    SyntaxDef::Environment {
                        args: Vec::new(),
                        body: BodyDef::new()
                            .mode(Mode::Math)
                            .scoped(lines::PACKAGE)
                            .parser(lines::body_behavior(numbered)),
                    },
                    environment_element(name, numbered, self.config.numbered_equations),
                )
                .docs(format!("`\\begin{{{name}}}…\\end{{{name}}}`: display math.")),
            )?;
        }
        text::register(b)?;
        ensuremath::register(b)?;
        // The `\label` of an equation line holds a label as the references
        // feature's `\label` does, with that feature's role; declared as a
        // peer, so that a registry without that feature still builds.
        b.peer_argument_role(refs::DEFINED_LABEL_ARG_ROLE)?;
        b.scoped_package(lines::package(owner.clone()))?;
        // The counter and its style belong to the feature that numbers
        // documents; this feature hands them over as peer contributions, so
        // that it builds where that feature is not registered.
        b.peer_slot::<CounterDecl>(COUNTERS_SLOT, Arc::new(counter_decl()))?;
        b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, Arc::new(counter_style()))?;
        // The feature that processes mathematics provides the capability and
        // adds its check to `flm.validated`; this feature only declares it.
        // See decision rationale at [§flm-dd-dr:math-processor-demands-content].
        b.capability(CapabilityDef::new(CONTENT).docs(
            "The mathematics of the document has been checked. No stage of the `math` feature \
             provides it: a feature that processes mathematics provides it with a check of its \
             own and adds that check to the collection `flm.validated` (the feature of the \
             package `math-passthrough` accepts every formula).",
        ))?;
        b.capability(CapabilityDef::new(RENDERED).docs(
            "Some rendered form of the mathematics exists. No standard feature provides it; \
             an extension that renders mathematics provides a refinement.",
        ))?;
        // The counter table belongs to the feature that numbers; a registry
        // need not hold it, so the `\eqref` rule reads it optionally, under
        // this peer declaration, and shows no number without it.
        b.peer_table::<Counters>(
            COUNTERS.name(),
            "the numbering feature's counter table, which the `\\eqref` rule reads for the \
             number of the equation line it refers to",
        )?;
        b.digest_input(&self.config.digest_bytes())?;
        latex::register(b)?;
        eqref::register(b, &owner)?;
        unprocessed::register(b)?;
        Ok(())
    }
}

/// The [`Math`] feature with the default configuration, as a shared value
/// ready to be listed in a package entry.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Math::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_environment_value_is_a_block_carrying_its_name() {
        let element = environment_element("align*", false, true);
        assert_eq!(element.name, ENVIRONMENT);
        let value = element.value_as::<MathEnvironment>().expect("valued");
        assert_eq!(value.name, "align*");
        assert!(!value.numbered);
        assert!(value.numbered_equations);
    }

    #[test]
    fn the_configuration_digests_its_one_field() {
        assert_ne!(
            MathConfig::new().digest_bytes(),
            MathConfig::new().numbered_equations(false).digest_bytes()
        );
    }
}
