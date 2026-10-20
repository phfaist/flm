//! The LaTeX family: the parts every LaTeX backend of the library shares.
//!
//! A backend is the registration bundle for one output format
//! ([`BackendDef`](crate::core::registry::BackendDef)); a LaTeX backend writes a
//! document as LaTeX. The library has one,
//! [`latex-recomposed`](crate::stdcore::backends::latex_recomposed), which writes
//! the document back as LaTeX source close to what an author would have typed,
//! together with a *style file*: a `.sty` file holding the definitions that
//! LaTeX source needs. This module holds everything that does not depend on one
//! particular LaTeX backend: the elements a lowering rule stages to express
//! LaTeX output, the escaping of document text, the sink through which an
//! emitter writes bytes, the LaTeX packages and definitions a document can
//! require, the style file built from those requirements, and the names under
//! which a document's own definitions are written. The `latex` feature
//! ([`LatexFamily`], a [`FlmFeature`](crate::core::registry::FlmFeature))
//! registers all of it once, so that a LaTeX backend consists only of its own
//! lowering rules, its emitter and its stages.
//!
//! # How the parts work together
//!
//! 1. A lowering rule stages (builds into the lowered tree;
//!    [`Lowerer`](crate::core::lowering::Lowerer)) the LaTeX construct elements
//!    of [`construct`], and records *requirements*: identifiers saying that its
//!    output needs a definition (the general notion is described at
//!    [`StyleDef`](crate::core::registry::StyleDef)). On LaTeX a requirement is
//!    either a **package request**, `flm.latex.pkg.<name>`, which asks for a
//!    LaTeX package ([`packages`]), or a **definition piece**,
//!    `flm.latex.def.<name>`, which asks for a unit of LaTeX definitions
//!    ([`LatexPreambleDef`]).
//! 2. The backend's emitter writes each lowered tree through a [`LatexSink`],
//!    which escapes document text ([`escape`]) and records what the escaped
//!    characters need in the preamble.
//! 3. The backend resolves every requirement through two service slots
//!    ([`services`](crate::core::services)): `flm.latex.packages` gives the
//!    package definitions of a package request, and `flm.latex.preamble` gives
//!    the definition piece of a piece identifier. It merges the package
//!    requests into one `\RequirePackage` line per package and LaTeX engine
//!    ([`merge_requests`](packages::merge_requests)) and writes the style file
//!    ([`style`]).
//!
//! The concepts overview introduces these terms under [LaTeX package requests,
//! definition pieces and
//! hooks](crate::guide::concepts_overview#latex-package-requests-definition-pieces-and-hooks);
//! the rendering guide describes the recomposed output under [the recomposed LaTeX
//! backend](crate::guide::rendering#the-recomposed-latex-backend).
//!
//! # Map of the module
//!
//! - [`construct`]: the four LaTeX construct elements (`flm.pres.latex.*`): a
//!   macro call, an environment, raw LaTeX text and a definition. A construct
//!   element is a presentation element that mirrors one construct of an
//!   output format, so that format-specific output is structured data in the
//!   lowered tree ([construct
//!   element](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//!   The module also holds [`LatexPoint`] and the placeholder rule.
//! - [`escape`]: how document text is escaped: the ten special ASCII
//!   characters, every non-ASCII character through the Unicode-to-LaTeX table
//!   of [`untechxt`] under one of two Unicode modes, the user's overrides, and
//!   the record ([`Declared`]) of what the style file must declare as a
//!   result.
//! - [`sink`]: the [`LatexSink`], the only interface through which a LaTeX
//!   emitter writes bytes, with separate methods for document text (escaped),
//!   byte-exact LaTeX (which only a raw construct or the document's own source
//!   can supply), verbatim content, and the emitter's own syntax.
//! - [`packages`]: package requests: [`LatexPackageDef`], the service slot
//!   `flm.latex.packages`, the standard package definitions, and the merge of
//!   every request for one package into one line per engine.
//! - [`preamble`]: definition pieces: [`LatexPreambleDef`], the service slot
//!   `flm.latex.preamble`, the family's own pieces, among them the LaTeX hooks
//!   `\flmHeading`, `\flmLabels`, `\flmRef` and `\flmHyperref`, and the rules
//!   every piece follows.
//! - [`style`]: the style file: [`assemble_style`] writes it from the merged
//!   package requests, the declared characters, the definition pieces, the
//!   definitions features write from their configuration, the recomposed user
//!   definitions and the LaTeX the author gives to be written as it is ([`ExtraPreamble`]); the file is named after a hash of its content.
//! - [`names`]: the names a kept user definition
//!   ([`kept`](crate::stdcore::features::userdefs::kept)) is written under
//!   (`\hello` becomes `\flmUDhello`, the environment `note` becomes
//!   `flmUEnote`), so that it never collides with a command that LaTeX, a
//!   LaTeX package or the style file relies on.
//! - [`mod@feature`]: the `latex` feature, which registers all of the above.
//!
//! The construct elements extend the presentation elements every backend
//! shares ([`crate::stdcore::backends::pres`]).

pub mod construct;
pub mod escape;
pub mod feature;
pub mod names;
pub mod packages;
pub mod preamble;
pub mod sink;
pub mod style;

pub use construct::{
    LatexArgShape, LatexDefinitionKind, LatexDefinitionPayload, LatexEnvironmentPayload,
    LatexMacroPayload, LatexPoint, LatexRawPayload, PLACEHOLDER_LATEX_RULE, PlaceholderLatexRule,
    placeholder_rule,
};
pub use escape::{Declared, EscapeConfig, UnicodeMode, escape_into, escape_url};
pub use feature::{FEATURE_NAME, LatexFamily, feature};
pub use names::{
    ENVIRONMENT_PREFIX, FAMILY_PREFIX, LatexDefinitionMode, MACRO_ENCODED_PREFIX, MACRO_PREFIX,
    USER_DEFINITION_PREFIX, is_family_name, mode_of_twin_name, recomposed_name,
};
pub use packages::{LatexPackageDef, PackageLine, PackageOptionConflict, PackageRequest};
pub use preamble::LatexPreambleDef;
pub use sink::{LatexRaw, LatexSink, RawCommentRefused, SinkOutput, comment_percent};
pub use style::{ExtraPreamble, RecomposedDefinition, assemble_style};
