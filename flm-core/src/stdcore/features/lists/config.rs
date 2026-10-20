//! The configuration of the [`lists`](super) feature: which list
//! environments exist, whether the items of each are numbered, and the
//! labels shown beside its items and in references to them.
//!
//! [`ListsConfig`] maps each list environment's name to its list kind
//! ([`ListKind`]). The value is the feature's configuration (see
//! [configuration](crate::guide::concepts_overview#settings-options-and-configuration)):
//! the plain Rust value the feature is constructed from, fixed when the
//! registry is built.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use crate::model::names::Name;
use crate::stdcore::backends::pres::LIST_RESERVED_KINDS;
use crate::stdcore::features::numbering::WithinJoinerSpec;

/// A list kind: what every list environment of one name is, how its items
/// are numbered and which labels are shown beside them.
///
/// A list kind states three facts:
///
/// - [`numbered`](ListKind::numbered): whether its items are numbered. The
///   items of a numbered list kind are numbered by the
///   [`numbering`](crate::stdcore::features::numbering) feature, in a counter
///   of the list kind's own (`flm.lists.<kind>`; see
///   [`CounterDecl`](crate::stdcore::features::numbering::CounterDecl)),
///   and a reference to one of them shows its number. The items of an
///   unnumbered list kind are counted in no counter.
/// - [`labels`](ListKind::labels): how the label beside an item is written,
///   one *label template* per nesting depth. The first template serves a list
///   that is inside no list of the same list kind, the second a list nested
///   in one such list, and so on; the last template serves every deeper list.
///   A label template is written in the language of a counter style's number
///   template ([`NumberTemplate`](crate::stdcore::features::numbering::NumberTemplate)):
///   every `#{name}` is the item's number in the number format `name`
///   (`#{arabic}`, `#{alph}`, `#{roman}`, …), and every other character is
///   FLM text written as is, so `(#{roman})` writes `(ii)` for the second
///   item. A template that writes no number format, such as `•`, is a *fixed
///   label*, shown beside every item.
/// - [`ref_labels`](ListKind::ref_labels): how a reference to an item writes
///   the item's number, one *reference template* per nesting depth, by the
///   same rules and in the same language; read for a numbered list kind only.
///   The two templates differ where the label carries punctuation that does
///   not belong in a sentence: `enumerate` shows `1.` beside its first item,
///   and `\ref` to that item shows `Item~1`, because its first label template
///   is `#{arabic}.` and its first reference template is `#{arabic}`. The
///   constructors [`numbered`](ListKind::numbered()) and
///   [`unnumbered`](ListKind::unnumbered()) give a list kind the same
///   templates for both; [`ref_labels`](ListKind::ref_labels()) sets the
///   reference templates separately.
///
/// Whether a list kind is numbered is set by `numbered` alone and never
/// follows from its templates. A numbered list kind may show a fixed label
/// beside each item; every item still has a number, which a reference to it
/// or another feature can read. An unnumbered list kind whose label template
/// writes a number format is refused when the registry is built, since its
/// items have no number to write. An unnumbered list kind's reference
/// templates are ignored (neither compiled nor checked), so that switching a
/// list kind between numbered and unnumbered needs no other change; a
/// reference to one of its items shows the item's label.
///
/// A numbered list kind also has a [`nesting`](ListKind::nesting) joiner:
/// what an item of a list nested in another list of the same list kind
/// writes of the enclosing item's number before its own. By default it
/// writes nothing, so the second item of a list nested in the second item of
/// an `enumerate` shows `(ii)`; the template `#{_parent}.` makes it
/// `2.(ii)`. The enclosing item's number is written through the enclosing
/// list's reference template, as a reference to that item writes it: `2` for
/// the shipped `enumerate`, not `2.`. The joiner is data of the numbering
/// feature, the same [`WithinJoinerSpec`] that a counter numbered within
/// another counter declares, and that feature compiles it; see
/// [scoped instances](crate::model::aspects::NumberSource#scoped-instances)
/// and the [numbering guide](crate::guide::numbering#scoped-instances).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::lists::{ListKind, ListsConfig};
/// use flm_core::stdcore::features::numbering::WithinJoinerSpec;
/// # use std::sync::Arc;
/// # use flm_core::core::plan::{Overrides, Plan};
/// # use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
/// # use flm_core::core::run::{Externals, Run, RunOptions};
/// # use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
/// # use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
/// # use flm_core::stdcore::backends::text::TEXT_DOCUMENT;
/// # use flm_core::stdcore::{self, StdcoreConfig};
/// # /// The text `document` renders to under the standard package with `lists`.
/// # fn text(lists: ListsConfig, document: &str) -> String {
/// #     let mut builder = RegistryBuilder::new();
/// #     builder.use_package(stdcore::package(StdcoreConfig::new().lists(lists)));
/// #     builder.use_package(flm_core::math_passthrough::package());
/// #     let registry = builder.build(&RegistryInputs::new()).expect("a registry");
/// #     let source = Arc::new(FlmSource::new(document).with_origin(FlmOrigin::none()));
/// #     let fragment = parse_fragment(
/// #         registry.lang(), source, FragmentLayout::default(), &ParseOptions::default(),
/// #     ).expect("a parse");
/// #     let mut content = DocContent::new();
/// #     content.place(Arc::new(fragment));
/// #     let plan = Plan::for_workflow(&registry, "text", &Overrides::none()).expect("a plan");
/// #     let mut run = Run::new(Arc::new(Doc::new(content)), Arc::new(plan), Externals::none())
/// #         .expect("a run");
/// #     run.execute(&RunOptions::new()).expect("executed");
/// #     let artifacts = run.table(TEXT_DOCUMENT).expect("declared").expect("written");
/// #     let bytes = artifacts.entry().and_then(|a| a.as_bytes()).expect("an entry");
/// #     String::from_utf8_lossy(bytes).trim_end().to_string()
/// # }
///
/// // Items labeled `1.`, `2.`, referred to as `Step~1`, `Step~2`; a nested
/// // list's items labeled `1.a`, `1.b`: the enclosing number is written
/// // through its reference template, then the joiner's full stop.
/// let steps = ListKind::numbered(["#{arabic}.", "#{alph}"])
///     .ref_labels(["#{arabic}", "#{alph}"])
///     .nesting(WithinJoinerSpec::Template("#{_parent}.".to_string()));
/// assert!(steps.numbered);
/// assert_eq!(steps.label_at(5), Some("#{alph}"));
/// assert_eq!(steps.ref_label_at(1), Some("#{arabic}"));
/// assert_eq!(
///     text(
///         ListsConfig::new().environment("steps", steps),
///         r"\begin{steps}\item a\begin{steps}\item\label{item:y} x\item y\end{steps}\end{steps}
/// See \ref{item:y}.",
///     ),
///     "1. a\n\n   1.a x\n\n   1.b y\n\nSee Item\u{a0}1.a.",
/// );
///
/// // A list of dashes at every depth.
/// let dashes = ListKind::unnumbered(["--"]);
/// assert_eq!(dashes.label_at(1), Some("--"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListKind {
    /// Whether the items of a list of this list kind are numbered.
    pub numbered: bool,
    /// The label templates by nesting depth, the outermost first; the last
    /// one serves every deeper list. An empty vector means that no label is
    /// shown unless the author writes one: every item must then carry a tag,
    /// as in a `description` list. A numbered list kind has at least one.
    pub labels: Vec<String>,
    /// The reference templates by nesting depth, the outermost first; the
    /// last one serves every deeper list. For a numbered list kind, each is
    /// the number format of the counter style of its depth: it is what a
    /// reference to an item shows (`Item~1`) and what the nesting joiner
    /// writes of an enclosing item. A numbered list kind has at least one.
    /// For an unnumbered list kind they are ignored: its items have no
    /// number, and a reference to one of them shows its label.
    pub ref_labels: Vec<String>,
    /// For a numbered list kind, what an item of a nested list of this list
    /// kind writes of the enclosing item's number; unused for an unnumbered
    /// one. [`WithinJoinerSpec::Bare`], which writes nothing, by default.
    pub nesting: WithinJoinerSpec,
}

impl ListKind {
    /// A numbered list kind that writes `labels`, by depth, beside its items
    /// and in a reference to one of them, with a nesting joiner that writes
    /// nothing of the enclosing item. [`ref_labels`](ListKind::ref_labels())
    /// sets the reference templates separately.
    pub fn numbered(labels: impl IntoIterator<Item = impl Into<String>>) -> ListKind {
        let labels: Vec<String> = labels.into_iter().map(Into::into).collect();
        ListKind { numbered: true, ref_labels: labels.clone(), labels, nesting: WithinJoinerSpec::Bare }
    }

    /// An unnumbered list kind that writes `labels`, by depth, beside its
    /// items and in a reference to one of them. No label template may write a
    /// number format: the registry build refuses one. The reference templates
    /// are set to the same texts and are ignored.
    pub fn unnumbered(labels: impl IntoIterator<Item = impl Into<String>>) -> ListKind {
        let labels: Vec<String> = labels.into_iter().map(Into::into).collect();
        ListKind { numbered: false, ref_labels: labels.clone(), labels, nesting: WithinJoinerSpec::Bare }
    }

    /// Builder: sets the reference templates by depth, which the constructors
    /// set to the label templates. Read for a numbered list kind only.
    pub fn ref_labels(mut self, ref_labels: impl IntoIterator<Item = impl Into<String>>) -> ListKind {
        self.ref_labels = ref_labels.into_iter().map(Into::into).collect();
        self
    }

    /// Builder: sets the nesting joiner, read for a numbered list kind only.
    pub fn nesting(mut self, nesting: WithinJoinerSpec) -> ListKind {
        self.nesting = nesting;
        self
    }

    /// The label template of a list at nesting depth `depth`, where depth 1
    /// is a list inside no list of the same list kind (depth 0 is read as 1).
    /// A depth past the last template gets the last template; a list kind
    /// that declares no template gets `None`.
    pub fn label_at(&self, depth: usize) -> Option<&str> {
        template_at(&self.labels, depth)
    }

    /// The reference template of a list at nesting depth `depth`, chosen as
    /// [`label_at`](ListKind::label_at) chooses a label template; `None` for a
    /// list kind that declares none.
    pub fn ref_label_at(&self, depth: usize) -> Option<&str> {
        template_at(&self.ref_labels, depth)
    }

    /// The number of nesting depths for which the list kind declares a
    /// template, label or reference: the length of the longer of its two
    /// vectors. A numbered list kind declares one counter style per such depth
    /// ([`numbers`](super::numbers)).
    pub fn depth_count(&self) -> usize {
        self.labels.len().max(self.ref_labels.len())
    }
}

/// The template of `templates` for nesting depth `depth`, 1 for the
/// outermost, the last serving every deeper depth.
fn template_at(templates: &[String], depth: usize) -> Option<&str> {
    let index = depth.saturating_sub(1).min(templates.len().checked_sub(1)?);
    templates.get(index).map(String::as_str)
}

/// The configuration of the [`Lists`](super::Lists) feature: the list
/// environments, each environment's name mapped to its list kind
/// ([`ListKind`]).
///
/// The default ([`ListsConfig::new`]) declares three list kinds:
///
/// | environment | numbered | labels by depth | reference templates by depth |
/// |---|---|---|---|
/// | `itemize` | no | `•`, `-`, `▸` | ignored: a reference shows the label |
/// | `enumerate` | yes | `#{arabic}.`, `(#{roman})`, `#{alph}-` | `#{arabic}`, `(#{roman})`, `#{alph}` |
/// | `description` | no | none: every item carries its term as a tag | ignored: a reference shows the tag |
///
/// An `enumerate` therefore shows `1.`, `2.` beside its items, a list
/// nested in one of its items `(i)`, `(ii)`, and a list nested one level
/// deeper `a-`, `b-`, while references to those items show `Item~2`,
/// `Item~(ii)`, `Item~b`. A program that embeds the library declares other
/// list kinds, or changes these, through the same type:
///
/// ```
/// use flm_core::stdcore::features::lists::{ListKind, ListsConfig};
///
/// let config = ListsConfig::new()
///     .environment("steps", ListKind::numbered(["Step #{arabic}:"]))
///     .environment("itemize", ListKind::unnumbered(["--"]));
/// assert!(config.kind("steps").is_some_and(|kind| kind.numbered));
/// assert_eq!(config.kind("itemize").and_then(|kind| kind.label_at(1)), Some("--"));
/// assert_eq!(config.environments.len(), 4);
/// ```
///
/// [`Lists::new`](super::Lists::new) takes the value, and the standard
/// package carries it as
/// [`StdcoreConfig::lists`](crate::stdcore::StdcoreConfig::lists). It is the
/// feature's configuration (see
/// [configuration](crate::guide::concepts_overview#settings-options-and-configuration)):
/// fixed when the registry is built, before any document is parsed, because
/// the environments are part of the parsing language the registry builds
/// ([`BuiltLang`](crate::lang::BuiltLang)) and the counters and counter
/// styles the feature declares depend on it. The feature therefore feeds
/// [`digest_bytes`](ListsConfig::digest_bytes) into the registry digest, the
/// fingerprint that decides whether a cached result may be reused, as every
/// feature whose declarations depend on its configuration must.
///
/// The registry build checks the configuration and fails on:
///
/// - an environment name that is empty or holds whitespace, a brace, a
///   backslash or a `%` ([`ListsConfig::validate`]);
/// - an environment name that is one of the kind tokens a presentation list
///   reserves ([`LIST_RESERVED_KINDS`]; `validate`);
/// - a numbered list kind with no label template or no reference template
///   (`validate`);
/// - once the parsing language is built, a label template, or a numbered list kind's
///   reference template, that is not valid in the template language, and a
///   label template of an unnumbered list kind that writes a number format.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ListsConfig {
    /// The list kinds, by environment name.
    pub environments: BTreeMap<Name, ListKind>,
}

impl Default for ListsConfig {
    fn default() -> ListsConfig {
        ListsConfig::new()
    }
}

impl ListsConfig {
    /// The shipped configuration: the list kinds `itemize`, `enumerate` and
    /// `description` (see the type's documentation).
    // The three kinds and their labels by depth are the Python product's
    // shipped defaults; the reference templates drop the labels' trailing
    // punctuation. See decision rationale at [§flm-dd-dr:std-list-kinds].
    pub fn new() -> ListsConfig {
        ListsConfig::empty()
            .environment("itemize", ListKind::unnumbered(["\u{2022}", "-", "\u{25b8}"]))
            .environment(
                "enumerate",
                ListKind::numbered(["#{arabic}.", "(#{roman})", "#{alph}-"])
                    .ref_labels(["#{arabic}", "(#{roman})", "#{alph}"]),
            )
            .environment("description", ListKind::unnumbered(Vec::<String>::new()))
    }

    /// A configuration declaring no list environment at all.
    pub fn empty() -> ListsConfig {
        ListsConfig { environments: BTreeMap::new() }
    }

    /// Builder: declares the list environment `name` with the list kind
    /// `kind`, replacing the list kind already declared under that name, if
    /// any.
    pub fn environment(mut self, name: impl Into<Name>, kind: ListKind) -> ListsConfig {
        self.environments.insert(name.into(), kind);
        self
    }

    /// The list kind of the list environment `name`, if one is declared.
    pub fn kind(&self, name: &str) -> Option<&ListKind> {
        self.environments.get(&Name::from(name))
    }

    /// Checks the parts of the configuration that can be checked before the
    /// registry's parsing language ([`BuiltLang`](crate::lang::BuiltLang)) is
    /// built. The feature calls it when it registers, and a
    /// failure fails the registry build.
    ///
    /// # Errors
    ///
    /// A sentence describing the first problem found: an environment name
    /// that is empty or holds whitespace, a brace, a backslash or a `%`; an
    /// environment name that is one of the kind tokens a presentation list
    /// reserves ([`LIST_RESERVED_KINDS`]); a numbered list kind with no label
    /// template or no reference template.
    pub fn validate(&self) -> Result<(), String> {
        for (name, kind) in &self.environments {
            let text = name.as_str();
            if text.is_empty()
                || text.chars().any(|ch| ch.is_whitespace() || matches!(ch, '{' | '}' | '\\' | '%'))
            {
                return Err(format!(
                    "the list environment name `{text}` must not be empty or contain whitespace, a \
                     brace, a backslash or `%`"
                ));
            }
            if LIST_RESERVED_KINDS.contains(&text) {
                return Err(format!(
                    "the list environment name `{text}` is reserved: a list environment must \
                     not be named `numbered`, `unnumbered`, `terms` or `labeled`, the words that \
                     the output uses to mark the kinds of lists"
                ));
            }
            if kind.numbered && kind.labels.is_empty() {
                return Err(format!(
                    "the numbered list environment `{text}` must declare at least one label \
                     template, for its outermost lists"
                ));
            }
            if kind.numbered && kind.ref_labels.is_empty() {
                return Err(format!(
                    "the numbered list environment `{text}` must declare at least one reference \
                     template, for its outermost lists"
                ));
            }
        }
        Ok(())
    }

    /// The bytes that represent the configuration in the registry digest;
    /// the type's `Hash` implementation hashes the same bytes.
    ///
    /// Every list kind is written in the order of its name: its name, whether
    /// it is numbered, its label templates, its reference templates and its
    /// nesting joiner, each text preceded by its length. Two configurations
    /// therefore give the same bytes exactly when they are equal. Two
    /// configurations that differ only in an unnumbered list kind's reference
    /// templates, which have no effect, still give different bytes: a digest
    /// that differs without need only prevents the reuse of a cached result,
    /// and never gives a wrong one. The bytes are written field by field
    /// rather than taken from the type's `Debug` output, which would change
    /// with a field rename and silently change every registry digest.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        push_len(&mut out, self.environments.len());
        for (name, kind) in &self.environments {
            push_text(&mut out, name.as_str());
            out.push(u8::from(kind.numbered));
            push_len(&mut out, kind.labels.len());
            for label in &kind.labels {
                push_text(&mut out, label);
            }
            push_len(&mut out, kind.ref_labels.len());
            for label in &kind.ref_labels {
                push_text(&mut out, label);
            }
            kind.nesting.digest_into(&mut out);
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

impl Hash for ListsConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_kinds_are_the_three_of_the_python_product() {
        let config = ListsConfig::new();
        let names: Vec<&str> = config.environments.keys().map(Name::as_str).collect();
        assert_eq!(names, ["description", "enumerate", "itemize"]);
        let enumerate = config.kind("enumerate").expect("shipped");
        assert!(enumerate.numbered);
        assert_eq!(enumerate.labels, ["#{arabic}.", "(#{roman})", "#{alph}-"]);
        assert_eq!(enumerate.ref_labels, ["#{arabic}", "(#{roman})", "#{alph}"]);
        assert_eq!(enumerate.nesting, WithinJoinerSpec::Bare);
        let itemize = config.kind("itemize").expect("shipped");
        assert!(!itemize.numbered);
        assert_eq!(itemize.labels, ["\u{2022}", "-", "\u{25b8}"]);
        assert_eq!(itemize.ref_labels, itemize.labels);
        let description = config.kind("description").expect("shipped");
        assert!(!description.numbered);
        assert!(description.labels.is_empty());
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn the_last_template_serves_every_deeper_depth() {
        let kind = ListKind::numbered(["a", "b"]);
        assert_eq!(kind.label_at(0), Some("a"));
        assert_eq!(kind.label_at(1), Some("a"));
        assert_eq!(kind.label_at(2), Some("b"));
        assert_eq!(kind.label_at(20), Some("b"));
        assert_eq!(ListKind::unnumbered(Vec::<String>::new()).label_at(1), None);
        let kind = ListKind::numbered(["a", "b"]).ref_labels(["r"]);
        assert_eq!(kind.ref_label_at(3), Some("r"));
        assert_eq!(kind.label_at(3), Some("b"));
        assert_eq!(kind.depth_count(), 2);
    }

    #[test]
    fn a_numbered_kind_without_a_template_and_a_bad_name_are_refused() {
        let config = ListsConfig::empty().environment("steps", ListKind::numbered(Vec::<String>::new()));
        assert!(config.validate().expect_err("refused").contains("`steps`"));
        let config = ListsConfig::empty().environment("my list", ListKind::unnumbered(["-"]));
        assert!(config.validate().expect_err("refused").contains("`my list`"));
        let config = ListsConfig::empty()
            .environment("steps", ListKind::numbered(["#{arabic}."]).ref_labels(Vec::<String>::new()));
        assert!(config.validate().expect_err("refused").contains("reference template"));
    }

    #[test]
    fn the_digest_bytes_change_with_every_field() {
        let base = ListsConfig::new();
        let changed = [
            base.clone().environment("steps", ListKind::numbered(["#{arabic}"])),
            base.clone().environment("itemize", ListKind::numbered(["\u{2022}", "-", "\u{25b8}"])),
            base.clone().environment("itemize", ListKind::unnumbered(["\u{2022}", "-"])),
            base.clone().environment(
                "itemize",
                ListKind::unnumbered(["\u{2022}", "-", "\u{25b8}"]).ref_labels(["*"]),
            ),
            base.clone().environment(
                "enumerate",
                ListKind::numbered(["#{arabic}.", "(#{roman})", "#{alph}-"])
                    .ref_labels(["#{arabic}", "(#{roman})", "#{alph}"])
                    .nesting(WithinJoinerSpec::Template("#{_parent}.".to_string())),
            ),
        ];
        for other in &changed {
            assert_ne!(other.digest_bytes(), base.digest_bytes(), "{other:?}");
        }
        assert_eq!(ListsConfig::new().digest_bytes(), base.digest_bytes());
    }
}
