//! The FLM package `phfpack`: notation and quantity macros for quantum
//! information, written as an extension of FLM.
//!
//! This crate holds one [package](PackageEntry) (a named bundle of features
//! together with the packages it builds on), `phfpack`, with one
//! [feature](flm_core::core::registry::FlmFeature), `phfqit`. The feature
//! defines macros for bras and kets, norms, operators, gates, groups and
//! entropies; each macro is a [user
//! definition](flm_core::lang::rewrites::userdefs::Definition) (a macro defined
//! by substitution: a name, an argument syntax and a template), and every use of
//! one is replaced by its template before any output is written.
//!
//! The crate is an **extension**: among FLM's crates it depends on the library
//! `flm-core` alone, uses only its public interface, and reaches the parser
//! toolkit only through the library's re-export [`flm_core::techy`], as a crate
//! written by a third party would. Its other dependencies, `serde`, `schemars`
//! and `serde_json`, read and describe its settings. The section [The flm-core
//! items the crate uses](#the-flm-core-items-the-crate-uses) lists the items of
//! `flm-core` it uses, for an author of another extension.
//!
//! A program that reads the list of packages a document uses gets this package
//! by its name, [`PACKAGE`]; with the `flm` program, a document lists it under
//! `use` in its front matter:
//!
//! ```yaml
//! ---
//! flm:
//!   use: [phfpack]
//! ---
//! ```
//!
//! # What an author may write
//!
//! The macros are written in formulas, `\(\ket{\psi}\)`. Each is replaced by its
//! template in the document's tree before any output is written: the HTML
//! output keeps the formula as LaTeX text, `\lvert{\psi}\rangle`, and the
//! recomposed LaTeX output holds the same text. Each built-in macro except the
//! backtick form (below) is declared for both running text and formulas
//! ([`DefinitionModes::Both`](flm_core::lang::rewrites::userdefs::DefinitionModes::Both)).
//! In each of the two, the macro is available when its template parses there.
//! A macro whose template is valid in running text, such as `\Ident` or
//! `\bit{0}`, therefore works there too. Any other macro, `\ket{x}` among them,
//! is an Error where it is used in running text; the rest of the document is
//! still rendered.
//!
//! **Delimited macros.** Their arguments are written between delimiters drawn
//! at the size the use asks for:
//!
//! | macro | gives |
//! |---|---|
//! | `\abs{x}` | `\lvert x \rvert` |
//! | `\norm{x}` | `\lVert x \rVert` |
//! | `\avg{x}` | `\langle x \rangle` |
//! | `\ket{x}`, `\bra{x}` | `\lvert x \rangle`, `\langle x \rvert` |
//! | `\braket{x}{y}` | `\langle x \vert y \rangle` |
//! | `\ketbra{x}{y}`, `\proj{x}` | `\lvert x \rangle \langle y \rvert`, the same with `x` twice |
//! | `\matrixel{x}{A}{y}`, `\dmatrixel{x}{A}` | `\langle x \vert A \vert y \rangle`, the same with `x` twice |
//! | `\innerprod{x}{y}` | `\langle x, y \rangle` |
//! | `\oket`, `\obra`, `\obraket`, `\oketbra`, `\oproj`, `\omatrixel`, `\odmatrixel` | the same with double angle brackets `\llangle`, `\rrangle` |
//! | `\intervalc{a}{b}`, `\intervalo`, `\intervalco`, `\intervaloc` | `[a, b]`, `]a, b[`, `[a, b[`, `]a, b]` |
//!
//! **Sizes.** A delimited macro takes its size in one of three ways: a size
//! command after a backtick (``\abs`\Big{x}``), a size command in brackets
//! (`\abs[\Big]{x}`), or a star (`\abs*{x}`, also `\abs[*]{x}` and
//! ``\abs`*{x}``), which draws the delimiters as tall as the content. The size
//! commands are `\big`, `\Big`, `\bigg` and `\Bigg` ([`SIZE_COMMANDS`]); a use
//! gives at most one size. Every delimiter of the macro, the bar of `\braket`
//! included, is drawn at the size asked for. Sizes apply in formulas only: a
//! delimited macro used in running text (possible when its template parses
//! there) writes its delimiters at their natural size, and a use in running
//! text that asks for a size is an Error.
//!
//! **Simple macros.** `\Hs` (`\mathscr{H}`), `\Ident` (the character `𝟙`),
//! `\bit{0}` (a bit in a monospace font), `\bitstring{0110}` (bits in a
//! monospace font, with a line above and below), `\gate{H}` and `\AND`, `\XOR`,
//! `\CNOT`, `\NOT`, `\NOOP` (gates in small capitals), the groups and algebras
//! `\uu(n)`, `\UU(n)`, `\su(n)`, `\SU(n)`, `\so(n)`, `\SO(n)`, `\slalg(n)`,
//! `\SL(n)`, `\GL(n)` and `\SN(n)` (each written with its argument in
//! parentheses; `\SN(n)` gives `\mathrm{S}_{n}`), and the operators `\tr`, `\supp`, `\rank`, `\linspan`,
//! `\spec`, `\diag`, `\Re`, `\Im`, `\poly`.
//!
//! **The backtick form**, in formulas only: a backtick, an optional size, and a
//! group in braces, brackets, parentheses or angle brackets write the group
//! between its own delimiters at that size. `` `\Big(a+b) `` gives
//! `\Bigl (a+b\Bigr )`, `` `*{x} `` gives `\mathopen{}\left \{x\right
//! \}\mathclose{}`, and `` `<x> `` gives `\langle x\rangle`. In running text a
//! backtick is a quotation mark, as it is without the package.
//!
//! **Quantity macros**: entropies and other measures of information. Each is
//! written with optional parts that appear in the output only when given. A
//! quantity macro that takes a size takes it after a backtick only (a bracket
//! after the macro is its first optional part, not a size); `\ee` and
//! `\emptysystem` take none. In the table below, `[state]`, `[epsilon]` and
//! `[cond]` are optional arguments. A subscript or superscript that would hold
//! nothing is left out, and the parenthesis is drawn at the size asked for. The
//! parenthesis, with the state's subscript after it, is left out when every part
//! it holds is blank: `\Hmaxf{}` is the symbol alone, `H_{\mathrm{max}}`, and
//! `\DD{}{}` is `D`.
//!
//! | macro | arguments | gives |
//! |---|---|---|
//! | `\HH`, `\Hmin`, `\Hzero`, `\Hmaxf` | size, `[state]`, `[epsilon]`, `{target}`, `[cond]` | `H_{sub}^{epsilon}(target \,\vert\, cond)_{state}`, *sub* being nothing, `\mathrm{min}`, `\mathrm{max},0` or `\mathrm{max}`: ``\Hmin`\Big[\rho][\epsilon]{X}[Y]`` |
//! | `\Hfn` | size, `(arg)` | `H(arg)`: ``\Hfn`\big(p_1\rho_1 + p_2\rho_2)`` |
//! | `\II` | size, `[state]`, `[epsilon]`, `{A}`, `{B}`, `[cond]` | `I^{epsilon}(A \,:\, B \,\vert\, cond)_{state}`: `\II[\rho]{A}{C}[B]` |
//! | `\DD` | `_{sub}` and `^{sup}` in either order, each optional; size; `{rho}`; `{sigma}` | `D_{sub}^{sup}(rho \,\Vert\, sigma)`: `\DD_{\mathrm{Rob}}^{\epsilon}{\rho}{\sigma}` |
//! | `\Dmax`, `\Dminz`, `\Dminf`, `\Dr`, `\DHyp`, `\Dhyp` | `[epsilon]`, size, `{rho}`, `{sigma}` | `D_{sub}^{epsilon}(rho \,\Vert\, sigma)`, *sub* being `\mathrm{max}`, `\mathrm{min,0}`, `\mathrm{min}`, `\mathrm{r}`, `\mathrm{H}` or `\mathrm{h}`; `\DHyp` and `\Dhyp` write `\eta` when no epsilon is given: ``\Dmax[\epsilon]`\big{\rho}{\sigma}`` |
//! | `\IdentProc` | size, `[A]`, `[A']`, `{arg}` | `\operatorname{id}_{A \to A'}(arg)`, the parenthesis only around an argument that is not empty: `\IdentProc[A][A']{\rho}` |
//! | `\ee` | `^{x}`, optional | `e^{x}`: `\ee^{-i\theta}` |
//! | `\emptysystem` | none | `\emptyset`, a trivial system |
//!
//! The bar before a conditioning system, and the double bar of a relative
//! entropy, are drawn at the size of the parenthesis. For an entropy of its own
//! (another symbol or subscript), a document declares a macro with the same
//! arguments and a template of its own (next section).
//!
//! # Declaring macros
//!
//! The feature's settings declare more macros under the key
//! `macro_definitions` (the module [`settings`] gives the keys and an example;
//! with the `flm` program, a document writes them in its front matter under
//! `flm`, `features`, `phfqit`, `macro_definitions`). A declaration is one of three forms:
//!
//! - a text: the template of a macro that takes no argument;
//! - a map with `delimited_args`: a delimited macro like the built-in ones, which
//!   takes the three size forms before the arguments listed;
//! - a map with `args`: any list of arguments, in which `SizeArgBacktick`,
//!   `SizeArgStar` and `SizeArgOptArg` are the three ways of giving a size (a
//!   size after a backtick, a star, a size in brackets).
//!
//! A declared macro with the name of a built-in macro of `phfqit` replaces it. A
//! declared macro with the name of a command another feature defines in the same
//! parsing mode, such as `\emph`, makes the registry build fail.
//!
//! A template is written in the [template
//! language](flm_core::lang::rewrites::userdefs::template), FLM text with
//! placeholders, as a user definition's template is: `#1`, `#2` and `#{name}`
//! are the arguments. The feature adds four placeholders of its own, whose
//! content the feature computes at each use:
//!
//! - `#[\lVert]{DelimLeft}`, `#[\vert]{DelimMiddle}` and `#[\rVert]{DelimRight}`
//!   write the delimiter in brackets, on the left, in the middle or on the
//!   right, at the size the use asked for;
//! - `#[spaceKets.Bar]{config}` writes a named value of the settings (below).
//!
//! **Named values.** A named value is a text held in the feature's settings
//! under a key and an entry name, such as `spaceKets.Bar`, which a template
//! inserts with `#[key.entry]{config}`. The keys are `spaceKets` and
//! `spaceOKets`, each with the entries `Bar` and `RLAngle` (the spacing around the bar of `\braket` and
//! between the angle brackets of `\ketbra`, and of their double-bracket
//! versions), and `space`, with `BeforeComma` and `AfterComma` (around the
//! comma of `\innerprod`).
//!
//! # Adding the package to a program
//!
//! [`factory`] returns the package in the form a program adds to its catalog of
//! packages: a [`PackageFactory`] that builds the package from the settings the
//! program read for the package's features (for the `flm` program, the settings
//! it combines for the document being processed from its own defaults, the
//! packages of the extensions that the document uses, the front matter, the
//! workflow and the command line). A program adds it with
//! [`PackageCatalog::add_compiled`](flm_core::core::registry::PackageCatalog::add_compiled),
//! together with the standard package of `flm-core`
//! ([`flm_core::stdcore::factory`]), which this package uses. The `flm`
//! command-line program includes the package when its cargo feature `phfpack`
//! is on, which it is by default. [`package`] returns the package
//! for a given configuration, for a program that builds its registry directly
//! with a [`RegistryBuilder`](flm_core::core::registry::RegistryBuilder).
//!
//! The feature `phfqit` requires the `userdefs` feature of the standard package,
//! whose stage replaces the macros by their templates: a registry that holds
//! `phfqit` without `userdefs` fails to build.
//!
//! # The flm-core items the crate uses
//!
//! This crate shows how a feature written outside `flm-core` declares its
//! elements, rules and settings and registers them. Each step uses an item of
//! `flm-core`'s public interface:
//!
//! - **The package.** [`package`] builds a [`PackageEntry`] that holds the
//!   feature. [`factory`] wraps [`package`] in a [`PackageFactory`]. The
//!   factory's build function receives the settings of the package's features
//!   ([`PackageSettings`]), reads the feature's configuration from them, and
//!   leaves the feature out when the settings switch it off; `factory` also
//!   declares that the package uses the standard package
//!   ([`PackageFactory::uses`]). The entry states the feature order
//!   `math-mathjax < phfqit` ([`PackageEntry::feature_order`], [`ORDER`]), so
//!   that `phfqit`'s definitions apply over MathJax's where both are loaded.
//! - **The feature.** [`Phfqit`] implements
//!   [`FlmFeature`](flm_core::core::registry::FlmFeature): `info` returns a
//!   [`FeatureInfo`](flm_core::core::registry::FeatureInfo) with the feature's
//!   name ([`FEATURE`]), title and description; `settings` returns the
//!   declaration of its settings; `register` declares everything else through
//!   the [`FeatureBuilder`](flm_core::core::registry::FeatureBuilder) it
//!   receives.
//! - **Elements.** Each macro is a [user
//!   definition](flm_core::lang::rewrites::userdefs::Definition) held in a
//!   [`UserDefinition`](flm_core::lang::rewrites::userdefs::UserDefinition).
//!   `register` declares the macro's element with
//!   [`FeatureBuilder::element`](flm_core::core::registry::FeatureBuilder::element),
//!   passing the element declaration that
//!   [`UserDefinition::element_def_with`](flm_core::lang::rewrites::userdefs::UserDefinition::element_def_with)
//!   builds with the feature's own expansion hook (an
//!   [`Expand`](flm_core::lang::rewrites::userdefs::Expand) implementation). Every
//!   element name begins with [`PREFIX`] ([`element_name`]).
//! - **Lowering rules.** For each element, `register` registers with
//!   [`FeatureBuilder::lowering_rule`](flm_core::core::registry::FeatureBuilder::lowering_rule)
//!   the rule that
//!   [`expand_in_tree_rule_for`](flm_core::stdcore::features::userdefs::expand_in_tree_rule_for)
//!   builds. The rule requires the
//!   [capability](flm_core::core::registry::CapabilityDef)
//!   [`ALWAYS_EXPANDED`](flm_core::stdcore::features::userdefs::ALWAYS_EXPANDED)
//!   of the `userdefs` feature, so every plan that lowers one of the macros runs
//!   the `userdefs` stage that replaces it by its template, and a registry
//!   without that feature fails to build. This requirement is the feature's
//!   only dependency on another feature: it refers to no backend, element,
//!   element type, table, service slot or scoped package that another feature
//!   owns, so it makes no peer declaration
//!   ([`PeerName`](flm_core::core::registry::PeerName)).
//! - **Placeholders computed by the feature.** The four placeholders `DelimLeft`,
//!   `DelimMiddle`, `DelimRight` and `config` are declared on each definition
//!   with
//!   [`Definition::extra_placeholder`](flm_core::lang::rewrites::userdefs::Definition::extra_placeholder).
//!   At each use, the feature's expansion hook gives them their content through
//!   placeholder bindings of its own
//!   ([`PlaceholderBindings`](flm_core::lang::rewrites::userdefs::PlaceholderBindings)),
//!   which pass every other placeholder to
//!   [`DefinitionBindings`](flm_core::lang::rewrites::userdefs::DefinitionBindings).
//! - **A build hook.** `register` registers a
//!   [`LateInit`](flm_core::core::registry::LateInit) hook with
//!   [`FeatureBuilder::late_init`](flm_core::core::registry::FeatureBuilder::late_init).
//!   Once the language is built, the hook compiles every macro's templates
//!   ([`UserDefinition::compile_with`](flm_core::lang::rewrites::userdefs::UserDefinition::compile_with)),
//!   then each delimiter at each size and each named value that the templates'
//!   placeholders refer to, through
//!   [`compile_in_modes`](flm_core::lang::rewrites::userdefs::compile_in_modes).
//!   A text that does not compile is reported among the registry's build
//!   diagnostics.
//! - **The registry digest.** `register` passes the configuration's bytes
//!   ([`PhfqitConfig::digest_bytes`](settings::PhfqitConfig::digest_bytes)) to
//!   [`FeatureBuilder::digest_input`](flm_core::core::registry::FeatureBuilder::digest_input),
//!   so that two registries built from different declared macros or named values
//!   have different [digests](flm_core::model::digest).
//! - **Settings.** The feature's settings declaration
//!   ([`SettingsDecl`](flm_core::core::settings::SettingsDecl)) reads the data
//!   type [`settings::PhfqitData`] and converts it to the configuration
//!   [`settings::PhfqitConfig`]
//!   ([`SettingsDecl::config_from`](flm_core::core::settings::SettingsDecl::config_from)),
//!   marks its four keys that hold maps of entries (`macro_definitions`,
//!   `spaceKets`, `spaceOKets`, `space`) as combined entry by entry when a
//!   program combines several sources of settings
//!   ([`SettingsDecl::merge_entries`](flm_core::core::settings::SettingsDecl::merge_entries)),
//!   and marks the templates and named values as FLM markup
//!   ([`SettingsDecl::markup`](flm_core::core::settings::SettingsDecl::markup)).
//! - **An argument parser of its own.** [`SizeToken`] reads a size written as one
//!   token after a backtick, which no standard argument reads. It implements
//!   techy's
//!   [`ArgumentParser`](flm_core::techy::core::constructs::ArgumentParser) and
//!   [`FlmArgumentParser`](flm_core::lang::mint::FlmArgumentParser).
//! - **Conditions.** [`BadSize`] and [`UnknownValue`] are the feature's
//!   conditions (the typed body of a diagnostic), each implementing techy's
//!   [`DiagnosticInfo`](flm_core::techy::error::DiagnosticInfo) with an
//!   identifier under `phfpack.phfqit.`.
//!
//! The guide *Extending FLM: features and packages* walks through the same crate in its section "An
//! extension crate, end to end".

mod feature;
mod macros;
pub mod settings;
mod sizing;
mod tables;

use std::sync::Arc;

use flm_core::core::registry::{FeatureChain, PackageEntry, PackageError, PackageFactory};
use flm_core::core::settings::PackageSettings;

pub use feature::{element_name, Phfqit, UnknownValue, FEATURE, PREFIX};
pub use sizing::{BadSize, SizeProblem, SizeToken, FIT, SIZE_COMMANDS};

/// The package's name, `phfpack`: the name by which a program looks the
/// package up in its catalog of packages (with the `flm` program, the name a
/// document lists under `use` in its front matter).
pub const PACKAGE: &str = "phfpack";

/// Returns the package `phfpack`, holding the feature `phfqit` built with the
/// configuration `config` (the macros declared in addition to the built-in
/// ones, and the named values).
///
/// The returned [`PackageEntry`] uses no other package: a program that
/// registers it must also register the standard package
/// ([`flm_core::stdcore::package`]), whose `userdefs` feature the macros need.
/// [`factory`] returns the form of the package that declares that dependency
/// and reads the configuration from settings.
///
/// The entry states the feature order `math-mathjax < phfqit` ([`ORDER`]), with
/// the origin `package phfpack`: where the feature `math-mathjax` (the
/// extension that processes mathematics with MathJax) is loaded, the macros of
/// `phfqit` that MathJax also defines, such as `\Im` and `\Re`, take
/// `phfqit`'s definitions. Where `math-mathjax` is not loaded, the chain says
/// nothing.
pub fn package(config: settings::PhfqitConfig) -> PackageEntry {
    let order = FeatureChain::parse(ORDER, ORIGIN).expect("the chain is well formed");
    PackageEntry::new(PACKAGE).feature(Arc::new(Phfqit::new(config))).feature_order(order)
}

/// The chain of the feature order that the package states, `math-mathjax <
/// phfqit`: `phfqit`'s definitions apply over those of `math-mathjax`.
pub const ORDER: &str = "math-mathjax < phfqit";

/// The origin of [`ORDER`], which a message about the feature order quotes:
/// `package phfpack`.
const ORIGIN: &str = "package phfpack";

/// Returns the factory of the package `phfpack`: the form in which a program
/// adds the package to its catalog of packages
/// ([`PackageCatalog::add_compiled`](flm_core::core::registry::PackageCatalog::add_compiled)),
/// which builds the package from the settings of the feature `phfqit`.
///
/// The [`PackageFactory`] declares that the package uses the standard package
/// (`stdcore`). Its build function receives the settings a program read for
/// the package's features ([`PackageSettings`]) and returns [`package`] with
/// the configuration read from the settings of `phfqit`, or with the default
/// configuration (the built-in macros and the default named values) when there
/// are none. When the settings switch `phfqit` off, the returned package holds
/// no feature.
///
/// # Errors
///
/// A [`PackageError`] when the build function fails for empty settings
/// ([`PackageFactory::new`] calls it once). This build function never fails, so
/// the error does not occur.
pub fn factory() -> Result<PackageFactory, PackageError> {
    let factory = PackageFactory::new(PACKAGE, |settings: &PackageSettings| {
        let config = settings.config::<settings::PhfqitConfig>(FEATURE).cloned().unwrap_or_default();
        let mut entry = package(config);
        entry.features.retain(|feature| !settings.is_off(&feature.info().name));
        Ok(entry)
    })?;
    Ok(factory.uses(flm_core::stdcore::PACKAGE))
}
