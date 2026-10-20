//! The configuration of the [`refs`](super) feature, [`RefsConfig`]: the
//! texts that join the items of a reference to several targets, with the
//! compilation of those texts when the registry is built and the joining of
//! the items at lowering.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

use techy::core::node::BuildId;

use crate::core::lowering::{LowerCx, LowerError, Lowerer};
use crate::core::registry::{LateInit, LateInitError};
use crate::lang::{BuiltLang, FlmDiagnostics};
use crate::stdcore::features::numbering::{CompiledJoin, JoinDecl, ListPiece, list_shape};

/// The configuration of the [`refs`](super) feature: the texts that join the
/// items of a reference to several targets.
///
/// A `\ref` with several targets is written as one list. Its first items are
/// the numbers of the targets the document numbered, one item per counter
/// style, each written by that style, the styles in the order the reference
/// first reaches them; then comes each target without a number, in the order
/// the reference wrote them. [`join`](RefsConfig::join) joins the items. It is
/// a [`JoinDecl`], the type a counter style declares its own joining texts
/// with, with the same keys, the same variables `${and}` and `${sep}` and the
/// same defaults, so that a caller changes the word "and" the same way in both
/// places. By default, two items read `Eq. (1) and Thm. 1.1`, and three read
/// `Eq. (1), Thm. 1.1, and A note`.
///
/// One item is written `one_pre`, the item, `one_post`. Two items are written
/// `pair_pre`, the first, `pair_mid`, the second, `pair_post`. Three or more
/// are written `list_pre`, the items with `list_mid` between them and
/// `list_midlast` before the last, then `list_post`. The range texts
/// (`range_pre`, `range_mid`, `range_pairmid`, `range_post`) and the `empty`
/// text never apply to a reference list, whose items are never ranges of
/// several items; they are still accepted, so that one [`JoinDecl`] can serve
/// a counter style and this list.
///
/// A feature's configuration is the plain Rust value the feature is
/// constructed from; it is fixed when the registry is built ([settings,
/// options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
/// Every text is parsed as FLM once, when the registry is built: a text that
/// does not parse, or that refers to a variable nothing defines, fails the
/// registry build, as a counter style's text does. Because the configuration
/// changes what the registry produces, the feature adds
/// [`digest_bytes`](RefsConfig::digest_bytes) to the registry digest.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::JoinDecl;
/// use flm_core::stdcore::features::refs::RefsConfig;
///
/// let mut german = JoinDecl::default();
/// german.and = Some("und".to_string());
/// let config = RefsConfig::new().join(german);
/// assert_eq!(config.join.and.as_deref(), Some("und"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct RefsConfig {
    /// The texts that join the items of a reference to several targets.
    /// Default: the defaults of [`JoinDecl`].
    pub join: JoinDecl,
}

impl RefsConfig {
    /// The default configuration: the default joining texts.
    pub fn new() -> RefsConfig {
        RefsConfig::default()
    }

    /// Builder: sets the texts that join the items of a reference to several
    /// targets.
    pub fn join(mut self, join: JoinDecl) -> RefsConfig {
        self.join = join;
        self
    }

    /// The bytes this configuration adds to the [registry
    /// digest](crate::model::digest): every joining text, each preceded by
    /// whether it is set and by its length, so that two different
    /// configurations never produce the same bytes.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.join.digest_into(&mut out);
        out
    }
}

impl Hash for RefsConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

/// The origin name under which the joining texts of a reference list are
/// parsed: what a diagnostic about one of those texts gives as its source.
const LIST_TEXT_ORIGIN: &str = "reference list joining text";

/// The joining texts of one registration of the feature, compiled once the
/// language is built and read by the `\ref` lowering rule.
#[derive(Default)]
pub(super) struct ListTexts {
    compiled: OnceLock<CompiledJoin>,
}

impl fmt::Debug for ListTexts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListTexts").field("compiled", &self.compiled.get().is_some()).finish()
    }
}

/// The build hook ([`LateInit`]) that compiles the configured joining texts
/// against the built language, in the same way as a counter style's joining
/// texts.
///
/// A text that is not valid FLM, or that parses with an error, fails the
/// registry build; the warnings that the compilation reports are added to the
/// build diagnostics.
pub(super) struct CompileList {
    pub(super) texts: Arc<ListTexts>,
    pub(super) decl: JoinDecl,
}

impl LateInit for CompileList {
    fn late_init(&self, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let mut warnings = Vec::new();
        let compiled = CompiledJoin::compile(&self.decl, LIST_TEXT_ORIGIN, lang, &mut warnings)
            .map_err(|error| {
                LateInitError::new(format!(
                    "a text that joins the targets of a reference to several labels (the \
                     configuration key `join` of the feature `refs`) is not valid: {error}"
                ))
            })?;
        for warning in warnings {
            diags.push(warning);
        }
        // The hook is registered once per registration of the feature, so the
        // cell is empty here; were it called twice, the first compilation
        // stays.
        let _ = self.texts.compiled.set(compiled);
        Ok(())
    }
}

/// Joins `items` into one list with the compiled texts of `texts`; see
/// [`RefsConfig`] for where each text is placed.
///
/// The joining texts are spliced as content and link to nothing; each item is
/// written unchanged, with the links it holds.
///
/// # Errors
///
/// Returns [`LowerError`] when a text could not be spliced, and when the texts
/// were never compiled, which means the rule is lowering in a registry whose
/// build did not run the feature's late-initialization hook — a fault of the
/// program rather than of a document.
pub(super) fn join_items(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    texts: &ListTexts,
    mut items: Vec<Vec<BuildId>>,
) -> Result<Vec<BuildId>, LowerError> {
    let Some(join) = texts.compiled.get() else {
        return Err(LowerError::structure(
            "the texts that join the targets of a reference were never compiled",
        ));
    };
    let mut out = Vec::new();
    for piece in list_shape(items.len()) {
        match piece {
            ListPiece::Text(which) => out.extend(lw.splice(cx, join.text(which))?),
            ListPiece::Item(index) => out.append(&mut items[index]),
        }
    }
    Ok(out)
}
