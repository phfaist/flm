//! What another feature declares to the numbering feature: a counter
//! ([`CounterDecl`]), the counter it is numbered within and how that counter's
//! number is written before its own ([`Within`]), and a parent set
//! ([`ParentSetDecl`]).
//!
//! A feature declares each of these by contributing a value to one of the
//! numbering feature's five service slots, whose names this module defines.
//! A service slot is the declaration, by the feature that owns an
//! interaction, of a dotted name, the type of the values the slot takes and
//! its arity ([`services`](crate::core::services)). The five service slots,
//! and the type of the values each takes, are:
//!
//! - [`COUNTERS_SLOT`] takes [`CounterDecl`];
//! - [`STYLES_SLOT`] takes [`CounterStyleDecl`](super::CounterStyleDecl);
//! - [`WITHIN_JOINERS_SLOT`] takes [`WithinJoinerDecl`](super::WithinJoinerDecl);
//! - [`NUMBER_WRITERS_SLOT`] takes [`NumberWriterDecl`](super::NumberWriterDecl);
//! - [`PARENT_SETS_SLOT`] takes [`ParentSetDecl`].
//!
//! The module also defines the reserved name prefix ([`RESERVED_PREFIX`]) and
//! the placeholder of the parent that applies ([`PARENT_PLACEHOLDER`]). The
//! [module documentation of the numbering feature](super) describes what the
//! feature does with these declarations.

use crate::model::names::Name;

/// The name of the service slot into which a feature declares its counters,
/// `flm.numbering.counters`.
///
/// The service slot takes [`CounterDecl`] values, any number of them, in
/// contribution order. A feature adds one counter per call of
/// [`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot).
/// The call is a peer declaration
/// ([`PeerName`](crate::core::registry::PeerName)) of the service slot, so the
/// feature still registers in a registry that holds no numbering feature: the
/// value is then dropped and the registry build records one Note ([services and
/// service slots](crate::guide::concepts_overview#services-and-service-slots)).
pub const COUNTERS_SLOT: &str = "flm.numbering.counters";

/// The name of the service slot into which a feature declares its counter
/// styles, `flm.numbering.styles`.
///
/// The service slot takes [`CounterStyleDecl`](super::CounterStyleDecl)
/// values, contributed the same way as the counters of [`COUNTERS_SLOT`].
pub const STYLES_SLOT: &str = "flm.numbering.styles";

/// The name of the service slot into which a feature contributes a
/// within-joiner registered by name, `flm.numbering.within-joiners`.
///
/// The service slot takes [`WithinJoinerDecl`](super::WithinJoinerDecl)
/// values, contributed the same way as the counters of [`COUNTERS_SLOT`]. A
/// counter asks for a registered within-joiner by its name with
/// [`Within::joiner_named`]. Two within-joiners of one name fail the registry
/// build.
pub const WITHIN_JOINERS_SLOT: &str = "flm.numbering.within-joiners";

/// The name of the service slot into which a feature contributes a number
/// writer, `flm.numbering.number-writers`.
///
/// The service slot takes [`NumberWriterDecl`](super::NumberWriterDecl)
/// values, contributed the same way as the counters of [`COUNTERS_SLOT`]. A
/// counter style asks for a number writer by its name with
/// [`CounterStyleDecl::with_number_writer`](super::CounterStyleDecl::with_number_writer).
/// Two number writers of one name fail the registry build.
pub const NUMBER_WRITERS_SLOT: &str = "flm.numbering.number-writers";

/// The name of the service slot into which a feature contributes a parent
/// set, `flm.numbering.parent-sets`.
///
/// The service slot takes [`ParentSetDecl`] values, contributed the same way
/// as the counters of [`COUNTERS_SLOT`]. A counter is numbered within a parent
/// set with [`Within::parent_set`]. Two parent sets of one name fail the
/// registry build.
pub const PARENT_SETS_SLOT: &str = "flm.numbering.parent-sets";

/// The prefix, `_`, that begins every name the numbering feature reserves for
/// itself.
///
/// A within template writes `#{_parent}` for the parent that applies
/// ([`PARENT_PLACEHOLDER`]), so no counter may have a name that begins with
/// this prefix: a counter or a parent set whose name begins with it fails the
/// registry build.
pub const RESERVED_PREFIX: &str = "_";

/// The name, `_parent`, under which a within template writes the parent that
/// applies.
///
/// In a within template, `#{_parent}` is the number of the parent that
/// applies at the point (the only parent, or among several the one advanced
/// most recently), with that parent's own ancestors written before it, as
/// `#{flm.headings.section}` is the section's number. See
/// [`Within::template`].
pub const PARENT_PLACEHOLDER: &str = "_parent";

/// A parent set: a named list of counters, any one of which may be the parent
/// of a counter numbered within the set with [`Within::parent_set`].
///
/// A feature that owns a family of counters contributes a parent set to the
/// service slot [`PARENT_SETS_SLOT`], so that another feature, or a document's
/// numbering options, can number a counter within whichever of these counters
/// was advanced most recently without listing them. The headings feature
/// contributes `flm.headings.levels`, the counters of its heading kinds from
/// the outermost level in.
///
/// The set's name is dotted and owned by the feature that contributes the
/// set, as a counter style's name is. A name beginning with
/// [`RESERVED_PREFIX`] fails the registry build, and so does a set that holds
/// a counter no feature declares.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::ParentSetDecl;
///
/// let levels = ParentSetDecl::new("example.levels", ["example.chapter", "example.section"]);
/// assert_eq!(levels.counters.len(), 2);
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ParentSetDecl {
    /// The set's dotted name, such as `flm.headings.levels`.
    pub name: Name,
    /// The counters of the set, in the order the contributing feature gives
    /// them.
    pub counters: Vec<Name>,
}

impl ParentSetDecl {
    /// Returns the parent set `name`, which holds `counters`.
    pub fn new(
        name: impl Into<Name>,
        counters: impl IntoIterator<Item = impl Into<Name>>,
    ) -> ParentSetDecl {
        ParentSetDecl { name: name.into(), counters: counters.into_iter().map(Into::into).collect() }
    }
}

/// Which counters a counter may be numbered within: one counter, any counter
/// of a list, or any counter of a registered parent set.
///
/// This is the `parents` field of [`Within`], which the constructors
/// [`Within::parent`], [`Within::parents`] and [`Within::parent_set`] set.
///
/// Where there are several possible parents, the **parent that applies** at a
/// reference point is the one among them that was advanced most recently in
/// document order. For example, an equation numbered within any of the
/// sectioning counters is written `4.1` after a chapter and before the
/// chapter's first section, and `4.1.2` once a section has begun in that
/// chapter.
#[derive(Clone, PartialEq, Eq, Hash, Debug, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Parents {
    /// Numbered within this one counter.
    One(Name),
    /// Numbered within whichever of these counters was advanced most
    /// recently.
    Any(Vec<Name>),
    /// Numbered within whichever counter of the parent set of this name was
    /// advanced most recently. A name no feature registers fails the registry
    /// build when a feature declares it, and is the Error
    /// `flm.numbering.unknown-parent-set` when a document's numbering options
    /// give it.
    Set(Name),
}

impl Parents {
    /// Returns the parents as a message writes them: `` `section` ``,
    /// `` any of `section`, `subsection` `` or
    /// `` the parent set `flm.headings.levels` ``.
    pub(super) fn describe(&self) -> String {
        match self {
            Parents::One(name) => format!("`{name}`"),
            Parents::Any(names) => format!("any of {}", quoted_list(names)),
            Parents::Set(name) => format!("the parent set `{name}`"),
        }
    }
}

/// Returns `names` in backquotes, separated by commas: `` `a`, `b` ``.
pub(super) fn quoted_list(names: &[Name]) -> String {
    names.iter().map(|name| format!("`{name}`")).collect::<Vec<_>>().join(", ")
}

/// The form of a within-joiner, which writes a counter's ancestors' numbers
/// before the counter's own number: one of the four forms a [`Within`] may
/// carry.
///
/// A form is data, not behavior: it can be compared, hashed, and carried in a
/// document's numbering options. What a form writes is decided when it is
/// compiled, against the built language and against the within-joiners the
/// registry holds ([`Within::compile`]).
#[derive(Clone, PartialEq, Eq, Hash, Debug, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WithinJoinerSpec {
    /// The parent's number, then this joining text, which is FLM: what
    /// [`Within::parent`] and [`Within::joined_by`] make. A subsection joined
    /// by `.` is written `1.2`.
    ///
    /// The parent is written with its own ancestors before it, each as a part
    /// of its own, so a chain three counters deep is written `1.2.3` with
    /// every counter declaring only its joining text.
    Joined(String),
    /// A within template, in the language that [`Within::template`]
    /// describes.
    Template(String),
    /// Nothing of the ancestors is written: the counter restarts under its
    /// parent, and its number is shown **bare**, that is, without its
    /// parents' numbers. What [`Within::displayed_bare`] makes.
    Bare,
    /// The within-joiner registered under this name in
    /// [`WITHIN_JOINERS_SLOT`], which writes what its code writes. A name no
    /// feature registers is refused: in a feature's declaration it fails the
    /// registry build, and in a document's numbering options it is the Error
    /// `flm.numbering.unknown-within-joiner`, and the counter keeps the
    /// hierarchy it was declared with.
    Named(Name),
}

/// The counter within which a counter is numbered, and how that counter's
/// number is written before the counter's own: the counter's place in the
/// hierarchy of counters.
///
/// A subsection numbered within a section is written `2.3`: the parent is the
/// counter `flm.headings.section`, and the within-joiner is the joining text
/// `.`. The two are declared together because the parent decides what
/// restarts the counter and the within-joiner decides what is shown of the
/// parent, and neither is of any use without the other. The [module
/// documentation](super#numbering-within-another-counter) describes the
/// mechanism.
///
/// Three constructors say which counter is the parent:
///
/// - [`Within::parent`]: numbered within that one counter.
/// - [`Within::parents`]: numbered within whichever of several counters was
///   advanced most recently in document order.
/// - [`Within::parent_set`]: the same over the counters of a parent set that
///   a feature registered ([`ParentSetDecl`]).
///
/// Each writes the parent's number and a full stop before the counter's own
/// number, `1.2`. Four builders change what is written:
///
/// - [`joined_by`](Within::joined_by): another joining text.
/// - [`template`](Within::template): a within template, which may write
///   markup around an ancestor's number and may refer to more than one
///   ancestor.
/// - [`displayed_bare`](Within::displayed_bare): nothing of the parent is
///   written; the counter restarts under the parent and its number is shown
///   bare.
/// - [`joiner_named`](Within::joiner_named): a within-joiner that a feature
///   registered by name, which writes what its code writes.
///
/// The builder [`no_restart`](Within::no_restart) keeps the parent's number
/// written before the counter's own but stops the parent from restarting the
/// count: `1.1`, `1.2`, `2.3`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::Within;
///
/// // `1.2`, the usual shape.
/// let plain = Within::parent("flm.headings.section");
/// // `1-2`.
/// let dashed = Within::parent("flm.headings.section").joined_by("-");
/// // The section's number in bold, then a full stop.
/// let bold = Within::parent("flm.headings.section").template(r"\textbf{#{flm.headings.section}}.");
/// // The same, referring to the parent that applies rather than to the counter.
/// let short = Within::parent("flm.headings.section").template(r"\textbf{#{_parent}}.");
/// // Restarted by the section, and shown without the section's number.
/// let bare = Within::parent("flm.headings.section").displayed_bare();
/// // What the within-joiner registered under this name writes.
/// let custom = Within::parent("flm.headings.section").joiner_named("example.compact");
/// // Within whichever of the three was advanced last: `4.1`, `4.1.2`.
/// let any = Within::parents([
///     "example.chapter",
///     "flm.headings.section",
///     "flm.headings.subsection",
/// ]);
/// // The same over the heading counters a registry declares.
/// let levels = Within::parent_set("flm.headings.levels");
/// // `1.1`, `1.2`, then `2.3`: the section is written and does not restart.
/// let continued = Within::parent("flm.headings.section").no_restart();
/// # let _ = (plain, dashed, bold, short, bare, custom, any, levels, continued);
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Within {
    /// The counter within which this counter is numbered, or the counters any
    /// one of which may be its parent. The parent that applies restarts this
    /// counter: the counter's count is kept separately for each list of
    /// ancestors it is advanced under, each ancestor identified by its name
    /// and its value.
    pub parents: Parents,
    /// The within-joiner: how the ancestors' numbers are written before this
    /// counter's own.
    pub joiner: WithinJoinerSpec,
    /// Whether the parent is left out of what restarts the count. `true`
    /// continues the count across the parent's advances, and the parent's
    /// number is still written before the counter's own (`1.1`, `1.2`,
    /// `2.3`). `false` by default.
    pub no_restart: bool,
}

impl Within {
    /// Returns the place of a counter numbered within `parent`, whose number
    /// is written before the counter's own with a full stop after it: `1.2`.
    pub fn parent(parent: impl Into<Name>) -> Within {
        Within::with_parents(Parents::One(parent.into()))
    }

    /// Returns the place of a counter numbered within whichever of `parents`
    /// was advanced most recently in document order; that counter's number is
    /// written before the counter's own with a full stop after it.
    ///
    /// A reference point that the document reaches before any of `parents`
    /// was advanced takes no number and is reported as the Error
    /// `flm.numbering.parent-uninitialized`, as is a point under a single
    /// parent that has no value yet.
    pub fn parents(parents: impl IntoIterator<Item = impl Into<Name>>) -> Within {
        Within::with_parents(Parents::Any(parents.into_iter().map(Into::into).collect()))
    }

    /// Returns the place of a counter numbered within whichever counter of the
    /// parent set `name` was advanced most recently, as [`Within::parents`]
    /// does over the counters of the set. The parent set is one that a feature
    /// registered as a [`ParentSetDecl`].
    pub fn parent_set(name: impl Into<Name>) -> Within {
        Within::with_parents(Parents::Set(name.into()))
    }

    /// Returns the place of a counter numbered within `parents`, joined by a
    /// full stop and restarted by them.
    fn with_parents(parents: Parents) -> Within {
        Within { parents, joiner: WithinJoinerSpec::Joined(".".to_string()), no_restart: false }
    }

    /// Builder: the parent no longer restarts the count, which goes on across
    /// the parent's advances; the parent's number is still written before the
    /// counter's own: `1.1`, `1.2`, then `2.3` after the second section.
    ///
    /// Only the parent is left out of what the count is kept under; the
    /// parent's own ancestors still restart the count.
    pub fn no_restart(mut self) -> Within {
        self.no_restart = true;
        self
    }

    /// Builder: the parent's number is followed by `text` instead of a full
    /// stop. The text is FLM.
    pub fn joined_by(mut self, text: impl Into<String>) -> Within {
        self.joiner = WithinJoinerSpec::Joined(text.into());
        self
    }

    /// Builder: the ancestors' numbers are written by the within template
    /// `text`.
    ///
    /// A within template is FLM in which `#{name}` writes the counter `name`:
    /// that counter's value in that counter's own counter style, the number
    /// alone with no word and no delimiters, preceded by its own ancestors'
    /// numbers. `#{_parent}` ([`PARENT_PLACEHOLDER`]) writes the parent that
    /// applies in the same way: the only parent, or among several the one
    /// advanced most recently. Every other character is written as given, and
    /// markup may surround a substitution: `\textbf{#{flm.headings.section}}.`
    /// writes the section's number in bold. A counter's name is the whole text
    /// between the braces, dots included.
    ///
    /// A substitution may carry keys between brackets, separated by commas:
    /// `#[Roman]{flm.headings.section}` writes the section in that [number
    /// format](super::NumberFormat) instead of in its own counter style's, and
    /// `#[bare]{flm.headings.section}` writes the section's own number without
    /// its ancestors' numbers. A key that is neither `bare` nor the name of a
    /// number format is refused when the template is compiled.
    ///
    /// The template writes one [part](super::Part) per substitution, holding
    /// the text after it (the text before the first substitution belongs to
    /// the first part); a template that markup prevents from being cut at its
    /// substitutions writes one part for all of them. A template that holds
    /// text and no substitution writes one part that shows no ancestor, and
    /// the empty template writes no part. To write nothing of the parent,
    /// [`displayed_bare`](Within::displayed_bare) says so directly.
    pub fn template(mut self, text: impl Into<String>) -> Within {
        self.joiner = WithinJoinerSpec::Template(text.into());
        self
    }

    /// Builder: nothing of the ancestors is written. The counter still
    /// restarts under its parent (it is counted separately for each value of
    /// the parent), and its number is shown **bare**, that is, without its
    /// parents' numbers, as `#[bare]{counter}` writes an ancestor in a
    /// [within template](Within::template).
    pub fn displayed_bare(mut self) -> Within {
        self.joiner = WithinJoinerSpec::Bare;
        self
    }

    /// Builder: the ancestors' numbers are written by the within-joiner
    /// registered under `name` in [`WITHIN_JOINERS_SLOT`].
    pub fn joiner_named(mut self, name: impl Into<Name>) -> Within {
        self.joiner = WithinJoinerSpec::Named(name.into());
        self
    }
}

/// A counter, as the feature that declares it contributes it to the service
/// slot [`COUNTERS_SLOT`].
///
/// A counter is a dotted name under which things are counted in document
/// order, declared by a feature together with the counter it is numbered
/// within; its value at a reference point is a number with optional
/// sub-numbers ([`CounterValue`](super::CounterValue)). The declaration gives
/// the counter's name, the counter style its numbers are shown in by default,
/// the counter it is numbered within ([`Within`]), and the counter's nesting
/// joiner ([`nesting`](CounterDecl::nesting)). The concepts overview presents
/// counters and counter styles together ([counters and counter
/// styles](crate::guide::concepts_overview#counters-and-counter-styles)).
///
/// The numbering feature numbers every reference point whose number source
/// ([`NumberSource`](crate::model::aspects::NumberSource)) gives a declared
/// counter. The feature that owns the point decides whether the point
/// gives a number source (the numbering depth of the headings feature, the
/// switch for numbered equations of the mathematics feature); the numbering
/// feature has no switch of its own.
///
/// A counter's name is dotted and owned by the feature that declares it, like
/// every name the library registers: `flm.headings.section`,
/// `flm.math.equation`, and a name under a prefix of its own for a counter
/// that a third party declares. One feature declares a counter, and a name is
/// declared once per registry; the points of any feature may count in it, and
/// points of several kinds that count in one counter form one sequence. A
/// counter style's name is dotted and owned in the same way
/// (`flm.math.style.equation`) and is separate from the counter's name,
/// because two features may each declare a counter style for the same
/// counter.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::{CounterDecl, Within};
///
/// // Theorems count from one in every section, and are shown `2.13`.
/// let decl = CounterDecl::new("example.theorem", "example.style.theorem")
///     .docs("The theorems of the document.")
///     .within(Within::parent("flm.headings.section"));
/// assert!(decl.within.is_some());
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CounterDecl {
    /// The counter's name, dotted and owned by the declaring feature, such as
    /// `flm.headings.section` or `flm.math.equation`. Two declarations of one
    /// name fail the registry build, and so does a name beginning with
    /// [`RESERVED_PREFIX`].
    pub name: Name,
    /// One sentence saying what the counter counts. The numbering feature
    /// keeps it with the declaration and does not display it.
    pub docs: String,
    /// The name of the counter style in which a point's number is shown when
    /// the point's number source gives no other. It is also the counter style
    /// in which this counter's number is written as an ancestor until a point
    /// advances the counter. A counter style no feature declares fails the
    /// registry build.
    pub default_style: Name,
    /// The counter within which this counter is numbered, or `None` for a
    /// counter numbered within nothing, which is never restarted.
    pub within: Option<Within>,
    /// The counter's **nesting joiner**: what a point that counts in a
    /// *nested* scoped instance of this counter writes of the enclosing
    /// instance's number before its own, or `None` for
    /// [`WithinJoinerSpec::Bare`], which writes nothing of it.
    ///
    /// A scoped instance is an independent sequence of the counter's values
    /// attached to an enclosing node
    /// ([`NumberSource::instances`](crate::model::aspects::NumberSource::instances)).
    /// An instance inside another is numbered within the enclosing instance's
    /// current value, which the records of its points carry as their last
    /// ancestor. The nesting joiner writes that ancestor, in any of the four
    /// forms a [`Within`] may take: `Joined(".")` and
    /// `Template("#{_parent}.")` both write `2.(b)` for the second item of a
    /// list nested in the second item of another list whose items are written
    /// `1`, `2`, and the default writes `(b)`. The ancestor is written in its
    /// own counter style, so where the enclosing items are written `1.`, `2.`,
    /// `Template("#{_parent}")` writes `2.(b)`. In a within template,
    /// `#{_parent}` and the counter's own name both refer to the enclosing
    /// instance.
    ///
    /// The nesting joiner is compiled when the registry is built, as the
    /// counter's [`within`](CounterDecl::within) is. It may write only
    /// counters that are ancestors of a nested point under every chain of
    /// counters the counter may be numbered within, that is, the counter
    /// itself and the counters it is numbered within; any other counter fails
    /// the registry build.
    pub nesting: Option<WithinJoinerSpec>,
}

impl CounterDecl {
    /// Returns a counter named `name`, shown in the counter style
    /// `default_style`, numbered within nothing, with no nesting joiner and an
    /// empty description.
    pub fn new(name: impl Into<Name>, default_style: impl Into<Name>) -> CounterDecl {
        CounterDecl {
            name: name.into(),
            docs: String::new(),
            default_style: default_style.into(),
            within: None,
            nesting: None,
        }
    }

    /// Builder: sets the sentence that says what the counter counts
    /// ([`docs`](CounterDecl::docs)). Without the cargo feature `docs`, the
    /// text is dropped and the field stays empty.
    pub fn docs(mut self, docs: impl Into<String>) -> CounterDecl {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// Builder: sets the counter within which this counter is numbered
    /// ([`within`](CounterDecl::within)).
    pub fn within(mut self, within: Within) -> CounterDecl {
        self.within = Some(within);
        self
    }

    /// Builder: sets the nesting joiner, what a point of a nested scoped
    /// instance writes of the enclosing instance's number
    /// ([`nesting`](CounterDecl::nesting)).
    pub fn nesting(mut self, joiner: WithinJoinerSpec) -> CounterDecl {
        self.nesting = Some(joiner);
        self
    }
}

impl Within {
    /// Writes this value into `out` for a registry digest, every field of it:
    /// the parents, the within-joiner and `no_restart`, each text preceded by
    /// its length so that two different values never write the same bytes.
    ///
    /// The fields are destructured by name, so a field added to `Within` and
    /// not written here fails to compile.
    pub(crate) fn digest_into(&self, out: &mut Vec<u8>) {
        let Within { parents, joiner, no_restart } = self;
        match parents {
            Parents::One(name) => {
                out.push(0);
                digest_text(out, name.as_str());
            }
            Parents::Any(names) => {
                out.push(1);
                out.extend_from_slice(&(names.len() as u64).to_le_bytes());
                for name in names {
                    digest_text(out, name.as_str());
                }
            }
            Parents::Set(name) => {
                out.push(2);
                digest_text(out, name.as_str());
            }
        }
        joiner.digest_into(out);
        out.push(u8::from(*no_restart));
    }
}

impl WithinJoinerSpec {
    /// Writes this value into `out` for a registry digest: which form it is,
    /// and its text or name preceded by its length, so that two different
    /// values never write the same bytes. A feature whose configuration holds
    /// a nesting joiner adds it to its digest through this function.
    pub(crate) fn digest_into(&self, out: &mut Vec<u8>) {
        match self {
            WithinJoinerSpec::Joined(text) => {
                out.push(0);
                digest_text(out, text);
            }
            WithinJoinerSpec::Template(text) => {
                out.push(1);
                digest_text(out, text);
            }
            WithinJoinerSpec::Bare => out.push(2),
            WithinJoinerSpec::Named(name) => {
                out.push(3);
                digest_text(out, name.as_str());
            }
        }
    }
}

/// Why the parents of a [`Within`] give no counter to be numbered within.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum ParentsError {
    /// A parent set that no feature registers.
    UnknownSet(Name),
    /// An empty list of parents, or a parent set that holds no counter.
    Empty,
}

impl Parents {
    /// Returns the counters these parents may be, in their order: the one
    /// parent, the list, or the counters of the parent set, looked up in
    /// `sets`.
    ///
    /// # Errors
    ///
    /// [`ParentsError::UnknownSet`] for a parent set that `sets` does not
    /// hold, and [`ParentsError::Empty`] when the result would hold no
    /// counter.
    pub(super) fn resolve(
        &self,
        sets: &std::collections::BTreeMap<Name, Vec<Name>>,
    ) -> Result<Vec<Name>, ParentsError> {
        let resolved = match self {
            Parents::One(name) => vec![name.clone()],
            Parents::Any(names) => names.clone(),
            Parents::Set(name) => {
                sets.get(name).cloned().ok_or_else(|| ParentsError::UnknownSet(name.clone()))?
            }
        };
        if resolved.is_empty() {
            return Err(ParentsError::Empty);
        }
        Ok(resolved)
    }
}

/// Returns every chain of counters that `counter` may be numbered within,
/// each chain outermost first, by walking the hierarchy with `parents_of`.
///
/// A counter with one parent has one chain: the parent's chain followed by the
/// parent. A counter with several possible parents ([`Within::parents`],
/// [`Within::parent_set`]) has one chain per parent, and a parent with several
/// possible parents of its own multiplies the chains; which chain applies at a
/// reference point is known only when the document is read. A counter
/// numbered within nothing has one chain, the empty one.
///
/// The function serves two views of the hierarchy: the one the declarations
/// make, which the registry build checks, and the one a document's numbering
/// options leave, which the numbering stage counts by. `parents_of` returns
/// the counters a counter may be numbered within in its own view, and an
/// empty list both for a counter numbered within nothing and for a counter
/// the view does not hold; the walk stops in both cases.
///
/// # Errors
///
/// The counters of a cycle, when the hierarchy leads back, through any of the
/// parents, to a counter already on the path: the counter that the walk
/// reached a second time, then each counter walked through after it.
pub(super) fn chains_of(
    counter: &Name,
    parents_of: &impl Fn(&Name) -> Vec<Name>,
) -> Result<Vec<Vec<Name>>, Vec<Name>> {
    let mut path = vec![counter.clone()];
    walk_chains(&mut path, parents_of)
}

/// [`chains_of`] for the last counter of `path`, where `path` holds the
/// counters walked through to reach it.
fn walk_chains(
    path: &mut Vec<Name>,
    parents_of: &impl Fn(&Name) -> Vec<Name>,
) -> Result<Vec<Vec<Name>>, Vec<Name>> {
    let current = path.last().expect("the path holds the counter walked from").clone();
    let parents = parents_of(&current);
    if parents.is_empty() {
        return Ok(vec![Vec::new()]);
    }
    let mut chains = Vec::new();
    for parent in parents {
        if let Some(start) = path.iter().position(|seen| *seen == parent) {
            return Err(path[start..].to_vec());
        }
        path.push(parent.clone());
        let above = walk_chains(path, parents_of);
        path.pop();
        for mut chain in above? {
            chain.push(parent.clone());
            chains.push(chain);
        }
    }
    Ok(chains)
}

/// Writes `text` into `out` as its length followed by its bytes, so that two
/// different lists of texts never write the same bytes.
pub(super) fn digest_text(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u64).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// The counters of a declared hierarchy, as a map from a counter's name
    /// to the counters it may be numbered within.
    fn hierarchy(entries: &[(&str, &[&str])]) -> BTreeMap<Name, Vec<Name>> {
        entries
            .iter()
            .map(|(name, parents)| {
                (Name::new(*name), parents.iter().map(|parent| Name::new(*parent)).collect())
            })
            .collect()
    }

    /// [`chains_of`] over such a map, each chain written with dots.
    fn chains(counters: &BTreeMap<Name, Vec<Name>>, counter: &str) -> Result<Vec<String>, Vec<Name>> {
        let walked =
            chains_of(&Name::new(counter), &|name| counters.get(name).cloned().unwrap_or_default())?;
        Ok(walked
            .iter()
            .map(|chain| chain.iter().map(Name::as_str).collect::<Vec<_>>().join("."))
            .collect())
    }

    #[test]
    fn a_chain_runs_outermost_first() {
        let counters = hierarchy(&[
            ("section", &[]),
            ("subsection", &["section"]),
            ("equation", &["subsection"]),
        ]);
        assert_eq!(chains(&counters, "equation"), Ok(vec!["section.subsection".to_string()]));
        assert_eq!(chains(&counters, "section"), Ok(vec![String::new()]));
    }

    #[test]
    fn a_counter_with_several_parents_has_one_chain_per_parent() {
        let counters = hierarchy(&[
            ("chapter", &[]),
            ("section", &["chapter"]),
            ("equation", &["chapter", "section"]),
        ]);
        let walked = chains(&counters, "equation").expect("acyclic");
        assert_eq!(walked, ["chapter", "chapter.section"]);
    }

    #[test]
    fn a_hierarchy_that_leads_back_to_itself_is_a_cycle() {
        let counters = hierarchy(&[("a", &["b"]), ("b", &["c"]), ("c", &["a"])]);
        let cycle = chains(&counters, "a").expect_err("a cycle");
        let names: Vec<&str> = cycle.iter().map(Name::as_str).collect();
        assert_eq!(names, ["a", "b", "c"]);
    }

    #[test]
    fn a_cycle_through_any_one_parent_is_a_cycle() {
        let counters = hierarchy(&[("a", &["x", "b"]), ("b", &["a"]), ("x", &[])]);
        let cycle = chains(&counters, "a").expect_err("a cycle");
        let names: Vec<&str> = cycle.iter().map(Name::as_str).collect();
        assert_eq!(names, ["a", "b"]);
    }

    #[test]
    fn a_walk_stops_at_a_counter_the_view_does_not_know() {
        let counters = hierarchy(&[("equation", &["section"])]);
        assert_eq!(chains(&counters, "equation"), Ok(vec!["section".to_string()]));
    }

    #[test]
    fn a_counter_is_declared_within_nothing() {
        let decl = CounterDecl::new("equation", "flm.math.style.equation");
        assert!(decl.within.is_none());
        assert_eq!(decl.name.as_str(), "equation");
    }

    #[test]
    fn the_builders_set_what_they_name() {
        let decl = CounterDecl::new("subsection", "flm.heading.style.subsection")
            .docs("Subsections.")
            .within(Within::parent("section"));
        assert_eq!(decl.docs, if cfg!(feature = "docs") { "Subsections." } else { "" });
        let within = decl.within.expect("declared");
        assert_eq!(within.parents, Parents::One(Name::new("section")));
        assert_eq!(within.joiner, WithinJoinerSpec::Joined(".".to_string()));
        assert!(!within.no_restart);

        let any = Within::parents(["chapter", "section"]).no_restart();
        assert_eq!(any.parents, Parents::Any(vec![Name::new("chapter"), Name::new("section")]));
        assert!(any.no_restart);
        let set = Within::parent_set("flm.headings.levels").displayed_bare();
        assert_eq!(set.parents, Parents::Set(Name::new("flm.headings.levels")));
        assert_eq!(set.joiner, WithinJoinerSpec::Bare);
    }

    #[test]
    fn a_counter_has_no_nesting_joiner_until_it_declares_one() {
        let decl = CounterDecl::new("t.list", "t.style.list");
        assert!(decl.nesting.is_none());
        let nested = decl.nesting(WithinJoinerSpec::Template("#{_parent}.".to_string()));
        assert_eq!(nested.nesting, Some(WithinJoinerSpec::Template("#{_parent}.".to_string())));
    }

    #[test]
    fn every_form_of_a_joiner_digests_differently() {
        let digest = |spec: &WithinJoinerSpec| {
            let mut out = Vec::new();
            spec.digest_into(&mut out);
            out
        };
        let forms = [
            WithinJoinerSpec::Joined(".".to_string()),
            WithinJoinerSpec::Template(".".to_string()),
            WithinJoinerSpec::Bare,
            WithinJoinerSpec::Named(Name::new(".")),
        ];
        for (index, one) in forms.iter().enumerate() {
            for other in &forms[index + 1..] {
                assert_ne!(digest(one), digest(other), "{one:?} and {other:?}");
            }
        }
    }

    #[test]
    fn every_field_of_a_within_changes_its_digest() {
        let digest = |within: &Within| {
            let mut out = Vec::new();
            within.digest_into(&mut out);
            out
        };
        let base = Within::parent("section");
        let variants = [
            Within::parent("chapter"),
            Within::parents(["section"]),
            Within::parent_set("section"),
            Within::parent("section").joined_by("-"),
            Within::parent("section").template("#{section}."),
            Within::parent("section").displayed_bare(),
            Within::parent("section").joiner_named("section"),
            Within::parent("section").no_restart(),
        ];
        for variant in &variants {
            assert_ne!(digest(&base), digest(variant), "{variant:?}");
        }
        assert_eq!(digest(&base), digest(&Within::parent("section")));
    }
}
