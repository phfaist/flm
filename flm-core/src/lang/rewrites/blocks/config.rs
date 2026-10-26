//! The options of text resolution: [`BlocksConfig`], which says which families
//! of typographic substitution the [block-structure pass](super) applies.
//!
//! The block-structure pass resolves the text of a tree once (see the
//! [module documentation](super)), and in the same scan applies the typographic
//! substitutions that the source spells out: `--` means an en dash in the same
//! way that `\emph{…}` means emphasis, so the substitution is a property of the
//! text and not of one output format. [`BlocksConfig`] holds one flag per family
//! of substitutions. Collapsing whitespace is not one of the families: it
//! always happens and has no flag.
//!
//! The `blocks` feature ([`crate::stdcore::features::blocks`]) takes a
//! [`BlocksConfig`] as its configuration, a value fixed when the registry is
//! built (see [settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//! The feature feeds the flags into the registry's [structural
//! digest](crate::model::digest)
//! ([`FlmRegistry::digest`](crate::core::registry::FlmRegistry::digest)), which
//! every cache key of the library includes, so that two registries that differ
//! in these options never share cached trees.

/// The options of text resolution in the [block-structure pass](super): one
/// flag per family of typographic substitutions. Collapsing whitespace always
/// happens and has no flag.
///
/// Every family is on by default, so that FLM source written with the usual TeX
/// spellings (`` `quoted' ``, `--`, `...`) is shown as its author meant it in
/// every output format. Switch a family off to have those spellings reach the
/// output unchanged.
///
/// A value is passed to [`structure`](super::structure), or to the `blocks`
/// feature's constructor
/// ([`Blocks::new`](crate::stdcore::features::blocks::Blocks::new)). Because
/// every field is public, a value can be built from the default with one change,
/// or from [`no_typography`](BlocksConfig::no_typography) with one family
/// switched on. In settings written as data ([`crate::core::settings`]), the
/// fields are the settings keys of the `blocks` feature, and a key left out
/// takes its default.
///
/// # Examples
///
/// ```
/// use flm_core::lang::rewrites::blocks::BlocksConfig;
///
/// let default = BlocksConfig::new();
/// assert!(default.any_typography());
///
/// // Everything but the dash ligatures.
/// let config = BlocksConfig { ligature_unicode_dashes: false, ..default };
/// assert!(config.any_typography());
/// assert!(!BlocksConfig::no_typography().any_typography());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BlocksConfig {
    /// Whether the straight double quote `"` and the straight single quote `'`
    /// become curly quotes, opening (`“`, `‘`) or closing (`”`, `’`) according
    /// to their position. When
    /// [`ligature_unicode_quotes`](BlocksConfig::ligature_unicode_quotes) is
    /// also on, the two-character ligature `''` is that family's, and this test
    /// applies to a single `'`.
    ///
    /// A quote opens at the start of a text run that no content precedes (a
    /// comment does not count, nor the spaces `~` and `\ `), and after
    /// whitespace, an opening bracket
    /// (`(`, `[`, `{`) or a dash (`-`, `–`, `—`). A single quote also opens
    /// before a digit (`x'9` becomes `x‘9`), except in a decade abbreviation:
    /// after a letter, a digit or an underscore, and before two digits and an
    /// `s` (`x'90s` becomes
    /// `x’90s`). A quote closes everywhere else: `don't` becomes `don’t`. The
    /// test looks at the node before the text run, so in `\(A_j\)'s` the quote
    /// after the formula closes.
    pub auto_unicode_quotes: bool,
    /// Whether the TeX quote ligatures apply: ``` `` ``` becomes `“`, `''`
    /// becomes `”`, and `` ` `` becomes `‘`. A single `'` becomes `’`, as in
    /// TeX, only when [`auto_unicode_quotes`](BlocksConfig::auto_unicode_quotes) is
    /// off; when both families are on, the position test of
    /// `auto_unicode_quotes` decides a single `'`.
    pub ligature_unicode_quotes: bool,
    /// Whether the TeX dash ligatures apply: `---` becomes an em dash `—` and
    /// `--` an en dash `–`.
    pub ligature_unicode_dashes: bool,
    /// Whether the ellipsis ligature applies: `...` becomes `…`.
    pub ligature_unicode_ellipses: bool,
}

impl BlocksConfig {
    /// The default options: every family of substitutions on. Returns the same
    /// value as [`BlocksConfig::default`].
    pub fn new() -> BlocksConfig {
        BlocksConfig::default()
    }

    /// The options with every family of substitutions off. Whitespace is still
    /// collapsed, which no option can switch off.
    pub fn no_typography() -> BlocksConfig {
        BlocksConfig {
            auto_unicode_quotes: false,
            ligature_unicode_quotes: false,
            ligature_unicode_dashes: false,
            ligature_unicode_ellipses: false,
        }
    }

    /// Whether at least one family of substitutions is on.
    pub fn any_typography(&self) -> bool {
        self.auto_unicode_quotes
            || self.ligature_unicode_quotes
            || self.ligature_unicode_dashes
            || self.ligature_unicode_ellipses
    }
}

impl Default for BlocksConfig {
    fn default() -> BlocksConfig {
        BlocksConfig {
            auto_unicode_quotes: true,
            ligature_unicode_quotes: true,
            ligature_unicode_dashes: true,
            ligature_unicode_ellipses: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_is_on_by_default() {
        assert!(BlocksConfig::default().any_typography());
        assert_eq!(BlocksConfig::new(), BlocksConfig::default());
    }

    #[test]
    fn no_typography_switches_every_family_off() {
        assert!(!BlocksConfig::no_typography().any_typography());
    }

    #[test]
    fn a_partial_configuration_fills_in_the_defaults() {
        let config: BlocksConfig =
            serde_json::from_str(r#"{"ligature_unicode_dashes": false}"#).unwrap();
        assert!(!config.ligature_unicode_dashes);
        assert!(config.ligature_unicode_quotes);
    }
}
