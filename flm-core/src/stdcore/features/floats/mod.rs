//! The `floats` feature: figures, tables and other **floats**, blocks of
//! content shown with an optional caption, labels and a number.
//!
//! # What an author writes
//!
//! ```text
//! \begin{figure}
//!   \includegraphics{plot.png}
//!   \caption[alt=A plot]{A plot of the data, before and after.}
//!   \label{figure:plot}
//! \end{figure}
//! ```
//!
//! prints, on the plain-text output,
//!
//! ```text
//! [image: plot.png]
//!
//! Figure 1: A plot of the data, before and after.
//! ```
//!
//! A **float type** is one kind of float, such as `figure` or `table`
//! ([`FloatType`]). Each float type is an environment of the type's name that
//! takes no argument. Its body is laid out in paragraphs and blocks, and three
//! commands may be written in it, anywhere directly in the body and in any
//! order:
//!
//! - `\caption[alt=…]{…}`: the float's **caption**, inline content shown
//!   under the float's content. Its optional argument holds key-value pairs
//!   with the only key `alt`, the caption's **alternative text**, which a
//!   reference to an unnumbered float shows in place of the full caption. A
//!   float holds at most one caption.
//! - `\label{type:label}`: a
//!   [label](crate::guide::concepts_overview#reference-points-labels-and-anchors),
//!   the text an author writes to refer to the float from elsewhere. Its
//!   label type (the part before the colon) must be one of the float type's
//!   accepted label types ([`FloatType::label_types`]), by default the type's
//!   own name: `\label{figure:plot}`.
//! - `\nonumber`: the float is not numbered.
//!
//! The three commands are the elements of the scoped package
//! [`FIELDS_PACKAGE`] (`flm.floats.fields`). A scoped package
//! ([`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef)) is a set of
//! element declarations that can be written only inside the environment bodies
//! and arguments whose declarations refer to the package by name ([scoped
//! packages](crate::guide::concepts_overview#scoped-packages)); every float
//! body refers to this one, so the three commands can be written in a float's
//! body and nowhere else. Each is a field, a macro the float reads as data
//! rather than as content
//! ([fields](crate::guide::concepts_overview#fields-data-a-construct-reads));
//! the float reads them from its body ([`body`]). `\label` is the references
//! feature's own declaration ([`refs::label_package`]), declared here a second
//! time so that a label can be written in a float's body whether or not that
//! feature is registered.
//!
//! The rest of the body is the float's **content**. What the content may be is
//! decided by the float type: a type lists the **content handlers** it
//! accepts ([`FloatType::handlers`]), and a content handler
//! ([`FloatContentHandler`]) is a value, contributed by the feature that owns
//! a kind of content, that accepts a float's content or refuses it. The
//! handlers are tried in the type's order, and the first that accepts the
//! content decides. The shipped `figure` accepts exactly one image (the
//! handler `includegraphics`, which the `graphics` feature contributes), the
//! shipped `table` exactly one table of cells (the handler `cells`, which the
//! `cells` feature contributes) or exactly one image; this feature contributes
//! only the handler `any` ([`AnyContent`]), which accepts any content. Content
//! that no handler of the type accepts is reported, and is shown as written.
//!
//! On the plain-text and HTML outputs, a float is shown where it is written:
//! it is not moved, as LaTeX moves a float. By default, the recomposed LaTeX
//! sets a numbered float in the LaTeX float environment of the type's name,
//! which LaTeX places ([`latex`]).
//!
//! # Numbers, references and anchors
//!
//! Each float type has a **numbering policy** ([`FloatNumbering`]):
//!
//! - `AutoNumbered`, the default: a float of the type is numbered when it
//!   carries a `\label` and no `\nonumber`;
//! - `Numbered`: every float of the type is numbered unless it carries
//!   `\nonumber`, as a displayed equation is;
//! - `Unnumbered`: no float of the type is numbered.
//!
//! A `\nonumber` that changes nothing is reported with a Note
//! ([`NonumberWithoutEffect`]). A numbered float counts in its type's counter,
//! `flm.floats.<type>` ([`counter_name`]), and its number is shown in its
//! type's counter style, `flm.floats.style.<type>` ([`style_name`]), whose
//! words are the float type's: `Fig.~` and, under `\ref[S]{…}`, `Figure~` for
//! `figure` (`~` is a space at which no line break is made). A counter is a
//! dotted name under which things are counted in document order, and a counter
//! style says how a counter value is shown ([counters and counter
//! styles](crate::guide::concepts_overview#counters-and-counter-styles)). The
//! feature declares both for each type to the numbering feature, through
//! [`contribute_type`].
//!
//! Every float, numbered or not, is one reference point of the reference kind
//! of its type's name. A reference point
//! ([`RefPoint`](crate::model::aspects::RefPoint)) is one place in a document
//! that can be referred to, numbered, or given an anchor ([reference points,
//! labels and anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)).
//! A reference to a numbered float, `\ref{figure:plot}`, shows `Fig.~1`; a
//! reference to an unnumbered float shows its caption's alternative text where
//! one is written, else its caption. A reference to an unnumbered float with
//! no caption has nothing to show, and the references feature reports it. The
//! float's anchor, the identifier a link to it points at, is its type and
//! number (`figure-1`), else the anchor of its first label (`figure-plot` for
//! `\label{figure:plot}`), else none; the references feature computes it.
//!
//! # What the feature provides to the pipeline
//!
//! The `floats` feature ([`Floats`], an [`FlmFeature`]) registers, from its
//! configuration [`FloatsConfig`]:
//!
//! - **The element** [`FLOAT`] (`flm.floats.float`), with one element value per
//!   float type, each a [`FloatElement`] declared as the environment of the
//!   type's name, in text mode, as a block. [`contribute_type`] makes every
//!   declaration a float type needs; another feature calls it to declare a
//!   float type of its own.
//! - **The scoped package** [`FIELDS_PACKAGE`] (`flm.floats.fields`) of the
//!   three commands above. A float type may list further scoped packages
//!   ([`FloatType::scoped_packages`]), which make the commands of its content
//!   available in its body: `\includegraphics` in a `figure`.
//! - **Two service slots.** A service slot is the declaration, by the feature
//!   that owns an interaction, of a dotted name, the type of the values the
//!   slot takes and its arity; any feature may contribute a value to it
//!   ([services and service slots](crate::core::services)). [`TYPES_SLOT`]
//!   (`flm.floats.types`) holds every float type of the registry, and
//!   [`CONTENT_SLOT`] (`flm.floats.content`) every content handler; the
//!   feature contributes its configured types and [`AnyContent`].
//! - **The stage** [`CHECK_HANDLERS`] (`flm.floats.check-handlers`), which
//!   reports each content handler that a float type used in the document lists
//!   and that no registered feature contributes ([`MissingHandler`]). Every
//!   rule that lowers a float requires it, and it is a member of the
//!   collection [`flm.validated`](crate::stdcore::VALIDATED), so the `check`
//!   workflow ([`CHECK_WORKFLOW`](crate::stdcore::CHECK_WORKFLOW)), which
//!   validates a document and writes no output, runs it too.
//! - **The lowering rules.** A role and its kind tokens are feature-owned
//!   names on a presentation payload that say what a container is
//!   ([`pres::payload`](crate::stdcore::backends::pres::payload)). The general
//!   rule [`FLOAT_RULE`] ([`FloatRule`]), the default on every backend, lowers
//!   a float to a block of role [`FLOAT_ROLE`] (`flm.floats.float`) whose kind
//!   tokens are the float type's name and [`NUMBERED_KIND`] or [`UNNUMBERED_KIND`]. The block holds the
//!   content in a block of role [`CONTENT_ROLE`] and, where the float is
//!   numbered or has a caption, **the line under the content** in a block of
//!   role [`CAPTION_ROLE`]: `Figure 1: ` followed by the caption, `Figure 1`
//!   alone, or the caption alone, the caption name and number in a span of
//!   role [`NUMBER_ROLE`]. The plain-text output prints the two blocks one
//!   blank line apart. The rule requires the feature's style identifier
//!   `flm.floats.style` at every float, which on the `html` backend names the
//!   feature's stylesheet ([`html`]); the HTML backend writes the float as a
//!   `<figure>` and the line under its content as a `<figcaption>`. On the
//!   `latex-recomposed` backend, the rule [`latex::LATEX_RULE`] writes the
//!   float as the environment `flmFloat`, a LaTeX hook: an environment that
//!   the recomposed LaTeX writes and the style file defines with a plain
//!   default, which a user redefines in the document's preamble ([LaTeX
//!   hooks](crate::stdcore::backends::latex::preamble)). The feature's
//!   definition piece defines it ([`latex`]).
//! - **The settings.** The configuration [`FloatsConfig`] holds the float
//!   types. A feature's configuration is the plain Rust value the feature is
//!   constructed from; it is fixed when the registry is built and cannot vary
//!   per document or per render ([settings, options and
//!   configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//!   As settings, the float types are the key `types`, a map from a type's
//!   name to its fields. When a host combines several sources of settings, a
//!   source that writes one float type's entry leaves the other types' entries
//!   as the lower-ranked sources wrote them
//!   ([`SettingsDecl::merge_entries`](crate::core::settings::SettingsDecl::merge_entries));
//!   the values of `words` and `number_format` are FLM markup.
//!
//! The rendering guide shows what each output writes: [floats on the plain-text
//! output](crate::guide::rendering#floats), [floats in
//! HTML](crate::guide::rendering#floats-in-html) and [floats in the recomposed
//! LaTeX](crate::guide::rendering#floats-in-the-recomposed-latex). The language
//! guide describes floats for authors ([Floats](crate::guide::language_syntax#floats)).
//!
//! # Interactions with other features
//!
//! The feature requires no other feature. It makes peer declarations of what
//! the other features own: a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName)) of an element, element type,
//! table, service slot, scoped package, argument role or backend tells the
//! registry build that another feature owns that name, and the build resolves it after every
//! feature has registered.
//!
//! - **Numbering.** The counter and the counter style of each float type are
//!   contributed to the numbering feature's service slots as peer
//!   contributions, and the rules read the counter table
//!   [`COUNTERS`] (`flm.counters`) optionally. Without the numbering feature, a float that
//!   its policy numbers is reported ([`NoNumbering`]) and shown unnumbered,
//!   except on the recomposed LaTeX, where LaTeX numbers it.
//! - **References.** The rules read the anchors table
//!   [`ANCHORS`](crate::stdcore::features::refs::ANCHORS) optionally. Without
//!   the references feature, a float has no anchor and a `\label` in it has no
//!   effect and is reported with a Note ([`LabelWithoutRefs`]); the label still
//!   counts for an `AutoNumbered` type. On the recomposed LaTeX, the rule also
//!   reads the safe-labels table
//!   ([`SAFE_LABELS`](crate::stdcore::features::refs::SAFE_LABELS)) optionally.
//!   The argument of the `\label` command in a float's body has the
//!   references feature's argument role
//!   [`DEFINED_LABEL_ARG_ROLE`](crate::stdcore::features::refs::DEFINED_LABEL_ARG_ROLE),
//!   which the feature declares as a peer.
//! - **Graphics and cells.** The shipped float types refer to the scoped
//!   packages and content handlers of the `graphics` and `cells` features by
//!   name only ([`INCLUDEGRAPHICS_PACKAGE`], [`INCLUDEGRAPHICS_HANDLER`],
//!   [`CELLS_PACKAGE`], [`CELLS_HANDLER`]). Without one of those features, its
//!   commands cannot be written in a float's body and its handler accepts
//!   nothing; where the document holds a float of a type that lists the
//!   handler, the stage reports the handler once per run with a Note
//!   ([`MissingHandler`]).
//! - **The element index.** The stage reads the element index
//!   ([`ElementIndex`](crate::stdcore::features::index::ElementIndex), table
//!   [`ELEMENT_INDEX`](crate::stdcore::features::index::ELEMENT_INDEX)) to find
//!   the floats of the document.
//!
//! What a float's body gets wrong is reported while the float is lowered, by
//! the reader that every rule lowering a float calls ([`read_float`]), so
//! every backend reports the same conditions. The `check` workflow lowers
//! nothing and does not report them ([`diagnostics`]).
//!
//! # Map of the items
//!
//! - [`Floats`], [`feature`], [`FEATURE`]: the feature.
//! - [`FloatsConfig`], [`FloatType`], [`FloatNumbering`]: the configuration.
//! - [`FLOAT`], [`FloatElement`], [`contribute_type`], [`float_point`]: the
//!   element, its float types and its reference point.
//! - [`FIELDS_PACKAGE`], [`CAPTION_FIELD`], [`NONUMBER_FIELD`],
//!   [`CAPTION_OPTIONS_ARG`], [`CAPTION_TEXT_ARG`], [`ALT_KEY`]: the commands of
//!   a float's body.
//! - [`body`]: reading a float's body ([`read_float`], [`float_fields`],
//!   [`float_body`], [`read_caption`]).
//! - [`content`]: content handlers ([`FloatContentHandler`], [`AnyContent`],
//!   [`Handlers`], [`Verdict`]); [`TYPES_SLOT`], [`CONTENT_SLOT`],
//!   [`ANY_HANDLER`], [`CHECK_HANDLERS`].
//! - [`FloatRule`], [`FLOAT_RULE`], [`float_decl`], [`float_number`]: the
//!   general rule; [`FLOAT_ROLE`], [`CONTENT_ROLE`], [`CAPTION_ROLE`],
//!   [`NUMBER_ROLE`], [`NUMBERED_KIND`], [`UNNUMBERED_KIND`],
//!   [`RESERVED_KINDS`]: the names it stages.
//! - [`html`], [`latex`]: the rules of the `html` and `latex-recomposed`
//!   backends; [`latex_rendered`]: the definition piece of the
//!   `latex-rendered` backend.
//! - [`counter_name`], [`style_name`]: the counter and counter style of a
//!   float type.
//! - [`diagnostics`]: the conditions the feature reports.

pub mod body;
mod config;
pub mod content;
pub mod diagnostics;
mod elements;
pub mod html;
pub mod latex;
pub mod latex_rendered;
mod lower;

use std::sync::Arc;

use crate::core::settings::SettingsDecl;
use crate::core::registry::{BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::core::services::Arity;
use crate::model::names::Name;
use crate::stdcore::features::numbering::{COUNTERS, Counters};
use crate::stdcore::features::refs;
use crate::stdcore::VALIDATED;

pub use body::{
    Caption, FloatBody, FloatFields, FloatReading, float_body, float_fields, read_caption,
    read_float,
};
pub use config::{
    CELLS_HANDLER, CELLS_PACKAGE, FloatNumbering, FloatType, FloatsConfig, INCLUDEGRAPHICS_HANDLER,
    INCLUDEGRAPHICS_PACKAGE,
};
pub use content::{AnyContent, FloatContentHandler, Handlers, Verdict};
pub use diagnostics::{
    CaptionMissingValue, CaptionRepeated, CaptionRepeatedKey, CaptionUnknownKey, ContentRefused,
    LabelWithoutRefs, MissingHandler, NoNumbering, NonumberWithoutEffect, StageFailed,
};
pub use elements::{FloatElement, contribute_type, float_point};
pub use lower::{FLOAT_RULE, FloatRule, float_decl, float_number};

/// The feature's registered name, `floats`.
pub const FEATURE: &str = "floats";

/// Returns the declaration of the feature's settings: the fields of [`FloatsConfig`] are its
/// keys; `types` is combined entry by entry, and the `words` and `number_format` of every type
/// are FLM markup. See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<FloatsConfig>(
            "The float types, each an environment of the type's name: its caption name, its \
            numbering policy, its number format, the words a reference writes before its \
            number, the content handlers it accepts, the scoped packages available in its body \
            and the label types it accepts. Fixed when the registry is built: it cannot vary \
            per document or per render.",
        )
        .merge_entries("types", || crate::core::settings::named::to_data(&FloatsConfig::default().types, "name"))
        .markup("types.*.words")
        .markup("types.*.number_format")
}

/// The name of the element of every float, `flm.floats.float`: one element
/// value per float type, each carrying a [`FloatElement`].
pub const FLOAT: &str = "flm.floats.float";

/// The role of the block a float lowers to, `flm.floats.float`, the same
/// name as the element's. The HTML backend writes a block of this role as a
/// `<figure>`.
pub const FLOAT_ROLE: &str = FLOAT;

/// The role of the block holding a float's content,
/// `flm.floats.float.content`.
pub const CONTENT_ROLE: &str = "flm.floats.float.content";

/// The role of the block holding the line under a float's content,
/// `flm.floats.float.caption`. The HTML backend writes a block of this role
/// as a `<figcaption>`.
pub const CAPTION_ROLE: &str = "flm.floats.float.caption";

/// The role of the span holding a float's caption name and number in the
/// line under its content (`Figure 1`), `flm.floats.float.number`.
pub const NUMBER_ROLE: &str = "flm.floats.float.number";

/// The kind token of a float shown with a number, `numbered`. It is written
/// on the float's block and on the block of the line under its content, after
/// the float type's name.
pub const NUMBERED_KIND: &str = "numbered";

/// The kind token of a float shown without a number, `unnumbered`, written
/// where [`NUMBERED_KIND`] would be.
pub const UNNUMBERED_KIND: &str = "unnumbered";

/// The kind tokens that a float's block carries beside its type's name,
/// [`NUMBERED_KIND`] and [`UNNUMBERED_KIND`]. Because a float type's name is
/// also a kind token of its floats, no float type may have one of these
/// names ([`FloatType::validate`]).
pub const RESERVED_KINDS: [&str; 2] = [NUMBERED_KIND, UNNUMBERED_KIND];

/// The name of the service slot of the float types, `flm.floats.types`. It
/// takes [`FloatType`] values, any number of them, kept in the order
/// contributed (`Arity::OrderedMany`). The feature contributes its
/// configuration's types, and another feature contributes a type of its own
/// through [`contribute_type`]. Two types of one name in the slot fail the
/// registry build.
pub const TYPES_SLOT: &str = "flm.floats.types";

/// The name of the service slot of the content handlers,
/// `flm.floats.content`. It takes [`FloatContentHandler`] trait objects, any
/// number of them, kept in the order contributed (`Arity::OrderedMany`). The
/// feature contributes [`AnyContent`]; the feature that owns a kind of content
/// contributes its own handler as a peer contribution (see
/// [`content`]). Two handlers of one name fail the registry build.
pub const CONTENT_SLOT: &str = "flm.floats.content";

/// The name of the scoped package of the three commands of a float's body,
/// `flm.floats.fields`: `\caption`, `\label` and `\nonumber`. Every float
/// body refers to it.
pub const FIELDS_PACKAGE: &str = "flm.floats.fields";

/// The name of the caption command, `caption`, written `\caption[alt=…]{…}`.
pub const CAPTION_FIELD: &str = "caption";

/// The name of the command that marks a float unnumbered, `nonumber`.
pub const NONUMBER_FIELD: &str = "nonumber";

/// The name of a caption's optional argument, `options`, which holds
/// key-value pairs.
pub const CAPTION_OPTIONS_ARG: &str = "options";

/// The name of a caption's mandatory argument, `text`.
pub const CAPTION_TEXT_ARG: &str = "text";

/// The only key of a caption's optional argument, `alt`: the caption's
/// alternative text, which a reference to an unnumbered float shows.
pub const ALT_KEY: &str = "alt";

/// The name of the content handler that accepts any content, `any`
/// ([`AnyContent`]).
pub const ANY_HANDLER: &str = "any";

/// The name of the feature's stage, and of the capability it provides,
/// `flm.floats.check-handlers`. The stage reports, once per run, each content
/// handler that a float type used in the document lists and that no
/// registered feature contributes ([`MissingHandler`]).
pub const CHECK_HANDLERS: &str = "flm.floats.check-handlers";

/// Returns the name of the counter in which the floats of the type
/// `float_type` are counted, `flm.floats.<type>`.
pub fn counter_name(float_type: &str) -> Name {
    Name::from(format!("flm.floats.{float_type}"))
}

/// Returns the name of the counter style in which the number of a float of
/// the type `float_type` is shown, `flm.floats.style.<type>`.
pub fn style_name(float_type: &str) -> Name {
    Name::from(format!("flm.floats.style.{float_type}"))
}

/// The `floats` feature: from a [`FloatsConfig`], it declares one environment
/// per configured float type, the scoped package of a float's body, the two
/// service slots, the lowering rules and the stage. See the [module
/// documentation](self).
///
/// Registering the feature fails with [`BuildError::Feature`] when the
/// configuration fails [`FloatsConfig::validate`], and when two float types or
/// two content handlers of one name are contributed. The feature may be
/// registered at most once in a registry.
#[derive(Debug, Default, Clone)]
pub struct Floats {
    config: FloatsConfig,
}

impl Floats {
    /// Returns the feature with the configuration `config`, which is fixed for
    /// the life of the registry it is built into.
    pub fn new(config: FloatsConfig) -> Floats {
        Floats { config }
    }

    /// Returns the configuration this feature was constructed with.
    pub fn config(&self) -> &FloatsConfig {
        &self.config
    }
}

impl FlmFeature for Floats {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Floats").docs(
            "Floats such as figures and tables: one environment per float type, each an element \
             value of `flm.floats.float`, holding content with an optional caption, labels and a \
             number given by the type's numbering policy.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        self.config
            .validate()
            .map_err(|detail| BuildError::Feature { feature: owner.clone(), detail })?;
        b.service_slot::<FloatType>(
            TYPES_SLOT,
            Arity::OrderedMany,
            "The float types; `floats::contribute_type` declares each one, with the environment of \
             its name.",
        )?;
        b.service_slot::<dyn FloatContentHandler>(
            CONTENT_SLOT,
            Arity::OrderedMany,
            "The content handlers that float types list by name: each accepts a float's content \
             or refuses it.",
        )?;
        b.contribute_to::<dyn FloatContentHandler>(CONTENT_SLOT, Arc::new(AnyContent))?;
        b.element(elements::float_type_def(&owner))?;
        // The `\label` command of a float's body is the references feature's,
        // with that feature's role on its argument; declared as a peer, so
        // that a registry without that feature still builds.
        b.peer_argument_role(refs::DEFINED_LABEL_ARG_ROLE)?;
        b.scoped_package(elements::fields_package(&owner))?;
        for float_type in &self.config.types {
            contribute_type(b, Arc::new(float_type.clone()))?;
        }
        // The two tables belong to the numbering and the references features,
        // which may be absent: the rule reads them optionally.
        b.peer_table::<Counters>(
            COUNTERS.name(),
            "the numbering feature's counter table, which the float's rules read for the number \
             shown in the line under the float's content",
        )?;
        b.peer_table::<refs::Anchors>(
            refs::ANCHORS.name(),
            "the references feature's anchors table, which the float's rules read for the \
             float's anchor and to find out whether labels have an effect",
        )?;
        let handlers = Arc::new(Handlers::default());
        b.finalizer(Arc::new(content::CollectHandlers { handlers: Arc::clone(&handlers) }))?;
        b.lowering_rule(lower::float_rule(Arc::clone(&handlers)))?;
        html::register(b)?;
        latex_rendered::register(b)?;
        latex::register(b, handlers)?;
        b.capability(CapabilityDef::new(CHECK_HANDLERS).docs(
            "Each content handler that a float type used in the document lists, and that no \
             registered feature contributes, has been reported once. A member of \
             `flm.validated`.",
        ))?;
        b.stage(Arc::new(content::CheckHandlers))?;
        b.collection_member(VALIDATED, CHECK_HANDLERS)?;
        b.digest_input(&self.config.digest_bytes())?;
        Ok(())
    }
}

/// Returns the [`Floats`] feature with the default configuration (the float
/// types `figure` and `table`), ready to be listed in a package entry.
///
/// Use `Arc::new(Floats::new(config))` to list the feature with a
/// [`FloatsConfig`] of your own.
pub fn feature() -> Arc<dyn crate::core::registry::FlmFeature> {
    Arc::new(Floats::default())
}
