//! The parsing configuration a host sets before a language is built
//! (`flm_core::lang::FlmParsing`): comment recognition switched off, a comment
//! prefix other than the default, and the empty prefix that opens nothing.

mod common;

use flm_core::lang::{BuiltLang, LangInputs};

use common::{build, parse};

/// A language whose comment recognition is `comments` under `prefix`.
fn lang_with(comments: bool, prefix: &str) -> BuiltLang {
    let mut inputs = LangInputs::new();
    inputs.parsing.comments = comments;
    inputs.parsing.comment_prefix = prefix.to_string();
    build(inputs)
}

/// The number of comment nodes a parse of `text` holds.
fn comments_in(lang: &BuiltLang, text: &str) -> usize {
    let fragment = parse(lang, text);
    fragment.tree().root().descendants().filter(|node| node.is_comment()).count()
}

#[test]
fn with_comments_off_no_comment_is_recognized() {
    let lang = lang_with(false, "%%");
    assert_eq!(comments_in(&lang, "a %% b\n"), 0);
}

#[test]
fn a_chosen_comment_prefix_is_what_opens_a_comment() {
    let lang = lang_with(true, "#");
    assert_eq!(comments_in(&lang, "a # b\n"), 1);
    // A prefix the configuration did not choose opens nothing.
    assert_eq!(comments_in(&lang, "a @@ b\n"), 0);
}

#[test]
fn an_empty_comment_prefix_recognizes_no_comment() {
    let lang = lang_with(true, "");
    assert_eq!(comments_in(&lang, "a # b\n"), 0);
}
