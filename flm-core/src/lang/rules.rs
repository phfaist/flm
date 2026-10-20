//! The parsing settings of a document ([`FlmParsing`]) and their translation
//! into techy's tokenizer rules.
//!
//! techy's tokenizer reads its syntax from data carried by the parsing state:
//! what counts as whitespace, which delimiter pairs open a group, which
//! characters start a command, what starts a comment, which characters are
//! forbidden. Changing the syntax means changing that data.
//!
//! [`FlmParsing`] holds the three settings of that syntax that a document may
//! change: whether comments are recognized, what starts a comment, and whether
//! `$` delimits mathematics. The library reads no front matter and no
//! configuration file, so the embedding program sets them: in
//! [`LangInputs::parsing`](crate::lang::LangInputs::parsing) for the document,
//! and in [`IncludedFile::parsing`](crate::lang::IncludedFile::parsing) for a
//! file included with `\input`. (The `flm` program reads them from the
//! `flm.parsing` key of a file's front matter.)
//!
//! When [the language is built](crate::lang::build),
//! [`FlmParsing::token_rules_overrides`] translates the document's settings into
//! changes to techy's standard tokenizer rules, applied to the initial parsing
//! state. For an included file with settings of its own,
//! [`FlmParsing::state_delta`] gives the change applied where the file is
//! included. Every parsing state also records the settings it was derived from
//! ([`FlmStateExt::parsing`](crate::lang::FlmStateExt::parsing)).
//!
//! Because the settings decide how a source is read, they are one of the inputs
//! of the [registry digest](crate::model::digest), the number that summarizes
//! everything about a registry that can change the result of processing. Data
//! computed under one parsing configuration is therefore never read back under
//! another.

use std::sync::Arc;

use techy::core::{ParsingState, ParsingStateDelta};
use techy::serialize::{DeserializableValue, SerializableValue};
use techy::core::token::{
    CommentOverrides, CommentRule, ForbiddenCharsOverrides, GroupOverrides, GroupRule,
    TokenRules, TokenRulesOverrides,
};

use crate::lang::flm::Flm;

/// The parsing settings of a document or of an included file: three settings
/// about how the source is split into tokens.
///
/// The settings are whether comments are recognized, what starts a comment,
/// and whether `$` delimits mathematics. Written as data (the type implements
/// serde's traits), the settings are a map from the field names to their
/// values, and a missing field takes its default; the `flm` program reads that
/// map from the `flm.parsing` key of a file's front matter.
///
/// [`FlmParsing::token_rules_overrides`] states what each setting changes.
/// A built language takes the document's settings from
/// [`LangInputs::parsing`](crate::lang::LangInputs::parsing); an included file's
/// own settings come from
/// [`IncludedFile::parsing`](crate::lang::IncludedFile::parsing).
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
    SerializableValue, DeserializableValue,
)]
#[serde(default)]
#[schemars(deny_unknown_fields)]
pub struct FlmParsing {
    /// Whether comments are recognized at all. Default: `true`.
    #[serial(name = "comments")]
    pub comments: bool,
    /// What starts a comment. Default: `%%` (two characters, unlike LaTeX's
    /// single `%`). An empty prefix recognizes no comment.
    ///
    /// This setting also decides whether a lone `%` is a forbidden character:
    /// see [`FlmParsing::percent_forbidden`].
    #[serial(name = "comment_prefix")]
    pub comment_prefix: String,
    /// Whether `$…$` and `$$…$$` delimit mathematics. Default: `false`, in which
    /// case a `$` in the source is reported as
    /// [`DollarMathDisabled`](crate::lang::DollarMathDisabled). `\(…\)` and
    /// `\[…\]` delimit mathematics either way.
    #[serial(name = "dollar_math")]
    pub dollar_math: bool,
}

impl Default for FlmParsing {
    fn default() -> FlmParsing {
        FlmParsing { comments: true, comment_prefix: String::from("%%"), dollar_math: false }
    }
}

impl FlmParsing {
    /// Returns the changes these settings make to `base`, the standard
    /// tokenizer rules of the language's initial parsing state (techy's
    /// `ParsingState::lang_initial`).
    ///
    /// [`build_language`](crate::lang::build_language) applies the result once,
    /// to the parsing state every parse of the built language starts from. There are three
    /// changes:
    ///
    /// - The comment rules are replaced by the single rule that starts with
    ///   [`comment_prefix`](Self::comment_prefix), and comment recognition is
    ///   switched on or off as [`comments`](Self::comments) says. An empty
    ///   prefix leaves no comment rule at all.
    /// - When [`dollar_math`](Self::dollar_math) is off, the group rules whose
    ///   opening delimiter starts with `$` are removed and `$` is added to the
    ///   forbidden characters, so that a stray `$` is reported instead of being
    ///   read as text.
    /// - When [`percent_forbidden`](Self::percent_forbidden) returns `true`, `%`
    ///   is added to the forbidden characters for the same reason.
    ///
    /// A *forbidden character* is a character that the tokenizer reports where it
    /// occurs as ordinary text; the character is kept in the text as written, and
    /// the parse continues. The
    /// tokenizer checks for a forbidden character only after it has failed to
    /// read a command, a comment and a special character sequence at that
    /// position, so forbidding `%` leaves `\%` and the comment prefix `%%`
    /// readable.
    pub fn token_rules_overrides(&self, base: &TokenRules<Flm>) -> TokenRulesOverrides<Flm> {
        let mut overrides = TokenRulesOverrides::<Flm>::default();
        let comment_rules = if self.comment_prefix.is_empty() {
            Vec::new()
        } else {
            vec![Arc::new(CommentRule { start: self.comment_prefix.clone() })]
        };
        overrides.comments =
            CommentOverrides { enabled: Some(self.comments), rules: Some(comment_rules) };
        let mut forbidden = String::from(base.forbidden_chars());
        if !self.dollar_math {
            let kept: Vec<Arc<GroupRule<Flm>>> = base
                .group_rules()
                .iter()
                .filter(|rule| !is_dollar_rule(rule))
                .map(Arc::clone)
                .collect();
            overrides.groups = GroupOverrides { rules: Some(kept), ..GroupOverrides::default() };
            if !forbidden.contains('$') {
                forbidden.push('$');
            }
        }
        // `%` needs no counterpart to `is_dollar_rule`: no group rule and no
        // command rule is spelled with `%`, and the one comment rule is already
        // replaced wholesale by `comment_prefix` above, so forbidding the
        // character is the whole change.
        if self.percent_forbidden() && !forbidden.contains('%') {
            forbidden.push('%');
        }
        if forbidden != base.forbidden_chars() {
            overrides.forbidden_chars = ForbiddenCharsOverrides { chars: Some(forbidden.into()) };
        }
        overrides
    }

    /// Returns the change of parsing state that makes these settings apply to
    /// any state derived from the language's initial parsing state (techy's
    /// `ParsingState::lang_initial`), regardless
    /// of the settings that state was derived under.
    ///
    /// The change holds the rules that
    /// [`token_rules_overrides`](Self::token_rules_overrides) computes against
    /// the initial state's rules, with the group rules and the forbidden
    /// characters always stated in full, so that nothing of the previous
    /// settings remains. The parser of `\input` applies it where an included
    /// file with settings of its own is read ([`crate::lang::input`]).
    pub fn state_delta(&self) -> ParsingStateDelta<Flm> {
        // FLM's seed cannot fail (its data are in-crate constants).
        let seed = ParsingState::<Flm>::lang_initial()
            .expect("the initial parsing state is built from constants and cannot fail");
        let base = seed.rules();
        let mut overrides = self.token_rules_overrides(base);
        if overrides.groups.rules.is_none() {
            overrides.groups.rules = Some(base.group_rules().iter().map(Arc::clone).collect());
        }
        if overrides.forbidden_chars.chars.is_none() {
            overrides.forbidden_chars =
                ForbiddenCharsOverrides { chars: Some(String::from(base.forbidden_chars()).into()) };
        }
        ParsingStateDelta::new().rules(overrides)
    }

    /// Returns whether a lone `%` is a forbidden character under these
    /// settings.
    ///
    /// It is, unless a lone `%` starts a comment, that is, unless
    /// [`comments`](Self::comments) is on *and*
    /// [`comment_prefix`](Self::comment_prefix) is exactly `"%"`. With the
    /// default prefix `%%`, and with comments switched off, `%` is forbidden.
    ///
    /// A lone `%` is forbidden because it is ambiguous. In `add 10% to the
    /// total` it is a percent sign; in `% a note to myself` it is a LaTeX
    /// comment. Either guess would silently produce the wrong result for an
    /// author who meant the other, so the language refuses the character and
    /// asks for an unambiguous spelling: `\%` for the percent sign, the comment
    /// prefix for a comment. Where a lone `%` starts a comment, there is no
    /// ambiguity, and the character is not forbidden.
    ///
    /// A forbidden `%` in the source is reported with the condition
    /// [`PercentForbidden`](crate::lang::PercentForbidden).
    pub fn percent_forbidden(&self) -> bool {
        !(self.comments && self.comment_prefix == "%")
    }

    /// Returns what starts a comment under these settings: the comment prefix,
    /// or `None` when no comment is recognized at all, either because
    /// [`comments`](Self::comments) is off or because
    /// [`comment_prefix`](Self::comment_prefix) is empty.
    ///
    /// The diagnostic for a forbidden `%`
    /// ([`PercentForbidden`](crate::lang::PercentForbidden)) suggests this
    /// prefix as the way to write a comment.
    pub fn comment_start(&self) -> Option<&str> {
        if self.comments && !self.comment_prefix.is_empty() {
            Some(&self.comment_prefix)
        } else {
            None
        }
    }
}

/// Returns whether `rule` is a group rule whose opening delimiter starts with
/// `$` (the mathematics delimiters `$…$` and `$$…$$`): the rules that
/// [`FlmParsing::token_rules_overrides`] removes when `$` is not a mathematics
/// delimiter.
pub fn is_dollar_rule(rule: &GroupRule<Flm>) -> bool {
    rule.open.starts_with('$')
}

#[cfg(test)]
mod tests {
    use super::*;
    use techy::core::{ParsingState, ParsingStateDelta};

    fn derived(parsing: &FlmParsing) -> ParsingState<Flm> {
        let seed = ParsingState::<Flm>::lang_initial().expect("seed state");
        let overrides = parsing.token_rules_overrides(seed.rules());
        seed.derived(&ParsingStateDelta::new().rules(overrides)).unwrap()
    }

    #[test]
    fn the_defaults_are_the_seed_keys() {
        let parsing = FlmParsing::default();
        assert!(parsing.comments);
        assert_eq!(parsing.comment_prefix, "%%");
        assert!(!parsing.dollar_math);
    }

    #[test]
    fn dollar_math_off_removes_the_dollar_rules_and_forbids_the_character() {
        let state = derived(&FlmParsing::default());
        assert!(state.rules().group_rules().iter().all(|rule| !is_dollar_rule(rule)));
        assert!(state.rules().group_rules().iter().any(|rule| rule.open == "\\("));
        assert!(state.rules().forbidden_chars().contains('$'));
    }

    #[test]
    fn dollar_math_on_keeps_the_preset_rules() {
        let state = derived(&FlmParsing { dollar_math: true, ..FlmParsing::default() });
        assert!(state.rules().group_rules().iter().any(|rule| rule.open == "$$"));
        assert!(!state.rules().forbidden_chars().contains('$'));
    }

    #[test]
    fn a_lone_percent_is_forbidden_unless_it_starts_a_comment() {
        // The default prefix is `%%`, so a lone `%` is ambiguous and forbidden.
        let state = derived(&FlmParsing::default());
        assert!(state.rules().forbidden_chars().contains('%'));
        assert!(FlmParsing::default().percent_forbidden());
        assert_eq!(FlmParsing::default().comment_start(), Some("%%"));

        // A lone `%` starts a comment: nothing is ambiguous, nothing is forbidden.
        let latexlike =
            FlmParsing { comment_prefix: String::from("%"), ..FlmParsing::default() };
        assert!(!latexlike.percent_forbidden());
        assert!(!derived(&latexlike).rules().forbidden_chars().contains('%'));

        // Comments off: `%` means nothing at all, and stays forbidden.
        let no_comments = FlmParsing { comments: false, ..FlmParsing::default() };
        assert!(no_comments.percent_forbidden());
        assert_eq!(no_comments.comment_start(), None);
        assert!(derived(&no_comments).rules().forbidden_chars().contains('%'));

        // An empty prefix recognizes no comment either.
        let empty = FlmParsing { comment_prefix: String::new(), ..FlmParsing::default() };
        assert!(empty.percent_forbidden());
        assert_eq!(empty.comment_start(), None);
    }

    #[test]
    fn the_comment_prefix_is_the_one_comment_rule() {
        let state = derived(&FlmParsing::default());
        let starts: Vec<&str> =
            state.rules().comment_rules().iter().map(|rule| rule.start.as_str()).collect();
        assert_eq!(starts, ["%%"]);
        let off = derived(&FlmParsing { comments: false, ..FlmParsing::default() });
        assert!(!off.rules().comments_enabled());
    }
}
