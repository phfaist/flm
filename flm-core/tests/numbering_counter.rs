//! The formatting library of the numbering feature: counter values, the number
//! formats, the two template languages, compiling a counter style, and writing
//! one value and several.
//!
//! The cases here are the port of the reference implementation's own suite for
//! this area, plus the checks the port owes on top of it: no input may make a
//! format loop, allocate without bound, or panic.
//!
//! What a style writes is content, not text, so each case reads its answer back
//! as the FLM source the content recomposes to. That is what makes the answers
//! comparable with the reference implementation's, which are strings.

use std::sync::Arc;

use flm_core::lang::{build_language, BuiltLang, LangInputs};
use flm_core::model::names::Name;
use flm_core::stdcore::backends::pres::read::recompose_source;
use flm_core::stdcore::features::numbering::{
    format, format_many, identifier_text, Ancestor, CounterStyle, CounterStyleDecl, CounterValue,
    Delimiters, FormatError, FormatOptions, JoinDecl, NumberFormat, NumberTemplate,
    NumberedValue, Parent,
    parts, CompiledWithinJoiner, JoinCx, Part, PluralWords, PrefixWords, Segment, StyleError,
    SubLevelDecl, Within, WithinCx, WithinJoiner,
};

// ============================================================================
// helpers
// ============================================================================

/// The language every style here is compiled against: FLM's own structure and
/// nothing a feature declares, which is all a counter style's texts need.
fn lang() -> BuiltLang {
    build_language(LangInputs::new())
}

/// `decl` compiled; a compilation error is a test failure.
fn compiled(decl: CounterStyleDecl, lang: &BuiltLang) -> CounterStyle {
    decl.compile(lang).expect("the style compiles")
}

/// The FLM source the segments write, one after the other.
fn written(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|segment| {
            let nodes: Vec<_> = segment.content().root().children().iter().collect();
            recompose_source(&nodes)
        })
        .collect()
}

/// The FLM source the segments write, with every linked segment wrapped in
/// `\mylink{<key>}{…}`; `key` answers the key of the value at a position.
fn linked(segments: &[Segment], key: impl Fn(usize) -> String) -> String {
    let mut out = String::new();
    for segment in segments {
        let nodes: Vec<_> = segment.content().root().children().iter().collect();
        let text = recompose_source(&nodes);
        match segment.link() {
            Some(position) => {
                out.push_str(&format!("\\mylink{{{}}}{{{text}}}", key(position)));
            }
            None => out.push_str(&text),
        }
    }
    out
}

/// A style writing the number in capital letters: the counter an ancestor part
/// is taken from in the cases below, so that the ancestor value 1 writes `A`.
fn outer_style(lang: &BuiltLang) -> Arc<CounterStyle> {
    let mut decl = CounterStyleDecl::new("test.style.outer");
    decl.number_format = NumberTemplate::formats("#{Alph}");
    Arc::new(compiled(decl, lang))
}

/// The ancestors of a value numbered within the counter `outer` at `value`.
fn outer_at(value: i64, style: &Arc<CounterStyle>) -> Vec<Ancestor> {
    vec![Ancestor::new("outer", CounterValue::new(value), Arc::clone(style))]
}

/// The joiner writing the counter `outer` followed by `separator`.
fn outer_joiner(separator: &str, lang: &BuiltLang) -> Arc<dyn CompiledWithinJoiner> {
    Within::parent("outer")
        .joined_by(separator)
        .compile(lang, &[Name::new("outer")], &[])
        .expect("the joiner compiles")
}

/// The joiner writing the within template `text`, which may name `counters`.
fn within_template(
    text: &str,
    counters: &[&str],
    lang: &BuiltLang,
) -> Arc<dyn CompiledWithinJoiner> {
    let names: Vec<Name> = counters.iter().map(|name| Name::new(*name)).collect();
    Within::parent(counters.first().copied().unwrap_or("outer"))
        .template(text)
        .compile(lang, &names, &[])
        .expect("the joiner compiles")
}

/// The style of the reference implementation's `test_join` and the three cases
/// that follow it: roman numerals, `eq.~`/`eqs.~` with a `capital` variant,
/// spaced delimiters, and a join specification that names every text.
fn join_style(lang: &BuiltLang, repeat_parents_in_range: bool) -> CounterStyle {
    let mut decl = CounterStyleDecl::new("test.style.join");
    decl.number_format = NumberTemplate::formats("#{Roman}");
    decl.prefix_words = PrefixWords::new("eq.~", "eqs.~")
        .with_variant("capital", PluralWords::new("Equation~", "Equations~"));
    decl.delimiters = Delimiters::new("!<! ", " !>!");
    decl.join = spelled_join();
    decl.name_in_link = true;
    decl.repeat_parents_in_range = repeat_parents_in_range;
    compiled(decl, lang)
}

/// The join specification of the reference implementation's `test_join`: every
/// text a distinctive mark, so that each one's place is visible in the answer.
fn spelled_join() -> JoinDecl {
    JoinDecl {
        one_pre: Some("<".to_string()),
        one_post: Some(">".to_string()),
        pair_pre: Some("((".to_string()),
        pair_mid: Some("||".to_string()),
        pair_post: Some("))".to_string()),
        range_pre: Some("[[".to_string()),
        range_mid: Some("--".to_string()),
        range_pairmid: Some("|".to_string()),
        range_post: Some("]]".to_string()),
        list_pre: Some("<<".to_string()),
        list_mid: Some(";".to_string()),
        list_midlast: Some(";&".to_string()),
        list_post: Some(">>".to_string()),
        ..JoinDecl::default()
    }
}

/// The values, as [`format_many`] takes them: each numbered within nothing.
fn alone(values: &[CounterValue]) -> Vec<NumberedValue<'_>> {
    values.iter().map(NumberedValue::new).collect()
}

/// The values `numbers` as counter values without sub-numbers.
fn values(numbers: impl IntoIterator<Item = i64>) -> Vec<CounterValue> {
    numbers.into_iter().map(CounterValue::new).collect()
}

// ============================================================================
// the number formats
// ============================================================================

#[test]
fn letters_count_by_repeating_a_letter() {
    let alph = NumberFormat::Letters;
    assert_eq!(alph.text(1), "a");
    assert_eq!(alph.text(2), "b");
    assert_eq!(alph.text(16), "p");
    assert_eq!(alph.text(26), "z");
}

#[test]
fn letters_repeat_rather_than_carry() {
    let alph = NumberFormat::Letters;
    assert_eq!(alph.text(27), "aa");
    assert_eq!(alph.text(28), "bb");
    assert_eq!(alph.text(52), "zz");
    assert_eq!(alph.text(53), "aaa");
}

#[test]
fn capital_letters_are_the_letters_in_uppercase() {
    let alph = NumberFormat::CapitalLetters;
    assert_eq!(alph.text(1), "A");
    assert_eq!(alph.text(16), "P");
    assert_eq!(alph.text(26), "Z");
    assert_eq!(alph.text(27), "AA");
    assert_eq!(alph.text(28), "BB");
}

#[test]
fn roman_numerals_are_written_by_subtraction() {
    let roman = NumberFormat::Roman;
    assert_eq!(roman.text(1), "i");
    assert_eq!(roman.text(2), "ii");
    assert_eq!(roman.text(4), "iv");
    assert_eq!(roman.text(5), "v");
    assert_eq!(roman.text(9), "ix");
    assert_eq!(roman.text(49), "xlix");
    assert_eq!(roman.text(2099), "mmxcix");
}

#[test]
fn capital_roman_numerals_are_the_roman_numerals_in_uppercase() {
    let roman = NumberFormat::CapitalRoman;
    assert_eq!(roman.text(1), "I");
    assert_eq!(roman.text(4), "IV");
    assert_eq!(roman.text(5), "V");
    assert_eq!(roman.text(9), "IX");
    assert_eq!(roman.text(49), "XLIX");
    assert_eq!(roman.text(2099), "MMXCIX");
}

#[test]
fn the_footnote_symbols_double_and_treble() {
    let symbols = NumberFormat::Symbols;
    assert_eq!(symbols.text(1), "*");
    assert_eq!(symbols.text(2), "\u{2020}");
    assert_eq!(symbols.text(3), "\u{2021}");
    assert_eq!(symbols.text(4), "\u{00a7}");
    assert_eq!(symbols.text(5), "\u{00b6}");
    assert_eq!(symbols.text(6), "\u{2016}");
    assert_eq!(symbols.text(7), "**");
    assert_eq!(symbols.text(12), "\u{2016}\u{2016}");
    assert_eq!(symbols.text(13), "***");
}

#[test]
fn the_superscript_and_subscript_formats_count_in_base_ten() {
    assert_eq!(NumberFormat::Superscript.text(17), "\u{00b9}\u{2077}");
    assert_eq!(NumberFormat::Subscript.text(17), "\u{2081}\u{2087}");
    assert_eq!(NumberFormat::Superscript.text(0), "");
    assert_eq!(NumberFormat::Subscript.text(0), "");
}

#[test]
fn arabic_writes_the_digits_of_every_number() {
    let arabic = NumberFormat::Arabic;
    assert_eq!(arabic.text(1), "1");
    assert_eq!(arabic.text(10), "10");
    assert_eq!(arabic.text(124), "124");
    assert_eq!(arabic.text(255), "255");
    assert_eq!(arabic.text(0), "0");
    assert_eq!(arabic.text(-992), "-992");
}

#[test]
fn every_format_but_arabic_writes_nothing_for_zero_and_for_a_negative_number() {
    for format in NumberFormat::ALL {
        if format == NumberFormat::Arabic {
            continue;
        }
        for number in [0, -1, -26, -27, -992, i64::MIN] {
            assert_eq!(format.text(number), "", "{format} of {number}");
        }
    }
}

#[test]
fn no_number_makes_a_format_loop_or_grow_without_bound() {
    for format in NumberFormat::ALL {
        for number in [1, 1_000, 1_000_000, i64::MAX, i64::MIN + 1] {
            let text = format.text(number);
            assert!(
                text.chars().count() <= NumberFormat::MAX_CHARS,
                "{format} of {number} wrote {} characters",
                text.chars().count()
            );
        }
    }
}

// ============================================================================
// counter values
// ============================================================================

#[test]
fn a_value_is_a_number_and_its_sub_numbers() {
    let plain = CounterValue::new(1);
    assert_eq!(plain.components(), [1]);
    assert_eq!(plain.number(), 1);
    assert_eq!(plain.sub_numbers(), [] as [i64; 0]);

    let nested = CounterValue::with_sub_numbers(1, [2, 3]);
    assert_eq!(nested.components(), [1, 2, 3]);
    assert_eq!(nested.number(), 1);
    assert_eq!(nested.sub_numbers(), [2, 3]);

    assert_eq!(CounterValue::with_sub_numbers(1, []).components(), [1]);
    assert_eq!(CounterValue::with_sub_numbers(1, [0]).components(), [1, 0]);
    assert_eq!(CounterValue::with_sub_numbers(3, [2, 1]).number(), 3);
    assert_eq!(CounterValue::with_sub_numbers(3, [2, 1]).sub_numbers(), [2, 1]);
}

#[test]
fn two_values_are_equal_when_their_components_are() {
    let value = CounterValue::with_sub_numbers(3, [2, 1]);
    assert_eq!(value, CounterValue::with_sub_numbers(3, [2, 1]));
    assert_ne!(value, CounterValue::with_sub_numbers(3, [2, 2]));
    assert_ne!(CounterValue::new(3), CounterValue::new(4));
}

#[test]
fn a_value_identifies_a_point_by_its_components_joined_by_hyphens() {
    assert_eq!(identifier_text(&[], &CounterValue::new(1)), "1");
    assert_eq!(identifier_text(&[], &CounterValue::with_sub_numbers(1, [])), "1");
    assert_eq!(identifier_text(&[], &CounterValue::with_sub_numbers(1, [0])), "1-0");
    assert_eq!(identifier_text(&[], &CounterValue::with_sub_numbers(1, [2, 3])), "1-2-3");
}

#[test]
fn an_identifier_puts_the_ancestors_values_first() {
    let lang = lang();
    let style = outer_style(&lang);
    let ancestors = outer_at(2, &style);
    assert_eq!(identifier_text(&ancestors, &CounterValue::new(3)), "2-3");
    assert_eq!(identifier_text(&ancestors, &CounterValue::with_sub_numbers(1, [2])), "2-1-2");
}

#[test]
fn a_value_immediately_succeeds_the_one_below_it_at_the_same_depth() {
    let succeeds = |a: CounterValue, b: CounterValue| a.immediately_succeeds(&b);
    assert!(succeeds(CounterValue::new(1), CounterValue::new(0)));
    assert!(succeeds(CounterValue::new(124), CounterValue::new(123)));
    assert!(!succeeds(CounterValue::new(1), CounterValue::new(1)));
    assert!(!succeeds(CounterValue::new(3), CounterValue::new(1)));
    assert!(!succeeds(CounterValue::with_sub_numbers(2, [1]), CounterValue::new(1)));
    assert!(!succeeds(CounterValue::new(2), CounterValue::with_sub_numbers(1, [9])));
    assert!(succeeds(
        CounterValue::with_sub_numbers(2, [3, 4]),
        CounterValue::with_sub_numbers(2, [3, 3])
    ));
    assert!(!succeeds(
        CounterValue::with_sub_numbers(1, [1]),
        CounterValue::with_sub_numbers(1, [1])
    ));
    assert!(!succeeds(
        CounterValue::with_sub_numbers(1, [1, 2, 1]),
        CounterValue::with_sub_numbers(1, [1, 2])
    ));
}

#[test]
fn advancing_a_level_zeroes_the_deeper_ones() {
    let value = CounterValue::with_sub_numbers(3, [2, 1]);
    assert_eq!(value.advanced(2).components(), [3, 2, 2]);
    assert_eq!(value.advanced(0).components(), [4, 0, 0]);
    assert_eq!(value.advanced(1).components(), [3, 3, 0]);
}

#[test]
fn advancing_a_level_the_value_has_not_got_creates_it() {
    assert_eq!(CounterValue::new(3).advanced(1).components(), [3, 1]);
    assert_eq!(CounterValue::new(3).advanced(2).components(), [3, 0, 1]);
}

// ============================================================================
// the template languages
// ============================================================================

#[test]
fn a_template_writes_the_number_in_every_format_it_names() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.template");
    decl.number_format = NumberTemplate::formats("#{arabic}.");
    let style = compiled(decl, &lang);
    let bare = FormatOptions::new();
    let segments = format(&CounterValue::new(5), &style, None, &bare).expect("written");
    assert_eq!(written(&segments), "5.");
}

#[test]
fn a_template_naming_one_format_writes_that_format_alone() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.named");
    decl.number_format = NumberTemplate::formats("#{arabic}");
    let style = compiled(decl, &lang);
    let segments =
        format(&CounterValue::new(5), &style, None, &FormatOptions::new()).expect("written");
    assert_eq!(written(&segments), "5");
}

#[test]
fn a_template_naming_a_format_that_does_not_exist_is_refused() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.bad");
    decl.number_format = NumberTemplate::formats("#{nonexistent}");
    match decl.compile(&lang) {
        Err(StyleError::Template { text, .. }) => assert_eq!(text, "#{nonexistent}"),
        other => panic!("expected a template that does not compile, got {other:?}"),
    }
}

#[test]
fn a_tag_template_replaces_the_first_of_the_five_characters() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.tag");
    decl.number_format = NumberTemplate::initial("(a)");
    let style = compiled(decl, &lang);
    let write = |n: i64| {
        written(&format(&CounterValue::new(n), &style, None, &FormatOptions::new()).expect("ok"))
    };
    assert_eq!(write(1), "(a)");
    assert_eq!(write(3), "(c)");
}

#[test]
fn a_tag_template_holding_none_of_the_five_characters_writes_itself() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.bullet");
    decl.number_format = NumberTemplate::initial("*");
    let style = compiled(decl, &lang);
    let write = |n: i64| {
        written(&format(&CounterValue::new(n), &style, None, &FormatOptions::new()).expect("ok"))
    };
    assert_eq!(write(1), "*");
    assert_eq!(write(99), "*");
}

#[test]
fn a_sub_level_writes_its_number_after_its_separator() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.sub");
    decl.sub_levels = vec![SubLevelDecl::new(".", NumberTemplate::formats("#{alph}"))];
    let style = compiled(decl, &lang);
    let segments =
        format(&CounterValue::with_sub_numbers(1, [2]), &style, None, &FormatOptions::new())
            .expect("written");
    assert_eq!(written(&segments), "1.b");
}

#[test]
fn a_style_writes_nothing_for_a_sub_number_of_zero() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.sub");
    decl.sub_levels = vec![SubLevelDecl::new(".", NumberTemplate::formats("#{alph}"))];
    let style = compiled(decl, &lang);
    let segments =
        format(&CounterValue::with_sub_numbers(4, [0]), &style, None, &FormatOptions::new())
            .expect("written");
    assert_eq!(written(&segments), "4");
}

#[test]
fn a_sub_number_at_a_level_the_style_does_not_declare_is_an_error() {
    let lang = lang();
    let style = compiled(CounterStyleDecl::new("test.style.plain"), &lang);
    let value = CounterValue::with_sub_numbers(1, [2]);
    match format(&value, &style, None, &FormatOptions::new()) {
        Err(FormatError::UndeclaredSubLevel { level, number, .. }) => {
            assert_eq!((level, number), (0, 2));
        }
        other => panic!("expected an undeclared sub-level, got {:?}", other.map(|s| written(&s))),
    }
}

// ============================================================================
// writing one value
// ============================================================================

#[test]
fn one_value_is_written_with_its_word_and_its_delimiters() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.simple");
    decl.number_format = NumberTemplate::formats("(#{arabic})");
    decl.prefix_words = PrefixWords::new("Eq.~", "Eq.~");
    decl.delimiters = Delimiters::new("[", "]");
    decl.join = JoinDecl::compact();
    let style = compiled(decl, &lang);

    let write = |value: CounterValue| {
        written(&format(&value, &style, None, &FormatOptions::new()).expect("written"))
    };
    assert_eq!(write(CounterValue::new(1)), "Eq.~[(1)]");
    assert_eq!(write(CounterValue::new(-992)), "Eq.~[(-992)]");
}

#[test]
fn one_value_without_the_word_or_without_the_delimiters() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.extras");
    decl.prefix_words = PrefixWords::new("Eq. ", "Eq. ");
    decl.delimiters = Delimiters::new("(", ")");
    let style = compiled(decl, &lang);
    let value = CounterValue::new(5);

    let write = |options: FormatOptions| {
        written(&format(&value, &style, None, &options).expect("written"))
    };
    assert_eq!(write(FormatOptions::new()), "Eq. (5)");
    assert_eq!(write(FormatOptions::new().without_delimiters()), "Eq. 5");
    assert_eq!(write(FormatOptions::new().without_word()), "(5)");
    assert_eq!(write(FormatOptions::new().without_word().without_delimiters()), "5");
}

#[test]
fn a_style_declares_no_word_and_no_delimiter_by_default() {
    let lang = lang();
    let style = compiled(CounterStyleDecl::new("test.style.default"), &lang);
    let segments =
        format(&CounterValue::new(5), &style, None, &FormatOptions::new()).expect("written");
    assert_eq!(written(&segments), "5");
}

#[test]
fn one_value_is_one_link_over_the_whole_text() {
    let lang = lang();
    let style = join_style(&lang, false);
    let segments =
        format(&CounterValue::new(1), &style, None, &FormatOptions::new()).expect("written");
    assert_eq!(linked(&segments, |_| "1".to_string()), "\\mylink{1}{eq.~!<! I !>!}");
}

#[test]
fn one_value_takes_the_words_of_the_variant_it_asks_for() {
    let lang = lang();
    let style = join_style(&lang, false);
    let write = |options: FormatOptions| {
        written(&format(&CounterValue::new(1), &style, None, &options).expect("written"))
    };
    assert_eq!(write(FormatOptions::new()), "eq.~!<! I !>!");
    assert_eq!(write(FormatOptions::new().variant("capital")), "Equation~!<! I !>!");
    // A variant the style does not declare falls back to the default words.
    assert_eq!(write(FormatOptions::new().variant("unknown")), "eq.~!<! I !>!");
}

#[test]
fn one_value_is_written_after_the_counters_it_is_numbered_within() {
    let lang = lang();
    let style = join_style(&lang, false);
    let outer = outer_style(&lang);
    let ancestors = outer_at(1, &outer);
    let template = outer_joiner("-", &lang);
    let parent = Parent::new(template.as_ref(), &ancestors);
    let segments = format(&CounterValue::new(1), &style, Some(parent), &FormatOptions::new())
        .expect("written");
    assert_eq!(written(&segments), "eq.~!<! A-I !>!");
    assert_eq!(
        linked(&segments, |_| "p/A-1".to_string()),
        "\\mylink{p/A-1}{eq.~!<! A-I !>!}"
    );
}

#[test]
fn one_value_with_sub_numbers_is_written_after_its_ancestors() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.subs");
    decl.number_format = NumberTemplate::formats("#{Roman}");
    decl.prefix_words = PrefixWords::new("eq.~", "eqs.~");
    decl.delimiters = Delimiters::new("!<! ", " !>!");
    decl.sub_levels = vec![
        SubLevelDecl::new(".", NumberTemplate::formats("#{alph}")),
        SubLevelDecl::new(".", NumberTemplate::formats("#{roman}")),
    ];
    let style = compiled(decl, &lang);
    let outer = outer_style(&lang);
    let ancestors = outer_at(1, &outer);
    let template = outer_joiner("-", &lang);
    let parent = Parent::new(template.as_ref(), &ancestors);

    let write = |value: CounterValue| {
        written(&format(&value, &style, Some(parent), &FormatOptions::new()).expect("written"))
    };
    assert_eq!(write(CounterValue::with_sub_numbers(1, [2, 3])), "eq.~!<! A-I.b.iii !>!");
    assert_eq!(write(CounterValue::with_sub_numbers(1, [3])), "eq.~!<! A-I.c !>!");
    assert_eq!(write(CounterValue::new(1)), "eq.~!<! A-I !>!");

    let segments = format(
        &CounterValue::with_sub_numbers(1, [2, 1]),
        &style,
        Some(parent),
        &FormatOptions::new(),
    )
    .expect("written");
    assert_eq!(
        linked(&segments, |_| "p/A-1.2.1".to_string()),
        "\\mylink{p/A-1.2.1}{eq.~!<! A-I.b.i !>!}"
    );
}

#[test]
fn an_ancestor_is_written_by_its_number_alone() {
    let lang = lang();
    // The ancestor's own style carries a word and delimiters; neither is
    // written when the ancestor stands in another counter's number.
    let mut outer_decl = CounterStyleDecl::new("test.style.loud");
    outer_decl.number_format = NumberTemplate::formats("#{Alph}");
    outer_decl.prefix_words = PrefixWords::new("Sec.~", "Secs.~");
    outer_decl.delimiters = Delimiters::new("<", ">");
    let outer = Arc::new(compiled(outer_decl, &lang));
    let ancestors = outer_at(2, &outer);
    let template = outer_joiner(".", &lang);
    let parent = Parent::new(template.as_ref(), &ancestors);

    let style = compiled(CounterStyleDecl::new("test.style.inner"), &lang);
    let segments = format(&CounterValue::new(13), &style, Some(parent), &FormatOptions::new())
        .expect("written");
    assert_eq!(written(&segments), "B.13");
}

#[test]
fn a_within_template_naming_a_counter_that_is_not_an_ancestor_is_an_error() {
    let lang = lang();
    let style = compiled(CounterStyleDecl::new("test.style.inner"), &lang);
    let template = within_template("#{chapter}.", &["chapter"], &lang);
    assert_eq!(template.counters_written(&[]), [Name::new("chapter")]);
    let parent = Parent::new(template.as_ref(), &[]);
    match format(&CounterValue::new(1), &style, Some(parent), &FormatOptions::new()) {
        Err(FormatError::UnknownAncestor { joiner, counter }) => {
            assert_eq!(counter.as_str(), "chapter");
            assert_eq!(joiner, "#{chapter}.");
        }
        other => panic!("expected an unknown ancestor, got {:?}", other.map(|s| written(&s))),
    }
}

#[test]
fn a_counter_named_in_a_within_template_brings_its_own_parent_part() {
    let lang = lang();
    // `section` counts on its own; `subsection` is numbered within it and says
    // so with `#{section}.`; the innermost counter names `#{subsection}.` only,
    // and the section's number comes with the subsection's.
    let section_style = Arc::new(compiled(CounterStyleDecl::new("test.style.section"), &lang));
    let subsection_style =
        Arc::new(compiled(CounterStyleDecl::new("test.style.subsection"), &lang));
    let within_section = within_template("#{section}.", &["section", "subsection"], &lang);
    let within_subsection = within_template("#{subsection}.", &["section", "subsection"], &lang);

    let ancestors = vec![
        Ancestor::new("section", CounterValue::new(2), Arc::clone(&section_style)),
        Ancestor::new("subsection", CounterValue::new(3), Arc::clone(&subsection_style))
            .within(Arc::clone(&within_section)),
    ];
    let parent = Parent::new(within_subsection.as_ref(), &ancestors);

    let style = compiled(CounterStyleDecl::new("test.style.inner"), &lang);
    let segments = format(&CounterValue::new(4), &style, Some(parent), &FormatOptions::new())
        .expect("written");
    assert_eq!(written(&segments), "2.3.4");
}

#[test]
fn a_chain_of_within_templates_ends_however_the_templates_name_each_other() {
    let lang = lang();
    // Every counter of the chain names the one before it, and the outermost
    // names the innermost — which is not written before it, so the writing
    // stops there with an error instead of going round for ever.
    let style = Arc::new(compiled(CounterStyleDecl::new("test.style.plain"), &lang));
    let within = |counter: &str| {
        within_template(&format!("#{{{counter}}}."), &["one", "two", "three"], &lang)
    };
    // The outermost counter's own template carries a literal the point's does
    // not, so that the error names which of the two raised it: the writing has
    // to have gone all the way round the chain to reach it.
    let outermost = within_template("x#{three}.", &["one", "two", "three"], &lang);
    let ancestors = vec![
        Ancestor::new("one", CounterValue::new(1), Arc::clone(&style))
            .within(Arc::clone(&outermost)),
        Ancestor::new("two", CounterValue::new(2), Arc::clone(&style)).within(within("one")),
        Ancestor::new("three", CounterValue::new(3), Arc::clone(&style)).within(within("two")),
    ];
    let template = within("three");
    let parent = Parent::new(template.as_ref(), &ancestors);
    match format(&CounterValue::new(4), &style, Some(parent), &FormatOptions::new()) {
        Err(FormatError::UnknownAncestor { joiner, counter }) => {
            assert_eq!(counter.as_str(), "three");
            assert_eq!(joiner, "x#{three}.");
        }
        other => panic!("expected an unknown ancestor, got {:?}", other.map(|s| written(&s))),
    }

    // The same chain with the outermost counter numbered within nothing writes
    // every number of the chain, outermost first.
    let ancestors = vec![
        Ancestor::new("one", CounterValue::new(1), Arc::clone(&style)),
        Ancestor::new("two", CounterValue::new(2), Arc::clone(&style)).within(within("one")),
        Ancestor::new("three", CounterValue::new(3), Arc::clone(&style)).within(within("two")),
    ];
    let parent = Parent::new(template.as_ref(), &ancestors);
    let segments = format(&CounterValue::new(4), &style, Some(parent), &FormatOptions::new())
        .expect("written");
    assert_eq!(written(&segments), "1.2.3.4");
}

// ============================================================================
// writing several values
// ============================================================================

#[test]
fn several_values_are_sorted_and_collapsed_into_ranges() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.simple");
    decl.number_format = NumberTemplate::formats("(#{arabic})");
    decl.prefix_words = PrefixWords::new("Eq.~", "Eq.~");
    decl.delimiters = Delimiters::new("[", "]");
    decl.join = JoinDecl::compact();
    let style = compiled(decl, &lang);

    let write = |numbers: Vec<i64>| {
        let values = values(numbers);
        written(
            &format_many(&style, &alone(&values), &FormatOptions::new()).expect("written"),
        )
    };
    assert_eq!(write(vec![1]), "Eq.~[(1)]");
    assert_eq!(write(vec![2, 3, 1]), "Eq.~[(1)–(3)]");
    assert_eq!(write(vec![8, 9, 10, 11]), "Eq.~[(8)–(11)]");
    assert_eq!(write(vec![2, 1, 99, 3]), "Eq.~[(1)–(3),(99)]");
    assert_eq!(write(vec![1, 3, 99, 2, 98, 54]), "Eq.~[(1)–(3),(54),(98),(99)]");
}

/// A compact arabic style with `[` `]`, ranges on or off.
fn ranges_style(lang: &BuiltLang, ranges: bool) -> CounterStyle {
    let mut decl = CounterStyleDecl::new("test.style.ranges");
    decl.delimiters = Delimiters::new("[", "]");
    decl.join = JoinDecl::compact();
    decl.ranges = ranges;
    compiled(decl, lang)
}

#[test]
fn a_style_collapses_consecutive_values_into_ranges_by_default() {
    assert!(CounterStyleDecl::new("test.style.default").ranges);
    let lang = lang();
    let style = ranges_style(&lang, true);
    let values = values([3, 1, 2, 5]);
    let segments = format_many(&style, &alone(&values), &FormatOptions::new()).expect("written");
    assert_eq!(written(&segments), "[1–3,5]");
}

#[test]
fn a_style_without_ranges_sorts_the_values_and_writes_each_alone() {
    let lang = lang();
    let style = ranges_style(&lang, false);
    let values = values([3, 1, 2, 5, 4]);
    let segments = format_many(&style, &alone(&values), &FormatOptions::new()).expect("written");
    assert_eq!(written(&segments), "[1,2,3,4,5]");
    // Every number is a link of its own, to its own value.
    assert_eq!(
        linked(&segments, |position| values[position].number().to_string()),
        "[\\mylink{1}{1},\\mylink{2}{2},\\mylink{3}{3},\\mylink{4}{4},\\mylink{5}{5}]"
    );
}

#[test]
fn a_lone_consecutive_pair_is_written_the_same_with_ranges_on_or_off() {
    let lang = lang();
    let values = values([2, 1]);
    for ranges in [true, false] {
        let style = ranges_style(&lang, ranges);
        let segments =
            format_many(&style, &alone(&values), &FormatOptions::new()).expect("written");
        assert_eq!(written(&segments), "[1,2]", "ranges {ranges}");
    }
}

#[test]
fn the_join_texts_stand_where_the_specification_says() {
    let lang = lang();
    let style = join_style(&lang, false);
    let write = |numbers: Vec<i64>, options: FormatOptions| {
        let values = values(numbers);
        written(&format_many(&style, &alone(&values), &options).expect("written"))
    };
    assert_eq!(write(vec![1], FormatOptions::new()), "eq.~!<! <I> !>!");
    assert_eq!(write(vec![2, 1], FormatOptions::new()), "eqs.~!<! ((I||II)) !>!");
    assert_eq!(write(vec![2, 3, 1], FormatOptions::new()), "eqs.~!<! <[[I--III]]> !>!");
    assert_eq!(
        write(vec![2, 1, 99, 3], FormatOptions::new().variant("capital")),
        "Equations~!<! (([[I--III]]||XCIX)) !>!"
    );
    assert_eq!(
        write(vec![1, 3, 99, 2, 98, 54], FormatOptions::new().variant("capital")),
        "Equations~!<! <<[[I--III]];LIV;&[[XCVIII|XCIX]]>> !>!"
    );
    assert_eq!(
        write(vec![2, 3, 1], FormatOptions::new().variant("capital")),
        "Equations~!<! <[[I--III]]> !>!"
    );
    assert_eq!(write(vec![1], FormatOptions::new().variant("capital")), "Equation~!<! <I> !>!");
}

#[test]
fn the_word_links_to_the_first_value_and_each_number_to_its_own() {
    let lang = lang();
    let style = join_style(&lang, false);
    let write = |numbers: Vec<i64>, options: FormatOptions| {
        let numbers_for_key = numbers.clone();
        let values = values(numbers);
        let segments = format_many(&style, &alone(&values), &options).expect("written");
        linked(&segments, |position| numbers_for_key[position].to_string())
    };
    assert_eq!(write(vec![1], FormatOptions::new()), "\\mylink{1}{eq.~!<! <I> !>!}");
    assert_eq!(
        write(vec![2, 3], FormatOptions::new()),
        "\\mylink{2}{eqs.~}!<! ((\\mylink{2}{II}||\\mylink{3}{III})) !>!"
    );
    assert_eq!(
        write(vec![2, 3, 1], FormatOptions::new()),
        "\\mylink{1}{eqs.~}!<! <[[\\mylink{1}{I}--\\mylink{3}{III}]]> !>!"
    );
    assert_eq!(
        write(vec![1, 3, 99, 2, 98, 54], FormatOptions::new().variant("capital")),
        "\\mylink{1}{Equations~}!<! <<[[\\mylink{1}{I}--\\mylink{3}{III}]];\
         \\mylink{54}{LIV};&[[\\mylink{98}{XCVIII}|\\mylink{99}{XCIX}]]>> !>!"
    );
}

#[test]
fn a_style_with_no_word_puts_the_delimiters_inside_the_link_of_a_lone_value() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.noinnerdelim");
    decl.number_format = NumberTemplate::formats("#{Alph}");
    decl.delimiters = Delimiters::new("[", "]");
    decl.join = JoinDecl {
        pair_mid: Some("|".to_string()),
        range_mid: Some("--".to_string()),
        range_pairmid: Some("|".to_string()),
        list_pre: Some("<<".to_string()),
        list_mid: Some(";".to_string()),
        list_midlast: Some(";".to_string()),
        list_post: Some(">>".to_string()),
        ..JoinDecl::default()
    };
    let style = compiled(decl, &lang);

    let write = |numbers: Vec<i64>| {
        let numbers_for_key = numbers.clone();
        let values = values(numbers);
        let segments =
            format_many(&style, &alone(&values), &FormatOptions::new()).expect("written");
        linked(&segments, |position| numbers_for_key[position].to_string())
    };
    assert_eq!(write(vec![1]), "\\mylink{1}{[A]}");
    assert_eq!(write(vec![2, 3]), "[\\mylink{2}{B}|\\mylink{3}{C}]");
    assert_eq!(write(vec![2, 3, 1]), "[\\mylink{1}{A}--\\mylink{3}{C}]");
}

#[test]
fn no_value_at_all_is_written_as_the_empty_text() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.empty");
    decl.prefix_words = PrefixWords::new("Eq. ", "Eqs. ");
    decl.delimiters = Delimiters::new("(", ")");
    let style = compiled(decl, &lang);
    let segments = format_many(&style, &[], &FormatOptions::new()).expect("written");
    assert_eq!(written(&segments), "(empty)");
    assert!(segments.iter().all(|segment| segment.link().is_none()));
}

// ============================================================================
// writing several values numbered within another counter
// ============================================================================

/// The groups of the reference implementation's numprefix cases: one group per
/// letter, each numbered within the counter `outer` at that letter's number.
struct Groups {
    template: Arc<dyn CompiledWithinJoiner>,
    ancestors: Vec<Vec<Ancestor>>,
    values: Vec<Vec<CounterValue>>,
    labels: Vec<String>,
}

impl Groups {
    /// The groups `(letter, separator, values)`, in the order given.
    fn new(lang: &BuiltLang, separator: &str, groups: &[(i64, Vec<CounterValue>)]) -> Groups {
        let outer = outer_style(lang);
        Groups {
            template: outer_joiner(separator, lang),
            ancestors: groups.iter().map(|(letter, _)| outer_at(*letter, &outer)).collect(),
            values: groups.iter().map(|(_, values)| values.clone()).collect(),
            labels: groups
                .iter()
                .map(|(letter, _)| {
                    format!("{}{separator}", NumberFormat::CapitalLetters.text(*letter))
                })
                .collect(),
        }
    }

    /// The values, as [`format_many`] takes them: each one numbered within its
    /// group's ancestors, the groups one after the other.
    fn groups(&self) -> Vec<NumberedValue<'_>> {
        self.values
            .iter()
            .zip(&self.ancestors)
            .flat_map(|(values, ancestors)| {
                values.iter().map(|value| {
                    NumberedValue::new(value)
                        .within(Parent::new(self.template.as_ref(), ancestors))
                })
            })
            .collect()
    }

    /// The key of the value at `position`: the group's label, the number, and
    /// the sub-numbers after full stops.
    fn key(&self, position: usize) -> String {
        let mut left = position;
        for (index, values) in self.values.iter().enumerate() {
            if left < values.len() {
                let value = &values[left];
                let mut key = format!("p/{}{}", self.labels[index], value.number());
                for sub in value.sub_numbers() {
                    key.push_str(&format!(".{sub}"));
                }
                return key;
            }
            left -= values.len();
        }
        panic!("no value at position {position}");
    }
}

#[test]
fn a_range_omits_the_parent_part_and_what_its_ends_share() {
    let lang = lang();
    let style = join_style(&lang, false);
    let write = |separator: &str, groups: &[(i64, Vec<CounterValue>)], options: FormatOptions| {
        let groups = Groups::new(&lang, separator, groups);
        written(&format_many(&style, &groups.groups(), &options).expect("written"))
    };
    assert_eq!(write("-", &[(1, values([1]))], FormatOptions::new()), "eq.~!<! <A-I> !>!");
    assert_eq!(
        write("-", &[(1, values([2, 1]))], FormatOptions::new()),
        "eqs.~!<! ((A-I||A-II)) !>!"
    );
    assert_eq!(
        write("-", &[(1, values([2])), (2, values([1]))], FormatOptions::new()),
        "eqs.~!<! ((A-II||B-I)) !>!"
    );
    assert_eq!(
        write(".", &[(1, values([2, 3, 1]))], FormatOptions::new()),
        "eqs.~!<! <[[A.I--III]]> !>!"
    );
    assert_eq!(
        write(".", &[(1, values([2, 3, 1])), (2, values([1]))], FormatOptions::new()),
        "eqs.~!<! (([[A.I--III]]||B.I)) !>!"
    );
    assert_eq!(
        write(".", &[(1, values([2, 1, 99, 3]))], FormatOptions::new().variant("capital")),
        "Equations~!<! (([[A.I--III]]||A.XCIX)) !>!"
    );
    assert_eq!(
        write(
            "-",
            &[(1, values([1, 3, 99, 98, 2])), (3, values([54]))],
            FormatOptions::new().variant("capital"),
        ),
        "Equations~!<! <<[[A-I--III]];[[A-XCVIII|A-XCIX]];&C-LIV>> !>!"
    );
    assert_eq!(
        write("-", &[(1, values([2, 3, 1]))], FormatOptions::new().variant("capital")),
        "Equations~!<! <[[A-I--III]]> !>!"
    );
    assert_eq!(
        write(".", &[(1, values([1]))], FormatOptions::new().variant("capital")),
        "Equation~!<! <A.I> !>!"
    );
}

#[test]
fn every_number_of_a_group_links_through_its_own_ancestors() {
    let lang = lang();
    let style = join_style(&lang, false);
    let write = |separator: &str, spec: &[(i64, Vec<CounterValue>)], options: FormatOptions| {
        let groups = Groups::new(&lang, separator, spec);
        let segments = format_many(&style, &groups.groups(), &options).expect("written");
        linked(&segments, |position| groups.key(position))
    };
    assert_eq!(
        write("-", &[(1, values([1]))], FormatOptions::new()),
        "\\mylink{p/A-1}{eq.~!<! <A-I> !>!}"
    );
    assert_eq!(
        write("-", &[(1, values([2, 3]))], FormatOptions::new()),
        "\\mylink{p/A-2}{eqs.~}!<! ((\\mylink{p/A-2}{A-II}||\\mylink{p/A-3}{A-III})) !>!"
    );
    assert_eq!(
        write("-", &[(1, values([2, 3, 1]))], FormatOptions::new()),
        "\\mylink{p/A-1}{eqs.~}!<! <[[\\mylink{p/A-1}{A-I}--\\mylink{p/A-3}{III}]]> !>!"
    );
    assert_eq!(
        write("-", &[(1, values([2, 3, 1])), (2, values([1, 3, 2]))], FormatOptions::new()),
        "\\mylink{p/A-1}{eqs.~}!<! (([[\\mylink{p/A-1}{A-I}--\\mylink{p/A-3}{III}]]||\
         [[\\mylink{p/B-1}{B-I}--\\mylink{p/B-3}{III}]])) !>!"
    );
    assert_eq!(
        write(
            "-",
            &[(2, values([1, 3, 99, 2, 98])), (1, values([54]))],
            FormatOptions::new().variant("capital"),
        ),
        "\\mylink{p/B-1}{Equations~}!<! <<[[\\mylink{p/B-1}{B-I}--\\mylink{p/B-3}{III}]];\
         [[\\mylink{p/B-98}{B-XCVIII}|\\mylink{p/B-99}{B-XCIX}]];&\
         \\mylink{p/A-54}{A-LIV}>> !>!"
    );
}

#[test]
fn a_style_that_repeats_the_parent_part_writes_it_at_both_ends_of_a_range() {
    let lang = lang();
    let style = join_style(&lang, true);
    let write = |separator: &str, spec: &[(i64, Vec<CounterValue>)], options: FormatOptions| {
        let groups = Groups::new(&lang, separator, spec);
        written(&format_many(&style, &groups.groups(), &options).expect("written"))
    };
    assert_eq!(write("-", &[(1, values([1]))], FormatOptions::new()), "eq.~!<! <A-I> !>!");
    assert_eq!(
        write("-", &[(1, values([2, 1]))], FormatOptions::new()),
        "eqs.~!<! ((A-I||A-II)) !>!"
    );
    assert_eq!(
        write("-", &[(1, values([2])), (2, values([1]))], FormatOptions::new()),
        "eqs.~!<! ((A-II||B-I)) !>!"
    );
    assert_eq!(
        write(".", &[(1, values([2, 3, 1]))], FormatOptions::new()),
        "eqs.~!<! <[[A.I--A.III]]> !>!"
    );
    assert_eq!(
        write(".", &[(1, values([2, 3, 1])), (2, values([1]))], FormatOptions::new()),
        "eqs.~!<! (([[A.I--A.III]]||B.I)) !>!"
    );
    assert_eq!(
        write(".", &[(1, values([2, 1, 99, 3]))], FormatOptions::new().variant("capital")),
        "Equations~!<! (([[A.I--A.III]]||A.XCIX)) !>!"
    );
    assert_eq!(
        write(
            "-",
            &[(1, values([1, 3, 99, 98, 2])), (3, values([54]))],
            FormatOptions::new().variant("capital"),
        ),
        "Equations~!<! <<[[A-I--A-III]];[[A-XCVIII|A-XCIX]];&C-LIV>> !>!"
    );
    assert_eq!(
        write("-", &[(1, values([2, 3, 1]))], FormatOptions::new().variant("capital")),
        "Equations~!<! <[[A-I--A-III]]> !>!"
    );
    assert_eq!(
        write(".", &[(1, values([1]))], FormatOptions::new().variant("capital")),
        "Equation~!<! <A.I> !>!"
    );
}

#[test]
fn a_style_that_repeats_the_parent_part_keeps_it_in_every_link() {
    let lang = lang();
    let style = join_style(&lang, true);
    let write = |separator: &str, spec: &[(i64, Vec<CounterValue>)], options: FormatOptions| {
        let groups = Groups::new(&lang, separator, spec);
        let segments = format_many(&style, &groups.groups(), &options).expect("written");
        linked(&segments, |position| groups.key(position))
    };
    assert_eq!(
        write("-", &[(1, values([1]))], FormatOptions::new()),
        "\\mylink{p/A-1}{eq.~!<! <A-I> !>!}"
    );
    assert_eq!(
        write("-", &[(1, values([2, 3]))], FormatOptions::new()),
        "\\mylink{p/A-2}{eqs.~}!<! ((\\mylink{p/A-2}{A-II}||\\mylink{p/A-3}{A-III})) !>!"
    );
    assert_eq!(
        write("-", &[(1, values([2, 3, 1]))], FormatOptions::new()),
        "\\mylink{p/A-1}{eqs.~}!<! <[[\\mylink{p/A-1}{A-I}--\\mylink{p/A-3}{A-III}]]> !>!"
    );
    assert_eq!(
        write("-", &[(1, values([2, 3, 1])), (2, values([1, 3, 2]))], FormatOptions::new()),
        "\\mylink{p/A-1}{eqs.~}!<! (([[\\mylink{p/A-1}{A-I}--\\mylink{p/A-3}{A-III}]]||\
         [[\\mylink{p/B-1}{B-I}--\\mylink{p/B-3}{B-III}]])) !>!"
    );
    assert_eq!(
        write(
            "-",
            &[(2, values([1, 3, 99, 2, 98])), (1, values([54]))],
            FormatOptions::new().variant("capital"),
        ),
        "\\mylink{p/B-1}{Equations~}!<! <<[[\\mylink{p/B-1}{B-I}--\\mylink{p/B-3}{B-III}]];\
         [[\\mylink{p/B-98}{B-XCVIII}|\\mylink{p/B-99}{B-XCIX}]];&\
         \\mylink{p/A-54}{A-LIV}>> !>!"
    );
}

// ============================================================================
// writing several values with sub-numbers
// ============================================================================

/// The style of the reference implementation's `test_join_withsubnums`: roman
/// numerals with two sub-levels, letters then roman numerals, each after a
/// full stop.
fn sub_number_style(lang: &BuiltLang) -> CounterStyle {
    let mut decl = CounterStyleDecl::new("test.style.subnums");
    decl.number_format = NumberTemplate::formats("#{Roman}");
    decl.prefix_words = PrefixWords::new("eq.~", "eqs.~")
        .with_variant("capital", PluralWords::new("Equation~", "Equations~"));
    decl.delimiters = Delimiters::new("!<! ", " !>!");
    decl.join = spelled_join();
    decl.sub_levels = vec![
        SubLevelDecl::new(".", NumberTemplate::formats("#{alph}")),
        SubLevelDecl::new(".", NumberTemplate::formats("#{roman}")),
    ];
    compiled(decl, lang)
}

#[test]
fn sub_numbers_are_written_after_the_number_and_collapse_like_it() {
    let lang = lang();
    let style = sub_number_style(&lang);
    let value = |number: i64, subs: &[i64]| CounterValue::with_sub_numbers(number, subs.to_vec());
    let write = |separator: &str, spec: &[(i64, Vec<CounterValue>)]| {
        let groups = Groups::new(&lang, separator, spec);
        written(&format_many(&style, &groups.groups(), &FormatOptions::new()).expect("written"))
    };
    assert_eq!(write("-", &[(1, vec![value(1, &[2, 3])])]), "eq.~!<! <A-I.b.iii> !>!");
    assert_eq!(write("-", &[(1, vec![value(1, &[2])])]), "eq.~!<! <A-I.b> !>!");
    assert_eq!(
        write("-", &[(1, vec![value(2, &[3]), value(1, &[3, 1])])]),
        "eqs.~!<! ((A-I.c.i||A-II.c)) !>!"
    );
    assert_eq!(
        write("-", &[(1, vec![value(2, &[])]), (2, vec![value(1, &[])])]),
        "eqs.~!<! ((A-II||B-I)) !>!"
    );
    assert_eq!(
        write("-", &[(1, vec![value(2, &[])]), (2, vec![value(1, &[]), value(1, &[1])])]),
        "eqs.~!<! <<A-II;B-I;&B-I.a>> !>!"
    );
    assert_eq!(
        write(".", &[(1, vec![value(2, &[]), value(3, &[1]), value(1, &[])])]),
        "eqs.~!<! (([[A.I|A.II]]||A.III.a)) !>!"
    );
    assert_eq!(
        write(
            ".",
            &[
                (1, vec![value(2, &[2]), value(2, &[3]), value(1, &[])]),
                (2, vec![value(1, &[])]),
            ],
        ),
        "eqs.~!<! <<A.I;[[A.II.b|A.II.c]];&B.I>> !>!"
    );
    assert_eq!(
        write(
            ".",
            &[
                (
                    1,
                    vec![
                        value(2, &[2]),
                        value(2, &[5]),
                        value(2, &[4]),
                        value(2, &[3]),
                        value(1, &[]),
                    ],
                ),
                (2, vec![value(1, &[])]),
            ],
        ),
        "eqs.~!<! <<A.I;[[A.II.b--e]];&B.I>> !>!"
    );
}

#[test]
fn a_link_key_carries_a_values_sub_numbers() {
    let lang = lang();
    let style = sub_number_style(&lang);
    let value = |number: i64, subs: &[i64]| CounterValue::with_sub_numbers(number, subs.to_vec());

    let groups = Groups::new(&lang, "-", &[(1, vec![value(1, &[1])])]);
    let segments =
        format_many(&style, &groups.groups(), &FormatOptions::new()).expect("written");
    assert_eq!(
        linked(&segments, |position| groups.key(position)),
        "\\mylink{p/A-1.1}{eq.~!<! <A-I.a> !>!}"
    );

    let groups = Groups::new(
        &lang,
        "-",
        &[
            (
                2,
                vec![
                    value(1, &[]),
                    value(3, &[]),
                    value(99, &[3]),
                    value(2, &[]),
                    value(99, &[2]),
                ],
            ),
            (1, vec![value(54, &[])]),
        ],
    );
    let segments =
        format_many(&style, &groups.groups(), &FormatOptions::new().variant("capital"))
            .expect("written");
    assert_eq!(
        linked(&segments, |position| groups.key(position)),
        "\\mylink{p/B-1}{Equations~}!<! <<[[\\mylink{p/B-1}{B-I}--\\mylink{p/B-3}{III}]];\
         [[\\mylink{p/B-99.2}{B-XCIX.b}|\\mylink{p/B-99.3}{B-XCIX.c}]];&\
         \\mylink{p/A-54}{A-LIV}>> !>!"
    );
}

// ============================================================================
// what the port owes beyond the reference implementation's cases
// ============================================================================

#[test]
fn a_sorted_run_of_three_or_more_consecutive_values_is_one_range() {
    let lang = lang();
    let style = compiled(CounterStyleDecl::new("test.style.plain"), &lang);
    for length in 3..=12i64 {
        let values = values(1..=length);
        let segments =
            format_many(&style, &alone(&values), &FormatOptions::new()).expect("written");
        assert_eq!(written(&segments), format!("1\u{2013}{length}"), "a run of {length}");
        let links: Vec<Option<usize>> = segments.iter().map(Segment::link).collect();
        assert_eq!(links, [Some(0), None, Some((length - 1) as usize)], "a run of {length}");
    }
}

#[test]
fn a_run_of_exactly_two_consecutive_values_is_written_as_a_pair() {
    let lang = lang();
    let style = compiled(CounterStyleDecl::new("test.style.plain"), &lang);
    let values = values([8, 7]);
    let segments =
        format_many(&style, &alone(&values), &FormatOptions::new()).expect("written");
    assert_eq!(written(&segments), "7 and 8");
}

#[test]
fn the_default_join_texts_spell_the_conjunction_out() {
    let lang = lang();
    let style = compiled(CounterStyleDecl::new("test.style.plain"), &lang);
    let write = |numbers: Vec<i64>| {
        let values = values(numbers);
        written(
            &format_many(&style, &alone(&values), &FormatOptions::new()).expect("written"),
        )
    };
    assert_eq!(write(vec![1]), "1");
    assert_eq!(write(vec![1, 2]), "1 and 2");
    assert_eq!(write(vec![1, 2, 3]), "1\u{2013}3");
    assert_eq!(write(vec![1, 2, 3, 7, 9]), "1\u{2013}3, 7, and 9");
}

#[test]
fn a_join_text_may_name_a_variable() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.variables");
    decl.join.and = Some("und".to_string());
    decl.join.sep = Some(";".to_string());
    let style = compiled(decl, &lang);
    let values = values([1, 2, 3, 7, 9]);
    let segments =
        format_many(&style, &alone(&values), &FormatOptions::new()).expect("written");
    assert_eq!(written(&segments), "1\u{2013}3; 7; und 9");
}

#[test]
fn a_join_text_naming_a_variable_that_does_not_exist_is_refused() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.badvariable");
    decl.join.list_mid = Some("${nonexistent}".to_string());
    match decl.compile(&lang) {
        Err(StyleError::UnknownVariable { name, .. }) => assert_eq!(name, "nonexistent"),
        other => panic!("expected an unknown variable, got {other:?}"),
    }
}

#[test]
fn a_text_that_is_not_valid_flm_is_refused() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.badtext");
    decl.prefix_words = PrefixWords::new("Eq.~{", "Eqs.~");
    match decl.compile(&lang) {
        Err(StyleError::Parse { diagnostics, .. }) => assert!(!diagnostics.is_empty()),
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn every_number_including_the_extremes_is_written_without_a_panic() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.everyformat");
    decl.number_format = NumberTemplate::formats(
        "#{arabic}#{alph}#{Alph}#{roman}#{Roman}#{fnsymbol}\
         #{unicodesuperscript}#{unicodesubscript}",
    );
    decl.sub_levels = vec![SubLevelDecl::new("-", NumberTemplate::formats("#{roman}"))];
    let style = compiled(decl, &lang);
    for number in [0, -1, -992, 1, i64::MAX, i64::MIN] {
        for sub in [0, -1, 7, i64::MAX] {
            let value = CounterValue::with_sub_numbers(number, [sub]);
            let one = format(&value, &style, None, &FormatOptions::new()).expect("written");
            let group_values = [value.clone()];
            let many = format_many(&style, &alone(&group_values), &FormatOptions::new())
                .expect("written");
            // Nothing beyond "it answered": the point is that no number loops,
            // allocates without bound, or panics.
            assert!(written(&one).len() < 10_000);
            assert!(written(&many).len() < 10_000);
        }
    }
}

#[test]
fn an_empty_template_and_an_empty_group_are_ordinary() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.emptytemplate");
    decl.number_format = NumberTemplate::formats("");
    let style = compiled(decl, &lang);
    let segments =
        format(&CounterValue::new(3), &style, None, &FormatOptions::new()).expect("written");
    assert_eq!(written(&segments), "");

    let none: [CounterValue; 0] = [];
    let segments = format_many(&style, &alone(&none), &FormatOptions::new())
        .expect("written");
    assert_eq!(written(&segments), "(empty)");
}

#[test]
fn a_value_given_twice_is_written_twice() {
    let lang = lang();
    let style = compiled(CounterStyleDecl::new("test.style.plain"), &lang);
    let values = values([4, 4]);
    let segments =
        format_many(&style, &alone(&values), &FormatOptions::new()).expect("written");
    assert_eq!(written(&segments), "4 and 4");
}

#[test]
fn a_chain_of_thousands_of_ancestors_is_written_without_overflowing_the_stack() {
    // The writing of a number recurses once per ancestor. This chain is far
    // deeper than any declaration a registry would accept, and it is written
    // because both entry points run on a worker thread.
    let lang = lang();
    let style = Arc::new(compiled(CounterStyleDecl::new("test.style.plain"), &lang));
    let depth = 4_000;
    let mut ancestors = Vec::with_capacity(depth);
    for level in 0..depth {
        let counter = format!("c{level}");
        let ancestor = Ancestor::new(&counter[..], CounterValue::new(1), Arc::clone(&style));
        ancestors.push(if level == 0 {
            ancestor
        } else {
            ancestor.within(
                Within::parent(format!("c{}", level - 1))
                    .compile(&lang, &[], &[])
                    .expect("the joiner compiles"),
            )
        });
    }
    let template = Within::parent(format!("c{}", depth - 1))
        .compile(&lang, &[], &[])
        .expect("the joiner compiles");
    let parent = Parent::new(template.as_ref(), &ancestors);
    let segments = format(&CounterValue::new(7), &style, Some(parent), &FormatOptions::new())
        .expect("written");
    let text = written(&segments);
    assert!(text.ends_with("1.7"), "{text}");
    assert_eq!(text.matches('.').count(), depth);
}

// ============================================================================
// the within-joiner: the three forms, the keys, and the parts
// ============================================================================

/// A within-joiner written here, registered by name: the parent's number
/// followed by a full stop. It writes what the joining-text form writes, so
/// that the three forms can be compared.
#[derive(Debug)]
struct DotJoiner {
    name: Name,
    parent: Name,
}

impl WithinJoiner for DotJoiner {
    fn name(&self) -> &Name {
        &self.name
    }

    fn compile(&self, _cx: &WithinCx<'_>) -> Result<Arc<dyn CompiledWithinJoiner>, StyleError> {
        Ok(Arc::new(DotJoiner { name: self.name.clone(), parent: self.parent.clone() }))
    }
}

impl CompiledWithinJoiner for DotJoiner {
    fn counters_written(&self, _chain: &[Name]) -> Vec<Name> {
        vec![self.parent.clone()]
    }

    fn parts(&self, ancestors: &[Ancestor], cx: &JoinCx<'_>) -> Result<Vec<Part>, FormatError> {
        let place = ancestors
            .iter()
            .position(|ancestor| ancestor.counter == self.parent)
            .ok_or_else(|| FormatError::UnknownAncestor {
                joiner: self.name.to_string(),
                counter: self.parent.clone(),
            })?;
        let ancestor = &ancestors[place];
        let mut found = cx.parts_before(ancestors, place)?;
        let content = cx.joined(&[cx.number_of(ancestor)?, cx.literal(".")?])?;
        found.push(Part::new(vec![(ancestor.counter.clone(), ancestor.value.clone())], content));
        Ok(found)
    }
}

/// The chain `section` at 1 and `subsection` at 2, the subsection numbered
/// within the section by `joiner`.
fn two_ancestors(
    lang: &BuiltLang,
    joiner: Arc<dyn CompiledWithinJoiner>,
) -> (Arc<CounterStyle>, Vec<Ancestor>) {
    let style = Arc::new(compiled(CounterStyleDecl::new("test.style.plain"), lang));
    let ancestors = vec![
        Ancestor::new("section", CounterValue::new(1), Arc::clone(&style)),
        Ancestor::new("subsection", CounterValue::new(2), Arc::clone(&style)).within(joiner),
    ];
    (style, ancestors)
}

/// The counters of the two-ancestor chain, as a template's vocabulary.
fn two_names() -> Vec<Name> {
    vec![Name::new("section"), Name::new("subsection")]
}

/// The joiner named `name` writing `parent`'s number and a full stop.
fn dot_joiner(name: &str, parent: &str) -> Arc<dyn WithinJoiner> {
    Arc::new(DotJoiner { name: Name::new(name), parent: Name::new(parent) })
}

#[test]
fn the_three_forms_of_a_joiner_write_the_same_number() {
    let lang = lang();
    let names = two_names();
    let outer_named = dot_joiner("test.dot.section", "section");
    let inner_named = dot_joiner("test.dot.subsection", "subsection");
    let outer = [
        Within::parent("section").compile(&lang, &names, &[]).expect("compiles"),
        Within::parent("section")
            .template("#{section}.")
            .compile(&lang, &names, &[])
            .expect("compiles"),
        Within::parent("section")
            .joiner_named("test.dot.section")
            .compile(&lang, &names, &[Arc::clone(&outer_named)])
            .expect("compiles"),
    ];
    let inner = [
        Within::parent("subsection").compile(&lang, &names, &[]).expect("compiles"),
        Within::parent("subsection")
            .template("#{subsection}.")
            .compile(&lang, &names, &[])
            .expect("compiles"),
        Within::parent("subsection")
            .joiner_named("test.dot.subsection")
            .compile(&lang, &names, &[Arc::clone(&inner_named)])
            .expect("compiles"),
    ];
    for (form, (outer, inner)) in outer.into_iter().zip(inner).enumerate() {
        let (style, ancestors) = two_ancestors(&lang, outer);
        let parent = Parent::new(inner.as_ref(), &ancestors);
        let segments = format(&CounterValue::new(3), &style, Some(parent), &FormatOptions::new())
            .expect("written");
        assert_eq!(written(&segments), "1.2.3", "form {form}");
    }
}

#[test]
fn a_named_format_key_writes_the_ancestor_in_that_format() {
    let lang = lang();
    let names = two_names();
    let outer = Within::parent("section").compile(&lang, &names, &[]).expect("compiles");
    let (style, ancestors) = two_ancestors(&lang, outer);
    let inner = Within::parent("subsection")
        .template("#[Roman]{subsection}.")
        .compile(&lang, &names, &[])
        .expect("compiles");
    let parent = Parent::new(inner.as_ref(), &ancestors);
    let segments = format(&CounterValue::new(3), &style, Some(parent), &FormatOptions::new())
        .expect("written");
    assert_eq!(written(&segments), "1.II.3");
}

#[test]
fn the_bare_key_writes_the_ancestor_without_its_own_ancestors() {
    let lang = lang();
    let names = two_names();
    let outer = Within::parent("section").compile(&lang, &names, &[]).expect("compiles");
    let (style, ancestors) = two_ancestors(&lang, outer);
    let inner = Within::parent("subsection")
        .template("#[bare]{subsection}.")
        .compile(&lang, &names, &[])
        .expect("compiles");
    let parent = Parent::new(inner.as_ref(), &ancestors);
    let segments = format(&CounterValue::new(3), &style, Some(parent), &FormatOptions::new())
        .expect("written");
    assert_eq!(written(&segments), "2.3");
}

#[test]
fn a_bracket_key_that_is_neither_a_format_nor_bare_is_refused() {
    let lang = lang();
    match Within::parent("section")
        .template("#[nosuchkey]{section}.")
        .compile(&lang, &two_names(), &[])
    {
        Err(StyleError::UnknownKey { key, .. }) => assert_eq!(key, "nosuchkey"),
        other => panic!("expected an unknown key, got {:?}", other.map(|_| "a joiner")),
    }
}

#[test]
fn markup_may_span_a_substitution_in_both_template_languages() {
    let lang = lang();
    // A number template: the number inside a group. The language here is FLM's
    // own structure and no feature's commands, so the test makes its point
    // with a group; a command behaves the same way, its braces being a group.
    let mut decl = CounterStyleDecl::new("test.style.wrapped");
    decl.number_format = NumberTemplate::formats("{#{arabic}}");
    let style = compiled(decl, &lang);
    let segments =
        format(&CounterValue::new(3), &style, None, &FormatOptions::new()).expect("written");
    assert_eq!(written(&segments), "{3}");

    // A within template: the ancestor's number inside a group.
    let names = two_names();
    let outer = Within::parent("section").compile(&lang, &names, &[]).expect("compiles");
    let (plain, ancestors) = two_ancestors(&lang, outer);
    let inner = Within::parent("subsection")
        .template("{#{subsection}}.")
        .compile(&lang, &names, &[])
        .expect("compiles");
    let parent = Parent::new(inner.as_ref(), &ancestors);
    let segments = format(&CounterValue::new(3), &plain, Some(parent), &FormatOptions::new())
        .expect("written");
    assert_eq!(written(&segments), "{1.2}.3");
}


#[test]
fn a_template_with_two_substitutions_writes_two_parts() {
    let lang = lang();
    let style = Arc::new(compiled(CounterStyleDecl::new("test.style.plain"), &lang));
    let joiner =
        |parent: &str| Within::parent(parent).compile(&lang, &[], &[]).expect("compiles");
    let ancestors = vec![
        Ancestor::new("one", CounterValue::new(1), Arc::clone(&style)),
        Ancestor::new("two", CounterValue::new(2), Arc::clone(&style)).within(joiner("one")),
    ];
    let names = [Name::new("one"), Name::new("two")];
    let template = Within::parent("two")
        .template("#[bare]{one}/#[bare]{two}.")
        .compile(&lang, &names, &[])
        .expect("compiles");
    let written_parts =
        parts(&style, Parent::new(template.as_ref(), &ancestors)).expect("written");
    let stands: Vec<Vec<String>> = written_parts
        .iter()
        .map(|part| part.stands_for.iter().map(|(name, _)| name.to_string()).collect())
        .collect();
    assert_eq!(stands, [["one"], ["two"]]);
    let text: String = written_parts
        .iter()
        .map(|part| {
            let nodes: Vec<_> = part.content.root().children().iter().collect();
            recompose_source(&nodes)
        })
        .collect();
    assert_eq!(text, "1/2.");
}

#[test]
fn a_template_writes_one_part_for_its_text_alone_and_none_when_it_is_empty() {
    let lang = lang();
    let style = Arc::new(compiled(CounterStyleDecl::new("test.style.plain"), &lang));
    let ancestors = vec![Ancestor::new("one", CounterValue::new(1), Arc::clone(&style))];
    let names = [Name::new("one")];

    // Text and no substitution: one part, standing for no ancestor.
    let literal =
        Within::parent("one").template("X-").compile(&lang, &names, &[]).expect("compiles");
    let written_parts = parts(&style, Parent::new(literal.as_ref(), &ancestors)).expect("ok");
    assert_eq!(written_parts.len(), 1);
    assert!(written_parts[0].stands_for.is_empty());

    // The empty template: no part at all.
    let empty =
        Within::parent("one").template("").compile(&lang, &names, &[]).expect("compiles");
    assert!(parts(&style, Parent::new(empty.as_ref(), &ancestors)).expect("ok").is_empty());
}

#[test]
fn the_bare_form_writes_no_counter_and_no_part() {
    let lang = lang();
    let style = Arc::new(compiled(CounterStyleDecl::new("test.style.plain"), &lang));
    let ancestors = vec![Ancestor::new("one", CounterValue::new(1), Arc::clone(&style))];
    let joiner = Within::parent("one")
        .displayed_bare()
        .compile(&lang, &[Name::new("one")], &[])
        .expect("compiles");
    assert!(joiner.counters_written(&[Name::new("one")]).is_empty());
    assert!(parts(&style, Parent::new(joiner.as_ref(), &ancestors)).expect("ok").is_empty());
    // The point's own number is written alone, with nothing of its parent.
    let segments = format(
        &CounterValue::new(3),
        &style,
        Some(Parent::new(joiner.as_ref(), &ancestors)),
        &FormatOptions::new(),
    )
    .expect("written");
    assert_eq!(written(&segments), "3");
}

#[test]
fn a_bracket_on_a_number_template_is_refused() {
    let lang = lang();
    let mut decl = CounterStyleDecl::new("test.style.bracketed");
    decl.number_format = NumberTemplate::formats("#[Roman]{arabic}");
    match decl.compile(&lang) {
        Err(StyleError::Template { text, .. }) => assert_eq!(text, "#[Roman]{arabic}"),
        other => panic!("expected a template that does not compile, got {other:?}"),
    }
}

#[test]
fn the_parts_of_a_chain_are_one_per_ancestor() {
    let lang = lang();
    let style = Arc::new(compiled(CounterStyleDecl::new("test.style.plain"), &lang));
    let joiner =
        |parent: &str| Within::parent(parent).compile(&lang, &[], &[]).expect("compiles");
    let ancestors = vec![
        Ancestor::new("one", CounterValue::new(1), Arc::clone(&style)),
        Ancestor::new("two", CounterValue::new(2), Arc::clone(&style)).within(joiner("one")),
        Ancestor::new("three", CounterValue::new(3), Arc::clone(&style)).within(joiner("two")),
    ];
    let own = joiner("three");
    let written_parts = parts(&style, Parent::new(own.as_ref(), &ancestors)).expect("written");
    let stands: Vec<Vec<String>> = written_parts
        .iter()
        .map(|part| {
            part.stands_for
                .iter()
                .map(|(counter, value)| format!("{counter}={}", value.number()))
                .collect()
        })
        .collect();
    assert_eq!(stands, [["one=1"], ["two=2"], ["three=3"]]);
    let text: String = written_parts
        .iter()
        .map(|part| {
            let nodes: Vec<_> = part.content.root().children().iter().collect();
            recompose_source(&nodes)
        })
        .collect();
    assert_eq!(text, "1.2.3.");

    // A template naming the counter immediately above writes one part holding
    // every ancestor it brought with it.
    let names = [Name::new("one"), Name::new("two"), Name::new("three")];
    let template = Within::parent("three")
        .template("#{three}.")
        .compile(&lang, &names, &[])
        .expect("compiles");
    let one = parts(&style, Parent::new(template.as_ref(), &ancestors)).expect("written");
    assert_eq!(one.len(), 1);
    let stands: Vec<String> =
        one[0].stands_for.iter().map(|(counter, _)| counter.to_string()).collect();
    assert_eq!(stands, ["one", "two", "three"]);
}
