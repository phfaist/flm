//! The converter's definitions and the commands the feature declares in math
//! mode: what a formula may hold.
//!
//! techxt's definitions library is a stack of categories, later ones
//! shadowing earlier ones (`techxt::defs::standard`). The first seven render
//! notation; the others render a document (headings, lists, references,
//! citations, links, …). The converter is built from the seven notation
//! categories alone ([`definitions`]), so nothing of the document library
//! exists in it.
//!
//! The feature declares in math mode, by name alone (techxt reads the
//! arguments), the macros and environments of that set that a formula may
//! use ([`Vocabulary::of`]):
//!
//! - every macro and environment of the categories [`WHOLE_CATEGORIES`] that
//!   is visible in math mode (in every mode, or in math mode only);
//! - of the category `base`, only the entries that mathematics uses
//!   ([`BASE_ADMITTED`]);
//! - except the names that techxt's whole library (`techxt::defs::standard`)
//!   attributes to another category than the converter's set does:
//!   `symbols_extra`, generated from pylatexenc's tables, holds a placeholder
//!   under some names of the document library (`\eqref`, `\ref`, `\cite`,
//!   `\section`, `\newcommand`, …), which the whole library replaces with a
//!   document category's definition;
//! - and except the names that the `math` feature gives a meaning of its own
//!   ([`is_math_feature_name`]): its text commands and `\ensuremath`, which
//!   it declares in math mode, where a second declaration of the name would
//!   make the registry build fail; and the tag, the no-number mark and the
//!   label of a display-math environment's body, which are its numbering and
//!   its labels.
//!
//! The line break `\\` is declared as techxt's other macros are, so that a
//! matrix, `cases`, `aligned`, `split` or `array` written in a math group holds
//! its line ends. The `math` feature declares its own `\\` in the body of a
//! display-math environment alone, where it ends the equation line and
//! resolves before the `\\` that the feature `math-techxt` declares.
//!
//! techxt's specials (`&`, `^`, `_`, `~`) are characters of the formula, not
//! commands, and are not declared; `\&` and `\_` are commands that write the
//! characters `&` and `_`. A command of techxt's document library, such as
//! `\eqref` or `\cite`, is an unknown command inside a formula.

use std::collections::BTreeMap;

use flm_ext::ExtError;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::lang::mint::{BodyDef, BodyLayout, ElementDef, Mode, SyntaxDef};
use flm_ext::guest::model::element::ElementSpec;
use flm_ext::guest::stdlib::features::math::{
    ENSUREMATH, NONUMBER_COMMAND, TAG_COMMAND, TEXT_COMMANDS,
};
use serde_json::json;
use techxt::convert::Latexlike;
use techxt::def::{CallableKind, DefinitionSet, ModeVisibility};
use techxt::defs;

use crate::math_techxt::{ENVIRONMENT, MACRO};

/// The converter's definitions: techxt's seven notation categories, stacked
/// in the order of `techxt::defs::standard`, later categories shadowing
/// earlier ones.
pub fn definitions() -> DefinitionSet<Latexlike> {
    DefinitionSet::new()
        .with(defs::symbols_extra::category())
        .with(defs::base::category())
        .with(defs::accents::category())
        .with(defs::fontstyles::category())
        .with(defs::mathcore::category())
        .with(defs::mathenvs::category())
        .with(defs::subsuperscripts::category())
}

/// The categories whose every macro and environment visible in math mode the
/// feature declares. Of the other two notation categories, `base` is read
/// through [`BASE_ADMITTED`] and `subsuperscripts` holds specials only.
pub const WHOLE_CATEGORIES: [&str; 5] = ["symbols_extra", "accents", "fontstyles", "mathcore", "mathenvs"];

/// The macros of the category `base` that the feature declares, each without
/// its backslash.
///
/// This is the one list of names written by hand. It exists because `base`
/// mixes the commands of mathematics with those of running text (`\par`,
/// `\footnote`, `\newpage`, the counters, the text symbols), and techxt's
/// table records no difference between them: every entry is visible in every
/// mode. Kept here: the line break `\\`, the escaped characters `\{`, `\}`,
/// `\$`, `\&`, `\#`, `\_`, `\%`, the spacing commands, the
/// decorations (bars, arrows, harpoons, braces, groups, brackets and segments
/// over or under their argument), the phantoms, `\smash` and `\boxed`.
pub const BASE_ADMITTED: &[&str] = &[
    // The line break, which ends a row of a matrix or an alignment.
    "\\",
    // The escaped characters `\{`, `\}`, `\$`, `\&`, `\#`, `\_`, `\%`: inside
    // a formula each is the only way to write its character.
    "{",
    "}",
    "$",
    "&",
    "#",
    "_",
    "%",
    // Spacing: `\,`, `\;`, `\:`, `\!`, the control space, `\quad`, `\qquad`.
    ",",
    ";",
    ":",
    "!",
    " ",
    "quad",
    "qquad",
    // Decorations.
    "overline",
    "underline",
    "widehat",
    "widetilde",
    "wideparen",
    "overleftarrow",
    "overrightarrow",
    "overleftrightarrow",
    "underleftarrow",
    "underrightarrow",
    "underleftrightarrow",
    "overleftharpoon",
    "overrightharpoon",
    "overbrace",
    "underbrace",
    "overgroup",
    "undergroup",
    "overbracket",
    "underbracket",
    "overlinesegment",
    "underlinesegment",
    // Phantoms, `\smash` and `\boxed`.
    "phantom",
    "hphantom",
    "vphantom",
    "smash",
    "boxed",
];

/// The label command of an environment's body, which the `math` feature
/// declares there beside its tag and no-number commands.
const LABEL_COMMAND: &str = "label";

/// Whether the `math` feature gives the macro `name` a meaning of its own: a
/// text command and `\ensuremath`, which it declares in math mode, and the
/// tag, the no-number mark and the label of an environment's body, which are
/// its numbering and its labels. Its line break is not one of them: `\\` is
/// also techxt's notation for the end of a row of a matrix or an alignment,
/// and is neither numbering nor a label.
pub fn is_math_feature_name(name: &str) -> bool {
    TEXT_COMMANDS.contains(&name) || [TAG_COMMAND, NONUMBER_COMMAND, LABEL_COMMAND, ENSUREMATH].contains(&name)
}

/// The commands the feature declares in math mode: the macros and the
/// environments, each grouped under the category techxt resolves its name to,
/// the categories and the names in their sorted order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vocabulary {
    /// The macros' names, without the backslash, by category.
    pub macros: BTreeMap<String, Vec<String>>,
    /// The environments' names, by category.
    pub environments: BTreeMap<String, Vec<String>>,
}

impl Vocabulary {
    /// The commands of `set`, the converter's definitions, that a formula may
    /// use, read from its table (`DefinitionSet::symbols`), as the module
    /// documentation states them.
    ///
    /// `library` is techxt's whole library (`techxt::defs::standard`). An
    /// entry of `set` is left out when `library`'s table attributes the same
    /// macro or environment to another category: `symbols_extra` holds a
    /// placeholder under some names of the document library (`\eqref`,
    /// `\ref`, `\cite`, `\section`, `\newcommand`, …), which a document
    /// category of the whole library replaces with its own definition, later
    /// categories shadowing earlier ones.
    pub fn of(set: &DefinitionSet<Latexlike>, library: &DefinitionSet<Latexlike>) -> Vocabulary {
        let library = library.symbols();
        let mut vocabulary = Vocabulary::default();
        for entry in set.symbols().entries() {
            if library.get(entry.kind, entry.name).is_some_and(|whole| whole.category != entry.category) {
                continue;
            }
            let in_math = matches!(entry.modes, ModeVisibility::Anywhere | ModeVisibility::MathOnly);
            let admitted = if entry.category == "base" {
                entry.kind == CallableKind::Macro && BASE_ADMITTED.contains(&entry.name)
            } else {
                WHOLE_CATEGORIES.contains(&entry.category)
            };
            if !in_math || !admitted {
                continue;
            }
            let names = match entry.kind {
                CallableKind::Macro if !is_math_feature_name(entry.name) => &mut vocabulary.macros,
                CallableKind::Environment => &mut vocabulary.environments,
                _ => continue,
            };
            names.entry(entry.category.to_string()).or_default().push(entry.name.to_string());
        }
        vocabulary
    }

    /// Declares the commands on `b`, for the feature `owner`: per category,
    /// the macros as one declaration of the element [`MACRO`] with the macro
    /// syntax and no arguments, and the environments as one declaration of
    /// the element [`ENVIRONMENT`] with the environment syntax, no arguments
    /// and a body read inline in math mode; each in math mode only, its
    /// element carrying the category as its data (`{"category": …}`).
    ///
    /// # Errors
    ///
    /// The host's refusal; a name that another feature declares in math mode
    /// makes the registry build fail with the registry's error for two
    /// declarations of one command.
    pub fn declare(&self, b: &mut FeatureBuilder, owner: &str) -> Result<(), ExtError> {
        for (category, names) in &self.macros {
            let docs = format!("A macro of techxt's definitions `{category}`, rendered by techxt inside a formula.");
            let element = ElementSpec::new(MACRO).with_value(json!({ "category": category })).docs(docs.as_str());
            let syntax = SyntaxDef::Macro { args: Vec::new() };
            b.element(ElementDef::new(owner, names, syntax, element).modes([Mode::Math])?.docs(docs)?)?;
        }
        let body = BodyDef::new().layout(BodyLayout::Inline).mode(Mode::Math);
        for (category, names) in &self.environments {
            let docs =
                format!("An environment of techxt's definitions `{category}`, rendered by techxt inside a formula.");
            let element = ElementSpec::new(ENVIRONMENT).with_value(json!({ "category": category })).docs(docs.as_str());
            let syntax = SyntaxDef::Environment { args: Vec::new(), body: body.clone() };
            b.element(ElementDef::new(owner, names, syntax, element).modes([Mode::Math])?.docs(docs)?)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `declared`, names by category, holds `name`.
    fn holds(declared: &BTreeMap<String, Vec<String>>, name: &str) -> bool {
        declared.values().any(|names| names.iter().any(|declared| declared == name))
    }

    #[test]
    fn every_admitted_entry_of_base_is_a_macro_of_base() {
        let set = definitions();
        let symbols = set.symbols();
        for name in BASE_ADMITTED {
            let entry = symbols.get(CallableKind::Macro, name).unwrap_or_else(|| panic!("`{name}` is defined"));
            assert_eq!(entry.category, "base", "{name}");
        }
    }

    #[test]
    fn the_vocabulary_holds_the_notation_and_nothing_of_the_document_library() {
        let vocabulary = Vocabulary::of(&definitions(), &defs::standard());
        for name in ["frac", "alpha", "mathbb", "hat", "quad", "overline", ",", "notag", "\\", "{", "}", "$", "&", "#", "_", "%"] {
            assert!(holds(&vocabulary.macros, name), "{name}");
        }
        assert!(holds(&vocabulary.environments, "pmatrix"));
        // The document library's names, which `symbols_extra` also holds.
        for name in ["eqref", "ref", "cite", "section", "item", "newcommand", "DeclareMathOperator"] {
            assert!(definitions().symbols().get(CallableKind::Macro, name).is_some(), "{name}");
            assert!(!holds(&vocabulary.macros, name), "{name}");
        }
        for name in ["footnote", "label", "par", "text", "mbox", "ensuremath", "nonumber"] {
            assert!(!holds(&vocabulary.macros, name), "{name}");
        }
        assert!(!holds(&vocabulary.environments, "itemize"));
        // No category of the document library.
        let categories: Vec<&String> = vocabulary.macros.keys().chain(vocabulary.environments.keys()).collect();
        for category in categories {
            assert!(category == "base" || WHOLE_CATEGORIES.contains(&category.as_str()), "{category}");
        }
    }
}
