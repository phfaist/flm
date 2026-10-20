//! The configuration of the [`theorems`](super) feature, [`TheoremsConfig`]:
//! the theorem classes ([`TheoremClass`]), the theorem kinds
//! ([`TheoremKind`]), the number format of every kind's number, and the label
//! types a numbered kind accepts; with the three shipped sets of classes and
//! kinds and the constants they use.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use crate::model::names::Name;
use crate::stdcore::features::numbering::{NumberTemplate, PluralWords, PrefixWords};

/// The heading template of every shipped theorem class,
/// `#{name}\IfValueT{#{title}}{ (#{title})}`: the kind's word and number,
/// then the title between parentheses where the theorem has one, as in
/// `Lemma 4 (Key lemma)`, or `Lemma 4` alone ([`TheoremClass::heading`]
/// describes the placeholders).
pub const DEFAULT_HEADING: &str = r"#{name}\IfValueT{#{title}}{ (#{title})}";

/// The closing mark of the shipped theorem class `prooflike`, `□`
/// ([`TheoremClass::closing_mark`]).
pub const DEFAULT_CLOSING_MARK: &str = "\u{25a1}";

/// The label types a numbered theorem kind accepts by default
/// ([`TheoremsConfig::label_types`]): `thm`, `prop`, `cor`, `lem`, `rem`,
/// `def`, `dfn`, `x` and `topic`. Every numbered kind accepts every one of
/// them: `\label{lem:x}` on a `theorem` is accepted.
pub const DEFAULT_LABEL_TYPES: [&str; 9] =
    ["thm", "prop", "cor", "lem", "rem", "def", "dfn", "x", "topic"];

/// The name of the variant of a theorem kind's words written all in
/// lowercase, `lowercase`: `\ref[lowercase]{lem:x}` reads `lemma~4`. The
/// references feature defines the name.
pub const LOWERCASE_VARIANT: &str = crate::stdcore::features::refs::LOWERCASE_VARIANT;

/// The name of the variant of a theorem kind's words written with a capital
/// initial, `capital`: the variant that `\ref[S]{…}` and `\ref[capital]{…}`
/// ask for. The references feature defines the name.
pub const CAPITAL_VARIANT: &str = crate::stdcore::features::refs::CAPITAL_VARIANT;

/// The name of the variant of a theorem kind's words written abbreviated,
/// `abbreviated`: `\ref[abbreviated]{lem:x}` reads `Lem.~4` where the kind
/// declares that variant. The references feature defines the name.
pub const ABBREVIATED_VARIANT: &str = crate::stdcore::features::refs::ABBREVIATED_VARIANT;

/// One **theorem class**: what a family of theorem kinds shares. A class
/// says whether its kinds are numbered and in which counter, how their
/// heading is written, whether a closing mark ends them, and whether a title
/// beginning with `*` is read as a back-reference to the statement a theorem
/// proves.
///
/// A class is declared under its name in [`TheoremsConfig::classes`], and a
/// kind refers to it by that name ([`TheoremKind::class`]). The three shipped
/// classes are `theoremlike` and `definitionlike`, numbered in the shared
/// counter, and `prooflike`, unnumbered, closed by the mark `□` and reading a
/// back-reference from its title (see [`TheoremsConfig`]). The constructors
/// [`numbered`](TheoremClass::numbered) and
/// [`unnumbered`](TheoremClass::unnumbered) start a class, and the builders
/// set its other fields.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::theorems::{TheoremClass, DEFAULT_HEADING};
///
/// // Numbered, each kind in a counter of its own, the title joined by "of".
/// let exercise = TheoremClass::numbered()
///     .shared_counter(false)
///     .heading(r"#{name}\IfValueT{#{title}}{ of #{title}}");
/// assert!(exercise.numbered && !exercise.shared_counter);
///
/// // Unnumbered, with a closing mark, reading `[*lem:x]` as a back-reference.
/// let prooflike = TheoremClass::unnumbered()
///     .closing_mark(Some(r"\(\blacksquare\)".to_string()))
///     .relation_ref(true);
/// assert_eq!(prooflike.heading, DEFAULT_HEADING);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TheoremClass {
    /// Whether the kinds of this class are numbered. A theorem of a numbered
    /// kind is one reference point numbered in the class's counter; a theorem
    /// of an unnumbered kind is a reference point that nothing numbers and that
    /// accepts no label.
    pub numbered: bool,
    /// For a numbered class, whether its kinds count in the shared counter
    /// `flm.theorems.theorem` (`true`), or each kind in a counter of its own,
    /// `flm.theorems.counter.<kind>` (`false`); see
    /// [`numbers`](super::numbers). Unused for an unnumbered class.
    pub shared_counter: bool,
    /// The **theorem heading template**: how the heading that opens every
    /// theorem of the class joins the kind's word and number with the
    /// theorem's title. It is written in the [template
    /// language](crate::guide::concepts_overview#the-template-language-shared-by-features)
    /// (the language guide describes it for authors under [the template
    /// language](crate::guide::language_syntax#the-template-language)), with
    /// two placeholders:
    ///
    /// - `#{name}` is the kind's word and the theorem's number (`Lemma~4`), or
    ///   the word alone where the theorem has no number;
    /// - `#{title}` is the theorem's title, or, for a class that reads
    ///   back-references, the statement a proof refers to. A theorem with no
    ///   title, or whose back-reference is not shown, binds nothing to
    ///   `#{title}`, so `\IfValueT{#{title}}{…}` tests whether there is one.
    ///   The template may show `#{title}` at most once; an occurrence as the
    ///   test of a conditional (`\IfValueT{#{title}}`) is not counted.
    ///
    /// [`DEFAULT_HEADING`] by default. A template that does not compile fails
    /// the registry build ([`heading`](super::heading)). The recomposed LaTeX
    /// does not use the template: LaTeX writes the heading
    /// ([`latex`](super::latex)).
    pub heading: String,
    /// The closing mark written at the end of every theorem of the class, as
    /// FLM markup (`□` for the shipped class `prooflike`), or `None` for no
    /// mark. A mark that does not parse fails the registry build.
    pub closing_mark: Option<String>,
    /// Whether a title beginning with `*` is read as a back-reference to the
    /// statement the theorem proves: `[*lem:x]` shows the statement's word
    /// and number in the heading, and `[**lem:x]` refers to the same
    /// statement and shows nothing in the heading
    /// ([`title_reading`](super::title_reading)). A class that does not read
    /// back-references takes every title as a title.
    pub relation_ref: bool,
}

impl TheoremClass {
    /// A numbered class counting in the shared counter, with the heading
    /// template [`DEFAULT_HEADING`], no closing mark, and titles that are
    /// never read as back-references.
    pub fn numbered() -> TheoremClass {
        TheoremClass {
            numbered: true,
            shared_counter: true,
            heading: DEFAULT_HEADING.to_string(),
            closing_mark: None,
            relation_ref: false,
        }
    }

    /// An unnumbered class, with the heading template [`DEFAULT_HEADING`], no
    /// closing mark, and titles that are never read as back-references.
    pub fn unnumbered() -> TheoremClass {
        TheoremClass { numbered: false, ..TheoremClass::numbered() }
    }

    /// Builder: whether the kinds count in the shared counter.
    pub fn shared_counter(mut self, shared_counter: bool) -> TheoremClass {
        self.shared_counter = shared_counter;
        self
    }

    /// Builder: sets the heading template.
    pub fn heading(mut self, heading: impl Into<String>) -> TheoremClass {
        self.heading = heading.into();
        self
    }

    /// Builder: sets the closing mark (FLM markup), or `None` for no mark.
    pub fn closing_mark(mut self, closing_mark: Option<String>) -> TheoremClass {
        self.closing_mark = closing_mark;
        self
    }

    /// Builder: whether a title beginning with `*` is read as a
    /// back-reference.
    pub fn relation_ref(mut self, relation_ref: bool) -> TheoremClass {
        self.relation_ref = relation_ref;
        self
    }
}

/// One **theorem kind**: one environment an author writes, such as
/// `theorem`, `lemma`, `proof` or a host's `exercise`. Its key in
/// [`TheoremsConfig::kinds`] is the environment's name.
///
/// A kind declares its theorem class, its words, and whether each of its
/// statements expects a proof.
///
/// The words are a [`PrefixWords`] value, the numbering feature's words
/// written before a number: a base pair (the singular and the plural), which
/// the heading and a plain `\ref` write, and named variants, each a pair,
/// which a reference asks for by name (`\ref[abbreviated]{lem:x}`). The base
/// pair of every shipped kind is the capitalized form (`Lemma`, `Lemmas`),
/// and every shipped kind declares the variants [`LOWERCASE_VARIANT`],
/// [`CAPITAL_VARIANT`] and [`ABBREVIATED_VARIANT`]. The words are FLM markup
/// and hold no space before the number: the kind's counter style writes a
/// non-breaking space after each ([`numbers`](super::numbers)). The base
/// singular word is also the word a heading shows alone where the theorem
/// has no number, and the display name the recomposed LaTeX gives the
/// environment in its declaration (`Lemma` in `\newtheorem{lemma}{Lemma}`).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::{PluralWords, PrefixWords};
/// use flm_core::stdcore::features::theorems::TheoremKind;
///
/// // The two regular forms derived from one word, no abbreviation.
/// let lemma = TheoremKind::from_word("theoremlike", "lemma");
/// assert_eq!(lemma.words.base, PluralWords::new("Lemma", "Lemmas"));
/// assert_eq!(lemma.words.variants["lowercase"], PluralWords::new("lemma", "lemmas"));
/// assert!(!lemma.words.variants.contains_key("abbreviated"));
///
/// // Every form written out, with a variant of the host's own naming.
/// let exercise = TheoremKind::new(
///     "definitionlike",
///     PrefixWords::new("Exercise", "Exercises")
///         .with_variant("short", PluralWords::new("Ex.", "Exs.")),
/// );
/// assert!(!exercise.expects_proof);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TheoremKind {
    /// The name of the kind's theorem class, a key of
    /// [`TheoremsConfig::classes`].
    pub class: Name,
    /// The kind's words: the word written before the number, singular and
    /// plural, with its named variants.
    pub words: PrefixWords,
    /// Whether each statement of this kind should have a proof that refers
    /// back to it, unless the statement carries `\noproofref`. `false` unless
    /// set. The feature's check reports a statement without such a proof
    /// ([`check`](super::check)).
    ///
    /// A proof refers back to a statement through one of its labels, so a
    /// kind that expects a proof must accept a label: it must be a kind of a
    /// numbered class, under a configuration declaring at least one of the
    /// [`label_types`](TheoremsConfig::label_types). A kind of an unnumbered
    /// class accepts no label and could never be proved, so
    /// [`TheoremsConfig::validate`] refuses such a configuration when the
    /// registry is built.
    pub expects_proof: bool,
}

impl TheoremKind {
    /// A kind of the class `class` with the words `words`, expecting no
    /// proof.
    pub fn new(class: impl Into<Name>, words: PrefixWords) -> TheoremKind {
        TheoremKind { class: class.into(), words, expects_proof: false }
    }

    /// A kind of the class `class`, expecting no proof, whose words are
    /// derived from the single lowercase word `word`: the plural adds an `s`,
    /// and the capitalized forms capitalize the first letter (`lemma`,
    /// `lemmas`, `Lemma`, `Lemmas`). The capitalized pair is the base, and the
    /// two pairs are the variants [`LOWERCASE_VARIANT`] and
    /// [`CAPITAL_VARIANT`]. No [`ABBREVIATED_VARIANT`] is declared: to declare
    /// an abbreviation, write the words out with [`TheoremKind::new`]. A
    /// `\ref[abbreviated]{…}` to a kind made with this function is reported by
    /// the references feature as an Error, because the kind does not declare
    /// the variant.
    pub fn from_word(class: impl Into<Name>, word: &str) -> TheoremKind {
        let plural = format!("{word}s");
        TheoremKind::new(class, regular(word, &plural))
    }

    /// Builder: whether each statement of this kind expects a proof.
    pub fn expects_proof(mut self, expects_proof: bool) -> TheoremKind {
        self.expects_proof = expects_proof;
        self
    }
}

/// `text` with its first letter capitalized.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The words of the lowercase pair `one`, `other`: the capitalized pair as
/// the base, and the two pairs as the variants `lowercase` and `capital`.
fn regular(one: &str, other: &str) -> PrefixWords {
    let capital = PluralWords::new(capitalized(one), capitalized(other));
    PrefixWords { base: capital.clone(), variants: BTreeMap::new() }
        .with_variant(LOWERCASE_VARIANT, PluralWords::new(one, other))
        .with_variant(CAPITAL_VARIANT, capital)
}

/// The configuration of the [`Theorems`](super::Theorems) feature: the
/// theorem classes and the theorem kinds, the number format of every kind,
/// and the label types every numbered kind accepts.
///
/// Every kind is an environment of its own name, `\begin{lemma}…\end{lemma}`,
/// with no code written for it: a host adds `exercise` by declaring it here.
///
/// The default ([`TheoremsConfig::new`]) declares three classes and eight
/// kinds:
///
/// | class | numbered | counter | closing mark | back-reference | kinds |
/// |---|---|---|---|---|---|
/// | `theoremlike` | yes | shared | none | no | `theorem`, `proposition`, `lemma`, `corollary`, `conjecture` |
/// | `definitionlike` | yes | shared | none | no | `definition`, `remark` |
/// | `prooflike` | no | — | `□` | yes | `proof` |
///
/// with the heading [`DEFAULT_HEADING`] for every class. `theorem`,
/// `proposition`, `lemma` and `corollary` expect a proof; the others do not.
/// [`simpleset`](TheoremsConfig::simpleset) and
/// [`richset`](TheoremsConfig::richset) are two other complete sets a host
/// may start from. A host edits any of them through the builders:
///
/// ```
/// use flm_core::stdcore::features::numbering::{PluralWords, PrefixWords};
/// use flm_core::stdcore::features::theorems::{TheoremKind, TheoremsConfig};
///
/// let config = TheoremsConfig::new()
///     .kind(
///         "exercise",
///         TheoremKind::new(
///             "definitionlike",
///             PrefixWords::new("Exercise", "Exercises")
///                 .with_variant("short", PluralWords::new("Ex.", "Exs.")),
///         ),
///     )
///     .without_kind("conjecture");
/// assert_eq!(config.kinds.len(), 8);
/// assert!(config.validate().is_ok());
/// ```
///
/// The value is what [`Theorems::new`](super::Theorems::new) takes, and a
/// host passes it through
/// [`StdcoreConfig::theorems`](crate::stdcore::StdcoreConfig::theorems). It is
/// the feature's configuration, fixed when the registry is built (see
/// [settings, options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)),
/// because the environments are part of the language and the counters and
/// counter styles the feature declares depend on it. For the same reason it
/// enters the registry digest ([`digest_bytes`](TheoremsConfig::digest_bytes)).
///
/// The configuration is checked when the registry is built. The feature's
/// registration fails, with a message that identifies the class or the kind,
/// on what [`validate`](TheoremsConfig::validate) refuses: a kind whose class
/// the configuration does not declare, a class or kind name that is not a
/// valid environment name, or a kind that expects a proof but accepts no
/// label. Once the language is built, the registry build also fails on a
/// heading template that does not compile, a closing mark that does not
/// parse and a word that does not parse. A kind whose environment name another
/// feature already declares is reported as any clash of names in the
/// registry is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TheoremsConfig {
    /// The theorem classes, by name.
    pub classes: BTreeMap<Name, TheoremClass>,
    /// The theorem kinds, by environment name.
    pub kinds: BTreeMap<Name, TheoremKind>,
    /// The number format every kind's number is written in, a number
    /// template of the numbering feature, `#{arabic}` by default. To write
    /// one kind's number otherwise, a host replaces that kind's counter style
    /// (`flm.theorems.style.<kind>`, see [`numbers`](super::numbers)) by name
    /// through the numbering feature's configuration.
    pub number_format: NumberTemplate,
    /// The label types that every numbered kind accepts on a `\label` written
    /// on one of its theorems; [`DEFAULT_LABEL_TYPES`] by default. A theorem
    /// of an unnumbered kind accepts no label.
    pub label_types: Vec<String>,
}

impl Default for TheoremsConfig {
    fn default() -> TheoremsConfig {
        TheoremsConfig::new()
    }
}

// The shipped classes and kinds, with the Python product's words. See
// decision rationale at [§flm-dd-dr:std-theorem-words].
impl TheoremsConfig {
    /// A configuration holding the three shipped classes and no kind.
    fn with_shipped_classes() -> TheoremsConfig {
        TheoremsConfig {
            classes: BTreeMap::new(),
            kinds: BTreeMap::new(),
            number_format: NumberTemplate::formats("#{arabic}"),
            label_types: DEFAULT_LABEL_TYPES.iter().map(|ty| ty.to_string()).collect(),
        }
        .class("theoremlike", TheoremClass::numbered())
        .class("definitionlike", TheoremClass::numbered())
        .class(
            "prooflike",
            TheoremClass::unnumbered()
                .closing_mark(Some(DEFAULT_CLOSING_MARK.to_string()))
                .relation_ref(true),
        )
    }

    /// Builder: adds the shipped kind `name` of the class `class`, whose
    /// lowercase words are `one`, `other` and whose abbreviated words are
    /// `abbreviated`.
    fn shipped(
        self,
        name: &str,
        class: &str,
        (one, other): (&str, &str),
        abbreviated: (&str, &str),
        expects_proof: bool,
    ) -> TheoremsConfig {
        let words = regular(one, other)
            .with_variant(ABBREVIATED_VARIANT, PluralWords::new(abbreviated.0, abbreviated.1));
        let kind = TheoremKind::new(class, words).expects_proof(expects_proof);
        self.kind(name, kind)
    }

    /// The four theorem-like kinds every set holds.
    fn statements(self) -> TheoremsConfig {
        self.shipped("theorem", "theoremlike", ("theorem", "theorems"), ("Thm.", "Thms."), true)
            .shipped(
                "proposition",
                "theoremlike",
                ("proposition", "propositions"),
                ("Prop.", "Props."),
                true,
            )
            .shipped("lemma", "theoremlike", ("lemma", "lemmas"), ("Lem.", "Lems."), true)
            .shipped(
                "corollary",
                "theoremlike",
                ("corollary", "corollaries"),
                ("Cor.", "Cors."),
                true,
            )
    }

    /// The kind `definition`.
    fn definition(self) -> TheoremsConfig {
        self.shipped(
            "definition",
            "definitionlike",
            ("definition", "definitions"),
            ("Def.", "Defs."),
            false,
        )
    }

    /// The kind `proof`.
    fn proof(self) -> TheoremsConfig {
        self.shipped("proof", "prooflike", ("proof", "proofs"), ("Proof", "Proofs"), false)
    }

    /// The default set of kinds, the set the Python implementation of FLM
    /// calls `defaultset`: eight kinds in the three shipped classes,
    /// `theorem`, `proposition`, `lemma`, `corollary`, `conjecture`,
    /// `definition`, `remark` and `proof` ([`TheoremsConfig`] lists them by
    /// class).
    pub fn new() -> TheoremsConfig {
        TheoremsConfig::with_shipped_classes()
            .statements()
            .shipped(
                "conjecture",
                "theoremlike",
                ("conjecture", "conjectures"),
                ("Conj.", "Conjs."),
                false,
            )
            .definition()
            .shipped("remark", "definitionlike", ("remark", "remarks"), ("Rem.", "Rems."), false)
            .proof()
    }

    /// The smallest shipped set, the set the Python implementation of FLM
    /// calls `simpleset`: six kinds, `theorem`, `proposition`, `lemma`,
    /// `corollary`, `definition` and `proof`, in the three shipped classes.
    pub fn simpleset() -> TheoremsConfig {
        TheoremsConfig::with_shipped_classes().statements().definition().proof()
    }

    /// The largest shipped set, the set the Python implementation of FLM
    /// calls `richset`: thirteen kinds, the eight of [`TheoremsConfig::new`],
    /// and `idea`, `question`, `claim`, `observation` and `problem` in the
    /// class `definitionlike`.
    pub fn richset() -> TheoremsConfig {
        let mut config = TheoremsConfig::new();
        for (name, plural, abbreviated) in [
            ("idea", "ideas", ("Idea", "Ideas")),
            ("question", "questions", ("Qtn.", "Qtns.")),
            ("claim", "claims", ("Clm.", "Clms.")),
            ("observation", "observations", ("Obs.", "Obs.")),
            ("problem", "problems", ("Prob.", "Probs.")),
        ] {
            config = config.shipped(name, "definitionlike", (name, plural), abbreviated, false);
        }
        config
    }

    /// A configuration declaring no class and no kind, with the default
    /// number format and label types: the start of a configuration built
    /// entirely by the builders.
    pub fn empty() -> TheoremsConfig {
        TheoremsConfig { classes: BTreeMap::new(), kinds: BTreeMap::new(), ..TheoremsConfig::new() }
    }

    /// Builder: declares the kind `name`, replacing the kind of that name if
    /// there is one.
    pub fn kind(mut self, name: impl Into<Name>, kind: TheoremKind) -> TheoremsConfig {
        self.kinds.insert(name.into(), kind);
        self
    }

    /// Builder: removes the kind `name`, if there is one.
    pub fn without_kind(mut self, name: &str) -> TheoremsConfig {
        self.kinds.remove(&Name::from(name));
        self
    }

    /// Builder: declares the class `name`, replacing the class of that name
    /// if there is one.
    pub fn class(mut self, name: impl Into<Name>, class: TheoremClass) -> TheoremsConfig {
        self.classes.insert(name.into(), class);
        self
    }

    /// Builder: sets the number format of every kind.
    pub fn number_format(mut self, number_format: NumberTemplate) -> TheoremsConfig {
        self.number_format = number_format;
        self
    }

    /// Builder: sets the label types that every numbered kind accepts.
    pub fn label_types(
        mut self,
        label_types: impl IntoIterator<Item = impl Into<String>>,
    ) -> TheoremsConfig {
        self.label_types = label_types.into_iter().map(Into::into).collect();
        self
    }

    /// The kind `name` and its class; `None` where the kind or its class is
    /// not declared.
    pub fn kind_and_class(&self, name: &str) -> Option<(&TheoremKind, &TheoremClass)> {
        let kind = self.kinds.get(&Name::from(name))?;
        Some((kind, self.classes.get(&kind.class)?))
    }

    /// Checks the parts of the configuration that can be checked before the
    /// language is built. The feature calls it when it registers; a host may
    /// call it earlier to report a mistake in its own terms.
    ///
    /// The texts (heading templates, closing marks, words) are compiled only
    /// once the language is built, so a text that does not compile passes
    /// this check and fails the registry build later.
    ///
    /// # Errors
    ///
    /// A sentence that describes the first problem found: a class or kind
    /// name that is empty or holds whitespace, a brace, a backslash or a
    /// comment character (`%`); a kind whose class is not declared; a kind
    /// that [expects a proof](TheoremKind::expects_proof) but accepts no
    /// label (a kind of an unnumbered class, or any kind under a
    /// configuration declaring no [`label_types`](TheoremsConfig::label_types)),
    /// which no proof could refer back to.
    pub fn validate(&self) -> Result<(), String> {
        let cannot_name = |text: &str| {
            text.is_empty()
                || text.chars().any(|ch| ch.is_whitespace() || matches!(ch, '{' | '}' | '\\' | '%'))
        };
        for name in self.classes.keys() {
            if cannot_name(name.as_str()) {
                return Err(format!(
                    "the theorem class name `{name}` must not be empty or contain whitespace, \
                     a brace, a backslash or `%`"
                ));
            }
        }
        for (name, kind) in &self.kinds {
            if cannot_name(name.as_str()) {
                return Err(format!(
                    "the theorem kind name `{name}` must not be empty or contain whitespace, a \
                     brace, a backslash or `%`, since it is the name of an environment"
                ));
            }
            if !self.classes.contains_key(&kind.class) {
                return Err(format!(
                    "the theorem kind `{name}` belongs to the class `{}`, which the \
                     configuration does not declare",
                    kind.class
                ));
            }
        }
        // A proof refers back to a statement through a label; a kind that
        // expects a proof and accepts none would draw the Warning at every
        // statement.
        for (name, kind) in self.kinds.iter().filter(|(_, kind)| kind.expects_proof) {
            let numbered = self.classes.get(&kind.class).is_some_and(|class| class.numbered);
            if !numbered {
                return Err(format!(
                    "the theorem kind `{name}` expects a proof, but its class `{}` is unnumbered \
                     and accepts no label, so no proof can refer back to it",
                    kind.class
                ));
            }
            if self.label_types.is_empty() {
                return Err(format!(
                    "the theorem kind `{name}` expects a proof, but the configuration declares no \
                     label types, so no proof can refer back to it"
                ));
            }
        }
        Ok(())
    }

    /// The configuration written out as bytes: what the feature feeds into
    /// the registry digest, and what the `Hash` implementation hashes.
    ///
    /// Every class and every kind is written in the order of its name, with
    /// every field of it, each text preceded by its length; then the number
    /// format and the label types. Two configurations give the same bytes
    /// exactly when they are equal. The code takes the fields apart by name,
    /// so a field added to a type and not written here fails to compile.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let TheoremsConfig { classes, kinds, number_format, label_types } = self;
        let mut out = Vec::new();
        push_len(&mut out, classes.len());
        for (name, class) in classes {
            let TheoremClass { numbered, shared_counter, heading, closing_mark, relation_ref } = class;
            push_text(&mut out, name.as_str());
            out.push(u8::from(*numbered));
            out.push(u8::from(*shared_counter));
            push_text(&mut out, heading);
            match closing_mark {
                Some(mark) => {
                    out.push(1);
                    push_text(&mut out, mark);
                }
                None => out.push(0),
            }
            out.push(u8::from(*relation_ref));
        }
        push_len(&mut out, kinds.len());
        for (name, kind) in kinds {
            let TheoremKind { class, words, expects_proof } = kind;
            push_text(&mut out, name.as_str());
            push_text(&mut out, class.as_str());
            push_plural(&mut out, &words.base);
            push_len(&mut out, words.variants.len());
            for (variant, plural) in &words.variants {
                push_text(&mut out, variant);
                push_plural(&mut out, plural);
            }
            out.push(u8::from(*expects_proof));
        }
        match number_format {
            NumberTemplate::Formats(text) => {
                out.push(0);
                push_text(&mut out, text);
            }
            NumberTemplate::Initial(text) => {
                out.push(1);
                push_text(&mut out, text);
            }
        }
        push_len(&mut out, label_types.len());
        for label_type in label_types {
            push_text(&mut out, label_type);
        }
        out
    }
}

/// Writes a count as eight little-endian bytes.
fn push_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&(len as u64).to_le_bytes());
}

/// Writes a text as its length and its bytes.
fn push_text(out: &mut Vec<u8>, text: &str) {
    push_len(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// Writes a pair of words.
fn push_plural(out: &mut Vec<u8>, plural: &PluralWords) {
    push_text(out, &plural.one);
    push_text(out, &plural.other);
}

impl Hash for TheoremsConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_is_capitalized_by_its_first_letter() {
        assert_eq!(capitalized("lemma"), "Lemma");
        assert_eq!(capitalized("énoncé"), "Énoncé");
        assert_eq!(capitalized(""), "");
    }

    #[test]
    fn the_digest_bytes_change_with_every_field() {
        let base = TheoremsConfig::new();
        let changed = [
            base.clone().without_kind("remark"),
            base.clone().kind("remark", TheoremKind::from_word("definitionlike", "note")),
            base.clone().kind(
                "remark",
                TheoremKind::from_word("definitionlike", "remark").expects_proof(true),
            ),
            base.clone().class("prooflike", TheoremClass::unnumbered()),
            base.clone().class("theoremlike", TheoremClass::numbered().shared_counter(false)),
            base.clone().class("theoremlike", TheoremClass::numbered().heading("#{name}")),
            base.clone().class("theoremlike", TheoremClass::numbered().relation_ref(true)),
            base.clone().class(
                "theoremlike",
                TheoremClass::numbered().closing_mark(Some(String::new())),
            ),
            base.clone().number_format(NumberTemplate::formats("#{roman}")),
            base.clone().number_format(NumberTemplate::initial("#{arabic}")),
            base.clone().label_types(["thm"]),
        ];
        for other in &changed {
            assert_ne!(other.digest_bytes(), base.digest_bytes(), "{other:?}");
        }
        assert_eq!(TheoremsConfig::new().digest_bytes(), base.digest_bytes());
    }
}
