# Numbering: counters, counter styles and the numbers a reader sees

FLM numbers the constructs of a document — sections, equation lines, theorems,
list items, floats, footnotes — and writes those numbers where a reader sees them:
beside the construct and in every reference to it. Two declarations govern every
number:

- a **counter**, a dotted name under which things are counted in document order,
  such as `flm.headings.section` or `flm.math.equation`, declared by a feature
  together with the counter it is numbered within
  ([`CounterDecl`](crate::stdcore::features::numbering::CounterDecl));
- a **counter style**, which says how a counter value is shown: the number template,
  the sub-levels, the prefix words with their variants, the delimiters and the join
  texts, each text written in FLM and parsed once, when the counter style is compiled
  ([`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl)).

The two are introduced in the concepts overview, under [Counters and counter
styles](crate::guide::concepts_overview#counters-and-counter-styles). This chapter
describes the whole mechanism, which is implemented by the module
[`stdcore::features::numbering`](crate::stdcore::features::numbering). It serves
three readers: a host or an embedding program that changes how a document is
numbered, the author of a feature that numbers a construct of its own, and the
author of a lowering rule that shows a number.

## How a number is made, from declaration to display

A number goes through four steps.

1. **Declaration, when the registry is built.** A feature declares its counters and
   its counter styles to the `numbering` feature
   ([`Numbering`](crate::stdcore::features::numbering::Numbering)). The numbering
   feature checks every declaration against every other and parses every text of
   every counter style once.
2. **A number source on each reference point.** A [reference
   point](crate::model::aspects::RefPoint) is one place in a document that can be
   referred to, numbered, or given an anchor. A reference point that is to be
   numbered states a **number source**
   ([`NumberSource`](crate::model::aspects::NumberSource)): the counter it counts
   in and the counter style its number is shown in.
3. **Counting, once per run.** The numbering feature's one stage,
   `flm.numbering.number`, reads every reference point of the document in document
   order, advances the counters, and records each point's counter value in the
   counter table `flm.counters`
   ([`COUNTERS`](crate::stdcore::features::numbering::COUNTERS)). The table holds
   counter values, never text.
4. **Writing, where the number is shown.** A lowering rule that shows a number asks
   the counter table to write it. The table writes the recorded counter value with
   its counter style and returns content, which the rule places in its output.

The first half of this chapter describes counter styles and the writing of counter
values (step 4), which a caller can use on its own, without any counting. The second
half, [The numbering feature](#the-numbering-feature-counters-options-and-the-counter-table),
describes the declarations, the counting and the options (steps 1 to 3).

## Counter values, counter styles and segments

Three types are used to write a number:

- A **counter value**
  ([`CounterValue`](crate::stdcore::features::numbering::CounterValue)) is the value
  a counter has at a numbered point: a number, and any **sub-numbers** below it. The
  counter value `3` is shown as `3`; the counter value `(3, 2)` is shown as `3b` by a
  counter style whose first sub-level is written in letters with no separator.
- A **counter style** is declared as a
  [`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl) and
  compiled into a [`CounterStyle`](crate::stdcore::features::numbering::CounterStyle).
  It holds everything needed to show a counter value: the number template, the
  sub-levels, the prefix words, the delimiters around it, and the texts
  that join several numbers.
- A **segment** ([`Segment`](crate::stdcore::features::numbering::Segment)) is one
  part of the written result: a small tree of content, and the position, in the
  caller's list of counter values, of the counter value that this content links to,
  or no position (its link mark; see [Segments and their link
  marks](#segments-and-their-link-marks)).

## Every text of a counter style is FLM source, parsed once

The word before a number, the delimiters, the texts that join several numbers and the
literal parts of a number template are all FLM source, not plain text. `Eq.~` is the
word `Eq.` followed by a non-breaking space, and a counter style may declare
`\textbf{(}` as its opening delimiter as well as `(`.

These texts are parsed **once**, when the counter style is compiled against the built
language, and never again when a number is written. What a counter style writes is
therefore content: it is lowered through the ordinary lowering rules and rendered by
each backend like any other content, and nothing has to parse it back.

[`CounterStyleDecl::compile`](crate::stdcore::features::numbering::CounterStyleDecl::compile)
compiles a counter style. It returns a
[`StyleError`](crate::stdcore::features::numbering::StyleError) for a text that is not
valid FLM, for a number template that refers to a number format that does not exist
or carries a bracket key, for a join text that refers to a name that is not a join key
(see [Writing several numbers at once](#writing-several-numbers-at-once)), and for a
counter style that selects a number writer (see [The number writer](#the-number-writer)).

## The number formats

Eight number formats write a number as characters
([`NumberFormat`](crate::stdcore::features::numbering::NumberFormat)). A template
refers to each format by the name in the first column:

| Written | Format | 1, 2, 3, … |
|---|---|---|
| `arabic` | decimal digits | `1`, `2`, `3` |
| `alph` | letters, repeated | `a`, `b`, … `z`, `aa`, `bb`, … `aaa` |
| `Alph` | capital letters, repeated | `A`, `B`, … `Z`, `AA` |
| `roman` | roman numerals | `i`, `ii`, `iii`, `iv` |
| `Roman` | capital roman numerals | `I`, `II`, `III`, `IV` |
| `fnsymbol` | footnote symbols | `*`, `†`, `‡`, `§`, `¶`, `‖`, `**`, `††` |
| `unicodesuperscript` | superscript digits | `¹`, `²`, `³` |
| `unicodesubscript` | subscript digits | `₁`, `₂`, `₃` |

The letters do not count in base twenty-six: the twenty-seventh is `aa`, the
fifty-third `aaa`. The footnote symbols are doubled from the seventh number on and
tripled from the thirteenth.

Every format except `arabic` writes the empty text for a number it cannot write: zero,
a negative number, and a number whose text would be longer than
[`NumberFormat::MAX_CHARS`](crate::stdcore::features::numbering::NumberFormat::MAX_CHARS)
characters. `arabic` writes the decimal digits of any number, including `0` and a
leading minus sign.

```rust
use flm_core::stdcore::features::numbering::NumberFormat;

assert_eq!(NumberFormat::Letters.text(27), "aa");
assert_eq!(NumberFormat::CapitalRoman.text(2099), "MMXCIX");
assert_eq!(NumberFormat::Arabic.text(0), "0");
assert_eq!(NumberFormat::Roman.text(0), "");
```

## Number templates and their two languages

A counter style writes a number through a **number template**
([`NumberTemplate`](crate::stdcore::features::numbering::NumberTemplate)): FLM text
with a place for the number in it, which selects the number format. A number template
is written in one of two languages, the two variants of `NumberTemplate`.

**The template-language form: number formats referred to by name
([`NumberTemplate::Formats`](crate::stdcore::features::numbering::NumberTemplate::Formats)).**
Every `#{name}` is the number written in the number format `name`, and every other
character is written as given. This is the [template language shared by
features](crate::guide::concepts_overview#the-template-language-shared-by-features),
the language in which a user definition's template is written, so `\textbf`, `~` and
a conditional mean in a number template what they mean in a user definition.

| Number template | 1, 2, 3 |
|---|---|
| `#{arabic}` | `1`, `2`, `3` |
| `#{arabic}.` | `1.`, `2.`, `3.` |
| `(#{roman})` | `(i)`, `(ii)`, `(iii)` |
| `\textbf{#{arabic}}` | `1`, `2`, `3`, in bold |

A `Formats` template is parsed as one text, so markup may surround the
number as well as precede or follow it: `\textbf{Eq.}~#{arabic}` and
`\textbf{#{arabic}}` are both number templates. A `#{…}` that refers to anything other
than one of the eight number formats is refused when the counter style is compiled,
and so is a bracket key on one of them (`#[Roman]{arabic}`): bracket keys belong to
within templates only ([Numbering within another
counter](#numbering-within-another-counter)).

**The single-letter form: a number format shown by an example character
([`NumberTemplate::Initial`](crate::stdcore::features::numbering::NumberTemplate::Initial)).**
The first character among `a`, `A`, `i`, `I` and `1` is the number, written in the
number format that this character represents (letters, capital letters, roman
numerals, capital roman numerals, decimal digits); every other character is literal
FLM. This is the language of an author's own item tags: `(a)` gives `(a)`, `(b)`,
`(c)`, and `1.` gives `1.`, `2.`, `3.`. A text that holds none of the five characters,
such as `*`, writes itself for every number and writes nothing of the counters the
number is numbered within: it is a fixed mark, such as a bullet. Because the *first*
of the five characters is replaced wherever it occurs, a text that must keep one of
them as a letter of its own must be written as a `Formats` template: as an `Initial`
template, `Part a.` writes `Pcrt a.` for the number three. An `Initial` template is
cut at the replaced character, and the text on each side of it is parsed as FLM on its
own.

## Sub-numbers

A counter style declares one
[`SubLevelDecl`](crate::stdcore::features::numbering::SubLevelDecl) per level of
sub-numbering: the number template of that level, and the separator written before
the sub-number when something has been written before it. A sub-number of zero is
written as nothing. A counter value that carries a non-zero sub-number at a level the
counter style does not declare cannot be written: the writing fails with a
[`FormatError`](crate::stdcore::features::numbering::FormatError).

With one sub-level written in letters after a full stop, the counter value `(3, 2)` is
written `3.b` and the counter value `(3, 0)` is written `3`.

## Prefix words and their variants

[`PrefixWords`](crate::stdcore::features::numbering::PrefixWords) holds the word
written before a number, in the two forms a language needs (`one` for a single number,
`other` for several), and any number of named **variants**, each a complete pair of
words of its own. A counter style for equations declares `Eq.~` and `Eqs.~` by
default, and `Equation~` and `Equations~` under the variant `capital`.

A caller asks for a variant by name, and
[`CounterStyle::declares_variant`](crate::stdcore::features::numbering::CounterStyle::declares_variant)
tells whether a counter style declares it. A number asked for under a variant that the
counter style does not declare is written with the default words. The references
feature checks the variants a reference asks for: `\ref[capital]{…}` asks for the
variant `capital` outright, and a numbered target whose counter style does not declare
it is the error `flm.refs.undeclared-variant`; `\ref[S]{…}`
([`CAPITAL_KEY`](crate::stdcore::features::refs::CAPITAL_KEY)) asks for `capital` only
where the counter style declares it, and is never reported.

## The delimiters

[`Delimiters`](crate::stdcore::features::numbering::Delimiters) holds the text before
and the text after the number: `(` and `)` for an equation number shown as `(3)`.
When several numbers are written at once, the delimiters are written once around the
whole list: `Eqs. (1–3, 7, and 9)`.

A caller can leave out the prefix word, the delimiters, or both, through
[`FormatOptions`](crate::stdcore::features::numbering::FormatOptions).

```rust
use flm_core::lang::{build_language, LangInputs};
use flm_core::stdcore::backends::pres::read::recompose_source;
use flm_core::stdcore::features::numbering::{
    format, CounterStyleDecl, CounterValue, Delimiters, FormatOptions, PluralWords,
    PrefixWords, Segment,
};

/// The FLM source a list of segments writes.
fn written(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|segment| {
            let nodes: Vec<_> = segment.content().root().children().iter().collect();
            recompose_source(&nodes)
        })
        .collect()
}

let lang = build_language(LangInputs::new());
let mut decl = CounterStyleDecl::new("example.style.equation");
decl.delimiters = Delimiters::new("(", ")");
decl.prefix_words = PrefixWords::new("Eq.~", "Eqs.~")
    .with_variant("capital", PluralWords::new("Equation~", "Equations~"));
let style = decl.compile(&lang)?;

let three = CounterValue::new(3);
let write = |options| written(&format(&three, &style, None, &options).expect("written"));

assert_eq!(write(FormatOptions::new()), "Eq.~(3)");
assert_eq!(write(FormatOptions::new().variant("capital")), "Equation~(3)");
assert_eq!(write(FormatOptions::new().without_word()), "(3)");
assert_eq!(write(FormatOptions::new().without_word().without_delimiters()), "3");
# Ok::<(), flm_core::stdcore::features::numbering::StyleError>(())
```

## Numbering within another counter

A counter may be numbered within another counter: an equation numbered within
sections is shown as `(2.13)`. By default, being numbered within a counter means two
things: that counter (the **parent**) restarts the count when it advances, and its
number is written before the counter's own. What is written of the counter's
**ancestors** (its parent, the parent's own parent, and so on up the chain) before the
counter's own number is decided by a **within-joiner**. A counter declares its parent
and its within-joiner together in one value, a
[`Within`](crate::stdcore::features::numbering::Within), whose within-joiner takes one
of four forms:

- **A joining text.** `Within::parent("flm.headings.section")` writes the section's
  number followed by a full stop;
  `Within::parent("flm.headings.section").joined_by("-")` writes it followed by a
  dash. This is the usual form; the shipped `subsection`, `subsubsection` and
  `subparagraph` heading kinds use it.
- **A within template.** `Within::parent("flm.headings.section").template(text)`:
  FLM source in which `#{flm.headings.section}` is that counter's number, written in
  that counter's own counter style without prefix word and without delimiters, and
  preceded by the numbers of that counter's own ancestors; every other character is
  written as given. A counter is
  referred to by its full name: the whole text between the braces is the name, dots
  included. `#{_parent}`
  ([`PARENT_PLACEHOLDER`](crate::stdcore::features::numbering::PARENT_PLACEHOLDER))
  refers to the parent that applies (see below) without writing its name. The whole
  template is parsed as one text, so markup may surround a placeholder:
  `\textbf{#{_parent}}.` writes the section's number in bold. A placeholder may carry
  **bracket keys**, separated by commas: `#[Roman]{_parent}` writes the section's
  number in that number format instead of the one its own counter style writes, and
  `#[bare]{_parent}` writes the section's own number without the numbers of the
  section's own ancestors before it. A bracket key that is neither `bare` nor the name
  of a number format is refused when the template is compiled, and so is a `#{…}` that
  refers to a counter no feature declares. A within template writes one part per
  placeholder (see [The parts written before a
  number](#the-parts-written-before-a-number)), and one part for its text when it holds
  no placeholder at all; the empty within template writes no part.
- **Nothing of the parent.**
  `Within::parent("flm.headings.section").displayed_bare()` keeps the counter
  restarting at every section and shows its number **bare**, that is, without the
  numbers of its parents, as the bracket key `#[bare]{…}` writes an ancestor. The shipped `paragraph` heading kind is declared this way, within
  whichever of the section, subsection and subsubsection counters was advanced last,
  so a paragraph heading restarts at each of them and still reads `a`.
- **A within-joiner registered by name.**
  `Within::parent("flm.headings.section").joiner_named(name)` uses the
  [`WithinJoiner`](crate::stdcore::features::numbering::WithinJoiner) that a feature
  contributed under that name to the [service slot](crate::core::services)
  `flm.numbering.within-joiners` (a service slot is the declaration, by the feature
  that owns an interaction, of a dotted name, the type of the values the slot takes
  and its arity) ([`WITHIN_JOINERS_SLOT`](crate::stdcore::features::numbering::WITHIN_JOINERS_SLOT)),
  wrapped in a
  [`WithinJoinerDecl`](crate::stdcore::features::numbering::WithinJoinerDecl). Such a
  within-joiner writes what its own code writes; it is the extension point for a form
  that the other three cannot express. It is compiled into a
  [`CompiledWithinJoiner`](crate::stdcore::features::numbering::CompiledWithinJoiner),
  which answers two questions: which counters it writes for a given chain of
  ancestors, and the parts it writes. The recipe [Number a construct of your
  own](crate::guide::extending#number-a-construct-of-your-own) of the extending guide
  shows how a feature contributes to the numbering feature's service slots.

The constructor that starts a `Within` says **which counter is the parent**, in one
of three ways:

- `Within::parent("flm.headings.section")`: the one counter `flm.headings.section`.
- `Within::parents(["example.chapter", "flm.headings.section",
  "flm.headings.subsection"])`: any of these counters. At each point, the **parent
  that applies** is the one among them that was advanced most recently in document
  order. An equation numbered this way is `(4.1)` after a chapter heading and before
  any section, `(4.1.2)` once a section has begun, and `(4.3.2.3)` inside a
  subsection.
- `Within::parent_set("flm.headings.levels")`: any counter of a **parent set**, a
  named list of counters that a feature contributes as a
  [`ParentSetDecl`](crate::stdcore::features::numbering::ParentSetDecl) to the service
  slot `flm.numbering.parent-sets`
  ([`PARENT_SETS_SLOT`](crate::stdcore::features::numbering::PARENT_SETS_SLOT)); the
  parent that applies is chosen as for `Within::parents`. The headings feature
  contributes the parent set `flm.headings.levels`: the counters of its heading kinds,
  from the outermost level inward. A counter numbered within this parent set follows
  whichever heading was advanced last, without listing the heading kinds a registry
  declares.

The joining-text form writes the parent that applies, and a within template refers to
it as `#{_parent}`: that counter's number with the numbers of its own ancestors before
it, with the same bracket keys as a counter referred to by name (`#[bare]{_parent}`,
`#[Roman]{_parent}`). Names beginning with an underscore
([`RESERVED_PREFIX`](crate::stdcore::features::numbering::RESERVED_PREFIX)) are
reserved for the numbering feature: no counter and no parent set may be given one.

**`no_restart()`** keeps the parent written before the number and stops it from
restarting the count: with
`Within::parent("flm.headings.section").no_restart()`, the subsections read `1.1`,
`1.2`, then `2.3` after the second section.

A counter written in any of these forms is written with its own ancestors before it,
so a within-joiner refers only to the counter immediately above. A subsection numbered
within sections declares `Within::parent("flm.headings.section")`, and a theorem
numbered within subsections declares `Within::parent("flm.headings.subsection")`; the
theorem is then written `2.3.4`, because the subsection's number is written with the
section's number before it. A within template may refer to any counter written before
its own counter, and to more than one. Each counter it refers to is written with its
own ancestors before it, so a template that refers to a counter and to one of that
counter's ancestors writes the ancestor's number twice: a template that refers to both
the section and the subsection writes the section's number twice, unless it refers to
the subsection as `#[bare]{flm.headings.subsection}`.

### The parts written before a number

A compiled within-joiner
([`CompiledWithinJoiner`](crate::stdcore::features::numbering::CompiledWithinJoiner))
writes a list of [`Part`](crate::stdcore::features::numbering::Part)s. **The parts
always come first**: they are written in order before the point's own number, and the
point's own number is never inside a part. Each part lists the ancestors it represents,
each with its counter value, so that a caller writing several numbers at once can see
which parts two numbers share.

- The joining-text form gives one part per ancestor, along the whole chain (`1.` and
  then `2.`, before the point's own number `3`); the last ancestor is the parent that
  applies.
- A within template gives one part per placeholder, holding the literal text that
  follows the placeholder; the text before the first placeholder belongs to the first
  part. When the text between two placeholders is not FLM on its own, because markup
  spans a placeholder, the whole within template is one part.
- The form that writes nothing of the parent gives no part.
- A within-joiner registered by name gives the parts its code writes, possibly one
  part holding everything.

[`parts`](crate::stdcore::features::numbering::parts) returns the parts on their own,
without a number written after them.

The within-joiner and the ancestors' counter values are passed together as a
[`Parent`](crate::stdcore::features::numbering::Parent). Each ancestor is an
[`Ancestor`](crate::stdcore::features::numbering::Ancestor), which carries its own
counter value, its own counter style and its own within-joiner. The ancestors are the
whole chain, outermost first, and this order states which counters each ancestor may in
turn be written within: the ones before it in the chain. A within-joiner that refers to
a counter that is not before it in the chain makes the writing fail with
[`FormatError::UnknownAncestor`](crate::stdcore::features::numbering::FormatError::UnknownAncestor).
Each step of the writing therefore has strictly fewer counters before it than the step
before, so a chain of n ancestors takes at most n steps for any input.

```rust
use flm_core::lang::{build_language, LangInputs};
use flm_core::stdcore::backends::pres::read::recompose_source;
use flm_core::model::names::Name;
use flm_core::stdcore::features::numbering::{
    format, Ancestor, CounterStyleDecl, CounterValue, FormatOptions, Parent, Segment, Within,
};
use std::sync::Arc;

fn written(segments: &[Segment]) -> String {
    segments.iter().map(|segment| {
        let nodes: Vec<_> = segment.content().root().children().iter().collect();
        recompose_source(&nodes)
    }).collect()
}

let lang = build_language(LangInputs::new());
let plain = Arc::new(CounterStyleDecl::new("example.style.plain").compile(&lang)?);
// The counters a within template may refer to, and the within-joiners registered by
// name: none here, since the joining-text form uses none.
let counters = [Name::new("flm.headings.section"), Name::new("flm.headings.subsection")];
let within = |parent: &str| {
    Within::parent(parent).compile(&lang, &counters, &[]).expect("the joiner compiles")
};

// The section is numbered within nothing; the subsection is written within the
// section; the theorem is joined to the subsection alone, and the subsection's
// number is written with the section's number before it.
let ancestors = [
    Ancestor::new("flm.headings.section", CounterValue::new(2), Arc::clone(&plain)),
    Ancestor::new("flm.headings.subsection", CounterValue::new(3), Arc::clone(&plain))
        .within(within("flm.headings.section")),
];
let joiner = within("flm.headings.subsection");
let parent = Parent::new(joiner.as_ref(), &ancestors);

let segments = format(&CounterValue::new(4), &plain, Some(parent), &FormatOptions::new())
    .expect("the number is written");
assert_eq!(written(&segments), "2.3.4");
# Ok::<(), flm_core::stdcore::features::numbering::StyleError>(())
```

[`Within::compile`](crate::stdcore::features::numbering::Within::compile) compiles a
`Within` outside the numbering feature, as this example does; the numbering feature
compiles every declared `Within` itself when the registry is built.

## Writing several numbers at once

[`format_many`](crate::stdcore::features::numbering::format_many) writes several
counter values of one counter style as one text: `Eqs. (1–3, 7, and 9)`. The counter
values come as a list of
[`NumberedValue`](crate::stdcore::features::numbering::NumberedValue)s, each carrying
the counters it is numbered within, in the caller's order. The counter style's [number
writer](#the-number-writer) decides what is made of them. The **default writer**, used
by every counter style that selects no other number writer, works in four steps.

1. The counter values are put into **groups**. Two counter values fall into one group
   when the same within-joiner writes the same ancestors, with the same counter values
   and counted in the same scoped instances, before them, and when the two were
   counted in the same [scoped instance](#scoped-instances), or both in none
   ([`NumberedValue::in_instance`](crate::stdcore::features::numbering::NumberedValue::in_instance)).
   A group keeps the position of the first counter value that fell into it.
2. Each group is sorted, and stretches of consecutive counter values are collapsed into
   **runs**. A run never spans two groups, because a range may only join numbers that
   are written the same way. Nothing is removed: a counter value given twice is written
   twice. A counter style whose
   [`ranges`](crate::stdcore::features::numbering::CounterStyleDecl::ranges) setting is
   off (it is on by default) collapses nothing: every counter value is a run of its
   own, so `\ref{eq:a,eq:b,eq:c}` reads `Eqs. (1,2,3)`, with one link per number,
   rather than `Eqs. (1–3)`. When all the counter values form one run of exactly two
   consecutive values, that run is split into two runs of one, so the two are joined
   by `pair_mid`: `1 and 2`. A run of exactly two among other runs stays one run and is
   written with `range_pairmid`.
3. A run of one counter value is written as that number. A run of several is written as
   its two ends with a join text between them. The end of a run of three or more leaves
   out the parts written for the counters it is numbered within, and the number and
   each sub-number that are equal to the start's — `A.II.b–e` — unless the counter
   style's `repeat_parents_in_range` setting is on, in which case the end is written in
   full: `A.II.b–A.II.e`. The end of a run of exactly two is always written in full.
4. The runs are joined, and the prefix word and the delimiters are written around the
   result.

[`JoinDecl`](crate::stdcore::features::numbering::JoinDecl) holds every text used in
step 4, and the number of runs decides which of them are used:

| Runs | Written as |
|---|---|
| one | `one_pre`, the run, `one_post` |
| two | `pair_pre`, the first, `pair_mid`, the second, `pair_post` |
| three or more | `list_pre`, the runs with `list_mid` between them and `list_midlast` before the last, `list_post` |

A run of several numbers is written as `range_pre`, the first number, then
`range_pairmid` for a run of exactly two or `range_mid` for a longer one, then the last
number, then `range_post`.

A `${name}` in a join text is replaced, when the counter style is compiled, by the
text of the join key `name`, in one pass (the replacement text is not scanned again).
Any of the seventeen keys may be referred to. Four of them, `and`, `sep`, `endash` and
`empty`, are **variables**: the other join texts refer to them, so that a word such as
`and` is changed once for every join text. The defaults of the variables are `and`, `,`, `–` and `(empty)`. The
default join texts set `pair_mid` to ` ${and} `, `range_mid` to `${endash}`,
`range_pairmid` to `${sep}`, `list_mid` to `${sep} ` and `list_midlast` to
`${sep} ${and} `, and leave the others empty, which gives `1–3, 7, and 9`. The
[compact](crate::stdcore::features::numbering::JoinDecl::compact) preset sets
`pair_mid`, `range_pairmid`, `list_mid` and `list_midlast` to `,`, which gives
`1–3,7,9`.

With no counter value at all, the default writer writes the `empty` text, without the
prefix word and without the delimiters.

```rust
use flm_core::lang::{build_language, LangInputs};
use flm_core::stdcore::backends::pres::read::recompose_source;
use flm_core::stdcore::features::numbering::{
    format_many, CounterStyleDecl, CounterValue, Delimiters, FormatOptions,
    NumberedValue, PrefixWords, Segment,
};
# fn written(segments: &[Segment]) -> String {
#     segments.iter().map(|segment| {
#         let nodes: Vec<_> = segment.content().root().children().iter().collect();
#         recompose_source(&nodes)
#     }).collect()
# }

let lang = build_language(LangInputs::new());
let mut decl = CounterStyleDecl::new("example.style.equation");
decl.delimiters = Delimiters::new("(", ")");
decl.prefix_words = PrefixWords::new("Eq.~", "Eqs.~");
let style = decl.compile(&lang)?;

let write = |numbers: &[i64]| {
    let values: Vec<CounterValue> = numbers.iter().copied().map(CounterValue::new).collect();
    let written_values: Vec<NumberedValue<'_>> =
        values.iter().map(NumberedValue::new).collect();
    written(
        &format_many(&style, &written_values, &FormatOptions::new())
            .expect("written"),
    )
};

assert_eq!(write(&[3]), "Eq.~(3)");
assert_eq!(write(&[2, 1]), "Eqs.~(1 and 2)");
assert_eq!(write(&[3, 1, 2]), "Eqs.~(1–3)");
assert_eq!(write(&[9, 2, 1, 7, 3]), "Eqs.~(1–3, 7, and 9)");
assert_eq!(write(&[]), "(empty)");
# Ok::<(), flm_core::stdcore::features::numbering::StyleError>(())
```

## The number writer

The **number writer** turns the counter values a number is asked for into the content
a reader sees: the prefix word, the delimiters, the parts and the number, and, for
several counter values, the grouping, the runs and the join texts. Everything the four
steps above describe is the default writer's behavior. The within-joiner is one
extension point of the writing; the number writer is the other, and it controls more.

A counter style selects another number writer with
[`with_number_writer`](crate::stdcore::features::numbering::CounterStyleDecl::with_number_writer),
and a feature registers that number writer by name:

- [`NumberWriter`](crate::stdcore::features::numbering::NumberWriter) is the
  declaration: a name, and a `compile` method that receives a
  [`WriterCx`](crate::stdcore::features::numbering::WriterCx) (the built language, the
  name of the counter style being compiled, and a parser for the number writer's own
  texts), called once per counter style that selects the number writer, when the
  registry is built. This is where a number writer parses the markup of its own texts.
- [`CompiledNumberWriter`](crate::stdcore::features::numbering::CompiledNumberWriter)
  is what `compile` returns.
- A feature wraps the number writer in a
  [`NumberWriterDecl`](crate::stdcore::features::numbering::NumberWriterDecl) and
  contributes it to the service slot `flm.numbering.number-writers`
  ([`NUMBER_WRITERS_SLOT`](crate::stdcore::features::numbering::NUMBER_WRITERS_SLOT)).
  Two number writers of one name fail the registry build, and so does a counter style
  that selects a number writer no feature registers.

A compiled number writer has two entry points: `write_one` for one counter value, and
`write_many` for several counter values of one counter style. Every number is written
through one of them: a construct showing its own number, a reference to one point, and
a reference to several points. Each counter value arrives as a
[`NumberedValue`](crate::stdcore::features::numbering::NumberedValue): the counter
value, the counters it is numbered within, and its position among the caller's
counter values; a segment's link mark holds this position. With them come the caller's
[`FormatOptions`](crate::stdcore::features::numbering::FormatOptions), a
[`WriteCx`](crate::stdcore::features::numbering::WriteCx) and a
[`NumberOut`](crate::stdcore::features::numbering::NumberOut).

[`WriteCx`](crate::stdcore::features::numbering::WriteCx) gives a number writer the
counter style's data and the library's operations: the prefix word for a count and a
variant, the two delimiters, a counter value written in the counter style's number
template or in any number format, an ancestor's number in the ancestor's own counter
style, the join texts by name, content made from characters, and the three settings
the counter style declares — whether the prefix word belongs to the link to the first
number ([`name_in_link`](crate::stdcore::features::numbering::WriteCx::name_in_link)),
whether the end of a run repeats the parts of the counters the counter value is
numbered within
([`repeat_parents_in_range`](crate::stdcore::features::numbering::WriteCx::repeat_parents_in_range)),
and whether consecutive numbers collapse into runs at all
([`ranges`](crate::stdcore::features::numbering::WriteCx::ranges)). Two of its
operations serve a number writer that changes only part of the default behavior:

- [`parts`](crate::stdcore::features::numbering::WriteCx::parts) returns the parts
  that a counter value's within-joiner writes for the counters it is numbered within.
  The within-joiner runs when the parts are first asked for, once per chain of
  ancestors and per call, so a number writer that arranges the ancestors itself never
  runs it. A number writer compares the parts to see which of them two numbers share,
  and can then write one range across two parents, `Eqs. (V.3.1–4.2)`, which the
  default writer never does.
- [`default_one`](crate::stdcore::features::numbering::WriteCx::default_one) and
  [`default_many`](crate::stdcore::features::numbering::WriteCx::default_many) write
  what the default writer would write, so that a number writer can add to that text
  instead of rewriting it: `Eq. (5)`, followed by ` in Section V` of its own.

[`NumberOut`](crate::stdcore::features::numbering::NumberOut) collects what a number
writer writes: content or characters, each marked with the position of the counter
value it links to, or with no position. Neighboring items carrying the same mark are
merged, and each merged run of items becomes one
[`Segment`](crate::stdcore::features::numbering::Segment). The result of a number
writer of a feature's own therefore has exactly the shape of the default writer's
result.

```rust
use std::sync::Arc;

use flm_core::lang::{build_language, LangInputs};
use flm_core::lang::rewrites::userdefs::template::TemplateLanguage;
use flm_core::model::names::Name;
use flm_core::stdcore::backends::pres::read::recompose_source;
use flm_core::stdcore::features::numbering::{
    format, CompiledNumberWriter, CounterStyleDecl, CounterValue, FormatError, FormatOptions,
    NumberOut, NumberWriter, NumberedValue, Segment, StyleError, WriteCx, WriterCx,
};

fn written(segments: &[Segment]) -> String {
    segments.iter().map(|segment| {
        let nodes: Vec<_> = segment.content().root().children().iter().collect();
        recompose_source(&nodes)
    }).collect()
}

/// A writer that writes what the default writer writes, and an exclamation
/// mark after it.
#[derive(Debug)]
struct Excited;

impl NumberWriter for Excited {
    fn name(&self) -> &Name {
        static NAME: std::sync::OnceLock<Name> = std::sync::OnceLock::new();
        NAME.get_or_init(|| Name::new("example.writer.excited"))
    }

    fn compile(&self, _cx: &WriterCx<'_>) -> Result<Arc<dyn CompiledNumberWriter>, StyleError> {
        Ok(Arc::new(Excited))
    }
}

impl CompiledNumberWriter for Excited {
    fn write_one<'a>(
        &self,
        value: &NumberedValue<'a>,
        options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        cx.default_one(value, options, out)?;
        out.text("!", Some(value.index()));
        Ok(())
    }

    fn write_many<'a>(
        &self,
        values: &[NumberedValue<'a>],
        options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        cx.default_many(values, options, out)
    }
}

let lang = build_language(LangInputs::new());
// The numbering feature compiles every counter style against the number writers that
// features registered, when the registry is built; this is that step on its own.
let language = TemplateLanguage::new(&lang).expect("the template language is built");
let writers: Vec<Arc<dyn NumberWriter>> = vec![Arc::new(Excited)];
let style = CounterStyleDecl::new("example.style.item")
    .with_number_writer("example.writer.excited")
    .compile_with(&lang, &language, &writers)?;

let segments = format(&CounterValue::new(3), &style, None, &FormatOptions::new())
    .expect("the number is written");
assert_eq!(written(&segments), "3!");
# Ok::<(), flm_core::stdcore::features::numbering::StyleError>(())
```

[`CounterStyleDecl::compile`](crate::stdcore::features::numbering::CounterStyleDecl::compile)
is given no number writer, so it refuses a counter style that selects one;
[`compile_with`](crate::stdcore::features::numbering::CounterStyleDecl::compile_with)
takes the number writers to choose from, as the example shows.

## Segments and their link marks

[`format`](crate::stdcore::features::numbering::format) and
[`format_many`](crate::stdcore::features::numbering::format_many) return a list of
[`Segment`](crate::stdcore::features::numbering::Segment)s rather than one tree of
content, so that a caller can put a link around each part that needs one.

A segment's **link mark** is the position, in the caller's list, of the counter value
the segment links to (`0` for the only counter value of `format`), or no position. The
position stays the caller's even though the default writer sorts and groups the
counter values: for the counter values `3`, `1`, `2`, the segment that writes `1` links
to position `1`. The caller decides what a position refers to and turns it into a
link.

The default writer marks the segments as follows:

- `format` writes one counter value as one segment, which links to it: the whole text,
  prefix word and delimiters included, is one link.
- `format_many` makes each number link to its own counter value, and the join texts
  between numbers link to nothing. The prefix word links to the counter value of the first
  number written when the counter style's `name_in_link` setting is on (the default),
  and to nothing when it is off. With a single counter value and `name_in_link` on, the delimiters
  join the same link, so the whole text is one link.

A caller shows a segment's content by splicing it
([`Lowerer::splice`](crate::core::lowering::Lowerer::splice)). The content is a
[synthesized value](crate::guide::concepts_overview#synthesized-values), a small tree
built while a run is under way, and splicing brings it up to date and lowers it, so a
counter style's texts may hold a text style or a reference of their own.

## Identifying a point by its number

[`identifier_text`](crate::stdcore::features::numbering::identifier_text) writes the
ancestors' counter values and a point's own counter value as decimal numbers joined by
hyphens: a theorem numbered 3 within section 2 gives `2-3`. An identifier derived from a
number is built from this text. It is written in decimal digits regardless of what the
counter style writes, so that a point's identifier does not change when its counter
style does.

# The numbering feature: counters, options and the counter table

Everything above writes counter values and counts nothing. The `numbering` feature
([`Numbering`](crate::stdcore::features::numbering::Numbering)) holds the declared
counters, advances them over a document, and records the counter value of each
numbered point. The feature has no switch: every reference point that states a number
source is numbered. Whether a point states one is decided by the feature the point
belongs to, as the next section shows for the standard features.

## The counters of the standard features

The standard features declare the following counters and counter styles. The text of
a document refers to no counter by name: the heading commands, the label types and
what a reference shows do not depend on the counters' names. Counter names appear in a
document's numbering options (see [The numbering options of a
document](#the-numbering-options-of-a-document)), in a counter declaration's `Within`,
and in the counter style replacements of `NumberingConfig` (see [Replacing a declared
counter style](#replacing-a-declared-counter-style)).

- **The headings feature** declares a counter for each heading kind of its
  configuration that has one
  ([`headings::numbers`](crate::stdcore::features::headings::numbers)). The five
  shipped kinds count in `flm.headings.section`, `flm.headings.subsection`,
  `flm.headings.subsubsection`, `flm.headings.paragraph` and
  `flm.headings.subparagraph`. Each is numbered within the kind above it, except the
  section, which is numbered within nothing, and the paragraph, which is numbered
  within whichever of the three counters above it was advanced last and shows none of
  their numbers. Which kinds are numbered is set by the configuration's numbering depth,
  [`HeadingsConfig::numbering`](crate::stdcore::features::headings::HeadingsConfig::numbering),
  fixed when the registry is built: no kind by default, every kind that has a counter,
  or every kind down to a level ([`HeadingDecl::level`](crate::stdcore::features::headings::HeadingDecl::level):
  the shipped kinds are levels 1 to 5, and level 0 is for a kind above a section, such
  as a chapter). A kind that the depth leaves unnumbered still declares its counter,
  and its headings never advance it. The text written before a numbered heading's title
  is the kind's heading title template
  ([`HeadingDecl::title_number_format`](crate::stdcore::features::headings::HeadingDecl::title_number_format)),
  in which `#{number}` is the heading's number: `#{number}. ` by default. The headings
  feature also contributes the parent set `flm.headings.levels`.
- **The mathematics feature** declares the counter `flm.math.equation`, shown in the
  counter style `flm.math.style.equation`, and numbers each line of a numbering
  display-math environment (such as `equation` or `align`, not `equation*`) that
  carries no `\nonumber` and no `\tag`, when
  [`MathConfig::numbered_equations`](crate::stdcore::features::math::MathConfig::numbered_equations)
  is on (the default).
- **The lists feature** declares a counter for each numbered list kind
  ([`lists::numbers`](crate::stdcore::features::lists::numbers)): `flm.lists.enumerate`
  for the shipped `enumerate` kind. It also declares one counter style per nesting
  depth of the kind, `flm.lists.style.enumerate.1` to `.3`, each written through the
  depth's reference template (`#{arabic}`, `(#{roman})`, `#{alph}`, which write `1`,
  `(i)`, `a`), with the word `Item~` before a
  referenced number. Every list of the kind counts through its own [scoped
  instance](#scoped-instances) of the counter, and a list's own reference template — its
  `ref=`, else its `label=` — is its items' [number
  template](#a-points-own-number-template). The label written beside an item, `1.`, is
  written from the same record through the list's label template
  ([`Counters::format_in_style`](crate::stdcore::features::numbering::Counters::format_in_style)).
- **The theorems feature** declares the shared counter `flm.theorems.theorem`, in which
  every numbered kind of a theorem class that shares the counter counts, so that a
  theorem, a lemma and a definition in a row are `1`, `2`, `3`
  ([`theorems::numbers`](crate::stdcore::features::theorems::numbers)). It declares a
  counter `flm.theorems.counter.<kind>` for each numbered kind of a theorem class that
  does not share the counter, and one counter style per numbered kind,
  `flm.theorems.style.<kind>`, whose words are the kind's: `Lemma~` before one number.
  These counters are numbered within nothing. A document that numbers its theorems
  within sections says so in its numbering options, with
  `.within("flm.theorems.theorem", Within::parent("flm.headings.section"))`; a lemma
  then reads `Lemma~2.3` in its heading and in every reference to it. A kind's own
  counter is numbered within sections the same way, by its name:
  `.within("flm.theorems.counter.<kind>", …)`.
- **The floats feature** declares a counter for each float type, `flm.floats.figure`
  and `flm.floats.table` for the shipped types, numbered within nothing, and one
  counter style per float type, `flm.floats.style.<type>`
  ([`floats`](crate::stdcore::features::floats)): the float type's number template,
  `#{arabic}` by default, no delimiters, and the float type's words — `Fig.~` and
  `Figs.~`, with `Figure~` and `Figures~` as the variant `capital`, for `figure` — so
  that `\ref{figure:x}` reads `Fig.~1` and `\ref[S]{figure:x}` reads `Figure~1`. A
  float counts as its float type's numbering policy
  ([`FloatNumbering`](crate::stdcore::features::floats::FloatNumbering)) sets: only the
  floats that carry a `\label` and no `\nonumber` by default, every float without
  `\nonumber`, or none. The line under the
  float's content writes the number from the same record without the words, and joins
  the float type's caption name to it: `Figure 1`.
- **The xnotes feature** declares, for each numbered xnote kind, a counter
  `flm.xnotes.<kind>`, a counter style `flm.xnotes.style.<kind>` in which a reference
  writes the xnote's number (`Footnote~1`), and a mark style
  `flm.xnotes.style.<kind>.mark` in which the xnote mark writes it
  ([`xnotes`](crate::stdcore::features::xnotes); see [Showing a number from a lowering
  rule](#showing-a-number-from-a-lowering-rule)).

The other counters in the examples of this chapter, such as `example.theorem`, belong
to a feature author's own feature. The recipe [Number a construct of your
own](crate::guide::extending#number-a-construct-of-your-own) of the extending guide shows
a feature that declares one.

## Declaring a counter

A feature that numbers a construct declares two things when the registry is built: a
**counter**, the sequence its constructs count in, and at least one **counter style**,
which shows their numbers. It declares them into two service slots of the numbering
feature (a [service slot](crate::core::services) is the declaration, by the feature
that owns an interaction, of a dotted name, the type of the values the slot takes and
its arity):
`flm.numbering.counters`
([`COUNTERS_SLOT`](crate::stdcore::features::numbering::COUNTERS_SLOT)) and
`flm.numbering.styles`
([`STYLES_SLOT`](crate::stdcore::features::numbering::STYLES_SLOT)). It contributes one
value per call of
[`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot). Such a
value is a *peer contribution*, a contribution to a service slot that another feature
owns, which the registry build drops with one Note when that feature is not
registered. The declaring feature therefore still registers in a registry without the
numbering feature, and nothing is numbered there.

A [`CounterDecl`](crate::stdcore::features::numbering::CounterDecl) holds:

- the **counter's name**, dotted and owned by the feature that declares it, like every
  name the library registers: `flm.headings.section`, `flm.math.equation`, and a
  prefix of its own for a counter a third party declares. One feature declares a
  counter, and a name is declared once per registry. The reference points of any
  feature may count in it, and points of several kinds that count in one counter form
  one sequence;
- a one-sentence **description** of what the counter counts
  ([`docs`](crate::stdcore::features::numbering::CounterDecl::docs())), for readers of
  the registry's documentation;
- the **default counter style**, in which a point's number is shown when the point's
  number source selects no other. An ancestor's number is also written in it until a
  point advances that ancestor;
- what the counter is **numbered within**
  ([`Within`](crate::stdcore::features::numbering::Within)): the counter that restarts
  it, and the within-joiner that writes that counter's number before its own;
- for a counter counted through [scoped instances](#scoped-instances), its **nesting
  joiner**: what a point of a nested instance writes of the enclosing instance's
  number.

A counter style's name is dotted and owned the same way
(`flm.math.style.equation`), and it is separate from the counter's name, because two
features may each declare a counter style for one counter.

```rust
use flm_core::stdcore::features::numbering::{CounterDecl, CounterStyleDecl, Within};

// Theorems count in their own sequence, restart in every section, and are
// shown `2.13` — the section's number, a full stop, the theorem's number.
let counter = CounterDecl::new("example.theorem", "example.style.theorem")
    .docs("The theorems, lemmas and corollaries of the document.")
    .within(Within::parent("flm.headings.section"));

let style = CounterStyleDecl::new("example.style.theorem");
# let _ = (counter, style);
```

The numbering feature checks every declaration against every other when the registry
is built, so that a mistake fails the build instead of appearing when a document is
numbered. The registry build fails when:

- a counter, a counter style, a parent set, a within-joiner or a number writer is
  declared twice;
- the name of a counter or a parent set begins with an underscore;
- a counter's default counter style is not declared;
- a counter is numbered within a counter that is not declared, within a parent set
  that no feature registers, or within an empty list of counters, or a parent set holds
  a counter that is not declared;
- a counter's hierarchy leads back to the counter itself through any of its parents;
- a within-joiner, for one of the chains of ancestors the counter may have, writes a
  counter outside that chain, or a nesting joiner writes a counter that is neither in
  that chain nor the counter itself;
- a within-joiner or a number writer is selected by a name that no feature registers;
- a text of a counter style or a within template is not valid FLM, or a number
  template or a within template does not compile;
- a replacement in the numbering feature's configuration
  ([`NumberingConfig`](crate::stdcore::features::numbering::NumberingConfig)) refers to
  a counter style that no feature declares (see [Replacing a declared counter
  style](#replacing-a-declared-counter-style)).

## Numbering a construct: the number source

A construct is numbered through the reference points it declares. A [reference
point](crate::model::aspects::RefPoint) states a **number source**
([`NumberSource`](crate::model::aspects::NumberSource)): the counter it counts in, the
[scoped instances](#scoped-instances) of that counter it counts under where there are
any, the counter style its number is shown in, the levels of the counter it advances,
and [a number template of its own](#a-points-own-number-template) where it has one. A
point that can never be numbered states no number source.

The levels make sub-numbered groups possible. Level 0 is the number, level 1 the first
sub-number, and advancing a level returns the deeper levels to zero. Most points advance
`[0]` and are shown `(3)`, then `(4)`. The first line of a sub-numbered group advances
`[0, 1]` and is shown `(3a)`; the lines after it advance `[1]` and are shown `(3b)`,
`(3c)`; the next equation advances `[0]` and is shown `(4)`. A sub-numbered group may be
one construct carrying several points or several constructs each carrying one. A point
that advances a level its counter style declares no sub-level for is not numbered and
advances nothing; the numbering stage reports the error
`flm.numbering.undeclared-sub-level`. A point whose number source selects a counter
style that no feature declares is not numbered either, and is the error
`flm.numbering.undeclared-style`, reported at the point.

A point that counts in a counter no feature declares is the error
`flm.numbering.undeclared-counter`, reported once per run and per counter name rather
than at every point; the point is not numbered. A point that states a number source
asks to be numbered, so a registry that cannot number it is in error. A feature whose
points may stay unnumbered has a setting in its configuration under which they state no
number source: the mathematics feature's
[`numbered_equations`](crate::stdcore::features::math::MathConfig::numbered_equations)
and the headings feature's numbering depth.

## The numbering options of a document

[`NumberingOptions`](crate::stdcore::features::numbering::NumberingOptions) states, per
counter, what the counter is numbered within and the counter value it has before it is first
advanced. The options are bound on the document, and a render request may set them
again to override the document's options for that one run
([`Bind::Either`](crate::model::options::Bind::Either); see
[options](crate::model::options) and [Settings, options and
configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
Whether a point is numbered at all is not one of these options: the feature the point
belongs to decides it, as the headings feature's numbering depth does.

```rust
use flm_core::stdcore::features::numbering::{NumberingOptions, Within};

let options = NumberingOptions::new()
    // Number the equations within the sections: `(2.13)`.
    .within("flm.math.equation", Within::parent("flm.headings.section"))
    // Number the theorems in one sequence through the whole document.
    .within_nothing("example.theorem")
    // A subsection written before the first section is `0.1`.
    .initial("flm.headings.section", 0);
assert_eq!(options.of("flm.headings.section").and_then(|entry| entry.initial), Some(0));
```

**A counter has no value until it is first advanced.** A point whose number would need
an ancestor with no counter value — a subsection before any section, an equation numbered within
whichever heading was advanced last, before any heading — is not numbered and does not
advance its counter. The numbering stage reports the error
`flm.numbering.parent-uninitialized` once per counter and run, at the first such point.
This keeps a number from ever being shown with a part that does not exist.

`initial(counter, value)` gives a counter a counter value before its first advance, under every
combination of its ancestors' counter values, and the first advance starts from it:

- `initial("flm.headings.section", 0)` makes a subsection before any section `0.1`;
- `initial("flm.headings.section", 3)` makes the first section `4`;
- `initial("flm.headings.subsection", 0)` makes a `\subsubsection` written in a section
  that has no subsection read `1.0.1`.

When several parents have initial values and none has been advanced yet, the innermost
one counts as the most recently advanced. `within_nothing` detaches a counter from a
hierarchy and gives it a sequence of its own.

FLM has no command that sets a counter to a value in the middle of a document, because
the effects of such a command are structure or configuration: a counter restarts
because its parent advances, `0.1` before the first section is an `initial` value, and
a count that continues across a parent is `no_restart`. A construct that is not
numbered states no number source and advances no counter, so the constructs after it
continue from the last numbered counter value. A heading written with a star or with `numbered=false` behaves this way:
it takes no number, and the headings after it continue from the last numbered one.

What the numbering options ask for is checked once per run, before any point is
counted:

- Options for a counter that no feature declares change nothing and are the warning
  `flm.numbering.unknown-counter`. The same warning is reported when the options number
  a counter within a counter that no feature declares; the counter then keeps the
  hierarchy it was declared with.
- Options that number a counter within a parent set that no feature registers are the
  error `flm.numbering.unknown-parent-set`, and the counter keeps the hierarchy it was
  declared with.
- Options whose within-joiner selects a name that no feature registers are the error
  `flm.numbering.unknown-within-joiner`. Options whose joining text or within template
  does not compile, or that number a counter within an empty list of counters, are the
  error `flm.numbering.bad-within-joiner`. In both cases the counter keeps the
  hierarchy it was declared with.
- Options whose within-joiner writes, for one of the chains the counter may have, a
  counter outside that chain are the error `flm.numbering.not-an-ancestor`, and the
  counter keeps the hierarchy it was declared with. For example,
  `#{flm.headings.subsection}` is refused for a counter numbered within any of the
  section and subsection counters, because under the chain that ends with the section
  it refers to a counter that is not there. When the options move a counter's ancestors
  so that the counter's *declared* within-joiner or nesting joiner writes a counter
  outside a chain, the same error is reported and the counter is not numbered in that
  run.
- Options that make counters numbered within each other are the error
  `flm.numbering.hierarchy-cycle`; none of those counters, and no counter whose
  hierarchy leads into them, is numbered in that run.

## The numbering stage and the counter table

One stage, `flm.numbering.number`
([`NUMBER`](crate::stdcore::features::numbering::NUMBER)), reads every reference point
of the document in document order and advances the counters. It reads the points from
the table `flm.refpoints`
([`REFPOINTS`](crate::stdcore::features::refpoints::REFPOINTS)), in which the
[`refpoints`](crate::stdcore::features::refpoints) feature records, once per run, the
reference points each node's element declares. The stage requires that table: a plan that
schedules the numbering stage in a registry without the refpoints feature fails, and
its error refers to the table. It reads
nothing of the references feature, so a registry without references still numbers.

The stage writes the counter table `flm.counters`
([`COUNTERS`](crate::stdcore::features::numbering::COUNTERS), a
[`Counters`](crate::stdcore::features::numbering::Counters)). For every point it read,
the table holds a [`CounterRecord`](crate::stdcore::features::numbering::CounterRecord)
— the counter, the counter value, the counters it is numbered within with their own
counter values, the counter style, and the scoped instance it was counted in, if any —
or nothing for a point that this document does not number. A point the table does not
hold at all is a point the stage never read, such as a point inside a synthesized value;
[`Counters::lookup`](crate::stdcore::features::numbering::Counters::lookup) tells the
two cases apart.

A counter's count is kept separately for **each list of ancestors it is advanced
under, each ancestor identified by its name and its counter value**, and for each
scoped instance it is counted in. A list of ancestors encountered for the first time
starts at zero, or at the counter's `initial` value; a list encountered again continues
from where it was. This one rule produces both restarting and continuing. `no_restart` leaves the
parent out of the list, so the count continues across the parent's advances.

The table stores no text a reader sees: a record holds a counter value, and the number
is written from it where it is shown.

## Scoped instances

Most counters have **one sequence** of counter values through the whole document:
every section counts in `flm.headings.section`, and the count restarts only when a
parent advances. Some constructs need a sequence of their own for each occurrence of an
enclosing construct: each list of a document numbers its items from one, and a list
inside an item of another list numbers its own items from one again, within the item
that contains it. A counter used this way is counted through **scoped instances**. A
scoped instance is one independent sequence of the counter's counter values, attached to a node
(the environment that contains the points) and valid within that node's content.
Nothing is allocated or counted to identify an instance: an instance is identified by
its node, as a reference point is identified by its node.

A point lists the instances it counts under in its number source,
[`NumberSource::instances`](crate::model::aspects::NumberSource::instances): the nodes
that enclose the point and each hold a sequence of its counter, outermost first, the
last being the point's own instance. An empty list means the counter's one sequence,
in which every other point counts. The rule of the previous section (one count per list
of ancestors) applies, the instances being part of what a count is kept under:

- **Sibling instances** are two sequences, and each starts again: at zero, or at the
  counter's `initial` value.
- **The outermost instance** is numbered within the counters the counter is
  numbered within: nothing by default, or the sections where the declaration or a document's
  options say so. The items of `enumerate` lists numbered within sections read `1.1.`,
  `1.2.` beside the items and `Item~1.1` in a reference, and start again at `2.1.` in
  the next section.
- **An instance nested in another instance** of the same counter is numbered within
  the enclosing instance. The nested point's record holds, as its last ancestor, the
  counter value the enclosing instance has where the point is, in the counter style that
  counter value was written in, and with the enclosing instance's own instance. The
  enclosing instance's own sequence is not changed, and continues after the nested one.
- What a nested point writes of the enclosing counter value is the counter's **nesting
  joiner**,
  [`CounterDecl::nesting`](crate::stdcore::features::numbering::CounterDecl::nesting): a
  within-joiner in any of its four forms
  ([`WithinJoinerSpec`](crate::stdcore::features::numbering::WithinJoinerSpec)),
  compiled when the registry is built. By default it writes nothing, so in a list kind
  whose nested items read `(a)`, `(b)`, the second item of a list nested in the second
  item of another list reads `(b)`. The enclosing counter value is written in its own
  counter style, number template included: where the enclosing items are recorded as
  `1`, `2` (as the items of `enumerate` are, through its reference template),
  `Template("#{_parent}.")` and `Joined(".")` both make it `2.(b)`; where they are
  recorded as `1.`, `2.`, the full stop is already written, and `Template("#{_parent}")`
  makes it `2.(b)`. In a nesting joiner's template, `#{_parent}` and the counter's own
  name both refer to the enclosing instance. A nesting joiner may refer only to the
  counter itself and the counters it is numbered within; any other counter fails the
  registry build. Options that move the counter so that its nesting joiner refers to a
  counter that is no longer above it leave the counter unnumbered for the run
  (`flm.numbering.not-an-ancestor`).

Because the enclosing counter value is an ancestor like any other, the whole path that
writes numbers applies unchanged: the bare or joined form, the prefix word, and several
numbers written at once. Counter values of two instances never fall into one group of
the default writer, because two instances are two sequences regardless of what is
written before them: references to three items of one list read `Items~(a)–(c)`, and
references to the first items of two lists read `Items~(a),(a)`.

The numbering stage reads the points node by node in document order, the points of one
node together, so the counter value a nested point receives for an enclosing instance is
the one that instance has when the stage reaches the nested point's node. A construct
whose points are placed around other numbered constructs, such as the items of a list
around the lists nested in them, must therefore declare each of those points on the node
where it occurs.

A point whose instances do not enclose it one within the next — an instance that does
not contain the point, the node that carries the point itself, two instances in the
wrong order, a node of another tree — is not numbered and is the error
`flm.numbering.instance-not-enclosing`, reported at the point. A nested point read
before any point of its enclosing instance was numbered has an ancestor with no counter value,
and is the error `flm.numbering.parent-uninitialized`. Numbering another counter within a counter that
is counted through instances is not supported: such a counter sees only the counter's
one sequence, which the instances do not advance.

## A point's own number template

A point may carry a number template of its own,
[`NumberSource::number_format`](crate::model::aspects::NumberSource::number_format),
which writes this point's number instead of the number template of its counter style. It
is always read as a `Formats` number template (`#{name}`), for example
`(#{alph})` or `#{Roman}.`. It is the author's input, as the text of a `\tag` is: a
list whose author wrote `ref=(#{alph})`, or `label=(#{alph})` alone, gives that template
to each of its items, and the point passes the text on as written.

The numbering stage compiles each distinct text once per run for each counter style,
against the built language, and records the point in a **derived counter style**: the
counter style the point selects, with its number template replaced
([`CounterStyle::with_number_format`](crate::stdcore::features::numbering::CounterStyle::with_number_format)),
under the same name and with the same words, delimiters and join texts. The record holds
the derived counter style in the place where a record always holds the counter style
its number is written in. Everything that writes a number from the record therefore
writes it through the template — a reference to the point (`Item~(b)`), an enclosing
counter value written before a nested one — and nothing else changes: the point counts
as it would without the template.

A consumer that shows the number in another form writes it from the same record, in a
counter style it derives from the record's counter style with its own template, through
[`Counters::format_in_style`](crate::stdcore::features::numbering::Counters::format_in_style).
The lists feature writes the label beside an item this way: `2.` beside the item that a
reference shows as `Item~2`.

When several numbers are written at once, points are grouped by the counter style
*value* their records carry, not by its name: the points of one template and one counter
style form one group, whose consecutive counter values collapse into a range, and the
points written in the declared counter style form another group. A template that does
not compile is the error `flm.numbering.bad-number-format`, reported once per run for
each template text and counter style, and every point carrying it is numbered in the
declared counter style.

## Showing a number from a lowering rule

A lowering rule that shows a number reads the counter table `flm.counters`, asks it for
the number of the point it is showing, and stages the content it receives (to stage a
node is to build it into the lowered tree; see
[`Lowerer`](crate::core::lowering::Lowerer)):

- [`Counters::format`](crate::stdcore::features::numbering::Counters::format) writes
  one point's number, and returns `None` for a point the table holds no number for.
- [`Counters::format_many`](crate::stdcore::features::numbering::Counters::format_many)
  writes several points' numbers, one group per counter style. Each group keeps the
  position of the first point that fell into it, and each group is written in one call
  to that counter style's number writer, so a range never spans two counter styles. It
  returns the groups separately
  ([`ManyNumbers`](crate::stdcore::features::numbering::ManyNumbers)) and does not join
  them: the caller joins them. The `\ref` rule, for instance, makes each group an item
  of its own list, joined by the references feature's own texts. The points the table
  holds no number for are returned separately, in the caller's order.

Both return [`NumberSegment`](crate::stdcore::features::numbering::NumberSegment)s: a
tree of content, and the point that content links to, or no point. A rule stages each
segment's content with [`Lowerer::splice`](crate::core::lowering::Lowerer::splice),
which brings the content up to date ([value
catch-up](crate::guide::concepts_overview#value-catch-up)) and lowers it through the
lowering rules, and makes the link from the point, through the anchor the point was
given, for instance. Because the content is spliced, a counter style's texts may hold
a text style or a reference of their own.

A rule may also show a point's number in any declared counter style, selected by name,
instead of its record's counter style:
[`Counters::style`](crate::stdcore::features::numbering::Counters::style) returns the
compiled counter style of that name (the host's replacement, where
[`NumberingConfig`](crate::stdcore::features::numbering::NumberingConfig) replaced it),
and
[`Counters::format_in_style`](crate::stdcore::features::numbering::Counters::format_in_style)
writes the point's recorded counter value in it, preceded, as in every counter style, by
the parts of the counters the point is numbered within, as the same segments. The
counter style must declare at least as many sub-levels as the counter value carries.

The xnote mark is the worked case ([`xnotes`](crate::stdcore::features::xnotes)). A
numbered xnote kind declares, beside the counter style in which a reference writes its
number, a *mark style* `flm.xnotes.style.<kind>.mark`: the kind's mark template,
`#{arabic}` by default, with no word and no delimiters. The xnote's rule reads it by
name with `Counters::style` and writes the xnote's own number in it
([`stage_mark_text`](crate::stdcore::features::xnotes::stage_mark_text)). An xnote mark
locates the note on the page rather than giving its position in the document, so the
rule writes the recorded counter value alone, with the function
[`format`](crate::stdcore::features::numbering::format) and no parent: a footnote
numbered within sections, which a reference shows as `Footnote~2.1`, has the mark `1`.
A host that wants another mark replaces the mark style by name, as the next section
shows: `[#{arabic}]` makes the first mark `[1]`.

A rule that reads the counter table declares the table as a peer table
([`FeatureBuilder::peer_table`](crate::core::registry::FeatureBuilder::peer_table)) and
reads it as an optional read
([`LoweringDecl::reads_opt`](crate::core::lowering::LoweringDecl::reads_opt)), so that
the same rule still works in a registry that holds no numbering feature. Where the
numbering feature is registered, the plan schedules the numbering stage for the rule.

## Replacing a declared counter style

[`NumberingConfig`](crate::stdcore::features::numbering::NumberingConfig), the
`numbering` field of [`StdcoreConfig`](crate::stdcore::StdcoreConfig), replaces a
declared counter style by name. A host uses it to change the word before a number, the
number template or the delimiters, without writing a feature. A replacement that refers
to a counter style no feature declares fails the registry build, so a misspelled name is
reported once instead of changing nothing. The configuration is fixed when the registry
is built ([Settings, options and
configuration](crate::guide::concepts_overview#settings-options-and-configuration)),
because every text of a counter style is parsed then.

```rust
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::numbering::{
    CounterStyleDecl, Delimiters, NumberingConfig, PrefixWords,
};

let mut equations = CounterStyleDecl::new("flm.math.style.equation");
equations.delimiters = Delimiters::new("[", "]");
equations.prefix_words = PrefixWords::new("Gl.~", "Gln.~");
let config = StdcoreConfig::new().numbering(NumberingConfig::new().style(equations));
assert_eq!(config.numbering.styles.len(), 1);
```

A replacement is a whole counter style declaration: a field it leaves out takes the
default of [`CounterStyleDecl::new`](crate::stdcore::features::numbering::CounterStyleDecl::new),
not the value the replaced declaration had.
