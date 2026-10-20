//! The names a kept user definition is written under in the style file and
//! in the recomposed LaTeX: each is renamed under a prefix of its own
//! (`\hello` becomes `\flmUDhello`), so that it never collides with a command
//! that LaTeX, a LaTeX package or the style file relies on.
//!
//! A kept user definition
//! ([`kept`](crate::stdcore::features::userdefs::kept)) is a user definition
//! whose use sites a plan left in the tree (the `flm.userdefs.kept`
//! alternative); the `latex-recomposed` backend
//! writes the definition into the style file and its use sites as calls.
//! Written under the author's own name, such a definition would be made
//! unconditionally (`\long\def`, `\DeclareDocumentEnvironment`) and so
//! replace any definition that LaTeX or a package makes under that name. Some
//! of those replacements break the recomposed LaTeX itself (`\par`, `\label`,
//! the commands a character declaration in the same style file relies on) and
//! are very hard to debug. Rather than keep a list of dangerous names, the
//! library renames **every** definition ([`recomposed_name`]):
//!
//! - a macro whose name is ASCII letters only becomes
//!   [`MACRO_PREFIX`]`<name>`: `\hello` is written `\flmUDhello`;
//! - a macro whose name holds anything else (a control symbol such as `~`, a
//!   non-ASCII letter, a digit) becomes [`MACRO_ENCODED_PREFIX`] followed by
//!   an encoding of the name in ASCII letters (see [`recomposed_name`]), since
//!   a LaTeX control word may hold letters only: `\~` is written `\flmUXhE`;
//! - an environment becomes [`ENVIRONMENT_PREFIX`]`<name>`, its name kept
//!   exactly as written regardless of the characters it holds:
//!   `\begin{note}` is written `\begin{flmUEnote}`. An environment name is
//!   only ever looked up by `\begin{…}` and `\end{…}` and never written as a
//!   control word, so it needs no encoding.
//!
//! Macros and environments take different prefixes because LaTeX keeps them
//! in one name space: `\newenvironment{foo}` defines `\foo`, so a macro `foo`
//! and an environment `foo` written under one prefix would collide.
//!
//! # Per-mode definitions
//!
//! A document may declare one name twice, once for text mode and once for
//! mathematics mode, with different templates and even different arguments;
//! LaTeX has only one command of that name. These *per-mode definitions*
//! therefore take two names, and the recomposed LaTeX calls the one that the
//! mode of the use site requires. The rule: **the prefix of a
//! per-mode definition is the prefix above with the mode's letter inserted
//! after the `U`**, `T` for text mode and `M` for mathematics mode
//! ([`LatexDefinitionMode`]). So a document declaring `\vec` twice gets
//! `\flmUTDvec` and `\flmUMDvec`; a `\~` declared twice gets `\flmUTXhE` and
//! `\flmUMXhE`; an environment `note` declared twice gets `flmUTEnote` and
//! `flmUMEnote`. A name the document declares once keeps the plain prefix,
//! regardless of the modes it applies in.
//!
//! No two definitions can collide under this rule, because the letter after
//! `flmU` says which of five classes a name belongs to (`D`, `X` and `E` for a
//! name declared once, `T` and `M` for a per-mode definition) and the rest of
//! the name is the definition's own. A document macro named `Tvec` is
//! `\flmUDTvec`, never `\flmUTDvec`. The `latex-recomposed` backend chooses
//! between the two per-mode definitions, never LaTeX: an `\ifmmode` test written into the
//! output would be evaluated when a heading's title is *written to the table
//! of contents*, which is outside mathematics regardless of the title's
//! content, and would give the wrong answer there.
//!
//! # The `flmU` name space belongs to the document
//!
//! **Every name beginning with `flmU` is left to the definitions a document
//! declares, and nothing this library writes may take a name there.** Every
//! prefix above begins with [`USER_DEFINITION_PREFIX`], the four characters
//! `flmU`, and the rule is stated over those four characters rather than over
//! the prefixes: a name beginning with them may be the name of a definition
//! that some document declares (a document that declares a macro `blank` has
//! `\flmUDblank`), so a helper of this library written there would collide
//! with it. Because the rule covers the whole space, the per-mode prefixes,
//! and any prefix added later under `flmU`, fit in without changing it.
//!
//! The LaTeX family's own helpers take names of the form `\flm<Name>`
//! (`\flmHeading`, `\flmRef`, `\flmIfBlankTF`). One class of them belongs to
//! a feature: `\flmXN<kind>` is the command an xnote of the kind `<kind>` is
//! written as ([`xnotes::latex`](crate::stdcore::features::xnotes::latex)).
//! A feature that contributes a definition piece of its own must follow the
//! same rule. One name currently breaks it: the `href` feature's LaTeX hook
//! `\flmUrl` begins with `flmU`
//! ([`href::latex`](crate::stdcore::features::href::latex)). It cannot
//! collide with a recomposed name, because the letter after `flmU` in it
//! (`r`) is none of the five class letters (`D`, `X`, `E`, `T`, `M`).
//!
//! The style file also writes the declarations that the package chunks of
//! untechxt's Unicode-to-LaTeX table need
//! ([`chunk_pieces`](super::preamble::chunk_pieces)), whose commands and font
//! identifiers carry untechxt's own prefix, `UnxT`. That name space belongs to
//! untechxt: this library defines no name there and renames nothing the table
//! declares.
//!
//! # Where the names are used, and how to keep the author's own name
//!
//! The renaming applies to the definition in the style file and to every call
//! the recomposed LaTeX writes, in text and in mathematics alike, so the
//! definition keeps its form and only its name changes. The style file gives
//! the author's own name in the comment line above each definition.
//!
//! An author who *wants* LaTeX's own command replaced lists the name in the
//! render option
//! [`bare_names`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::bare_names):
//! the definition is then written under the name as the document spelled it,
//! and the author is responsible for the consequences for the whole document.
//! Some names cannot be listed there and are refused with the Error
//! [`DefinitionNameRefused`](crate::stdcore::backends::latex_recomposed::DefinitionNameRefused)
//! ([`DefinitionNameRefusal`](crate::stdcore::backends::latex_recomposed::DefinitionNameRefusal)
//! lists the reasons), among them a name beginning with [`FAMILY_PREFIX`],
//! the only name space this library owns, and a name the document declares
//! once per mode, since one name cannot hold both per-mode definitions.

use super::construct::LatexDefinitionKind;

/// The prefix `flm`, the only name space this library owns: every command
/// and environment that the definition pieces written in [`super::preamble`]
/// and the features' LaTeX hooks define begins with it. A name beginning with
/// it cannot be listed in the render option `bare_names` (see
/// [`is_family_name`]).
pub const FAMILY_PREFIX: &str = "flm";

/// The four characters every prefix of a recomposed user definition begins
/// with, `flmU`: the whole name space this library leaves to the definitions a
/// document declares (see the [module documentation](self)). Nothing this
/// library's own LaTeX defines may take a name beginning with it; the `href`
/// feature's LaTeX hook `\flmUrl` currently breaks this rule (see the
/// [module documentation](self#the-flmu-name-space-belongs-to-the-document)).
pub const USER_DEFINITION_PREFIX: &str = "flmU";

/// The prefix a recomposed macro whose name is ASCII letters only is
/// written under, `flmUD`: the user definition `\hello` becomes
/// `\flmUDhello`.
pub const MACRO_PREFIX: &str = "flmUD";

/// The prefix a recomposed macro whose name holds anything but ASCII
/// letters is written under, `flmUX`, followed by the letters-only encoding
/// of the name that [`recomposed_name`] describes: the user definition
/// `\~` becomes `\flmUXhE`.
pub const MACRO_ENCODED_PREFIX: &str = "flmUX";

/// The prefix a recomposed environment is written under, `flmUE`, followed
/// by the name exactly as the document spelled it: the user environment
/// `note` becomes `flmUEnote`, written `\begin{flmUEnote}`.
pub const ENVIRONMENT_PREFIX: &str = "flmUE";

/// Which of a name's definitions a recomposed name is wanted for.
///
/// A document may declare one name twice, once for text mode and once for
/// mathematics mode ([per-mode definitions](self#per-mode-definitions)); the
/// two are written under two names, and this value says which one is meant.
/// Every other definition (one the document declares once, regardless of the
/// parsing modes it applies in) is [`Single`](LatexDefinitionMode::Single).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LatexDefinitionMode {
    /// The document declares this name once: the plain prefix.
    Single,
    /// The text-mode definition of a name the document declares once per
    /// mode: the letter `T` after `flmU`.
    Text,
    /// The mathematics-mode definition of a name the document declares once
    /// per mode: the letter `M` after `flmU`.
    Mathematics,
}

impl LatexDefinitionMode {
    /// The letter this mode inserts after the `U` of a prefix, or `None` for
    /// [`Single`](LatexDefinitionMode::Single), which inserts nothing.
    fn marker(self) -> Option<char> {
        match self {
            LatexDefinitionMode::Single => None,
            LatexDefinitionMode::Text => Some('T'),
            LatexDefinitionMode::Mathematics => Some('M'),
        }
    }
}

/// How a definition of `kind` named `name` is written in `mode`: the prefix it
/// takes — one of the three prefixes above, with the mode's letter inserted
/// after the four characters [`USER_DEFINITION_PREFIX`] for a per-mode
/// definition (see the [module documentation](self)) — and whether the name
/// itself is written encoded as ASCII letters ([`encoded_name`]).
///
/// One function decides both because they are one classification: a
/// macro whose name is not ASCII letters only takes the encoded prefix *and*
/// has its name encoded, and nothing else does. Deciding it in two places
/// could let the two part company, which would give a plain prefix an encoded
/// name or the reverse.
fn prefix_of(kind: LatexDefinitionKind, name: &str, mode: LatexDefinitionMode) -> (String, bool) {
    let (single, encoded) = match kind {
        // A declaration is always a feature's, never a user definition's, so
        // it never reaches this function; the arm is here for the match to
        // be complete.
        LatexDefinitionKind::Environment | LatexDefinitionKind::Declaration => {
            (ENVIRONMENT_PREFIX, false)
        }
        // The emptiness test is what keeps an empty name out of the plain
        // branch, where it would give the bare prefix `\flmUD`. No definition
        // has an empty name, so the encoded branch takes it.
        LatexDefinitionKind::Macro
            if !name.is_empty() && name.chars().all(|ch| ch.is_ascii_alphabetic()) =>
        {
            (MACRO_PREFIX, false)
        }
        LatexDefinitionKind::Macro => (MACRO_ENCODED_PREFIX, true),
    };
    let prefix = match mode.marker() {
        None => single.to_string(),
        Some(letter) => {
            let rest = &single[USER_DEFINITION_PREFIX.len()..];
            format!("{USER_DEFINITION_PREFIX}{letter}{rest}")
        }
    };
    (prefix, encoded)
}

/// The mode letter of a per-mode definition's name: the mode that `name` (a
/// recomposed macro or environment name without a backslash) is written for,
/// or `None` where it is not the name of a per-mode definition at all (the
/// name of a definition the document declares once, or a name outside the
/// `flmU` name space).
///
/// The letter right after [`USER_DEFINITION_PREFIX`] classifies every name in
/// that space. The classification is **closed**: the five letters `D`, `X`
/// and `E` (a definition the document declares once) and `T` and `M` (a
/// per-mode definition), and no other (see the [module
/// documentation](self#per-mode-definitions)). A later class of recomposed
/// name must add a letter of its own rather than reuse one of the five.
///
/// [`Single`](LatexDefinitionMode::Single) is never returned: a name under a
/// plain prefix is not a per-mode name, which is `None`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::names::{LatexDefinitionMode, mode_of_twin_name};
///
/// assert_eq!(mode_of_twin_name("flmUTDvec"), Some(LatexDefinitionMode::Text));
/// assert_eq!(mode_of_twin_name("flmUMEnote"), Some(LatexDefinitionMode::Mathematics));
/// assert_eq!(mode_of_twin_name("flmUDvec"), None);
/// // A document macro named `Tvec` is `flmUDTvec`, which is not a per-mode name.
/// assert_eq!(mode_of_twin_name("flmUDTvec"), None);
/// assert_eq!(mode_of_twin_name("emph"), None);
/// ```
pub fn mode_of_twin_name(name: &str) -> Option<LatexDefinitionMode> {
    match name.strip_prefix(USER_DEFINITION_PREFIX)?.chars().next()? {
        'T' => Some(LatexDefinitionMode::Text),
        'M' => Some(LatexDefinitionMode::Mathematics),
        _ => None,
    }
}

/// Whether `name` (a macro or environment name without a backslash) is in
/// the name space this library's own LaTeX output owns, that is, whether it
/// begins with [`FAMILY_PREFIX`].
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::names::is_family_name;
///
/// assert!(is_family_name("flmHeading"));
/// assert!(!is_family_name("emph"));
/// ```
pub fn is_family_name(name: &str) -> bool {
    name.starts_with(FAMILY_PREFIX)
}

/// One character of a name, encoded as ASCII letters: its code point in
/// uppercase hexadecimal without leading zeros, with the decimal digits `0`
/// to `9` written as the letters `a` to `j` and the letters `A` to `F` kept
/// as they are.
fn encoded_char(ch: char) -> String {
    format!("{:X}", ch as u32)
        .chars()
        .map(|digit| match digit {
            '0'..='9' => char::from(b'a' + (digit as u8 - b'0')),
            other => other,
        })
        .collect()
}

/// A whole name encoded as ASCII letters: each character through
/// [`encoded_char`], the characters joined by the letter `x`.
fn encoded_name(name: &str) -> String {
    name.chars().map(encoded_char).collect::<Vec<String>>().join("x")
}

/// The name, without a backslash, under which a user definition of `kind`
/// named `name` is recomposed in `mode`. It is the only implementation of the
/// naming rule, used for the definition in the style file and for every call
/// the `latex-recomposed` output writes.
///
/// `mode` says which of a name's definitions this one is
/// ([`LatexDefinitionMode`]): [`Single`](LatexDefinitionMode::Single) for a
/// name the document declares once, and [`Text`](LatexDefinitionMode::Text) or
/// [`Mathematics`](LatexDefinitionMode::Mathematics) for one of the two
/// per-mode definitions of a name it declares once per mode.
///
/// `bare_names` is the render option
/// [`bare_names`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::bare_names):
/// a name listed there is returned unchanged, so that the definition
/// replaces LaTeX's own command of that name. Two kinds of name are never
/// returned unchanged, regardless of the option: a name beginning with
/// [`FAMILY_PREFIX`], the only name space this library owns, and the name of
/// a per-mode definition, since the document's two definitions of it would
/// then be one LaTeX command. This function renames both; the backend reports
/// the refused entry of the option as an Error of its own
/// ([`DefinitionNameRefused`](crate::stdcore::backends::latex_recomposed::DefinitionNameRefused)).
///
/// Otherwise the name is the prefix of its kind followed by the name, with the
/// mode's letter inserted after [`USER_DEFINITION_PREFIX`] for a per-mode
/// definition (see the [module documentation](self#per-mode-definitions)):
///
/// - an environment is [`ENVIRONMENT_PREFIX`] and `name` exactly as written;
/// - a macro whose name is ASCII letters only is [`MACRO_PREFIX`] and `name`;
/// - any other macro is [`MACRO_ENCODED_PREFIX`] and the name encoded as ASCII
///   letters: each character's
///   code point in uppercase hexadecimal without leading zeros, with the
///   decimal digits `0`–`9` written as the letters `a`–`j` and the letters
///   `A`–`F` kept, the characters joined by the letter `x`. So `~` (U+007E,
///   hexadecimal `7E`) encodes as `hE`, and `foo2` encodes as `ggxgFxgFxdc`.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeSet;
///
/// use flm_core::stdcore::backends::latex::LatexDefinitionKind;
/// use flm_core::stdcore::backends::latex::names::{LatexDefinitionMode, recomposed_name};
///
/// let none = BTreeSet::new();
/// let one = LatexDefinitionMode::Single;
/// assert_eq!(recomposed_name(LatexDefinitionKind::Macro, "hello", one, &none), "flmUDhello");
/// assert_eq!(recomposed_name(LatexDefinitionKind::Macro, "~", one, &none), "flmUXhE");
/// assert_eq!(
///     recomposed_name(LatexDefinitionKind::Environment, "note", one, &none),
///     "flmUEnote"
/// );
///
/// // A name the document declares twice, once per parsing mode.
/// let text = LatexDefinitionMode::Text;
/// let math = LatexDefinitionMode::Mathematics;
/// assert_eq!(recomposed_name(LatexDefinitionKind::Macro, "vec", text, &none), "flmUTDvec");
/// assert_eq!(recomposed_name(LatexDefinitionKind::Macro, "vec", math, &none), "flmUMDvec");
///
/// let bare: BTreeSet<String> = [String::from("emph")].into_iter().collect();
/// assert_eq!(recomposed_name(LatexDefinitionKind::Macro, "emph", one, &bare), "emph");
/// ```
pub fn recomposed_name(
    kind: LatexDefinitionKind,
    name: &str,
    mode: LatexDefinitionMode,
    bare_names: &std::collections::BTreeSet<String>,
) -> String {
    if mode == LatexDefinitionMode::Single
        && !is_family_name(name)
        && bare_names.contains(name)
    {
        return name.to_string();
    }
    let (prefix, encoded) = prefix_of(kind, name, mode);
    if encoded {
        format!("{prefix}{}", encoded_name(name))
    } else {
        format!("{prefix}{name}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn nothing_bare() -> BTreeSet<String> {
        BTreeSet::new()
    }

    /// The name of a definition the document declares once.
    fn single(kind: LatexDefinitionKind, name: &str, bare: &BTreeSet<String>) -> String {
        recomposed_name(kind, name, LatexDefinitionMode::Single, bare)
    }

    /// The nine prefixes: the three classes of name in the three modes.
    fn every_prefix() -> Vec<String> {
        let mut prefixes = Vec::new();
        for mode in
            [LatexDefinitionMode::Single, LatexDefinitionMode::Text, LatexDefinitionMode::Mathematics]
        {
            prefixes.push(prefix_of(LatexDefinitionKind::Macro, "hello", mode).0);
            prefixes.push(prefix_of(LatexDefinitionKind::Macro, "~", mode).0);
            prefixes.push(prefix_of(LatexDefinitionKind::Environment, "note", mode).0);
        }
        prefixes
    }

    #[test]
    fn every_prefix_lies_in_the_name_space_the_document_owns() {
        // The invariant is one rule over four characters: every prefix a
        // recomposed definition takes begins with them, and no piece of the
        // family's own LaTeX may.
        for prefix in every_prefix() {
            assert!(prefix.starts_with(USER_DEFINITION_PREFIX), "{prefix}");
        }
        for prefix in [MACRO_PREFIX, MACRO_ENCODED_PREFIX, ENVIRONMENT_PREFIX] {
            assert!(prefix.starts_with(USER_DEFINITION_PREFIX), "{prefix}");
        }
        assert!(USER_DEFINITION_PREFIX.starts_with(FAMILY_PREFIX));
    }

    #[test]
    fn the_nine_prefixes_are_distinct_and_none_is_the_start_of_another() {
        // No two definitions can meet: the letter after `flmU` says which of
        // the five classes the name belongs to, so a prefix is never the start
        // of another prefix and a name under one is never a name under another.
        let prefixes = every_prefix();
        let distinct: BTreeSet<&String> = prefixes.iter().collect();
        assert_eq!(distinct.len(), 9, "{prefixes:?}");
        for one in &prefixes {
            for other in &prefixes {
                if one != other {
                    assert!(!one.starts_with(other.as_str()), "`{one}` starts with `{other}`");
                }
            }
        }
    }

    #[test]
    fn a_twin_takes_the_prefix_with_its_modes_letter_after_the_u() {
        let bare = nothing_bare();
        let text = LatexDefinitionMode::Text;
        let math = LatexDefinitionMode::Mathematics;
        assert_eq!(recomposed_name(LatexDefinitionKind::Macro, "vec", text, &bare), "flmUTDvec");
        assert_eq!(recomposed_name(LatexDefinitionKind::Macro, "vec", math, &bare), "flmUMDvec");
        // An encoded name is encoded the same way under either prefix.
        assert_eq!(recomposed_name(LatexDefinitionKind::Macro, "~", text, &bare), "flmUTXhE");
        assert_eq!(recomposed_name(LatexDefinitionKind::Macro, "~", math, &bare), "flmUMXhE");
        // An environment keeps its name as written under either prefix.
        assert_eq!(
            recomposed_name(LatexDefinitionKind::Environment, "note", text, &bare),
            "flmUTEnote"
        );
        assert_eq!(
            recomposed_name(LatexDefinitionKind::Environment, "note", math, &bare),
            "flmUMEnote"
        );
    }

    #[test]
    fn a_twins_name_can_never_be_a_name_the_document_declares_once() {
        // The collision the rule is written to prevent: a document macro named
        // `Tvec` takes the plain prefix, and the text twin of `vec` takes the
        // twin prefix, so the two are different commands.
        let bare = nothing_bare();
        assert_eq!(single(LatexDefinitionKind::Macro, "Tvec", &bare), "flmUDTvec");
        assert_ne!(
            single(LatexDefinitionKind::Macro, "Tvec", &bare),
            recomposed_name(LatexDefinitionKind::Macro, "vec", LatexDefinitionMode::Text, &bare)
        );
        assert_eq!(single(LatexDefinitionKind::Macro, "Mvec", &bare), "flmUDMvec");
        assert_ne!(
            single(LatexDefinitionKind::Macro, "Mvec", &bare),
            recomposed_name(
                LatexDefinitionKind::Macro,
                "vec",
                LatexDefinitionMode::Mathematics,
                &bare
            )
        );
    }

    #[test]
    fn a_twin_is_never_written_bare() {
        // Two definitions written under one bare name would fight over it, so
        // the option is ignored for a twin; the backend reports the entry as
        // refused.
        let bare: BTreeSet<String> = [String::from("vec")].into_iter().collect();
        assert_eq!(single(LatexDefinitionKind::Macro, "vec", &bare), "vec");
        assert_eq!(
            recomposed_name(LatexDefinitionKind::Macro, "vec", LatexDefinitionMode::Text, &bare),
            "flmUTDvec"
        );
        assert_eq!(
            recomposed_name(
                LatexDefinitionKind::Macro,
                "vec",
                LatexDefinitionMode::Mathematics,
                &bare
            ),
            "flmUMDvec"
        );
    }

    #[test]
    fn a_macro_of_ascii_letters_takes_the_plain_prefix() {
        let bare = nothing_bare();
        assert_eq!(single(LatexDefinitionKind::Macro, "hello", &bare), "flmUDhello");
        assert_eq!(single(LatexDefinitionKind::Macro, "par", &bare), "flmUDpar");
        assert_eq!(single(LatexDefinitionKind::Macro, "emph", &bare), "flmUDemph");
        // A name this library's own output writes is renamed like any other.
        assert_eq!(single(LatexDefinitionKind::Macro, "flmHeading", &bare), "flmUDflmHeading");
    }

    #[test]
    fn a_macro_named_with_a_control_symbol_is_encoded() {
        let bare = nothing_bare();
        // `~` is U+007E: hexadecimal 7E, the digit 7 written as the letter h.
        assert_eq!(single(LatexDefinitionKind::Macro, "~", &bare), "flmUXhE");
        // `@` is U+0040: hexadecimal 40, written `ea`.
        assert_eq!(single(LatexDefinitionKind::Macro, "@", &bare), "flmUXea");
    }

    #[test]
    fn a_macro_named_with_a_non_ascii_character_is_encoded() {
        let bare = nothing_bare();
        // `é` is U+00E9: hexadecimal E9, the digit 9 written as the letter j.
        assert_eq!(single(LatexDefinitionKind::Macro, "é", &bare), "flmUXEj");
        // `→` is U+2192: hexadecimal 2192, written `cbjc`.
        assert_eq!(single(LatexDefinitionKind::Macro, "→", &bare), "flmUXcbjc");
    }

    #[test]
    fn a_mixed_macro_name_encodes_every_character() {
        let bare = nothing_bare();
        // f = U+0066 → `gg`, o = U+006F → `gF`, 2 = U+0032 → `dc`.
        assert_eq!(single(LatexDefinitionKind::Macro, "foo2", &bare), "flmUXggxgFxgFxdc");
        assert_eq!(single(LatexDefinitionKind::Macro, "a1", &bare), "flmUXgbxdb");
    }

    #[test]
    fn every_encoded_macro_name_is_ascii_letters_only() {
        let bare = nothing_bare();
        for mode in
            [LatexDefinitionMode::Single, LatexDefinitionMode::Text, LatexDefinitionMode::Mathematics]
        {
            for name in ["~", "@", "é", "foo2", "a1", "→", "x_y"] {
                let recomposed = recomposed_name(LatexDefinitionKind::Macro, name, mode, &bare);
                assert!(
                    recomposed.chars().all(|ch| ch.is_ascii_alphabetic()),
                    "`{recomposed}` is not a LaTeX control word"
                );
            }
        }
    }

    #[test]
    fn an_environment_keeps_its_name_whatever_it_holds() {
        let bare = nothing_bare();
        assert_eq!(single(LatexDefinitionKind::Environment, "note", &bare), "flmUEnote");
        assert_eq!(single(LatexDefinitionKind::Environment, "n2", &bare), "flmUEn2");
        assert_eq!(single(LatexDefinitionKind::Environment, "café", &bare), "flmUEcafé");
    }

    #[test]
    fn a_macro_and_an_environment_of_one_name_take_different_prefixes() {
        let bare = nothing_bare();
        assert_ne!(
            single(LatexDefinitionKind::Macro, "note", &bare),
            single(LatexDefinitionKind::Environment, "note", &bare)
        );
    }

    #[test]
    fn a_listed_name_is_written_bare_unless_it_is_the_librarys_own() {
        let bare: BTreeSet<String> =
            [String::from("emph"), String::from("note"), String::from("flmHeading")]
                .into_iter()
                .collect();
        assert_eq!(single(LatexDefinitionKind::Macro, "emph", &bare), "emph");
        assert_eq!(single(LatexDefinitionKind::Environment, "note", &bare), "note");
        // Listing a name of this library's own space changes nothing here;
        // the backend refuses the entry with an error.
        assert_eq!(single(LatexDefinitionKind::Macro, "flmHeading", &bare), "flmUDflmHeading");
        // A name not listed is renamed as usual.
        assert_eq!(single(LatexDefinitionKind::Macro, "hello", &bare), "flmUDhello");
    }
}
