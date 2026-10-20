//! Lowering rules: what a lowering rule is, what it declares, and the
//! interface through which it builds the presentation of one element for one
//! backend.
//!
//! Lowering rewrites each frozen tree into a presentation tree for one
//! backend, by applying the lowering rule selected for each element, in
//! rounds, until only elements the backend writes natively remain; the result
//! is computed data in the table `flm.lowered.<backend>` ([backends, lowering,
//! and outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//! A lowering rule is a named registry entity written for one element: it
//! declares what it requires, reads and produces, and implements [`Lower`],
//! which stages the element's presentation for one backend ([`LoweringRule`]).
//! To *stage* a node is to build it into the tree that the rule is producing
//! ([`Lowerer`] defines the term).
//!
//! This module defines what a rule is and what it may do. A feature registers
//! a rule with
//! [`FeatureBuilder::lowering_rule`](crate::core::registry::FeatureBuilder::lowering_rule).
//! The lowering stage that calls the rules, the lowering driver that
//! implements [`Lowerer`], and the presentation elements the rules build
//! are in [`crate::stdcore::backends::lowering`] and
//! [`crate::stdcore::backends::pres`].
//!
//! The items of this module:
//!
//! - [`Lower`], the trait a rule author implements, and [`Lowering`], what it
//!   returns for one node;
//! - [`LoweringDecl`], what a rule declares; [`DefaultMark`], on which
//!   backends the rule is chosen by default; and [`LoweringRule`], the
//!   registry entity that carries both together with the implementation;
//! - [`Lowerer`], the interface a rule works through, and [`LowerCx`], the
//!   tree under construction into which the rule stages nodes;
//! - [`NodeRegion`] and [`RegionShape`], with which a rule lowers an argument
//!   or a content slot of a node of another tree ([`Lowerer::lower_region`]);
//! - [`LowerError`], the failure of a rule at one node;
//! - [`chars_at_lowering`] and [`spelling_at_lowering`], the two ways a rule
//!   reads content that may hold a placeholder of a user definition's
//!   template: as characters, or as written.
//!
//! # How a rule is called
//!
//! A rule is written for one element. When a backend's lowering stage reaches
//! a node of that element, and the plan selected the rule for that element on
//! that backend, the stage calls [`Lower::lower`] with the frozen node. The
//! rule returns either [`Lowering::Default`] (copy this node and lower its
//! children) or [`Lowering::Emit`] with the nodes it built to replace the
//! node. The rule builds nodes through two values it is handed: the
//! [`Lowerer`], which lowers parts of the tree and builds presentation nodes,
//! and the [`LowerCx`], the tree under construction. Every [`Lowerer`] method
//! that builds something takes the [`LowerCx`] as an argument, because the
//! [`Lowerer`] cannot hold it: its own operations lower parts of the node, which
//! needs the same context, so the two are borrowed separately.
//!
//! # What a rule declares
//!
//! Plan construction reads a rule's [`LoweringDecl`] before any rule runs, and
//! uses each part as follows:
//!
//! - `requires`, `reads`, `reads_opt` and `options` are added to the needs of
//!   its backend's lowering stage; a rule that reads the table of resolved
//!   references therefore causes the stage that writes that table to be
//!   scheduled;
//! - `produces` lists every presentation element the rule may build. Plan
//!   construction follows these lists to check that the backend can, in the
//!   end, write every element a document may contain, and the [`Lowerer`]
//!   refuses to build an element the rule did not list;
//! - `selection_requires` lists the requirements (identifiers of definitions
//!   that the output needs, [`StyleDef`](crate::core::registry::StyleDef))
//!   that the rule has merely because it was selected, whether or not it ever
//!   runs.
//!
//! # Which rule is chosen
//!
//! Several rules may target one element: the rule of the feature that
//! declares the element, and a more specific one written for a particular backend. The plan
//! chooses one rule per backend and element, in a fixed order: a rule the
//! plan request prefers by name; otherwise no rule, if the backend writes the
//! element natively; otherwise the rule marked [`DefaultMark::OnBackends`] for
//! that backend; then the rule marked [`DefaultMark::Everywhere`]; then the
//! only candidate, if there is exactly one. Several candidates and nothing to
//! choose between them is a plan error that lists the candidates. See
//! [`SelectedRules`](crate::core::plan::SelectedRules) for the rule selection
//! order in full.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef, NodeSlice, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::transform::RestageContext;

use crate::model::addr::{NodeAddr, TreeId};
use crate::model::doc::FragmentLayout;
use crate::model::names::{BackendName, CapName, ElementName, Name, RuleName, StageName};
use crate::model::options::{DataValue, OptionError, OptionKey, OptionRef, Options};
use serde_json::Value;
use crate::core::data::store::AccessError;
use crate::core::data::table::{TableKey, TableRef};
use crate::lang::text::{NotCharacters, read_chars};
use crate::lang::{BuiltLang, Flm, FlmCallableType, FlmDiagnostic, FlmSpan, PresPayload};

/// The presentation tree under construction, into which a lowering rule
/// stages nodes: techy's restaging context over the FLM language.
///
/// Every [`Lowerer`] method that builds something takes this context, and the
/// [`BuildId`] values it returns are positions in it. Both type parameters
/// after the language are the unit type, because FLM's trees carry no
/// per-node annotation in the frozen tree or in the presentation tree.
///
/// A rule receives this context from the lowering stage and never creates
/// one.
pub type LowerCx<'a> = RestageContext<'a, Flm, (), ()>;

/// What a lowering rule returns for one node: copy it, or replace it.
///
/// [`Lower::lower`] returns it.
#[derive(Debug)]
pub enum Lowering {
    /// Copy this node into the presentation tree unchanged, and lower its
    /// children.
    ///
    /// Return it for an element the backend writes natively. For an element
    /// the backend does not write natively, the copied node is lowered again
    /// in the next round by the same rule, until the tree exceeds the number
    /// of rounds allowed and fails with `flm.lowering.rounds-exceeded`.
    Default,
    /// The nodes the rule built to replace this node, in their order among
    /// its siblings. An empty list removes the node.
    Emit(Vec<BuildId>),
}

/// What a lowering rule declares about itself: what it needs, what it may
/// build, and what it requires merely by being selected.
///
/// Plan construction reads the declaration instead of calling the rule, so a
/// rule takes part in ordering and scheduling before it has run; see the
/// [module documentation](self#what-a-rule-declares) for what each part is
/// used for.
///
/// Build one with [`LoweringDecl::new`] and the methods that take and return
/// it, and pass it to [`LoweringRule::new`].
///
/// # Examples
///
/// ```
/// use flm_core::core::lowering::LoweringDecl;
///
/// let decl = LoweringDecl::new()
///     .requires("flm.refs.resolved")
///     .produces("flm.pres.link")
///     .produces("flm.text");
///
/// assert_eq!(decl.requires.len(), 1);
/// assert_eq!(decl.produces.len(), 2);
/// ```
#[derive(Debug, Clone, Default)]
pub struct LoweringDecl {
    /// The capabilities the rule requires, which its backend's lowering stage
    /// then also requires.
    pub requires: Vec<CapName>,
    /// The tables the rule reads. Reading a table also requires that table's
    /// capability, so the plan schedules a stage that writes it.
    pub reads: Vec<TableRef>,
    /// The tables the rule reads if the run holds them. With such a read, the
    /// rule states that it can do its work without the feature that owns the
    /// table.
    ///
    /// The backend's lowering stage turns such a read into a *required* read
    /// whenever the registry declares the table and some stage writes it, so
    /// that the stage writing it is scheduled and the rule finds the values.
    /// For a table that no stage writes (an [external
    /// table](crate::core::run::Externals), which the caller installs when a
    /// run is constructed), it stays an optional read. For a table that no
    /// feature of the registry declares (a [peer
    /// declaration](crate::core::registry::PeerName) that resolved to
    /// nothing), it is left out of the stage's needs. In both of these cases
    /// the rule reads the table when the run holds it, and reads `None`
    /// otherwise.
    pub reads_opt: Vec<TableRef>,
    /// The option types the rule reads. A read of an option type the rule did
    /// not declare here returns [`LowerError::Option`].
    pub options: Vec<OptionRef>,
    /// Every presentation element the rule may build. Building a node of an
    /// element that is not listed here returns
    /// [`LowerError::UndeclaredProduces`]; the only exception is a text node
    /// ([`Lowerer::text`]) on a backend that writes text natively.
    pub produces: Vec<ElementName>,
    /// The requirements the rule records whenever it is selected, whether or
    /// not it ever runs at a node: identifiers of definitions that the output
    /// needs, such as a style definition on HTML
    /// ([`StyleDef`](crate::core::registry::StyleDef)). For a requirement
    /// recorded because of one particular node, see [`Lowerer::require`].
    pub selection_requires: Vec<Name>,
}

impl LoweringDecl {
    /// Returns a declaration that needs nothing, produces nothing and records
    /// no requirement.
    pub fn new() -> LoweringDecl {
        LoweringDecl::default()
    }

    /// Adds a required capability, and returns the declaration.
    pub fn requires(mut self, capability: impl Into<CapName>) -> LoweringDecl {
        self.requires.push(capability.into());
        self
    }

    /// Adds a table the rule reads, which also requires that table's
    /// capability, and returns the declaration.
    pub fn reads(mut self, table: impl TableKey) -> LoweringDecl {
        self.reads.push(table.table_ref());
        self
    }

    /// Adds a table the rule reads if the run holds it, and returns the
    /// declaration. The rule then works without the feature that owns the
    /// table, and the stage writing the table is still scheduled wherever the
    /// registry has one, as [`reads_opt`](LoweringDecl::reads_opt) describes.
    pub fn reads_opt(mut self, table: impl TableKey) -> LoweringDecl {
        self.reads_opt.push(table.table_ref());
        self
    }

    /// Adds the option type `T` to the option types the rule reads, and
    /// returns the declaration.
    pub fn option<T: Options>(mut self) -> LoweringDecl {
        self.options.push(OptionRef::of::<T>());
        self
    }

    /// Adds the option type declared from data under the name `name` to the
    /// option types the rule reads, and returns the declaration. The rule reads
    /// it with `option_named`, defined on `dyn Lowerer`.
    pub fn option_named(mut self, name: impl Into<Name>) -> LoweringDecl {
        self.options.push(OptionRef::named(name));
        self
    }

    /// Adds a presentation element the rule may build, and returns the
    /// declaration.
    pub fn produces(mut self, element: impl Into<ElementName>) -> LoweringDecl {
        self.produces.push(element.into());
        self
    }

    /// Adds a requirement the rule records whenever it is selected (see
    /// [`selection_requires`](LoweringDecl::selection_requires)), and returns
    /// the declaration.
    pub fn selection_requires(mut self, id: impl Into<Name>) -> LoweringDecl {
        self.selection_requires.push(id.into());
        self
    }
}

/// The default mark of a lowering rule: on which backends the rule is chosen
/// for its element when the plan request does not prefer a rule by name.
///
/// The plan reads the mark after the plan request's preferences and before
/// the case of a single candidate; see the [module
/// documentation](self#which-rule-is-chosen) for the whole order. Usually the
/// rule of the feature that declares the element is marked
/// [`Everywhere`](DefaultMark::Everywhere), a rule written for one backend is
/// marked [`OnBackends`](DefaultMark::OnBackends) with that backend, and an
/// alternative that a caller must ask for by name is marked
/// [`Never`](DefaultMark::Never).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultMark {
    /// The element's default rule on every backend.
    Everywhere,
    /// The default on the listed backends only, ranked above a rule marked
    /// [`Everywhere`](DefaultMark::Everywhere), and **never a candidate on any
    /// other backend**: not as the only rule for its element, and not when a
    /// plan request prefers it by name. The mark states where the rule
    /// belongs, because a rule written for one backend stages that backend's
    /// own elements.
    OnBackends(Vec<BackendName>),
    /// Never a default: chosen only when the plan request prefers it by name,
    /// or when it is the element's only rule. The mark restricts precedence
    /// and not place, so such a rule is a candidate on every backend.
    Never,
}

impl DefaultMark {
    /// Returns whether the mark is [`Everywhere`](DefaultMark::Everywhere):
    /// the rule is the default on every backend.
    pub fn is_general(&self) -> bool {
        matches!(self, DefaultMark::Everywhere)
    }

    /// Returns whether the mark is [`OnBackends`](DefaultMark::OnBackends)
    /// and lists `backend`.
    pub fn is_specific_to(&self, backend: &BackendName) -> bool {
        match self {
            DefaultMark::OnBackends(backends) => backends.contains(backend),
            _ => false,
        }
    }

    /// Returns whether a rule with this mark may be chosen on `backend` at
    /// all.
    ///
    /// [`OnBackends`](DefaultMark::OnBackends) states *where* the rule
    /// belongs, not merely where it ranks first: a rule marked for listed
    /// backends is never a candidate on another backend, not even as the
    /// element's only rule and not even when a plan request prefers it by
    /// name. A rule written for one backend produces that backend's own
    /// elements, which are wrong on any other backend.
    /// [`Never`](DefaultMark::Never) restricts precedence and not place, so
    /// such a rule may be chosen on every backend as the only candidate.
    pub fn takes_part_on(&self, backend: &BackendName) -> bool {
        match self {
            DefaultMark::OnBackends(backends) => backends.contains(backend),
            DefaultMark::Everywhere | DefaultMark::Never => true,
        }
    }
}

/// A lowering rule: a named registry entity written for one element, which
/// declares what it requires, reads and produces, and implements [`Lower`],
/// which stages the element's presentation for one backend.
///
/// The value holds the rule's name, the element it lowers, its declaration,
/// its implementation and its default mark. A feature registers it with
/// [`FeatureBuilder::lowering_rule`](crate::core::registry::FeatureBuilder::lowering_rule).
/// Rule names are dotted, and no two rules of a registry have the same name;
/// a plan request that prefers a rule refers to it by this name. The standard
/// features name a rule `<element>.lower.<variant>`, such as
/// `flm.cells.cell.lower.default` or `flm.parbreak.lower.latex-recomposed`.
#[derive(Clone)]
pub struct LoweringRule {
    /// The rule's name, which no other rule of the registry has.
    pub name: RuleName,
    /// The element whose nodes this rule lowers.
    pub element: ElementName,
    /// What the rule declares that it needs and may build.
    pub decl: LoweringDecl,
    /// The implementation that the lowering stage calls.
    pub rule: Arc<dyn Lower>,
    /// On which backends the rule is chosen when no rule is preferred by
    /// name.
    pub default: DefaultMark,
    /// A one-line description of the rule for a person reading the registry.
    pub docs: String,
}

impl LoweringRule {
    /// Returns the rule `name`, which lowers `element` through `rule`, with
    /// the declaration `decl`, the default mark `default` and no description.
    pub fn new(
        name: impl Into<RuleName>,
        element: impl Into<ElementName>,
        decl: LoweringDecl,
        rule: Arc<dyn Lower>,
        default: DefaultMark,
    ) -> LoweringRule {
        LoweringRule {
            name: name.into(),
            element: element.into(),
            decl,
            rule,
            default,
            docs: String::new(),
        }
    }

    /// Sets the rule's one-line description, and returns the rule.
    pub fn docs(mut self, docs: impl Into<String>) -> LoweringRule {
        self.docs = crate::kept_docs(docs);
        self
    }
}

impl fmt::Debug for LoweringRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoweringRule")
            .field("name", &self.name)
            .field("element", &self.element)
            .field("decl", &self.decl)
            .field("default", &self.default)
            .finish()
    }
}

/// The failure of a lowering rule at one node.
///
/// A rule returns this error from [`Lower::lower`], and every [`Lowerer`]
/// method that can refuse something returns it too. The lowering stage
/// handles it according to the variant
/// ([`crate::stdcore::backends::lowering::error`] lists the conditions):
///
/// - Most variants fail the node only. The stage reports one Error diagnostic
///   at the node, `flm.lowering.rule-failed`, whose message includes this
///   error's text; [`RegionNotFound`](LowerError::RegionNotFound) is reported
///   as `flm.lowering.region-not-found` instead, and
///   [`Refused`](LowerError::Refused) reports the rule's own diagnostic. The
///   node then lowers to a childless `flm.pres.unsupported` stand-in, as an
///   element the backend cannot render does, and the rest of the tree lowers
///   normally.
/// - [`Build`](LowerError::Build) fails the lowering of the whole tree, with
///   one `flm.lowering.failed` diagnostic; the tree then has no lowered form.
/// - [`PlaceholderCharacters`](LowerError::PlaceholderCharacters) is not a
///   failure of the node and is not reported: it is passed on to the rule that
///   spliced the template it arose in.
///
/// A rule must return every error the [`Lowerer`] gives it, rather than
/// continue: after a refused call the [`Lowerer`] has staged nothing, and a
/// rule that continues as though it had breaks an invariant of the lowering.
#[derive(Debug, Clone)]
pub enum LowerError {
    /// The rule read a table it did not declare, or read a table as the
    /// wrong value type.
    ///
    /// The error is the store's own ([`AccessError`]), shared with stages; its
    /// `stage` field holds the rule's name.
    Access(AccessError),
    /// The rule read an option type it did not declare. The error's `stage`
    /// field holds the rule's name.
    Option(OptionError),
    /// The rule tried to build a node of an element that its declaration's
    /// [`produces`](LoweringDecl::produces) does not list.
    UndeclaredProduces {
        /// The rule that tried.
        rule: RuleName,
        /// The element of the node it tried to build.
        element: ElementName,
        /// The elements the rule's declaration lists.
        declared: Vec<ElementName>,
    },
    /// The node, or the nodes a rule passed to a [`Lowerer`] method, do not
    /// have the structure the rule or the method requires: for example, a
    /// node without the payload its element always carries, an argument that
    /// is absent, or nodes of another tree.
    Structure {
        /// What was found, as a phrase that completes the error's message.
        detail: String,
    },
    /// Building a node failed inside the parser toolkit (techy). The rule
    /// cannot recover from it, and the lowering of the whole tree fails.
    Build {
        /// The toolkit's own text for the failure.
        detail: String,
    },
    /// A synthesized value could not be brought up to date ([value
    /// catch-up](crate::guide::concepts_overview#value-catch-up)), so it could
    /// not be spliced; see [`Lowerer::splice`].
    CatchUp {
        /// The text of the failure.
        detail: String,
    },
    /// Bringing a synthesized value up to date was refused because a stage,
    /// run again over it, stated a fact about the document as a whole, such as
    /// a label, which a synthesized value must not do; see [`Lowerer::splice`]
    /// and [value catch-up](crate::guide::concepts_overview#value-catch-up).
    ///
    /// It is a variant of its own, rather than a
    /// [`CatchUp`](LowerError::CatchUp) failure, so that a rule splicing a
    /// value it built from what an author wrote can tell the author what to
    /// change. The user-definitions feature does so for a kept definition
    /// whose template declares a reference point
    /// ([`KeptTemplateDeclaresPoint`](crate::stdcore::features::userdefs::kept::KeptTemplateDeclaresPoint)).
    /// A rule that returns this error unchanged gets
    /// `flm.lowering.rule-failed`, as for any other failure.
    DocumentFactInValue {
        /// The stage that stated the fact.
        stage: StageName,
        /// The table in which the fact was stated.
        table: CapName,
    },
    /// The rule refuses the node for the reason that the carried diagnostic
    /// states, in terms the author can act on.
    ///
    /// The lowering stage reports that diagnostic instead of
    /// `flm.lowering.rule-failed`, and lowers the node to the same stand-in as
    /// for any other failure. It does so at every node the rule refuses: a
    /// refusal about something every use shares, such as a user definition,
    /// is reported at each use.
    Refused(Box<FlmDiagnostic>),
    /// The node region that [`Lowerer::lower_region`] was asked to lower does
    /// not exist: no tree of that identifier is reachable, the tree holds no
    /// node at that address, or the node has no argument or content slot at
    /// that position.
    ///
    /// The lowering stage reports it at the node whose rule asked, under a
    /// condition of its own (`flm.lowering.region-not-found`) rather than as a
    /// rule failure, and that node lowers to the same stand-in as for a rule
    /// failure.
    RegionNotFound {
        /// The text saying what could not be found.
        detail: String,
    },
    /// The rule needs the **characters** of some content (an address, a
    /// reference key, verbatim content, or anything computed from them), and
    /// the content holds a placeholder of a user definition's template, whose
    /// characters are not known yet. A rule obtains characters through
    /// [`chars_at_lowering`], which returns this error for it; a rule that
    /// computes on the content as written rather than on its characters gets
    /// it from [`spelling_at_lowering`].
    ///
    /// Only a template lowered unexpanded, placeholders included, holds a
    /// placeholder. The recomposed LaTeX lowers the template of a kept
    /// definition this way, once, to write it as a LaTeX definition in which
    /// each placeholder is a parameter that LaTeX replaces at every call. A
    /// parameter may be placed where the template typesets content, but not
    /// where a rule needs characters, since those are known only at a call. So
    /// the rule returns this error, and the lowering passes it on, unreported,
    /// from rule to rule up to the rule that spliced the template, which acts
    /// on it. The user-definitions feature then expands every use site of that
    /// definition instead of writing a call. The lists feature, whose
    /// recomposed LaTeX lowers a list's own label template with its number
    /// formats written as placeholders, reports a number format placed where
    /// characters are read. Wherever a template was expanded (on every other
    /// output, and at every expanded use site), no placeholder of a user
    /// definition remains, and this error does not occur.
    ///
    /// An error of this variant that reaches the lowering of a whole tree
    /// fails that tree.
    PlaceholderCharacters,
    /// Any other failure the rule reports.
    Other {
        /// The text of the failure.
        detail: String,
    },
}

impl LowerError {
    /// Returns a [`Structure`](LowerError::Structure) error: the nodes do not
    /// have the structure required; `detail` says what was found.
    pub fn structure(detail: impl Into<String>) -> LowerError {
        LowerError::Structure { detail: detail.into() }
    }

    /// Returns an [`Other`](LowerError::Other) error described by `detail`.
    pub fn other(detail: impl Into<String>) -> LowerError {
        LowerError::Other { detail: detail.into() }
    }
}

impl From<AccessError> for LowerError {
    fn from(error: AccessError) -> LowerError {
        LowerError::Access(error)
    }
}

impl From<OptionError> for LowerError {
    fn from(error: OptionError) -> LowerError {
        LowerError::Option(error)
    }
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LowerError::Access(error) => fmt::Display::fmt(error, f),
            LowerError::Option(error) => fmt::Display::fmt(error, f),
            LowerError::UndeclaredProduces { rule, element, declared } => write!(
                f,
                "rule `{rule}` builds a node of `{element}`, so it must list `{element}` among \
                 the elements it produces; it lists: [{}]",
                declared.iter().map(|e| e.as_str()).collect::<Vec<_>>().join(", ")
            ),
            LowerError::Structure { detail } => {
                write!(
                    f,
                    "the content does not have the structure that lowering it requires: {detail}"
                )
            }
            LowerError::Build { detail } => write!(f, "a node could not be built: {detail}"),
            LowerError::CatchUp { detail } => {
                write!(f, "content generated during processing could not be prepared: {detail}")
            }
            LowerError::DocumentFactInValue { stage, table } => write!(
                f,
                "content generated during processing must not state a fact about the whole \
                 document, such as a label, but stage `{stage}` stated one in `{table}` when it \
                 ran over that content"
            ),
            LowerError::Refused(diagnostic) => f.write_str(&diagnostic.message()),
            LowerError::RegionNotFound { detail } => {
                write!(f, "the argument or content slot to lower does not exist: {detail}")
            }
            LowerError::PlaceholderCharacters => f.write_str(
                "a placeholder of a definition's template must not appear where the characters \
                 of the content are read (such as an address or a reference key): its \
                 characters are known only once the template is expanded",
            ),
            LowerError::Other { detail } => f.write_str(detail),
        }
    }
}

impl std::error::Error for LowerError {}

/// Returns the characters of the run of nodes `nodes`, for a lowering rule
/// that needs characters (an address, a reference key, verbatim content, a
/// word) while lowering.
///
/// This is the only way a rule reads characters at lowering, and it relies on
/// the only reader of content as characters ([`read_chars`]). The result is
/// one of three:
///
/// - `Ok(Ok(text))`, the characters, when the content holds nothing but text,
///   comments and groups other than mathematics;
/// - `Ok(Err(NotCharacters))`, when the content holds a macro, an
///   environment, a special character sequence or mathematics, which has no
///   characters to give; the rule reports this in terms of its own and
///   recovers;
/// - `Err(`[`LowerError::PlaceholderCharacters`]`)`, when the content is
///   characters and placeholders of a user definition's template, whose
///   characters are not known until the template is expanded. The rule
///   returns the error unchanged (`?`), and the rule that spliced the template
///   acts on it (see [`LowerError::PlaceholderCharacters`]). Only a template
///   lowered unexpanded holds a placeholder (the recomposed LaTeX of a kept
///   definition, or of a list's own label template), so on every other path
///   this result does not occur.
///
/// Content that holds both a placeholder and something that is not characters
/// gives `Ok(Err(NotCharacters))`, since no argument of a call could make it
/// characters.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`] as above.
pub fn chars_at_lowering<'t>(
    nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>,
) -> Result<Result<String, NotCharacters>, LowerError> {
    match read_chars(nodes) {
        Ok(read) => read.text().map(Ok).ok_or(LowerError::PlaceholderCharacters),
        Err(not) => Ok(Err(not)),
    }
}

/// Checks whether a lowering rule may compute on the **spelling** of the run
/// of nodes `nodes` while lowering, that is, on the content as written, markup
/// included, rather than on its characters ([`chars_at_lowering`]). Returns
/// `Ok(())` when no placeholder of a user definition's template occurs in the
/// content, at any depth, and [`LowerError::PlaceholderCharacters`] when one
/// does.
///
/// A rule that derives something from content as written cannot use
/// [`chars_at_lowering`], which returns "not characters" for content holding a
/// macro and so would not report the placeholder. Two examples: a defined
/// term's label derived from the term (`\term[\emph{#1} chain]{it}`, whose
/// target is computed from `\emph{…} chain`), and a list's number template
/// read from the value of `label=`. The spelling of a placeholder is not the
/// argument a call supplies, so what the rule would compute from it is known
/// only at a call, wherever in the content the placeholder occurs: inside a
/// group, in a macro's argument, anywhere. The rule returns the error
/// unchanged (`?`) before it computes, and the rule that spliced the template
/// acts on it as for [`chars_at_lowering`]. On every path where the template
/// was expanded, no placeholder remains and the result is `Ok(())`.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`] as above.
pub fn spelling_at_lowering<'t>(
    nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>,
) -> Result<(), LowerError> {
    let placeholder = |node: NodeRef<'t, Flm>| node.callable_type() == Some(FlmCallableType::Placeholder);
    let holds = nodes
        .into_iter()
        .any(|node| placeholder(node) || node.descendants().any(placeholder));
    if holds { Err(LowerError::PlaceholderCharacters) } else { Ok(()) }
}

/// A node region: one argument or one content slot of a node, given by
/// position, which a rule passes, with the node's address, to
/// [`Lowerer::lower_region`] to have that region's content lowered.
///
/// A node's children are divided into node regions: one per argument the
/// node's element declares, and one per content slot (a node slot in techy's
/// sense, `ParsedSlot`: a part of a node's content outside its arguments; the
/// body is one). Positions count from zero in the order in
/// which the element declares them. An argument's position counts the
/// optional arguments an author left out, so a position designates the same
/// argument in every node of one element. [`NodeRegion::argument_named`] finds
/// an argument's position from its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeRegion {
    /// The argument at this position.
    Argument(usize),
    /// The content slot at this position.
    Slot(usize),
}

impl NodeRegion {
    /// Returns the node region of `node`'s argument named `name`, or `None`
    /// when `node`'s element declares no argument of that name.
    pub fn argument_named(node: &NodeRef<'_, Flm>, name: &str) -> Option<NodeRegion> {
        node.arguments()?
            .iter()
            .position(|argument| argument.name() == Some(name))
            .map(NodeRegion::Argument)
    }

    /// Returns the content nodes of this node region of `node`, in the order
    /// in which they are written: the argument's or the slot's content
    /// without its delimiters, comments and whitespace included. An argument
    /// that is absent ([`NodeRegion::is_absent`]) has no content, and the
    /// result is empty.
    ///
    /// Returns `None` when `node` has no argument or no slot at this position.
    pub fn content_nodes<'t>(&self, node: &NodeRef<'t, Flm>) -> Option<Vec<NodeRef<'t, Flm>>> {
        match *self {
            NodeRegion::Argument(index) => {
                if index >= node.arguments()?.len() {
                    return None;
                }
                let content = node.argument_content_nodes(index);
                Some(content.map(|nodes| nodes.iter().collect()).unwrap_or_default())
            }
            NodeRegion::Slot(index) => Some(node.slot_content_nodes(index)?.iter().collect()),
        }
    }

    /// Returns whether this node region is an argument of `node` that is
    /// absent from the source: an optional argument the author left out, or a
    /// mandatory one that the parser left out after reporting it. An argument
    /// written with nothing in it is not absent. A content slot is never absent,
    /// and neither is a node region that `node` does not have.
    pub fn is_absent(&self, node: &NodeRef<'_, Flm>) -> bool {
        match *self {
            NodeRegion::Argument(index) => {
                node.arguments().is_some_and(|arguments| index < arguments.len())
                    && node.argument_content_nodes(index).is_none()
            }
            NodeRegion::Slot(_) => false,
        }
    }
}

impl fmt::Display for NodeRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeRegion::Argument(index) => write!(f, "argument {index}"),
            NodeRegion::Slot(index) => write!(f, "slot {index}"),
        }
    }
}

/// The function that [`Lowerer::lower_region`] calls to lower a node region's
/// content, in the form the caller chooses.
///
/// It receives the [`LowerCx`], the [`Lowerer`] (both now working in the
/// tree that holds the region), the node the region belongs to, and the
/// region's content nodes ([`NodeRegion::content_nodes`]), and it returns the
/// nodes it staged. Most callers lower the content nodes unchanged, as one
/// run of siblings:
/// [`lower_region_content`](crate::stdcore::backends::lowering::lower_region_content)
/// does that. A caller that places nodes of its own among the content (for
/// example, a link written at the end of the content's last paragraph) writes
/// a function of its own, which passes the content nodes to the
/// [`Lowerer`].
pub type RegionShape<'f> = dyn FnMut(
        &mut LowerCx<'_>,
        &mut dyn Lowerer,
        &NodeRef<'_, Flm>,
        &[NodeRef<'_, Flm>],
    ) -> Result<Vec<BuildId>, LowerError>
    + 'f;

/// The implementation of a lowering rule: what to do at a node of the rule's
/// element.
///
/// Implement this trait and register the implementation as part of a
/// [`LoweringRule`]. The implementation is registered once and may run over
/// many nodes of many documents in parallel, so it holds only its own
/// configuration and does not modify its own state when it runs. Everything it reads reaches it through
/// the [`Lowerer`] it is handed, so every read can be checked against the
/// rule's declaration.
///
/// See [`crate::stdcore::backends::lowering::rules`] for the default rules of
/// the structural elements, which implement this trait.
pub trait Lower: Send + Sync {
    /// Lowers `node`, a node of this rule's element, and returns what replaces
    /// it.
    ///
    /// Read the tables and option types the rule declared through `lw`, and
    /// build the replacement nodes through `lw` as well, which stages them
    /// into `cx`.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError`] when the rule cannot lower the node: a read it
    /// did not declare, an element it did not declare that it produces, a node
    /// structure it does not handle. See [`LowerError`] for how the lowering
    /// stage handles each variant.
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError>;
}

/// The interface a lowering rule works through.
///
/// To stage a node is to build it into the tree that a lowering rule or a
/// rewrite hook is producing, through the context the rule or hook is handed
/// (the [`LowerCx`] at lowering); the staged nodes exist in that tree only.
///
/// The `Lowerer` is the only interface a lowering rule has to the run: through
/// it the rule lowers parts of its node, stages presentation nodes into the
/// [`LowerCx`], splices a synthesized value and lowers a node region of
/// another node; the lowering driver implements it
/// ([`crate::stdcore::backends::lowering::driver`]). A rule author calls these
/// methods and does not implement this trait.
///
/// Because a rule never reaches the run directly, its reads can be checked:
/// every table and option read made here is checked against the rule's own
/// [`LoweringDecl`], which is narrower than everything its backend's lowering
/// stage declared.
///
/// The trait is object-safe (a rule receives it as `&mut dyn Lowerer`), so
/// the methods that need a type parameter are type-erased here, and their
/// typed forms (`get`, `option`, `pres_with`, `pres_slots_with`, `error`,
/// `warning` and `note`) are defined on `dyn Lowerer`. Call the typed forms;
/// the erased ones exist so that the typed forms can be written.
///
/// Every method that builds something also takes the [`LowerCx`], because the
/// nodes are staged into it and this interface does not own it.
///
/// # Errors
///
/// Each method that can fail returns [`LowerError`], which a rule must return
/// from [`Lower::lower`] instead of continuing.
pub trait Lowerer {
    /// Returns the name of the rule being applied.
    fn rule(&self) -> &RuleName;

    /// Returns the name of the backend for which the tree is lowered.
    fn backend(&self) -> &BackendName;

    /// Returns what starts a source comment under the parsing configuration
    /// the registry was built with, as
    /// [`FlmParsing::comment_start`](crate::lang::FlmParsing::comment_start)
    /// does: `Some("%%")` by default, and `None` when the configuration
    /// recognizes no comment, in which case no tree holds a comment node. A
    /// rule that writes a comment back takes the prefix from here rather than
    /// from the node, so that it writes the same prefix as the backend's
    /// emitter.
    fn comment_start(&self) -> Option<&str>;

    /// Returns the language the registry was built with. A rule compiles
    /// against it a text of the document that is written in a language of its
    /// own rather than as content: for example, the label template an author
    /// writes in a list's optional argument, which is compiled when the list
    /// is lowered, since only the list's rule reads that text as a template.
    fn lang(&self) -> &BuiltLang;

    /// Returns the identifier of the tree being lowered: a document tree, or a
    /// synthesized value while [`Lowerer::splice`] is lowering one.
    fn tree_id(&self) -> TreeId;

    /// Returns the region layout with which the block-structure pass laid out
    /// the top level of the tree being lowered: for a document tree, the
    /// layout of the fragment it was parsed from
    /// ([`Fragment::layout`](crate::model::doc::Fragment::layout)); for a
    /// synthesized value, the layout with which every synthesized value is
    /// brought up to date, [`FragmentLayout::default`]. A rule passes it to
    /// [`stands_in_blocks`](crate::lang::rewrites::blocks::stands_in_blocks)
    /// to learn whether a node is laid out in blocks.
    fn layout(&self) -> FragmentLayout;

    /// Returns the address of the node being lowered, the key of the computed
    /// data about that node.
    fn addr(&self) -> NodeAddr;

    /// Returns the address of another node of the same tree, for example an
    /// ancestor the rule walked up to.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError::Structure`] when `node` belongs to another tree
    /// or has no address in the tree being lowered.
    fn addr_of(&self, node: &NodeRef<'_, Flm>) -> Result<NodeAddr, LowerError>;

    /// Returns the tree of the document that the node being lowered places,
    /// or `None` when it places none, from the run's record of placements
    /// ([`StageCx::placement_of`](crate::core::run::StageCx::placement_of)).
    /// The rule of the element that places a tree reads it to identify the
    /// tree in the node it builds.
    fn placement(&self) -> Option<TreeId>;

    /// Returns a table the rule declared that it reads, type-erased. Call
    /// `get`, defined on `dyn Lowerer`, instead: it returns the table's value
    /// type.
    ///
    /// `Ok(None)` means that the run holds no value for the table, which can
    /// only happen for a table declared as an optional read.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError`] when the rule did not declare this read, or
    /// declared it under another value type.
    fn get_erased(&self, table: &TableRef)
    -> Result<Option<&(dyn Any + Send + Sync)>, AccessError>;

    /// Returns the name of the stage the plan scheduled to write a table the
    /// rule reads, or `None` when no scheduled stage writes it.
    ///
    /// Use it to identify that stage in a message about data the rule
    /// expected and did not find.
    fn producer_of(&self, table: &CapName) -> Option<StageName>;

    /// Returns the value of an option type the rule declared that it reads,
    /// type-erased. Call `option`, defined on `dyn Lowerer`, instead: it
    /// returns the option type.
    ///
    /// # Errors
    ///
    /// Returns [`OptionError`] when the rule did not declare that option type.
    fn option_erased(&self, key: OptionKey) -> Result<&(dyn Any + Send + Sync), OptionError>;

    /// Lowers every child of `node`, and returns the nodes they became.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError`] when lowering a child failed.
    fn lower_children(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: &NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, LowerError>;

    /// Lowers the content of `node`'s body (the slot the element marks as its
    /// body), and returns the nodes it became.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError::Structure`] when the node has no body slot, and
    /// any [`LowerError`] the lowering of its content returned.
    fn lower_body(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: &NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, LowerError>;

    /// Lowers the content of `node`'s argument at position `index`, counting
    /// from zero, and returns the nodes it became.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError::Structure`] when the node has no argument at that
    /// position or the argument is absent ([`NodeRegion::is_absent`]), and
    /// any [`LowerError`] the lowering of its content returned.
    fn lower_arg(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: &NodeRef<'_, Flm>,
        index: usize,
    ) -> Result<Vec<BuildId>, LowerError>;

    /// Lowers the content of `node`'s argument named `name`, and returns the
    /// nodes it became.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError::Structure`] when the node has no argument of that
    /// name or the argument is absent, and any [`LowerError`] the lowering of
    /// its content returned.
    fn lower_arg_named(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: &NodeRef<'_, Flm>,
        name: &str,
    ) -> Result<Vec<BuildId>, LowerError>;

    /// Lowers a run of sibling nodes of the tree this `Lowerer` currently
    /// works in, and returns the nodes they became. That tree is the tree
    /// being lowered, the synthesized value being spliced
    /// ([`Lowerer::splice`]), or the tree that holds the node region being
    /// lowered ([`Lowerer::lower_region`]).
    ///
    /// Nodes of any other tree are refused, since their computed data would be
    /// read at addresses of the wrong tree. Lower a tree the rule builds
    /// itself by splicing it as a synthesized value, and another tree's
    /// content through [`Lowerer::lower_region`].
    ///
    /// # Errors
    ///
    /// Returns [`LowerError::Structure`] when the nodes belong to another
    /// tree, and any [`LowerError`] lowering one of the nodes returned.
    fn lower_nodes(
        &mut self,
        cx: &mut LowerCx<'_>,
        nodes: NodeSlice<'_, Flm>,
    ) -> Result<Vec<BuildId>, LowerError>;

    /// Does what [`Lowering::Default`] asks for, and returns the result: the
    /// node copied, with its children lowered.
    ///
    /// Call it when the copy is one part of what the rule returns; return
    /// [`Lowering::Default`] when the copy is all the rule returns.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError`] when the copy or a child's lowering failed.
    fn default(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: &NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, LowerError>;

    /// Builds a node of the presentation element `element`, carrying `payload`
    /// and holding `body` as its content, and returns its position.
    ///
    /// This is how a rule creates a presentation node: the method attaches the
    /// payload, the span, and everything else a node of that element needs.
    /// Call `pres_with`, defined on `dyn Lowerer`, instead: it takes the
    /// payload by value instead of behind a pointer.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError::UndeclaredProduces`] when the rule's declaration
    /// does not list `element`, and [`LowerError::Build`] when the node could
    /// not be built.
    fn pres(
        &mut self,
        cx: &mut LowerCx<'_>,
        element: &ElementName,
        payload: Arc<dyn PresPayload>,
        body: Vec<BuildId>,
    ) -> Result<BuildId, LowerError>;

    /// Builds a node of the presentation element `element` whose content is
    /// several named content slots rather than one body, and returns its
    /// position.
    ///
    /// Some presentation elements hold their content in parts: a list node
    /// has a `label` and an `item` content slot per list entry. `slots` gives
    /// those parts in order, each as a name and the nodes in it; a slot named
    /// `body` becomes the node's body slot. Everything else works as in
    /// [`Lowerer::pres`], including the refusal of an undeclared element. Call
    /// `pres_slots_with`, defined on `dyn Lowerer`, instead: it takes the
    /// payload by value.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError::UndeclaredProduces`] when the rule's declaration
    /// does not list `element`, and [`LowerError::Build`] when the node could
    /// not be built. The default implementation of this method returns
    /// [`LowerError::Other`]: an implementation of [`Lowerer`] that does not
    /// override it cannot build a node with content slots.
    fn pres_slots(
        &mut self,
        cx: &mut LowerCx<'_>,
        element: &ElementName,
        payload: Arc<dyn PresPayload>,
        slots: Vec<(String, Vec<BuildId>)>,
    ) -> Result<BuildId, LowerError> {
        let _ = (cx, element, payload, slots);
        Err(LowerError::other("this lowerer cannot build a presentation node with content slots"))
    }

    /// Builds a text node holding `text`, and returns its position.
    ///
    /// The text is not part of the document, so it is given a source of its
    /// own that identifies the node being lowered and the rule that produced
    /// the text. Following that provenance leads back to the document.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError::UndeclaredProduces`] when the backend does not
    /// write text (`flm.text`) natively and the rule's declaration does not
    /// list it, and [`LowerError::Build`] when the node could not be built.
    fn text(&mut self, cx: &mut LowerCx<'_>, text: &str) -> Result<BuildId, LowerError>;

    /// Splices the synthesized value `value`: lowers it where the node being
    /// lowered is placed, and returns the nodes it became.
    ///
    /// A synthesized value is a small tree that a stage builds while a run is
    /// under way, held outside the document's trees ([synthesized
    /// values](crate::guide::concepts_overview#synthesized-values)). It has
    /// been through none of the run's work, so this method first brings it up
    /// to date: the plan's rewrite stages run over it, then the per-tree
    /// stages the lowering stage depends on, so that the rules lowering its
    /// nodes read the same kinds of data as for a document node ([value
    /// catch-up](crate::guide::concepts_overview#value-catch-up),
    /// [`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value)).
    ///
    /// Every diagnostic of the catch-up is reported, and so is every
    /// diagnostic of the value's lowering: a value spliced at several places
    /// (a kept definition's template, staged at each of its use sites) reports
    /// at each of them. Each such diagnostic keeps its own span, where the
    /// content was written, and gains one frame of its traceback
    /// ([`Diagnostic::frames`](techy::error::Diagnostic::frames)) whose span is
    /// the node being lowered, the place where the value is shown, and whose
    /// title is `a copy of this content, shown here`. A value spliced inside a
    /// value adds its own frame first, so the frames run innermost first.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError::DocumentFactInValue`] when a stage run again over
    /// the value stated a fact about the document as a whole,
    /// [`LowerError::CatchUp`] when the value could not be brought up to date
    /// for any other reason, and any [`LowerError`] that the rules lowering
    /// the value returned. When that error is
    /// [`LowerError::PlaceholderCharacters`], the value's lowering is
    /// discarded entirely: the diagnostics it reported and the requirements it
    /// recorded ([`require`](Lowerer::require)) are dropped, so that what the
    /// rule writes in its place reports and records only its own.
    fn splice(
        &mut self,
        cx: &mut LowerCx<'_>,
        value: &Arc<NodeTree<Flm>>,
    ) -> Result<Vec<BuildId>, LowerError>;

    /// Lowers the content of a node region of the node at `at` (its argument
    /// or its content slot at a position, [`NodeRegion`]) here, in the output being
    /// built, as content of that node's own tree, and returns the nodes it
    /// became.
    ///
    /// This is how a rule shows content written elsewhere without copying it.
    /// The rule of an element inside a synthesized value refers to a node of
    /// the document, or of another synthesized value, and that node's content
    /// is lowered where the element is placed. The content pointer
    /// ([`ContentPointer`](crate::stdcore::backends::lowering::ContentPointer))
    /// is built on this method. It is the counterpart of [`Lowerer::splice`],
    /// which lowers a synthesized value where the rule's node is placed in the
    /// document; this method is the only way to lower content of a tree other
    /// than the one being lowered.
    ///
    /// `at` designates a node of a document tree, or of a synthesized value
    /// this lowering holds: the value being lowered, or a value being spliced
    /// around the call. The region's content nodes are passed to `shape`,
    /// which lowers them in the form the caller wants; see [`RegionShape`].
    /// While `shape` runs, this `Lowerer` works in the tree that holds the
    /// region: [`Lowerer::tree_id`] and [`Lowerer::layout`] return that
    /// tree's, [`Lowerer::addr`] and [`Lowerer::addr_of`] return addresses in
    /// it, and each node lowered there keeps its own address as its origin, so
    /// that every later round of lowering reads the tables at that address
    /// too. A per-tree table is then read from that tree's own results: the
    /// run's store for a document tree, the synthesized value's own results
    /// for a synthesized value. The results of a synthesized value around the
    /// call are not read at another tree's address. A table computed for the
    /// whole document is read as everywhere else. The [recursion
    /// guard](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads)
    /// applies as to any lowering: content that leads back to itself fails the tree
    /// with `flm.lowering.nesting-too-deep` instead of looping.
    ///
    /// **The caller must lower each node region once.** The lowering driver
    /// does not check it: a rule whose element passes one of its regions to
    /// this method elsewhere must not lower that region itself, and only one
    /// node may point at a given region. Content lowered twice is shown twice
    /// and reported twice.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError::RegionNotFound`] when no tree of `at`'s
    /// identifier is reachable, when that tree holds no node at `at`, or when
    /// the node has no such region; nothing is staged then. Returns any
    /// [`LowerError`] `shape` returns.
    fn lower_region(
        &mut self,
        cx: &mut LowerCx<'_>,
        at: NodeAddr,
        region: NodeRegion,
        shape: &mut RegionShape<'_>,
    ) -> Result<Vec<BuildId>, LowerError>;

    /// Records the requirement `id` for the tree being lowered: the
    /// identifier of a definition that its output needs, such as a style or
    /// script definition on HTML, or a package request or a definition piece
    /// on LaTeX ([`StyleDef`](crate::core::registry::StyleDef)). The backend
    /// that writes the output resolves the requirements recorded for each
    /// lowered tree.
    ///
    /// Use this for a requirement that comes from this particular node. A
    /// requirement the rule has whenever it is selected belongs in
    /// [`LoweringDecl::selection_requires`] instead.
    fn require(&mut self, id: &Name);

    /// Records a diagnostic about the node being lowered.
    ///
    /// A rule reports at the node it is lowering, with that node's span. The
    /// methods `error`, `warning` and `note`, defined on `dyn Lowerer`, build
    /// the diagnostic from a condition and a span.
    fn diagnose(&mut self, diagnostic: FlmDiagnostic);
}

/// The typed methods a rule calls on its [`Lowerer`].
///
/// These methods need a type parameter, which an object-safe trait cannot
/// have, so they are defined on `dyn Lowerer`, the form a rule is handed,
/// rather than on the trait. Each one calls the corresponding type-erased
/// method and converts the value.
impl dyn Lowerer + '_ {
    /// Reads a table the rule declared that it reads, as the table's value
    /// type.
    ///
    /// `Ok(None)` means that the run holds no value for the table, which can
    /// only happen for a table declared as an optional read.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError`] when the rule did not declare this read, or
    /// declared it under another value type; see [`Lowerer::get_erased`].
    ///
    /// # Panics
    ///
    /// Panics if the [`Lowerer`] implementation's [`Lowerer::get_erased`]
    /// returns a value of a type other than the one the key designates. The
    /// implementation in this crate cannot: the registry compares the value
    /// type of the table with the key's before the read.
    pub fn get<K: TableKey>(&self, key: K) -> Result<Option<&K::Table>, AccessError> {
        Ok(self.get_erased(&key.table_ref())?.map(|table| {
            table
                .downcast_ref::<K::Table>()
                .expect("a table is stored under its declared type, which the key matched")
        }))
    }

    /// Reads the value of an option type the rule declared that it reads.
    ///
    /// # Errors
    ///
    /// Returns [`OptionError`] when the rule did not declare that option type;
    /// see [`Lowerer::option_erased`].
    ///
    /// # Panics
    ///
    /// Panics if the [`Lowerer`] implementation's [`Lowerer::option_erased`]
    /// returns a value that is not of type `T`. The implementation in this
    /// crate cannot: an option value is stored under its own type.
    pub fn option<T: Options>(&self) -> Result<&T, OptionError> {
        Ok(self
            .option_erased(OptionKey::of::<T>())?
            .downcast_ref::<T>()
            .expect("a resolved option is stored under its own type"))
    }

    /// Reads the value of the option type declared from data under the name
    /// `name`, which the rule declared that it reads
    /// ([`LoweringDecl::option_named`]): the data as written, or the empty map
    /// when no value is set.
    ///
    /// # Errors
    ///
    /// Returns [`OptionError`] when the rule did not declare that read; see
    /// [`Lowerer::option_erased`].
    ///
    /// # Panics
    ///
    /// Panics if the [`Lowerer`] implementation's [`Lowerer::option_erased`]
    /// returns a value that is not a [`DataValue`]. The implementation in this
    /// crate cannot: an option declared from data is stored as one.
    pub fn option_named(&self, name: &str) -> Result<&Value, OptionError> {
        Ok(&self
            .option_erased(OptionKey::named(name))?
            .downcast_ref::<DataValue>()
            .expect("an option declared from data is stored as a `DataValue`")
            .0)
    }

    /// Builds a presentation node carrying `payload`, taken by value, and
    /// returns its position.
    ///
    /// The same as [`Lowerer::pres`] in every other respect.
    ///
    /// # Errors
    ///
    /// Exactly as [`Lowerer::pres`]:
    /// [`LowerError::UndeclaredProduces`] when the rule's declaration does not
    /// list `element`, and [`LowerError::Build`] when the node could not be
    /// built.
    pub fn pres_with<P: PresPayload>(
        &mut self,
        cx: &mut LowerCx<'_>,
        element: &ElementName,
        payload: P,
        body: Vec<BuildId>,
    ) -> Result<BuildId, LowerError> {
        self.pres(cx, element, Arc::new(payload), body)
    }

    /// Builds a presentation node with content slots, carrying `payload` taken
    /// by value, and returns its position.
    ///
    /// The same as [`Lowerer::pres_slots`] in every other respect.
    ///
    /// # Errors
    ///
    /// Exactly as [`Lowerer::pres_slots`]:
    /// [`LowerError::UndeclaredProduces`] when the rule's declaration does not
    /// list `element`, [`LowerError::Build`] when the node could not be built,
    /// and [`LowerError::Other`] from an implementation that does not override
    /// [`Lowerer::pres_slots`].
    pub fn pres_slots_with<P: PresPayload>(
        &mut self,
        cx: &mut LowerCx<'_>,
        element: &ElementName,
        payload: P,
        slots: Vec<(String, Vec<BuildId>)>,
    ) -> Result<BuildId, LowerError> {
        self.pres_slots(cx, element, Arc::new(payload), slots)
    }

    /// Records a diagnostic of Error severity reporting `condition` at `span`.
    ///
    /// An Error says that the document is wrong in a way that has no sensible
    /// rendering. See [`Lowerer::diagnose`].
    pub fn error(&mut self, condition: impl DiagnosticInfo, span: FlmSpan) {
        self.diagnose(Diagnostic::error(condition, span));
    }

    /// Records a diagnostic of Warning severity reporting `condition` at
    /// `span`.
    ///
    /// A Warning says that something is probably wrong, and the rule still
    /// rendered it. See [`Lowerer::diagnose`].
    pub fn warning(&mut self, condition: impl DiagnosticInfo, span: FlmSpan) {
        self.diagnose(Diagnostic::warning(condition, span));
    }

    /// Records a diagnostic of Note severity reporting `condition` at `span`.
    ///
    /// A Note gives information and reports no problem. See
    /// [`Lowerer::diagnose`].
    pub fn note(&mut self, condition: impl DiagnosticInfo, span: FlmSpan) {
        self.diagnose(Diagnostic::note(condition, span));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_mark_answers_per_backend() {
        assert!(DefaultMark::Everywhere.is_general());
        assert!(!DefaultMark::Never.is_general());
        let specific = DefaultMark::OnBackends(vec!["latex-rendered".into()]);
        assert!(specific.is_specific_to(&"latex-rendered".into()));
        assert!(!specific.is_specific_to(&"text".into()));
        assert!(!specific.is_general());
    }
}
