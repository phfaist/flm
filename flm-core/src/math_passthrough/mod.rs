//! The package `math-passthrough` and its one feature, `math-passthrough`:
//! mathematics carried to the output as the author wrote it, without being read.
//!
//! The [`math`](crate::stdlib::features::math) feature, one of the standard
//! features ([`stdlib`](crate::stdlib)), introduces mathematics into the language (the
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
//! no other package: the feature's declarations state what it needs (see *It
//! requires `math`* below). A program written in Rust adds [`package`] after the
//! package that holds the standard features; a program that reads its settings
//! as data adds [`factory`] to its catalog of packages:
//!
//! ```
//! use flm_core::core::plan::{Overrides, Plan};
//! use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
//! use flm_core::math_passthrough;
//! use flm_core::stdlib::{self, StdlibConfig};
//!
//! let mut builder = RegistryBuilder::new();
//! builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
//! builder.use_package(math_passthrough::package());
//! let registry = builder.build(&RegistryInputs::new())?;
//!
//! // The plan of a standard workflow builds.
//! let plan = Plan::for_workflow(&registry, stdlib::backends::html::WORKFLOW, &Overrides::none());
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
//! [`math`](crate::stdlib::features::math) module documentation, *Processing
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
//! capability [`flm.math.content`](crate::stdlib::features::math::CONTENT),
//! and the feature adds that capability to the collection
//! [`flm.validated`](crate::stdlib::VALIDATED), which every standard workflow
//! demands. It accepts every formula and reports every equation line that
//! holds no mathematics ([`check`]).
//!
//! # The rules
//!
//! Each rule stages (builds into the lowered tree; see
//! [`Lowerer`](crate::core::lowering::Lowerer)) presentation nodes. Except on the
//! `latex-recomposed` backend, a formula becomes a
//! [`flm.pres.math-passthrough`](crate::stdlib::backends::pres::MATH_PASSTHROUGH)
//! node that holds its source; the rules write the source unchanged with one
//! exception: **a text command becomes content**, lowered through its own rule
//! into a [`flm.pres.math-text`](crate::stdlib::backends::pres::MATH_TEXT) node
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
//!   line's labels in the [LaTeX hook](crate::stdlib::backends::latex::preamble)
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
//! # The rules under another feature's names
//!
//! Another feature that processes mathematics may write formulas as LaTeX
//! with this feature's rules, registered under rule names of its own:
//! [`math_rule_named`] and [`environment_rule_named`] build the rules that
//! stage `flm.pres.math-passthrough` under a given name and default mark, and
//! [`latex::math_rule_named`] and [`latex::environment_rule_named`] the rules
//! of the `latex-recomposed` backend under a given name. Each rule carries
//! this feature's body, declaration and description, and the documentation of
//! each constructor lists the peer declarations the registering feature makes
//! beside it. The registering feature owns the rule, so the plan's warning on
//! its unchecked output (below) names that feature. [`OPAQUE_RULE`] has no
//! such constructor: its element is this feature's fallback's.
//!
//! The `math-techxt` extension, which renders formulas as Unicode text on the
//! `text` and `html` backends, registers the four on `latex-rendered` and
//! `latex-recomposed`, where LaTeX typesets each formula from its LaTeX, and
//! writes no LaTeX rule of its own. An extension reaches the constructors
//! through the extension API's mirror of this module; in JavaScript, its
//! modules are `flm:math_passthrough` and `flm:math_passthrough/latex`, which
//! keep this module's name, with its underscore, while the feature and its
//! package are named `math-passthrough`.
//!
//! # Output that nothing checks
//!
//! The feature does not read a formula, so nothing checks that the program
//! reading the output accepts what it writes: a misspelled command reaches
//! that program as written. Each rule declares so
//! ([`LoweringDecl::unchecked_output`]) on the backends where a program reads
//! the formulas after FLM:
//!
//! | backend | rules | what is written | what reads it |
//! |---|---|---|---|
//! | `html` | [`MATH_RULE`], [`ENVIRONMENT_RULE`], [`OPAQUE_RULE`] | each formula's LaTeX source | the formula renderer in the reader's browser |
//! | `latex-rendered` | [`MATH_RULE`], [`ENVIRONMENT_RULE`], [`OPAQUE_RULE`] | each formula's LaTeX source | LuaLaTeX |
//! | `latex-recomposed` | the rules of [`latex`], [`OPAQUE_RULE`] | each formula's recomposed LaTeX | LaTeX |
//!
//! A plan of one of these backends therefore keeps one
//! [`PlanWarning::UncheckedOutput`](crate::core::plan::PlanWarning::UncheckedOutput)
//! for this feature, or for the feature that registers the rules under its
//! own names, which the run reports as the Warning
//! `flm.run.unchecked-output`. The rules declare nothing on the `text`
//! backend: the text output prints each formula's source as it is, and no
//! program reads it afterwards.
//!
//! # Map of the module
//!
//! - [`PACKAGE`], [`package`], [`factory`] — the package.
//! - [`MathPassthrough`] — the feature; [`feature`] returns it.
//! - [`OPAQUE`] — the element of an unknown command in math mode.
//! - [`CHECK`], [`check`] — the check stage and its one condition.
//! - [`MATH_RULE`], [`ENVIRONMENT_RULE`], [`OPAQUE_RULE`], [`source`] — the
//!   rules marked for every backend and their bodies.
//! - [`math_rule_named`], [`environment_rule_named`] — the same rules under
//!   another feature's names.
//! - [`latex`] — the rules for the `latex-recomposed` backend.

// An environment fallback stands beside the macro one because an alignment
// environment nested inside displayed mathematics is the common case. There is
// no fallback for special characters: an unrecognized trigger character is not
// a special character, so such a fallback could never apply. The source of a
// formula is kept as written because it goes to a formula engine or is printed
// literally, where any normalization would corrupt it.

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
use crate::model::names::{FeatureName, RuleName};
use crate::stdlib::VALIDATED;
use crate::stdlib::backends::{html, latex_recomposed, latex_rendered, pres};
use crate::stdlib::features::math::{CONTENT, ENVIRONMENT};
use crate::stdlib::features::numbering::{COUNTERS, Counters};
use crate::stdlib::features::refs::{self, ANCHORS};

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
/// registers it registers the standard features
/// ([`stdlib::add_features`](crate::stdlib::add_features)) too, before it,
/// since the feature requires the `math` feature (see the [module
/// documentation](self)).
pub fn package() -> PackageEntry {
    PackageEntry::new(PACKAGE).feature(feature())
}

/// Returns the package factory of the package `math-passthrough`: the form in
/// which a program adds the package to its catalog of packages
/// ([`PackageCatalog::add_compiled`](crate::core::registry::PackageCatalog::add_compiled)).
///
/// The [`PackageFactory`] declares that the package uses no other package. The
/// feature takes no settings; its build function returns [`package`], without
/// the feature when the settings switch it off.
///
/// # Errors
///
/// A [`PackageError`] when the build function fails for empty settings
/// ([`PackageFactory::new`] calls it once). This build function never fails, so
/// the error does not occur.
pub fn factory() -> Result<PackageFactory, PackageError> {
    PackageFactory::new(PACKAGE, |settings: &PackageSettings| {
        let mut entry = package();
        entry.features.retain(|feature| !settings.is_off(&feature.info().name));
        Ok(entry)
    })
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

/// Adds to `decl` the two backends on which a rule of this feature writes a
/// formula's source and nothing checks it before the program that reads the
/// output: `html`, whose formulas the formula renderer in the reader's browser
/// typesets, and `latex-rendered`, whose formulas LuaLaTeX typesets (see the
/// module documentation, *Output that nothing checks*).
fn source_unchecked(decl: LoweringDecl) -> LoweringDecl {
    decl.unchecked_output(
        html::BACKEND,
        "each formula's LaTeX source",
        "the formula renderer in the reader's browser",
    )
    .unchecked_output(latex_rendered::BACKEND, "each formula's LaTeX source", "LuaLaTeX")
}

/// Adds to `decl` the `latex-recomposed` backend, on which a rule of this
/// feature writes each formula as LaTeX recomposed from its nodes and nothing
/// checks it before LaTeX reads it (see the module documentation, *Output that
/// nothing checks*).
fn recomposed_unchecked(decl: LoweringDecl) -> LoweringDecl {
    decl.unchecked_output(
        latex_recomposed::BACKEND,
        "each formula's recomposed LaTeX",
        "LaTeX",
    )
}

/// The element value of every node of [`OPAQUE`].
fn opaque_element() -> Arc<ElementSpec> {
    Arc::new(ElementSpec::new(OPAQUE).docs(
        "A macro or environment that no feature declares, written in math mode: the node \
         keeps its source as written, because mathematical notation uses an open-ended set \
         of commands.",
    ))
}

/// The rule [`OPAQUE_RULE`], ready to be registered: it targets [`OPAQUE`],
/// produces `flm.pres.math-passthrough`, is the default on every backend, and
/// declares its output unchecked on `html`, `latex-rendered` and
/// `latex-recomposed` (see the module documentation, *Output that nothing
/// checks*).
pub fn opaque_rule() -> LoweringRule {
    LoweringRule::new(
        OPAQUE_RULE,
        OPAQUE,
        recomposed_unchecked(source_unchecked(LoweringDecl::new().produces(pres::MATH_PASSTHROUGH))),
        Arc::new(OpaqueRule),
        DefaultMark::Everywhere,
    )
    .docs(
        "an unknown command in math mode lowers to `flm.pres.math-passthrough` over its \
         source, unchanged",
    )
}

/// The rule [`MATH_RULE`], ready to be registered: [`math_rule_named`] under
/// that name, marked [`DefaultMark::Everywhere`].
pub fn math_rule() -> LoweringRule {
    math_rule_named(MATH_RULE, DefaultMark::Everywhere)
}

/// The library's rule of `flm.math` that stages `flm.pres.math-passthrough`,
/// under the rule name `name` and the default mark `mark`, ready to be
/// registered by any feature: its body is [`MathRule`], it targets
/// `flm.math`, produces `flm.pres.math-passthrough`, declares its output
/// unchecked on `html` and `latex-rendered` (see the module documentation,
/// *Output that nothing checks*), and carries the library's description.
/// [`math_rule`] is this rule under [`MATH_RULE`].
///
/// The rule does not declare that it produces `flm.pres.math-text`: the
/// `flm.pres.math-text` node of a text command is staged by that command's own
/// rule ([`text::RULE`](crate::stdlib::features::math::text::RULE)).
///
/// The rule reads no table, so a feature that registers it declares nothing
/// beside it. The feature that registers the rule is its owner: a plan that
/// keeps one of its unchecked outputs names that feature in the
/// [`PlanWarning::UncheckedOutput`](crate::core::plan::PlanWarning::UncheckedOutput),
/// with the texts the library declares.
pub fn math_rule_named(name: impl Into<RuleName>, mark: DefaultMark) -> LoweringRule {
    LoweringRule::new(
        name,
        MATH,
        source_unchecked(LoweringDecl::new().produces(pres::MATH_PASSTHROUGH)),
        Arc::new(MathRule),
        mark,
    )
    .docs(
        "inline or displayed mathematics lowers to `flm.pres.math-passthrough` over its \
         source, with the argument of every text command inside it lowered as document \
         content",
    )
}

/// The rule [`ENVIRONMENT_RULE`], ready to be registered:
/// [`environment_rule_named`] under that name, marked
/// [`DefaultMark::Everywhere`].
pub fn environment_rule() -> LoweringRule {
    environment_rule_named(ENVIRONMENT_RULE, DefaultMark::Everywhere)
}

/// The library's rule of `flm.math.environment` that stages
/// `flm.pres.math-passthrough`, under the rule name `name` and the default
/// mark `mark`, ready to be registered by any feature: its body is
/// [`EnvironmentRule`], it targets `flm.math.environment`, reads the anchors
/// table and the counter table optionally, produces
/// `flm.pres.math-passthrough` and `flm.pres.math-text`, declares its output
/// unchecked on `html` and `latex-rendered` (see the module documentation,
/// *Output that nothing checks*), and carries the library's description.
/// [`environment_rule`] is this rule under [`ENVIRONMENT_RULE`].
///
/// A feature that registers the rule declares beside it, as
/// [`MathPassthrough`] does, a peer table
/// ([`FeatureBuilder::peer_table`]) of each table it reads: the anchors table
/// ([`ANCHORS`], of value type [`Anchors`](refs::Anchors)) and the counter
/// table ([`COUNTERS`], of value type [`Counters`]). The feature that
/// registers the rule is its owner: a plan that keeps one of its unchecked
/// outputs names that feature in the
/// [`PlanWarning::UncheckedOutput`](crate::core::plan::PlanWarning::UncheckedOutput),
/// with the texts the library declares.
pub fn environment_rule_named(name: impl Into<RuleName>, mark: DefaultMark) -> LoweringRule {
    LoweringRule::new(
        name,
        ENVIRONMENT,
        source_unchecked(
            LoweringDecl::new()
                .reads_opt(ANCHORS)
                .reads_opt(COUNTERS)
                .produces(pres::MATH_PASSTHROUGH)
                .produces(pres::MATH_TEXT),
        ),
        Arc::new(EnvironmentRule),
        mark,
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
        b.element_type_text(
            OPAQUE,
            "A command of mathematical notation that no feature declares, kept as written and \
             passed to the output, where the program that typesets the formula reads it.",
        )?;
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
    fn the_package_holds_the_feature_and_uses_no_package() {
        let entry = package();
        assert_eq!(entry.name, PACKAGE);
        assert!(entry.uses.is_empty());
        let names: Vec<String> = entry.features.iter().map(|feature| feature.info().name.to_string()).collect();
        assert_eq!(names, [FEATURE]);
        let factory = factory().expect("the factory");
        assert_eq!(factory.name().as_str(), PACKAGE);
        assert!(factory.used().is_empty());
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

    /// The backends each rule declares unchecked output on.
    fn unchecked_backends(rule: &LoweringRule) -> Vec<&str> {
        rule.decl.unchecked_output.iter().map(|decl| decl.backend.as_str()).collect()
    }

    #[test]
    fn the_rules_declare_unchecked_output_on_every_backend_but_text() {
        assert_eq!(unchecked_backends(&math_rule()), ["html", "latex-rendered"]);
        assert_eq!(unchecked_backends(&environment_rule()), ["html", "latex-rendered"]);
        assert_eq!(unchecked_backends(&opaque_rule()), ["html", "latex-rendered", "latex-recomposed"]);
        assert_eq!(unchecked_backends(&latex::math_rule()), ["latex-recomposed"]);
        assert_eq!(unchecked_backends(&latex::environment_rule()), ["latex-recomposed"]);
    }
}
