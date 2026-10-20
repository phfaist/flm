//! The package `math-passthrough` and its one feature, `math-passthrough`:
//! mathematics carried to the output as the author wrote it, without being read.
//!
//! The [`math`](crate::stdcore::features::math) feature of the standard package
//! [`stdcore`](crate::stdcore) introduces mathematics into the language (the
//! display-math environments, their equation lines and numbers, the text
//! commands, `\eqref`) and processes none of it. This feature is one of the
//! features that *process* mathematics, of which a registry holds one: it
//! accepts any formula whose macro calls and environments are syntactically
//! valid, reads nothing in it, and carries the source of each formula and of
//! each equation line to the output. Every name it registers carries
//! `passthrough`, as a reminder that FLM has not parsed the content. Each
//! backend writes that source in a form the reader of the output typesets: a
//! LaTeX compiler, or the MathJax script in a web browser.
//!
//! # The package
//!
//! The package `math-passthrough` ([`PACKAGE`]) holds this one feature and uses
//! `stdcore`, whose items it builds on. A program written in Rust adds
//! [`package`] after `stdcore`'s package; a program that reads its settings as
//! data adds [`factory`] to its catalog of packages, which declares that the
//! package uses `stdcore`:
//!
//! ```
//! use flm_core::core::plan::{Overrides, Plan};
//! use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
//! use flm_core::math_passthrough;
//! use flm_core::stdcore::{self, StdcoreConfig};
//!
//! let mut builder = RegistryBuilder::new();
//! builder.use_package(stdcore::package(StdcoreConfig::new()));
//! builder.use_package(math_passthrough::package());
//! let registry = builder.build(&RegistryInputs::new())?;
//!
//! // The plan of a standard workflow builds.
//! let plan = Plan::for_workflow(&registry, stdcore::backends::html::WORKFLOW, &Overrides::none());
//! assert!(plan.is_ok());
//! # Ok::<(), flm_core::core::registry::RegistryBuildError>(())
//! ```
//!
//! # It requires `math`
//!
//! The feature's rules lower `flm.math.environment` and its check stage
//! provides `flm.math.content`, both declared by the `math` feature. A registry
//! that holds this feature without `math` fails to build: a rule on an element
//! that no feature declares, and a stage providing a capability that no
//! feature declares, are each a [`BuildError::Undeclared`] naming the feature,
//! the place and the name. Conversely, in a registry that holds `math` and no
//! feature that processes mathematics, each formula is an Error (the
//! [`math`](crate::stdcore::features::math) module documentation, *Processing
//! mathematics*).
//!
//! The feature depends on nothing outside `flm-core`: an embedded program with
//! no JavaScript and no extension renders mathematics with it. It has no
//! configuration and no settings of its own; a program that reads its settings
//! as data switches it off like any other feature.
//!
//! # Unknown commands in mathematics
//!
//! Mathematical notation uses an open-ended set of commands that FLM does not
//! declare. This feature therefore registers a *fallback* ([`FallbackDef`]), a
//! declaration of the element that the parse gives to a macro or an environment
//! that no feature declares, for macros and for environments **in math mode
//! only**. Such a command becomes a node of the element [`OPAQUE`]
//! (`flm.math-passthrough.opaque`): it takes no argument, its source is kept
//! character for character, and nothing is reported. A command of another
//! feature written inside a formula (`\section`, `\href`) is such a command,
//! since every other feature declares its commands for text mode only.
//!
//! Outside math mode the language layer's own fallbacks apply: an unknown macro
//! becomes `flm.unknown.macro` and an unknown environment
//! `flm.unknown.environment`, each with an Error diagnostic.
//!
//! # The check
//!
//! The check stage [`CHECK`] (`flm.math-passthrough.check`) provides the
//! capability [`flm.math.content`](crate::stdcore::features::math::CONTENT),
//! and the feature adds that capability to the collection
//! [`flm.validated`](crate::stdcore::VALIDATED), which every standard workflow
//! demands. It accepts every formula and reports every equation line that
//! holds no mathematics ([`check`]).
//!
//! # The rules
//!
//! Each rule stages (builds into the lowered tree; see
//! [`Lowerer`](crate::core::lowering::Lowerer)) presentation nodes. Except on the
//! `latex-recomposed` backend, a formula becomes a
//! [`flm.pres.math-passthrough`](crate::stdcore::backends::pres::MATH_PASSTHROUGH)
//! node that holds its source; the rules write the source unchanged with one
//! exception: **a text command becomes content**, lowered through its own rule
//! into a [`flm.pres.math-text`](crate::stdcore::backends::pres::MATH_TEXT) node
//! among the formula's source ([`source`]).
//!
//! | element | every backend | `latex-recomposed` |
//! |---|---|---|
//! | `flm.math` (`\(…\)`, `\[…\]`) | [`MATH_RULE`] | [`latex::MATH_LATEX_RULE`] |
//! | `flm.math.environment` | [`ENVIRONMENT_RULE`] | [`latex::ENVIRONMENT_LATEX_RULE`] |
//! | `flm.math-passthrough.opaque` | [`OPAQUE_RULE`] | [`OPAQUE_RULE`] |
//!
//! - [`MATH_RULE`] stages a `flm.pres.math-passthrough` node, displayed for
//!   `\[…\]` and inline for `\(…\)`, over the group's source.
//! - [`ENVIRONMENT_RULE`] stages a displayed `flm.pres.math-passthrough` node
//!   that carries the environment's name, over the environment's lines. Each
//!   line gives, in order: an empty `flm.pres.math-text` node carrying the
//!   line's anchor, so that a link can lead to the line; the line's source;
//!   the line's tag, written `\tag*{…}` around a `flm.pres.math-text` node that
//!   holds the author's tag or the line's number; and, when another line
//!   follows, the `\\` that ends the line. A line's `\label` and `\nonumber` are
//!   not written: they are data of the line, which the `math` feature has
//!   already read.
//! - The rules of [`latex`] write mathematics back as LaTeX source, with each
//!   line's labels in the [LaTeX hook](crate::stdcore::backends::latex::preamble)
//!   `\flmLabels` (a macro of the style file that a user may redefine), and
//!   leave the numbering to LaTeX.
//! - [`OPAQUE_RULE`] stages a `flm.pres.math-passthrough` node over the opaque
//!   node's source. A parse produces opaque nodes only in math mode, and the
//!   rules above write a math group or an environment whole, so this rule runs
//!   only for math-mode content that is lowered node by node, for example, on
//!   the `latex-recomposed` backend, a math-mode argument of a user definition
//!   written as a call of that definition.
//!
//! The rules marked for every backend serve any backend that writes
//! `flm.pres.math-passthrough` natively, which the four standard backends do:
//! the `html` backend writes it as the [HTML
//! contract](crate::guide::rendering#the-html-contract) states and loads the
//! MathJax script at the first such formula of a page; the rendering guide
//! describes the result under [mathematics in
//! HTML](crate::guide::rendering#mathematics-in-html), [mathematics in the
//! rendered LaTeX](crate::guide::rendering#mathematics-in-the-rendered-latex) and
//! [mathematics in the recomposed
//! LaTeX](crate::guide::rendering#mathematics-in-the-recomposed-latex).
//!
//! [`ENVIRONMENT_RULE`] reads two tables that other features own: the anchors
//! table of the references feature
//! ([`ANCHORS`]) and the counter table of the numbering feature
//! ([`COUNTERS`]). It reads both
//! optionally, under a peer declaration ([`PeerName`](crate::core::registry::PeerName))
//! of each, so that a registry without those features still builds and renders:
//! the formulas then carry no anchor and no number.
//!
//! # Map of the module
//!
//! - [`PACKAGE`], [`package`], [`factory`] — the package.
//! - [`MathPassthrough`] — the feature; [`feature`] returns it.
//! - [`OPAQUE`] — the element of an unknown command in math mode.
//! - [`CHECK`], [`check`] — the check stage and its one condition.
//! - [`MATH_RULE`], [`ENVIRONMENT_RULE`], [`OPAQUE_RULE`], [`source`] — the
//!   rules marked for every backend and their bodies.
//! - [`latex`] — the rules for the `latex-recomposed` backend.

// Why an environment fallback beside the macro one, and why no specials
// fallback (an unknown trigger character is not a specials): see design
// rationale at [§flm-dd-dr:std-math-fallback] and [§flm-dd-dr:lang-math-opaque].

pub mod check;
pub mod latex;
pub mod source;

use std::sync::Arc;

use techy::latexlike::Mode;

use crate::core::lowering::{DefaultMark, LoweringDecl, LoweringRule};
use crate::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, PackageError, PackageFactory,
};
use crate::core::settings::PackageSettings;
use crate::lang::mint::{FallbackDef, FallbackKind};
use crate::lang::structural::MATH;
use crate::model::element::ElementSpec;
use crate::model::names::FeatureName;
use crate::stdcore::VALIDATED;
use crate::stdcore::backends::pres;
use crate::stdcore::features::math::{CONTENT, ENVIRONMENT};
use crate::stdcore::features::numbering::{COUNTERS, Counters};
use crate::stdcore::features::refs::{self, ANCHORS};

pub use check::{CHECK, EmptyLine};
pub use source::{EnvironmentRule, MathRule, OpaqueRule};

/// The name of the package, `math-passthrough`: the name by which a program
/// looks the package up in its catalog of packages (with the `flm` program,
/// the name a document lists under `use` in its front matter).
pub const PACKAGE: &str = "math-passthrough";

/// The feature's registered name, `math-passthrough`, the name of its package
/// too.
pub const FEATURE: &str = "math-passthrough";

/// Returns the package `math-passthrough`, holding the [`MathPassthrough`]
/// feature.
///
/// The returned [`PackageEntry`] uses no other package: a program that
/// registers it registers the standard package
/// ([`stdcore::package`](crate::stdcore::package)) too, before it, since the
/// feature requires the `math` feature (see the [module documentation](self)).
/// [`factory`] returns the form of the package that declares that dependency.
pub fn package() -> PackageEntry {
    PackageEntry::new(PACKAGE).feature(feature())
}

/// Returns the package factory of the package `math-passthrough`: the form in
/// which a program adds the package to its catalog of packages
/// ([`PackageCatalog::add_compiled`](crate::core::registry::PackageCatalog::add_compiled)).
///
/// The [`PackageFactory`] declares that the package uses the standard package
/// (`stdcore`, [`stdcore::PACKAGE`](crate::stdcore::PACKAGE)). The feature
/// takes no settings; its build function returns [`package`], without the
/// feature when the settings switch it off.
///
/// # Errors
///
/// A [`PackageError`] when the build function fails for empty settings
/// ([`PackageFactory::new`] calls it once). This build function never fails, so
/// the error does not occur.
pub fn factory() -> Result<PackageFactory, PackageError> {
    let factory = PackageFactory::new(PACKAGE, |settings: &PackageSettings| {
        let mut entry = package();
        entry.features.retain(|feature| !settings.is_off(&feature.info().name));
        Ok(entry)
    })?;
    Ok(factory.uses(crate::stdcore::PACKAGE))
}

/// The name of the element of a macro or an environment that no feature
/// declares, written in math mode, `flm.math-passthrough.opaque`: the node
/// keeps its source as written. See the module documentation, *Unknown
/// commands in mathematics*.
pub const OPAQUE: &str = "flm.math-passthrough.opaque";

/// The name of the default lowering rule of [`OPAQUE`] on every backend,
/// `flm.math-passthrough.opaque.lower.default`, whose body is [`OpaqueRule`].
pub const OPAQUE_RULE: &str = "flm.math-passthrough.opaque.lower.default";

/// The name of the rule of `flm.math` (inline and displayed mathematics
/// written without an environment), `flm.math.lower.passthrough`, whose body is
/// [`MathRule`]: the default on every backend. On the `latex-recomposed`
/// backend, [`MATH_LATEX_RULE`](latex::MATH_LATEX_RULE) is chosen instead,
/// because it is the default on that backend.
pub const MATH_RULE: &str = "flm.math.lower.passthrough";

/// The name of the rule of the display-math environments
/// (`flm.math.environment`), `flm.math.environment.lower.passthrough`, whose
/// body is [`EnvironmentRule`]: the default on every backend. On the
/// `latex-recomposed` backend,
/// [`ENVIRONMENT_LATEX_RULE`](latex::ENVIRONMENT_LATEX_RULE) is chosen
/// instead, because it is the default on that backend.
pub const ENVIRONMENT_RULE: &str = "flm.math.environment.lower.passthrough";

/// The element value of every node of [`OPAQUE`].
fn opaque_element() -> Arc<ElementSpec> {
    Arc::new(ElementSpec::new(OPAQUE).docs(
        "A macro or environment that no feature declares, written in math mode: the node \
         keeps its source as written, because mathematical notation uses an open-ended set \
         of commands.",
    ))
}

/// The rule [`OPAQUE_RULE`], ready to be registered: it targets [`OPAQUE`],
/// produces `flm.pres.math-passthrough`, and is the default on every backend.
pub fn opaque_rule() -> LoweringRule {
    LoweringRule::new(
        OPAQUE_RULE,
        OPAQUE,
        LoweringDecl::new().produces(pres::MATH_PASSTHROUGH),
        Arc::new(OpaqueRule),
        DefaultMark::Everywhere,
    )
    .docs(
        "an unknown command in math mode lowers to `flm.pres.math-passthrough` over its \
         source, unchanged",
    )
}

/// The rule [`MATH_RULE`], ready to be registered: it targets `flm.math`,
/// produces `flm.pres.math-passthrough`, and is the default on every backend.
///
/// The rule does not declare that it produces `flm.pres.math-text`: the
/// `flm.pres.math-text` node of a text command is staged by that command's own
/// rule ([`text::RULE`](crate::stdcore::features::math::text::RULE)).
pub fn math_rule() -> LoweringRule {
    LoweringRule::new(
        MATH_RULE,
        MATH,
        LoweringDecl::new().produces(pres::MATH_PASSTHROUGH),
        Arc::new(MathRule),
        DefaultMark::Everywhere,
    )
    .docs(
        "inline or displayed mathematics lowers to `flm.pres.math-passthrough` over its \
         source, with the argument of every text command inside it lowered as document \
         content",
    )
}

/// The rule [`ENVIRONMENT_RULE`], ready to be registered: it targets
/// `flm.math.environment`, reads the anchors table and the counter table
/// optionally, produces `flm.pres.math-passthrough` and `flm.pres.math-text`,
/// and is the default on every backend.
pub fn environment_rule() -> LoweringRule {
    LoweringRule::new(
        ENVIRONMENT_RULE,
        ENVIRONMENT,
        LoweringDecl::new()
            .reads_opt(ANCHORS)
            .reads_opt(COUNTERS)
            .produces(pres::MATH_PASSTHROUGH)
            .produces(pres::MATH_TEXT),
        Arc::new(EnvironmentRule),
        DefaultMark::Everywhere,
    )
    .docs(
        "a display-math environment lowers to a displayed `flm.pres.math-passthrough` that \
         carries its name, over its equation lines; each line begins with an empty \
         `flm.pres.math-text` carrying its anchor and ends with its tag (the author's tag or \
         the line's number) and, unless it is the last line, its line break; labels are not \
         written",
    )
}

/// The `math-passthrough` feature: it registers the fallback of math mode,
/// the check stage that provides `flm.math.content`, which it adds to the
/// collection `flm.validated`, and the lowering rules of the formulas on every
/// backend. See the module documentation.
#[derive(Debug, Default, Clone, Copy)]
pub struct MathPassthrough;

impl FlmFeature for MathPassthrough {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Math passthrough").docs(
            "Mathematics carried to the output as written: unknown macros and environments in \
             math mode kept as source (`flm.math-passthrough.opaque`), the check \
             `flm.math.content`, which accepts every formula, and the lowering rules of the \
             formulas on every backend. Requires the `math` feature.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner: FeatureName = b.feature().clone();
        let opaque = opaque_element();
        for kind in [FallbackKind::Macro, FallbackKind::Environment] {
            b.fallback(FallbackDef {
                owner: owner.clone(),
                modes: vec![Mode::Math],
                kind,
                element: Arc::clone(&opaque),
            })?;
        }
        b.stage(Arc::new(check::PassthroughCheck))?;
        // Every workflow that validates a document demands the check of its
        // mathematics, since this feature processes it.
        b.collection_member(VALIDATED, CONTENT)?;
        // The anchors table belongs to the references feature and the counter
        // table to the feature that numbers; a registry need hold neither, so
        // the environment rule reads both optionally, under these peer
        // declarations, and a formula then carries no anchor and no number.
        b.peer_table::<refs::Anchors>(
            ANCHORS.name(),
            "the references feature's anchors table, which the display-math environment rule \
             reads for the anchor of each equation line",
        )?;
        b.peer_table::<Counters>(
            COUNTERS.name(),
            "the numbering feature's counter table, which the display-math environment rule \
             reads for the number of each equation line",
        )?;
        b.lowering_rule(opaque_rule())?;
        b.lowering_rule(math_rule())?;
        b.lowering_rule(environment_rule())?;
        latex::register(b)?;
        Ok(())
    }
}

/// The [`MathPassthrough`] feature, as a shared value ready to be listed in a
/// package entry.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(MathPassthrough)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::names::ElementName;

    #[test]
    fn the_package_holds_the_feature_and_its_factory_uses_stdcore() {
        let entry = package();
        assert_eq!(entry.name, PACKAGE);
        assert!(entry.uses.is_empty());
        let names: Vec<String> = entry.features.iter().map(|feature| feature.info().name.to_string()).collect();
        assert_eq!(names, [FEATURE]);
        let factory = factory().expect("the factory");
        assert_eq!(factory.name().as_str(), PACKAGE);
        assert_eq!(factory.used().iter().map(|name| name.as_str()).collect::<Vec<_>>(), [crate::stdcore::PACKAGE]);
        let mut settings = PackageSettings::new();
        settings.switch_off(FEATURE);
        assert!(factory.build(&settings).expect("the package builds").features.is_empty());
    }

    #[test]
    fn the_rules_for_every_backend_are_marked_everywhere() {
        for rule in [opaque_rule(), math_rule(), environment_rule()] {
            assert!(rule.default.is_general(), "{}", rule.name);
            assert!(rule.decl.produces.contains(&ElementName::from(pres::MATH_PASSTHROUGH)));
        }
        assert_eq!(environment_rule().decl.reads_opt.len(), 2);
        assert!(math_rule().decl.reads_opt.is_empty());
    }
}
