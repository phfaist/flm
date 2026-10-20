//! The validity check: every HTML golden of this repository, parsed the way a
//! browser parses a page, and asserted against the contract the HTML backend
//! promises.
//!
//! # What this suite is
//!
//! The HTML backend writes bytes. A golden file pins those bytes, but a golden
//! cannot say whether what it holds is *HTML* — whether a browser reading it
//! builds the structure the emitter meant to write. This suite answers that
//! question, and it is the counterpart of `tests/latex_compile.rs`, which
//! compiles the LaTeX goldens with a real TeX installation. Unlike a TeX
//! installation, an HTML parser is an ordinary Rust crate, so this suite is
//! never skipped: it runs wherever `cargo test` runs.
//!
//! # The floor: what a failure here means
//!
//! The parser is `html5ever` **0.39**, the parser of the Servo browser
//! engine, held under `[dev-dependencies]` of this crate; the tree it builds
//! into is `markup5ever_rcdom` **0.39**, the reference tree html5ever's own
//! test suite uses. Those two version numbers are the floor's, so they are not
//! left to a reader's trust: the test `the_floor_names_the_versions_in_use`
//! reads both out of `Cargo.lock` and fails if either moves without this
//! comment moving with it. html5ever implements the HTML parsing algorithm of
//! the WHATWG HTML standard, including the **parse errors** that algorithm
//! defines, and it hands each one to the tree it builds into. This suite fails
//! on any of them.
//!
//! A parse error covers the two halves of the algorithm:
//!
//! - **Tokenizer errors** — what the character stream itself cannot be read
//!   as: an unterminated tag or comment, a `<` that begins nothing, an
//!   attribute name holding a quotation mark or a `<`, a character reference
//!   that is not one, a null character.
//! - **Tree-construction errors** — what the tags say that the tree cannot
//!   hold: an end tag that closes nothing, an end tag naming an element that
//!   is not the innermost open one (**misnesting**), an element that is not
//!   allowed where it stands and whose start tag therefore **closed** the
//!   element around it before it (an `<aside>` inside a `<p>` ends the
//!   paragraph; the `</p>` written afterwards then closes nothing and is the
//!   error the parser reports).
//!
//! That is the floor. It is deliberately a floor and not a verdict of
//! validity, and these are the things it does **not** cover:
//!
//! - **Content models beyond those the parser enforces.** The algorithm
//!   repairs what it can and reports what it cannot; a document that parses
//!   without an error can still break a rule of the standard that the parser
//!   has no opinion about — a `<p>` inside a `<span>`, an `<img>` with no
//!   `alt` text, a `<ul>` with no `<li>`. Such rules belong to a validator,
//!   not to a parser.
//! - **An element the standard lets stand open where the text ends.** The
//!   algorithm's end-of-file step names the elements that may still be open
//!   without an error — `p`, `li`, `dt`, `dd`, `body` and `html` — so a
//!   fragment that never closes its paragraph parses cleanly, while an `<em>`,
//!   a `<span>`, a `<div>` or a `<ul>` left open is a parse error. A forgotten
//!   `</p>` is therefore below this floor.
//! - **Which element an attribute stands on.** The element and the attribute
//!   checks below read names over the whole golden rather than element by
//!   element: an `href` written on a `<p>`, or a `charset` on a `<span>`, is a
//!   name this backend writes somewhere and passes. Holding each attribute to
//!   the elements that may carry it would need a second table, and is not what
//!   the contract states.
//! - **Accessibility.** Nothing here reads a heading order, a landmark, a
//!   label or a contrast ratio.
//! - **The stylesheets as CSS.** The chunks are scanned against the rules of
//!   this backend's own contract, described in the last section below; no CSS
//!   parser is involved, and no rule is checked for being CSS a browser
//!   accepts.
//! - **JavaScript.** The script definitions are text to this suite.
//! - **What a formula renderer does.** A formula reaches the page as LaTeX
//!   inside an element, for a renderer in the reader's browser to typeset.
//!   What that renderer makes of it is pinned by the probe under
//!   `prototypes/`, never here.
//!
//! An external validator — the WHATWG's own `vnu`, run where it is installed —
//! would raise the floor to the content models. It is a possible later
//! addition and is not this suite.
//!
//! # What the suite asserts
//!
//! Over every golden, once it has parsed:
//!
//! - no identifier (`id`) occurs twice, since a document with two nodes of one
//!   identifier has no answer for a link that names it;
//! - every link into the same page (`href="#x"`) names an identifier the
//!   golden holds;
//! - every class is one the contract can produce — the class of a
//!   presentation element, the class of a role, or one of the three fixed
//!   auxiliary classes;
//! - every element and every attribute is one the contract writes or one the
//!   page skeleton writes — an attribute the contract writes as a family
//!   (`data-flm-style-` for a cell's custom styles) being any strictly longer
//!   name under its prefix — or, for an element, the `<tbody>` the HTML
//!   parsing algorithm puts around the rows of a table, which the backend
//!   never writes (the bytes of no golden hold `<tbody` or `<thead`);
//! - a `style` attribute holds nothing but the custom properties the
//!   contract writes for a payload's length fields, each once, each valued
//!   with a length as the backend spells one — never a CSS property;
//! - a `role` attribute holds one of the document roles the backend writes
//!   (`doc-endnotes`, `doc-bibliography`, …), read from the vocabulary's
//!   list of them rather than copied here.
//!
//! The subtree of a **construct node** — the element `flm.pres.html.tag`, one
//! HTML tag a lowering rule staged itself — is exempt from the last two:
//! a rule that stages a tag owns everything about it, so its tag name, its
//! classes and its attributes are outside the contract by design. Which
//! goldens hold one, and which tag roots it, is recorded in this file
//! (`ALLOWANCES`), because the bytes of a golden do not say where a
//! construct node stood.
//!
//! `NOT_VALIDATED` excuses a golden whose output *must* be invalid, each
//! entry with the reason why; a test holds that every entry names a golden
//! that exists.
//!
//! # The stylesheet scanner
//!
//! The default stylesheet is a handful of CSS chunks in the source tree, one
//! per style definition. They are **scanned, not parsed**: there is no CSS
//! parser here, and the scan reads the text for the things this backend
//! promises about the sheets it ships:
//!
//! - a chunk **names itself** on its first line, in a comment holding nothing
//!   but the style definition's name (`/* flm.html.style.core */`), and the
//!   file's name is that name's last segment, so the file and the definition
//!   cannot drift apart;
//! - the braces balance, and everything sits inside the one cascade layer
//!   `@layer flm`;
//! - every class a selector names is a class of the contract, and every
//!   selector **begins** with one: the first part of a selector, before any
//!   combinator, carries a contract class, so no rule of ours reaches a host
//!   page's own `p`, `a` or `table`. A `:root` selector, where a chunk
//!   declares its custom properties, is the one shape excepted; an element a
//!   formula renderer writes, `tex-html`, is written scoped
//!   (`.flm-pres-math--passthrough tex-html`), since a host page running the same
//!   renderer has such elements too;
//! - every `data-flm-…` attribute a selector names is an attribute this
//!   backend writes, and every custom property a chunk uses is declared in
//!   that chunk or is one the page writes in an element's `style` attribute
//!   for a length field (`--flm-element-indent-left`), which the chunk applies
//!   with a fallback of its own.
//!
//! The chunk that styles the page skeleton, `flm.html.style.page`, is excepted
//! from the two rules about classes: it selects the page's own `<body>` and
//! content container, which stand for no node of a document.
//!
//! Whether the CSS is CSS a browser accepts is not checked, here or anywhere:
//! that is what the note above means by the stylesheets not being covered. The
//! chunks arrive with the default stylesheet; until it is delivered the
//! directory holds none, the scanner says so and passes, and its own fixtures
//! are what exercise it.

// Why this suite is shaped as it is — a parser held as a development
// dependency rather than an external validator run beside the build, the
// reference tree the parser builds into, the per-golden allowances, and a scan
// of the stylesheet chunks that uses no CSS parser — is recorded.
// See decision rationale at [§flm-dd-dr:rend-html-validity-check].

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use html5ever::driver::{ParseOpts, parse_document, parse_fragment};
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{Attribute, QualName, local_name, ns, tokenizer::TokenizerOpts};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use flm_core::stdcore::backends::html::class_of_role;

// ============================================================================
// the goldens: discovery
// ============================================================================

/// The three directories holding HTML goldens, relative to this crate's root:
/// the HTML backend's own; the spine suite's, where the pages of the two
/// whole documents written through the `html` workflow land; and the
/// program's, where the pages its templates make land.
///
/// A directory that does not exist holds no golden, so this suite runs over
/// whichever of the three a checkout carries.
const GOLDEN_DIRECTORIES: [&str; 3] =
    ["tests/goldens/html", "tests/goldens/spine", "../flm-cli/tests/goldens"];

/// How a golden is parsed: as a whole page, or as a piece of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A complete document, beginning with the document type line.
    Page,
    /// A fragment: content that stood inside an element of a page. It is
    /// parsed in the context of a `<div>`, which is what the page's content
    /// container amounts to for a parser.
    Fragment,
}

/// One golden file, as this suite reads it.
#[derive(Debug, Clone)]
struct Golden {
    /// The file's name (`emit-lists.html`), which is how this file names one.
    name: String,
    /// The path it was read from.
    path: PathBuf,
    /// Its bytes as text.
    text: String,
    /// How it is parsed.
    shape: Shape,
}

/// The path of `relative` under this crate's root.
fn crate_path(relative: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(relative);
    path
}

/// Every HTML golden of `GOLDEN_DIRECTORIES`, by name, in name order.
///
/// A golden is discovered by its extension, never by a list, so a golden
/// written later is checked the day it lands. Its shape follows its name: a
/// name ending in `.page.html` is a page, every other `.html` a fragment. A
/// test holds the name and the bytes against each other, so that a whole page
/// filed under a fragment's name is a failure that says so.
///
/// The directories share one space of file names, since a golden is known here
/// by its name alone: two goldens of one name, in whichever directories, are a
/// failure that names both.
fn goldens() -> Vec<Golden> {
    let mut found: BTreeMap<String, Golden> = BTreeMap::new();
    for directory in GOLDEN_DIRECTORIES {
        let path = crate_path(directory);
        let Ok(entries) = fs::read_dir(&path) else { continue };
        for entry in entries {
            let entry = entry.expect("a directory entry");
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("html") {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .expect("a golden's name is text")
                .to_string();
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read the golden {}: {e}", path.display()));
            let shape =
                if name.ends_with(".page.html") { Shape::Page } else { Shape::Fragment };
            let golden = Golden { name: name.clone(), path, text, shape };
            if let Some(other) = found.insert(name, golden.clone()) {
                panic!(
                    "two goldens are named {}: {} and {}",
                    other.name,
                    other.path.display(),
                    golden.path.display()
                );
            }
        }
    }
    found.into_values().collect()
}

// ============================================================================
// parsing
// ============================================================================

/// The parser this suite's floor is: the crate's name, and the version the
/// module comment above names, without the patch number a lock file also
/// carries.
const PARSER: (&str, &str) = ("html5ever", "0.39");

/// The tree the parser builds into, in the shape of [`PARSER`].
const TREE: (&str, &str) = ("markup5ever_rcdom", "0.39");

/// The version the lock file `lock` records for the crate `crate_name`, when
/// it holds one.
///
/// The lock file is read as lines rather than as TOML: a package's entry opens
/// with `[[package]]` and states its name before its version, which is all
/// this needs and costs no dependency.
fn locked_version(lock: &str, crate_name: &str) -> Option<String> {
    let mut name: Option<&str> = None;
    for line in lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            name = None;
        } else if let Some(value) = line.strip_prefix("name = ") {
            name = Some(value.trim_matches('"'));
        } else if let Some(value) = line.strip_prefix("version = ") {
            if name == Some(crate_name) {
                return Some(String::from(value.trim_matches('"')));
            }
        }
    }
    None
}

/// The versions the floor names are the versions this suite really runs: the
/// two numbers in the comment at the top of this file are held against
/// `Cargo.lock`, so that a dependency moved without the comment is a failure
/// rather than a stale sentence.
#[test]
fn the_floor_names_the_versions_in_use() {
    let path = crate_path("../Cargo.lock");
    let lock = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read the lock file {}: {e}", path.display()));
    for (crate_name, stated) in [PARSER, TREE] {
        let locked = locked_version(&lock, crate_name)
            .unwrap_or_else(|| panic!("`{crate_name}` is not in {}", path.display()));
        assert!(
            locked == stated || locked.starts_with(&format!("{stated}.")),
            "the comment at the top of this file says the floor is `{crate_name}` {stated}, \
             and the lock file holds {locked}: the comment moves with the dependency"
        );
    }
}

/// The parser's options: every parse error the standard defines, spelled out.
///
/// `exact_errors` costs a little speed and buys the message that says which
/// rule was broken, which is what a failure here must print to be of any use.
fn parse_options() -> ParseOpts {
    ParseOpts {
        tokenizer: TokenizerOpts { exact_errors: true, ..TokenizerOpts::default() },
        tree_builder: TreeBuilderOpts { exact_errors: true, ..TreeBuilderOpts::default() },
    }
}

/// Parses `text` the way a browser parses it, and hands back the tree and
/// every parse error the parser reported, in the order it reported them.
fn parse(text: &str, shape: Shape) -> (RcDom, Vec<String>) {
    let dom = match shape {
        Shape::Page => parse_document(RcDom::default(), parse_options()).one(text),
        Shape::Fragment => parse_fragment(
            RcDom::default(),
            parse_options(),
            QualName::new(None, ns!(html), local_name!("div")),
            Vec::new(),
            false,
        )
        .one(text),
    };
    let errors = dom.errors.borrow().iter().map(|error| error.to_string()).collect();
    (dom, errors)
}

// ============================================================================
// the parsed tree, as this suite reads it
// ============================================================================

/// One element of a parsed golden, with what the checks need of it.
#[derive(Debug, Clone)]
struct Element {
    /// The tag name, lowercase, as the parser resolved it.
    tag: String,
    /// The attributes, in the order they were written, each name spelled with
    /// its prefix where it has one (`xlink:href`).
    attributes: Vec<(String, String)>,
    /// Whether the element stands inside the subtree of a construct node,
    /// which the contract does not reach.
    in_construct: bool,
}

impl Element {
    /// The value of the attribute `name`, when the element carries one.
    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(attribute, _)| attribute == name)
            .map(|(_, value)| value.as_str())
    }

    /// The class tokens, in the order they were written.
    fn classes(&self) -> Vec<&str> {
        self.attribute("class").map(|value| value.split_whitespace().collect()).unwrap_or_default()
    }
}

/// Every element of a parsed tree, in document order.
///
/// `construct_roots` names the elements that root a construct node's subtree
/// in this golden, each by its tag (`math`), or by its tag and one of its
/// classes (`span.math-mathjax-formula`) where the backend writes that tag
/// itself too: such an element, and everything under it, is marked
/// `Element::in_construct`.
///
/// A fragment is parsed into a synthetic `<html>` root the bytes do not hold,
/// which is how the parsing algorithm's fragment case works, so every parsed
/// fragment carries one `<html>` element more than its text does. The name
/// check passes it because the page skeleton writes `html` anyway; an `<html>`
/// an author really wrote inside a fragment is caught as a parse error, not
/// here.
fn elements(dom: &RcDom, construct_roots: &[&str]) -> Vec<Element> {
    elements_with(dom, construct_roots, &[])
}

/// `elements`, the elements whose tag `template_tags` names marked
/// `Element::in_construct` themselves, their children not.
fn elements_with(dom: &RcDom, construct_roots: &[&str], template_tags: &[&str]) -> Vec<Element> {
    let mut found = Vec::new();
    collect(&dom.document, false, construct_roots, template_tags, &mut found);
    found
}

/// The walk behind `elements`.
fn collect(
    handle: &Handle,
    in_construct: bool,
    roots: &[&str],
    template_tags: &[&str],
    found: &mut Vec<Element>,
) {
    let mut inside = in_construct;
    if let NodeData::Element { name, attrs, .. } = &handle.data {
        let tag = name.local.to_string();
        let attributes: Vec<(String, String)> = attrs.borrow().iter().map(spell).collect();
        inside = in_construct || is_root(roots, &tag, &attributes);
        found.push(Element {
            in_construct: inside || template_tags.contains(&tag.as_str()),
            tag,
            attributes,
        });
    }
    for child in handle.children.borrow().iter() {
        collect(child, inside, roots, template_tags, found);
    }
}

/// Whether the element of the tag `tag` and the attributes `attributes` is one
/// that `roots` names (see `elements`).
fn is_root(roots: &[&str], tag: &str, attributes: &[(String, String)]) -> bool {
    roots.iter().any(|root| match root.split_once('.') {
        None => *root == tag,
        Some((root_tag, class)) => {
            let has_class = |(name, value): &(String, String)| {
                name == "class" && value.split_whitespace().any(|token| token == class)
            };
            root_tag == tag && attributes.iter().any(has_class)
        }
    })
}

/// One attribute as a name and a value, the name carrying its prefix where the
/// parser gave it one.
fn spell(attribute: &Attribute) -> (String, String) {
    let name = match attribute.name.prefix.as_ref() {
        Some(prefix) => format!("{prefix}:{}", attribute.name.local),
        None => attribute.name.local.to_string(),
    };
    (name, attribute.value.to_string())
}

// ============================================================================
// the goldens that hold a construct node
// ============================================================================

/// What one golden is allowed that the contract does not cover.
///
/// Two things can stand in a golden that the contract says nothing about, and
/// neither is visible in the bytes, so each golden that holds one says so
/// here:
///
/// - the subtree of a **construct node** — the element `flm.pres.html.tag`,
///   one HTML tag a lowering rule staged itself. The rule that stages a tag
///   owns its name, its classes and its attributes, so nothing in this suite
///   may judge them. `Allowance::construct_roots` names the elements that root
///   such a subtree in this golden (see `elements`).
/// - an element a **template** wrote itself around the regions it placed — the
///   header of the page the program's `simple` template makes. The template
///   owns that element, and `Allowance::template_tags` names its tag; what
///   stands inside it is the backend's and is checked as usual.
/// - a **role** the standard package under its default configuration does not
///   register. A role is a feature-owned dotted name saying what a container
///   is, and the class derived from it is a contract class exactly when some
///   feature owns the role; a test over a hand-built tree invents one to show
///   the mechanism, and a document rendered under another configuration
///   carries the role of an element only that configuration registers (the
///   list of a second kind of xnote). `Allowance::roles` names those roles.
///
/// Everything else in such a golden is checked as usual: an allowance is per
/// golden, and never a blanket exemption.
#[derive(Debug, Clone, Copy)]
struct Allowance {
    /// The golden's name.
    golden: &'static str,
    /// The elements that root a construct node's subtree in it, each by its tag
    /// or by its tag and a class (see `elements`).
    construct_roots: &'static [&'static str],
    /// The roles it carries that the standard package under its default
    /// configuration does not register.
    roles: &'static [&'static str],
    /// The tags a template wrote itself around the regions it placed (the
    /// header of a page): the element is the template's, and what stands
    /// inside it is checked as usual.
    template_tags: &'static [&'static str],
}

/// The goldens that need an allowance, and why.
///
/// An entry is added by whoever writes the golden. A golden that comes out of
/// a document rendered by the standard package under its default
/// configuration needs none: no lowering rule of the standard package stages a
/// construct node, and a role such a rule writes is a role its feature owns.
const ALLOWANCES: &[Allowance] = &[
    // The emitter's own construct golden: a `<figure>` carrying the rule's own
    // class and attribute, and a `<span>` whose body is already-formed markup
    // holding an `<svg>`, both written by `HtmlTagPayload` in the test.
    Allowance {
        golden: "emit-construct.html",
        construct_roots: &["figure", "svg"],
        roles: &[], template_tags: &[]
    },
    // A construct node standing inside a formula, which shows that a staged
    // tag's attribute values pass the check that keeps a formula from ending
    // early.
    Allowance {
        golden: "emit-math-construct-attribute.html",
        construct_roots: &["abbr"],
        roles: &[], template_tags: &[]
    },
    // A hand-built tree carrying a role, to show the second class: no theorem
    // feature exists.
    Allowance { golden: "emit-blocks.html", construct_roots: &[], roles: &["flm.theorem"], template_tags: &[] },
    // The program's fragment of a document with formulas, rendered under the
    // program's default list of packages, whose math processor is the
    // extension `math-mathjax`, in its default mode, `load-in-browser`: each
    // formula is a span its rule stages as a construct node, carrying the
    // extension's class and the formula's TeX.
    Allowance {
        golden: "cli-none.html",
        construct_roots: &["span.math-mathjax-formula"],
        roles: &[],
        template_tags: &[],
    },
    // The page the program's `simple` template makes holds the pieces placed
    // before the content in a `<header>` of the template's own, and the
    // formulas `math-mathjax` writes, as in the fragment above.
    Allowance {
        golden: "cli-simple.page.html",
        construct_roots: &["span.math-mathjax-formula"],
        roles: &[],
        template_tags: &["header"],
    },
    // The program's end-to-end page, made by the same template, with the
    // formulas and the displayed environment `math-mathjax` writes, each a
    // span as in the fragment above.
    Allowance {
        golden: "e2e-every-feature.page.html",
        construct_roots: &["span.math-mathjax-formula"],
        roles: &[],
        template_tags: &["header"],
    },
    // The program's end-to-end page of a document of several files, made by
    // the template a package ships: it places the regions inside an
    // `<article>` of its own.
    Allowance {
        golden: "e2e-several-files.page.html",
        construct_roots: &[],
        roles: &[],
        template_tags: &["article"],
    },
    // The end-to-end page under a configuration of two kinds of xnote: the
    // list of the kind `endnote` carries the role of the list element that
    // configuration registers beside the default kind's.
    Allowance {
        golden: "xnotes-e2e-two-kinds.page.html",
        construct_roots: &[],
        roles: &["flm.xnotes.endnote.list"], template_tags: &[]
    },
];

/// The allowance of the golden `name`, which is the empty one where it needs
/// none.
fn allowance(name: &str) -> Allowance {
    ALLOWANCES
        .iter()
        .copied()
        .find(|entry| entry.golden == name)
        .unwrap_or(Allowance { golden: "", construct_roots: &[], roles: &[], template_tags: &[] })
}

// ============================================================================
// the contract: what a class, an element and an attribute may be
// ============================================================================

/// The classes, tags and attributes this backend's output may carry, derived
/// from the contract rather than listed.
mod contract {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
    use flm_core::stdcore::backends::html::conventions::{
        BLOCK_TAGS, CONVENTIONS, DEFAULT_BLOCK_TAG, STYLE_ATTRIBUTE, TagRule, Written,
        doc_role_value,
    };
    use flm_core::stdcore::backends::html::{
        AUXILIARY_CLASSES, class_of_element, class_of_role, data_attribute,
    };
    use flm_core::stdcore::backends::pres;
    use flm_core::stdcore::features::{
        annotations, citations, defterm, floats, headings, quote, theorems, toc, xnotes,
    };
    use flm_core::stdcore::{StdcoreConfig, package};

    /// The registry a host building the standard package gets. It is read for
    /// the element names the package registers, which is where the roles a
    /// rule of the package can write come from.
    fn standard_registry() -> Arc<FlmRegistry> {
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(StdcoreConfig::new()));
        builder.use_package(flm_core::math_passthrough::package());
        builder.build(&RegistryInputs::new()).expect("the standard package builds")
    }

    /// The value of every document role the backend writes in a `role`
    /// attribute, `doc-` followed by the role's stable name.
    pub fn document_roles() -> BTreeSet<String> {
        pres::DocRole::ALL.into_iter().map(doc_role_value).collect()
    }

    /// The class of every element of the presentation vocabulary.
    pub fn element_classes() -> BTreeSet<String> {
        pres::ALL.iter().map(|element| class_of_element(element)).collect()
    }

    /// The three classes the contract holds that stand for no element and no
    /// role: the page's content container, a list entry's rich label, and the
    /// two marker lines of an unattributed environment.
    pub fn auxiliary_classes() -> BTreeSet<String> {
        AUXILIARY_CLASSES.iter().map(|class| String::from(*class)).collect()
    }

    /// The class of every **role** a rule of the standard package can write.
    ///
    /// A role is a feature-owned dotted name. A feature names it after the
    /// element it stages the container for, so most of the package's roles are
    /// read out of the registry's element names rather than listed here; a
    /// role that names a *part* of an element, spelled as the element's name
    /// followed by the part's, is read from the feature that owns it, which is
    /// where its name is published. A role no feature owns is a golden's own,
    /// declared in its `Allowance`.
    ///
    /// This set is **wider than the roles anything writes**, and deliberately
    /// so: the registry models elements, not roles, so there is nothing
    /// narrower to read. Every element name the standard package registers
    /// becomes a class here, while few rules write a role today — the heading
    /// feature's `flm.heading` on a run-in heading's title and its
    /// `flm.heading.number` on a numbered heading's number, and the xnotes
    /// feature's roles of a kind's list, a mark, an in-place xnote and the
    /// mark beside an entry of a list, the defterm feature's roles of a
    /// definition's lead-in and of a term inside its own definition, and the
    /// theorems feature's roles of a theorem, of its heading's name and of its
    /// title, the quote feature's roles of a quotation's four kinds of section,
    /// the annotations feature's roles of a highlight, a comment and an
    /// annotator's initials, the floats feature's roles of a float's
    /// content, of the line under it and of the number in that line, and the
    /// citations feature's roles of a mark, of the word before a mark, of
    /// the space before a mark and of the number labeling an entry of the list of cited works. The cost
    /// is a hole of a known shape — a class derived from a registered name that no rule
    /// can produce would pass this check — and the gain is that a feature
    /// which adds a role named after an element adds nothing here. The
    /// narrowing to make is to state at the backend that a role must name a
    /// registered element or a part of one, and to read the roles from
    /// wherever that rule is enforced.
    pub fn role_classes() -> BTreeSet<String> {
        standard_registry()
            .elements()
            .iter()
            .map(|entry| class_of_role(entry.element.name.as_str()))
            .chain(PART_ROLES.iter().map(|role| class_of_role(role)))
            .collect()
    }

    /// The roles of the standard package that name a part of an element
    /// rather than an element: each is published by the feature that writes
    /// it, and this list is what a feature adds to when it writes one.
    const PART_ROLES: [&str; 22] = [
        annotations::HIGHLIGHT_ROLE,
        annotations::COMMENT_ROLE,
        annotations::INITIALS_ROLE,
        headings::numbers::NUMBER_ROLE,
        xnotes::MARK_ROLE,
        xnotes::INPLACE_ROLE,
        xnotes::ENTRY_MARK_ROLE,
        defterm::LEAD_IN_ROLE,
        defterm::TERM_ROLE,
        theorems::TITLE_ROLE,
        quote::TEXT_ROLE,
        quote::LINES_ROLE,
        quote::ATTRIBUTED_ROLE,
        quote::BLOCK_ROLE,
        floats::CONTENT_ROLE,
        floats::CAPTION_ROLE,
        floats::NUMBER_ROLE,
        citations::MARK_ROLE,
        citations::WORD_ROLE,
        citations::SPACE_ROLE,
        citations::ENTRY_NUMBER_ROLE,
        toc::NUMBER_ROLE,
    ];

    /// The tags the contract's rows can produce, derived from every row's
    /// `TagRule`.
    ///
    /// The tags a rule picks from a payload — a heading's level, a span's
    /// style, a verbatim's display, a list's kind — are the emitter's and are
    /// spelled here beside the rule they belong to; the `match` is exhaustive,
    /// so a rule added to the contract fails to compile until this function
    /// says which tags it writes.
    ///
    /// Those four lists are a **copy**: the originals are three private
    /// functions of the emitter (`heading_tag`, `span_tag` and `list_tag` in
    /// `stdcore/backends/html/emit.rs`) and the arm that writes a verbatim
    /// block, which an integration test cannot call. The copy is held against
    /// the originals by `every_tag_the_contract_claims_is_written_by_a_golden`
    /// below: every tag named here must be one some golden really carries, so
    /// a tag the emitter stops writing cannot stay allowed here unnoticed.
    /// The other direction — a tag the emitter starts writing that is not
    /// named here — fails on the first golden that holds it.
    pub fn row_tags() -> BTreeSet<&'static str> {
        let mut tags = BTreeSet::new();
        for convention in CONVENTIONS {
            match convention.tag {
                TagRule::Fixed(tag) => {
                    tags.insert(tag);
                }
                TagRule::HeadingLevel => {
                    tags.extend(["h1", "h2", "h3", "h4", "h5", "h6"]);
                }
                TagRule::SpanStyle => {
                    tags.extend(["em", "strong", "i", "code", "u", "sup", "sub", "span"]);
                }
                TagRule::Display { inline, block } => {
                    tags.extend([inline, block]);
                }
                TagRule::VerbatimDisplay => {
                    tags.extend(["code", "pre"]);
                }
                TagRule::ListKind => {
                    tags.extend(["ol", "ul", "dl"]);
                }
                // Read from the backend's own table, which is public.
                TagRule::BlockRole => {
                    tags.insert(DEFAULT_BLOCK_TAG);
                    tags.extend(BLOCK_TAGS.iter().map(|entry| entry.tag));
                }
                // A node that writes no element of its own contributes no tag.
                TagRule::None => {}
            }
        }
        tags
    }

    /// The tags the emitter writes that no row names: the structure of a list
    /// and of a table of cells, and the element a formula renderer reads HTML
    /// out of.
    ///
    /// A list's entries are not presentation elements of their own — a list
    /// node holds its entries as slots — so the tags that carry an entry and a
    /// description's term and definition belong to the list's row's tag rule
    /// without being spelled in it; so do a table's rows and its cells, a
    /// header cell being a `<th>`. `tex-html` is the element MathJax reads
    /// HTML out of inside a formula, which is the contract's: content standing
    /// inside a formula reaches the page through it and through nothing else.
    pub fn extra_tags() -> BTreeSet<&'static str> {
        ["li", "dt", "dd", "tr", "td", "th", "tex-html"].into_iter().collect()
    }

    /// The six tags the page skeleton writes: the document, its head with the
    /// character encoding, the viewport line and the title, and the body with
    /// the container the document's content stands in.
    pub fn page_tags() -> BTreeSet<&'static str> {
        ["html", "head", "meta", "title", "body", "main"].into_iter().collect()
    }

    /// The tags a page will carry once the style and script definitions it
    /// requires are delivered into it — a sheet or a script written into the
    /// page, or an address the page loads.
    ///
    /// **Not yet written:** the page assembly writes none of these today. They
    /// are allowed in advance so that the goldens of the definitions pass this
    /// suite the day they land rather than failing on arrival; whoever
    /// delivers the definitions prunes from this list what the page in fact
    /// does not write, and adds what it writes beyond it.
    pub fn anticipated_page_tags() -> BTreeSet<&'static str> {
        ["style", "link", "script"].into_iter().collect()
    }

    /// The tags the parser puts in the tree where the backend writes none:
    /// the `<tbody>` the HTML parsing algorithm opens around the rows of a
    /// `<table>` that writes them directly, which is how the backend writes a
    /// table of cells.
    pub fn implied_tags() -> BTreeSet<&'static str> {
        ["tbody"].into_iter().collect()
    }

    /// Every tag this backend may write, and the ones the parser implies.
    pub fn tags() -> BTreeSet<String> {
        row_tags()
            .into_iter()
            .chain(extra_tags())
            .chain(implied_tags())
            .chain(page_tags())
            .chain(anticipated_page_tags())
            .map(String::from)
            .collect()
    }

    /// The attributes the contract's rows produce: an attribute HTML itself
    /// defines, one under the `data-flm-` prefix, or `style`, where a row has
    /// a length field written as a custom property — on the element of the
    /// row or on each entry of it (a list's `<li>`, a table's cells), a
    /// family of attributes excepted ([`attribute_families`]).
    pub fn row_attributes() -> BTreeSet<String> {
        let fields = || CONVENTIONS.iter().flat_map(|convention| convention.fields.iter());
        let style = fields()
            .any(|field| field.style_property().is_some())
            .then(|| String::from(STYLE_ATTRIBUTE));
        let entries = fields()
            .filter_map(|field| field.entry_attribute())
            .filter(|name| !name.ends_with('-'))
            .map(String::from);
        fields().filter_map(|field| field.attribute()).chain(entries).chain(style).collect()
    }

    /// The families of attributes the contract's rows write on each entry:
    /// the prefix of every attribute of the family, which is completed by a
    /// value's name (`data-flm-style-` for a cell's custom styles).
    pub fn attribute_families() -> BTreeSet<&'static str> {
        CONVENTIONS
            .iter()
            .flat_map(|convention| convention.fields.iter())
            .filter_map(|field| field.entry_attribute())
            .filter(|name| name.ends_with('-'))
            .collect()
    }

    /// Whether `name` is an attribute this backend may write: one of
    /// [`attributes`], or a member of one of [`attribute_families`].
    pub fn writes_attribute(name: &str) -> bool {
        attributes().contains(name)
            || attribute_families()
                .iter()
                .any(|family| name.len() > family.len() && name.starts_with(family))
    }

    /// The custom properties the contract's rows write in a `style`
    /// attribute, one per length field: the only thing that attribute may
    /// hold.
    pub fn style_properties() -> BTreeSet<String> {
        CONVENTIONS
            .iter()
            .flat_map(|convention| convention.fields.iter())
            .filter_map(|field| field.style_property())
            .collect()
    }

    /// The custom properties among [`style_properties`] that hold the ratio
    /// of two length fields rather than a length.
    pub fn ratio_properties() -> BTreeSet<String> {
        CONVENTIONS
            .iter()
            .flat_map(|convention| convention.fields.iter())
            .filter(|field| matches!(field.written, Written::Ratio(..)))
            .filter_map(|field| field.style_property())
            .collect()
    }

    /// The attributes the emitter writes that no row names: the class, and the
    /// one carrying an element's place in its source, which the render option
    /// `source_spans` asks for.
    pub fn extra_attributes() -> BTreeSet<String> {
        [String::from("class"), data_attribute("source_span")].into_iter().collect()
    }

    /// The attributes the page skeleton writes: the document's language, the
    /// character encoding, and the name and content of the viewport line.
    pub fn page_attributes() -> BTreeSet<String> {
        ["lang", "charset", "name", "content"].into_iter().map(String::from).collect()
    }

    /// The attributes a style or script definition delivered into a page will
    /// carry: which definition it is (`data-flm-def`), the relation of a
    /// stylesheet the page loads (`rel="stylesheet"`), and `defer` on a
    /// loaded script, which is written with no value at all — a browser reads
    /// the presence of the name. The address itself (`href` on a link, `src`
    /// on a script) is already the contract's.
    ///
    /// **Not yet written:** as with [`anticipated_page_tags`], nothing writes
    /// these today; they are allowed in advance, and whoever delivers the
    /// definitions prunes what the page does not write and adds what it writes
    /// beyond them (`media`, `crossorigin` and `integrity` are what a link or
    /// a script could still grow, and each must be added in the commit that
    /// writes it).
    pub fn anticipated_page_attributes() -> BTreeSet<String> {
        ["rel", "defer", "data-flm-def"].into_iter().map(String::from).collect()
    }

    /// Every attribute this backend may write.
    pub fn attributes() -> BTreeSet<String> {
        row_attributes()
            .into_iter()
            .chain(extra_attributes())
            .chain(page_attributes())
            .chain(anticipated_page_attributes())
            .collect()
    }
}

// ============================================================================
// the structural assertions
// ============================================================================

/// The identifiers of a parsed golden that occur more than once.
///
/// Two nodes of one identifier leave a link that names it without an answer,
/// and no document can be written that way, whatever produced it — so a
/// construct node's subtree is not exempt from this one.
fn repeated_identifiers(found: &[Element]) -> Vec<String> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut repeated: BTreeSet<&str> = BTreeSet::new();
    for element in found {
        if let Some(id) = element.attribute("id") {
            if !seen.insert(id) {
                repeated.insert(id);
            }
        }
    }
    repeated.into_iter().map(|id| format!("the identifier `{id}` occurs more than once")).collect()
}

/// The links into the golden itself whose target is not there.
///
/// A link written `href="#x"` names the node whose identifier is `x`; a link
/// to `#` alone names the top of the page and names no node.
fn dangling_links(found: &[Element]) -> Vec<String> {
    let identifiers: BTreeSet<&str> =
        found.iter().filter_map(|element| element.attribute("id")).collect();
    let mut problems = Vec::new();
    for element in found {
        let Some(href) = element.attribute("href") else { continue };
        let Some(target) = href.strip_prefix('#') else { continue };
        if target.is_empty() || identifiers.contains(target) {
            continue;
        }
        problems.push(format!("the link `href=\"{href}\"` names no identifier of this golden"));
    }
    problems
}

/// The classes of a parsed golden that the contract cannot produce.
///
/// A class is the class of a presentation element, the class of a role, or one
/// of the three auxiliary classes; a node that carries a role carries it as a
/// second class, after the element's own. A class standing under a construct
/// node is the staging rule's and is not judged here.
fn classes_outside_the_contract(found: &[Element], allowed: &Allowance) -> Vec<String> {
    let elements = contract::element_classes();
    let auxiliary = contract::auxiliary_classes();
    let mut roles = contract::role_classes();
    roles.extend(allowed.roles.iter().map(|role| class_of_role(role)));
    let mut problems = Vec::new();
    for element in found {
        if element.in_construct {
            continue;
        }
        let classes = element.classes();
        for (position, class) in classes.iter().enumerate() {
            let is_element = elements.contains(*class);
            let is_auxiliary = auxiliary.contains(*class);
            if !is_element && !is_auxiliary && !roles.contains(*class) {
                problems.push(format!(
                    "`<{}>` carries the class `{class}`, which the contract cannot produce",
                    element.tag
                ));
            } else if position == 0 && !is_element && !is_auxiliary {
                problems.push(format!(
                    "`<{}>` carries the role class `{class}` first; the element's own class \
                     comes first",
                    element.tag
                ));
            }
        }
    }
    problems
}

/// The elements and attributes of a parsed golden that neither the contract
/// nor the page skeleton writes.
///
/// A construct node's subtree is exempt: the rule that stages a tag owns its
/// name and its attributes.
fn names_outside_the_contract(found: &[Element]) -> Vec<String> {
    let tags = contract::tags();
    let attributes = contract::attributes();
    let mut problems = Vec::new();
    for element in found {
        if element.in_construct {
            continue;
        }
        if !tags.contains(&element.tag) {
            problems.push(format!("`<{}>` is not an element this backend writes", element.tag));
        }
        for (name, _) in &element.attributes {
            if !attributes.contains(name) && !contract::writes_attribute(name) {
                problems.push(format!(
                    "`<{}>` carries the attribute `{name}`, which this backend does not write",
                    element.tag
                ));
            }
        }
    }
    problems
}

/// The `role` attributes of a parsed golden holding anything but one of the
/// document roles the backend writes. A construct node's subtree is exempt,
/// as for every other name.
fn roles_outside_the_contract(found: &[Element]) -> Vec<String> {
    let roles = contract::document_roles();
    found
        .iter()
        .filter(|element| !element.in_construct)
        .filter_map(|element| {
            let role = element.attribute("role")?;
            (!roles.contains(role)).then(|| {
                format!("`<{}>` carries the role `{role}`, which this backend does not write", element.tag)
            })
        })
        .collect()
}

/// The `style` attributes of a parsed golden holding anything but the custom
/// properties of a payload's length fields, each once and valued with a CSS
/// length as the backend spells one, and of the ratios of two such fields,
/// each valued with two positive amounts over a slash.
///
/// The attribute is admitted for those properties and for nothing else: a CSS
/// property written there — `padding-left`, `color` — would outrank every
/// stylesheet rule, which is exactly what the contract promises never to do.
/// A construct node's subtree is exempt, as for every other name.
fn styles_outside_the_contract(found: &[Element]) -> Vec<String> {
    let properties = contract::style_properties();
    let mut problems = Vec::new();
    for element in found {
        if element.in_construct {
            continue;
        }
        let Some(style) = element.attribute("style") else { continue };
        problems.extend(style_problems(style, &properties));
    }
    problems
}

/// What is wrong with one `style` attribute's value, `style`, against the
/// properties the contract writes there, `properties`.
fn style_problems(style: &str, properties: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for declaration in style.split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            problems.push(format!("the style `{style}` holds `{declaration}`, which is no declaration"));
            continue;
        };
        let (name, value) = (name.trim(), value.trim());
        if !properties.contains(name) {
            problems.push(format!(
                "the style `{style}` sets `{name}`, which is not a custom property the contract \
                 writes for a length field"
            ));
        } else if !seen.insert(name) {
            problems.push(format!("the style `{style}` sets `{name}` twice"));
        }
        if contract::ratio_properties().contains(name) {
            if !is_css_ratio(value) {
                problems.push(format!("the style `{style}` gives `{name}` the value `{value}`, \
                     which is not a ratio as the backend spells one"));
            }
        } else if !is_css_length(value) {
            problems.push(format!("the style `{style}` gives `{name}` the value `{value}`, which is \
                 not a length as the backend spells one"));
        }
    }
    problems
}

/// Whether `value` is a ratio as the backend writes one: two amounts greater
/// than zero, `<first> / <second>`.
fn is_css_ratio(value: &str) -> bool {
    let is_positive = |text: &str| {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, "0"));
        !whole.is_empty()
            && !fraction.is_empty()
            && whole.bytes().chain(fraction.bytes()).all(|byte| byte.is_ascii_digit())
            && text.bytes().any(|byte| (b'1'..=b'9').contains(&byte))
    };
    value.split_once(" / ").is_some_and(|(first, second)| is_positive(first) && is_positive(second))
}

/// Whether `value` is a length as the backend's one spelling writes it: a
/// decimal amount followed by one of the vocabulary's units other than the
/// tab, or `calc(<amount> * var(--flm-tab))` for a length in tabs.
fn is_css_length(value: &str) -> bool {
    let is_amount = |text: &str| {
        let digits = text.strip_prefix('-').unwrap_or(text);
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, "0"));
        !whole.is_empty()
            && !fraction.is_empty()
            && whole.bytes().chain(fraction.bytes()).all(|byte| byte.is_ascii_digit())
    };
    if let Some(amount) = value.strip_prefix("calc(").and_then(|rest| rest.strip_suffix(" * var(--flm-tab))")) {
        return is_amount(amount);
    }
    ["em", "pt", "mm", "cm", "in"].iter().any(|unit| value.strip_suffix(unit).is_some_and(is_amount))
}

/// Every structural problem of one parsed golden.
fn structural_problems(dom: &RcDom, allowed: &Allowance) -> Vec<String> {
    let found = elements_with(dom, allowed.construct_roots, allowed.template_tags);
    let mut problems = repeated_identifiers(&found);
    problems.extend(dangling_links(&found));
    problems.extend(classes_outside_the_contract(&found, allowed));
    problems.extend(names_outside_the_contract(&found));
    problems.extend(styles_outside_the_contract(&found));
    problems.extend(roles_outside_the_contract(&found));
    problems
}

// ============================================================================
// the tests
// ============================================================================

/// Every golden this suite found, so that a mistake in the discovery — a
/// directory renamed, an extension changed — is a failure rather than a silent
/// pass over nothing.
#[test]
fn the_discovery_finds_the_goldens_of_this_backend() {
    let goldens = goldens();
    assert!(
        goldens.len() >= 20,
        "the HTML goldens are not where the discovery looks: found {}",
        goldens.len()
    );
    assert!(
        goldens.iter().any(|golden| golden.shape == Shape::Page),
        "no golden is a whole page"
    );
    assert!(
        goldens.iter().any(|golden| golden.shape == Shape::Fragment),
        "no golden is a fragment"
    );
}

/// A golden's name says how it is parsed, and the bytes agree: a `.page.html`
/// golden begins with the document type line, and no other golden does.
#[test]
fn a_goldens_name_says_whether_it_is_a_page() {
    for golden in goldens() {
        let is_document = golden.text.trim_start().to_ascii_lowercase().starts_with("<!doctype");
        match golden.shape {
            Shape::Page => assert!(
                is_document,
                "{} is named as a page but does not begin with the document type line",
                golden.name
            ),
            Shape::Fragment => assert!(
                !is_document,
                "{} begins with the document type line but is not named `*.page.html`",
                golden.name
            ),
        }
    }
}

/// Every golden parses without a parse error.
#[test]
fn every_golden_parses_without_a_parse_error() {
    let mut problems: Vec<String> = Vec::new();
    for golden in goldens() {
        if excused(&golden.name).is_some() {
            continue;
        }
        let (_, errors) = parse(&golden.text, golden.shape);
        if !errors.is_empty() {
            problems.push(format!(
                "{} ({}):\n  {}",
                golden.name,
                golden.path.display(),
                errors.join("\n  ")
            ));
        }
    }
    assert!(problems.is_empty(), "the parser reported errors:\n{}", problems.join("\n"));
}

/// Every golden holds what the contract can produce: no identifier twice, no
/// link into the golden without its target, no class outside the contract, no
/// element and no attribute this backend does not write.
#[test]
fn every_golden_holds_what_the_contract_can_produce() {
    let mut problems: Vec<String> = Vec::new();
    for golden in goldens() {
        if excused(&golden.name).is_some() {
            continue;
        }
        let (dom, _) = parse(&golden.text, golden.shape);
        let mut found = structural_problems(&dom, &allowance(&golden.name));
        // The parsed tree holds a `<tbody>` the parser implied: the bytes say
        // whether the backend wrote one.
        for group in ["<tbody", "<thead"] {
            if golden.text.contains(group) {
                found.push(format!("the bytes hold `{group}`, which the backend never writes"));
            }
        }
        if !found.is_empty() {
            problems.push(format!("{}:\n  {}", golden.name, found.join("\n  ")));
        }
    }
    assert!(
        problems.is_empty(),
        "the goldens hold what the contract does not:\n{}",
        problems.join("\n")
    );
}

/// The classes of the contract are the classes the goldens carry: the check
/// above is not passing because its sets are empty.
#[test]
fn the_contract_sets_hold_what_the_backend_writes() {
    let elements = contract::element_classes();
    assert!(elements.contains("flm-pres-par"), "the element classes are empty");
    assert!(contract::auxiliary_classes().contains("flm-content"));
    assert!(
        contract::role_classes().contains("flm-heading"),
        "every element name of the standard package is read as a possible role, the heading's among them"
    );
    assert!(
        contract::role_classes().contains("flm-heading-number"),
        "the role of a numbered heading's number is read from the feature that writes it"
    );
    let tags = contract::tags();
    for tag in ["p", "div", "h1", "h6", "em", "strong", "code", "pre", "ol", "ul", "dl", "li",
                "dt", "dd", "a", "img", "span", "tex-html", "main", "html", "blockquote",
                "figure", "figcaption"] {
        assert!(tags.contains(tag), "`<{tag}>` is missing from the contract's tags");
    }
    let attributes = contract::attributes();
    for attribute in
        ["class", "id", "href", "src", "alt", "srcset", "data-flm-kind",
         "data-flm-level", "data-flm-style", "data-flm-display", "data-flm-env",
         "data-flm-lang", "data-flm-align", "style",
         "data-flm-element", "data-flm-name", "data-flm-command",
         "data-flm-source-span", "lang", "charset", "role", "aria-label"]
    {
        assert!(attributes.contains(attribute), "`{attribute}` is missing from the contract");
    }
    // Nothing a browser defines and this backend never writes has crept in,
    // and a length field is never also a `data-flm-` attribute, nor an
    // image's dimensions an attribute of their own.
    for attribute in ["onclick", "data-flm-kinds", "data-flm-doc-role", "aria-hidden",
                      "data-flm-indent-left", "data-flm-indent-right", "width", "height"] {
        assert!(!attributes.contains(attribute), "`{attribute}` is not this backend's");
    }
    let properties = contract::style_properties();
    for property in ["--flm-element-indent-left", "--flm-element-indent-right",
                     "--flm-element-width", "--flm-element-height", "--flm-element-aspect-ratio"] {
        assert!(properties.contains(property), "`{property}` is missing from the contract");
    }
    assert_eq!(
        contract::ratio_properties().into_iter().collect::<Vec<_>>(),
        ["--flm-element-aspect-ratio"]
    );
}

/// Every tag the contract claims is a tag some golden really holds.
///
/// Four of the contract's tag rules pick a tag from a payload — a heading's
/// level, a span's style, a verbatim's display, a list's kind — and the lists
/// of tags they pick from are copied into `contract::row_tags` from three
/// private functions of the emitter, which an integration test cannot call. So
/// the goldens are what holds the copy to the original: they exercise every
/// heading level, every span style, both verbatim displays and every list
/// kind, and a tag the copy claims that no golden holds means either that the
/// emitter stopped writing it — and the claim must go with it — or that the
/// golden which showed it is gone.
///
/// The anticipated page names are left out, since nothing writes them yet.
#[test]
fn every_tag_the_contract_claims_is_written_by_a_golden() {
    let mut written: BTreeSet<String> = BTreeSet::new();
    for golden in goldens() {
        let (dom, _) = parse(&golden.text, golden.shape);
        for element in elements(&dom, &[]) {
            written.insert(element.tag);
        }
    }
    let claimed: BTreeSet<String> = contract::row_tags()
        .into_iter()
        .chain(contract::extra_tags())
        .chain(contract::page_tags())
        .map(String::from)
        .collect();
    let missing: Vec<&str> = claimed
        .difference(&written)
        .map(|tag| tag.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "the contract claims tags no golden holds: {}",
        missing.join(", ")
    );
}

// ============================================================================
// the goldens excused
// ============================================================================

/// The goldens this suite does not check, each with the reason.
///
/// An entry belongs here only when the output **must** be invalid — when the
/// property the golden pins is that the backend writes something a parser
/// refuses, so that repairing it would destroy the test. A golden that is
/// invalid by accident is a defect of the emitter, not an entry here.
///
/// An entry waives **every** check of this suite, not the parse alone: an
/// excused golden is not parsed, and none of the structural assertions is run
/// over it either. That is deliberate — a structural walk over a tree the
/// parser had to repair reports what the repair did rather than what the
/// emitter wrote — and it is what makes an entry expensive: the golden stops
/// being watched here altogether.
///
/// The list is empty: everything this backend writes today parses.
const NOT_VALIDATED: &[(&str, &str)] = &[];

/// The reason the golden `name` is excused, when it is.
fn excused(name: &str) -> Option<&'static str> {
    NOT_VALIDATED.iter().find(|(golden, _)| *golden == name).map(|(_, reason)| *reason)
}

/// Every entry of `NOT_VALIDATED` names a golden that exists and carries a
/// reason.
#[test]
fn every_excused_golden_exists_and_says_why() {
    let names: BTreeSet<String> = goldens().into_iter().map(|golden| golden.name).collect();
    let problems = excuse_problems(NOT_VALIDATED, &names);
    assert!(problems.is_empty(), "the excuse list:\n{}", problems.join("\n"));
}

/// What is wrong with the excuse list `entries` against the goldens `names`:
/// an entry naming a golden that does not exist, an entry with no reason, and
/// a golden named twice.
///
/// It is a function of its own so that the test above and the fixture that
/// proves it catches a stale entry read the same code.
fn excuse_problems(entries: &[(&str, &str)], names: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (name, reason) in entries {
        if !names.contains(*name) {
            problems.push(format!("`{name}` is excused but is not a golden"));
        }
        if reason.trim().is_empty() {
            problems.push(format!("`{name}` is excused with no reason"));
        }
        if !seen.insert(name) {
            problems.push(format!("`{name}` is excused twice"));
        }
    }
    problems
}

// ============================================================================
// the fixtures: each assertion fails when it should
// ============================================================================

/// A block-level element inside a paragraph is what the emitter must never
/// write: the parser ends the paragraph at it, and the `</p>` written
/// afterwards closes nothing.
#[test]
fn a_block_inside_a_paragraph_is_a_parse_error() {
    let (_, errors) = parse("<p class=\"flm-pres-par\"><div>x</div></p>", Shape::Fragment);
    assert!(!errors.is_empty(), "a `<div>` inside a `<p>` went unreported");
    // The case that decided how a margin decoration is written: an `<aside>`
    // cannot stand inside a paragraph, so the decoration is a `<span>` at its
    // mark.
    let (_, errors) = parse(
        "<p class=\"flm-pres-par\">a<aside class=\"flm-pres-deco-margin\">n</aside></p>",
        Shape::Fragment,
    );
    assert!(!errors.is_empty(), "an `<aside>` inside a `<p>` went unreported");
}

/// A misnested pair of tags is a parse error.
#[test]
fn misnested_tags_are_a_parse_error() {
    let (_, errors) = parse("<em><strong>x</em></strong>", Shape::Fragment);
    assert!(!errors.is_empty(), "misnested tags went unreported");
}

/// A tag the text ends in the middle of is a parse error: the tokenizer's
/// half of the floor.
#[test]
fn a_tag_the_text_ends_inside_is_a_parse_error() {
    let (_, errors) = parse("<p class=\"a\">text</p", Shape::Fragment);
    assert!(!errors.is_empty(), "a tag the text ends inside went unreported");
}

/// An element left open where the text ends is a parse error: the tree
/// builder's half.
///
/// The element left open is an `<em>` on purpose. The algorithm's end-of-file
/// step names the elements that may still be open without an error — `p`,
/// `li`, `dt`, `dd`, `body` and `html` — so writing this fixture with an
/// unclosed `<p>` would assert nothing: that fragment parses cleanly, as the
/// floor at the top of this file says.
#[test]
fn an_element_left_open_is_a_parse_error() {
    let (_, errors) = parse("<p class=\"a\">text<em>more</p>", Shape::Fragment);
    assert!(!errors.is_empty(), "an element left open went unreported");
    // The exemption itself, so that the floor's statement of it is held by a
    // test rather than by a reading of the standard: a paragraph left open is
    // no parse error.
    let (_, errors) = parse("<p class=\"flm-pres-par\">text", Shape::Fragment);
    assert!(
        errors.is_empty(),
        "an unclosed `<p>` is exempt at the end of a file, yet was reported: {errors:?}"
    );
}

/// What this backend writes today parses: the fixtures above fail for a
/// reason, not because every fragment does.
#[test]
fn a_well_formed_fragment_is_no_parse_error() {
    let (_, errors) = parse(
        "<p class=\"flm-pres-par\">a &amp; b <em class=\"flm-pres-span\">c</em></p>\n",
        Shape::Fragment,
    );
    assert!(errors.is_empty(), "a well-formed fragment was reported: {errors:?}");
}

/// The excuse list's own check catches an entry that names no golden, one with
/// no reason, and one written twice.
#[test]
fn a_stale_excuse_is_caught() {
    let names: BTreeSet<String> = ["emit-paragraphs.html".to_string()].into_iter().collect();
    assert!(excuse_problems(&[("emit-paragraphs.html", "a reason")], &names).is_empty());
    assert_eq!(excuse_problems(&[("gone.html", "a reason")], &names).len(), 1);
    assert_eq!(excuse_problems(&[("emit-paragraphs.html", "  ")], &names).len(), 1);
    assert_eq!(
        excuse_problems(
            &[("emit-paragraphs.html", "a reason"), ("emit-paragraphs.html", "another")],
            &names
        )
        .len(),
        1
    );
}

/// An identifier written twice is caught.
#[test]
fn a_repeated_identifier_is_caught() {
    let (dom, _) = parse(
        "<p class=\"flm-pres-par\" id=\"x\">a</p><p class=\"flm-pres-par\" id=\"x\">b</p>",
        Shape::Fragment,
    );
    assert_eq!(structural_problems(&dom, &allowance("")).len(), 1);
}

/// A link into the golden whose target is not there is caught.
#[test]
fn a_dangling_link_is_caught() {
    let (dom, _) = parse(
        "<p class=\"flm-pres-par\"><a class=\"flm-pres-link\" href=\"#gone\">t</a></p>",
        Shape::Fragment,
    );
    assert_eq!(structural_problems(&dom, &allowance("")).len(), 1);
    // The same link with its target present is no problem, and neither is a
    // link to the top of the page.
    let (dom, _) = parse(
        "<p class=\"flm-pres-par\" id=\"gone\"><a class=\"flm-pres-link\" href=\"#gone\">t</a>\
         <a class=\"flm-pres-link\" href=\"#\">u</a></p>",
        Shape::Fragment,
    );
    assert!(structural_problems(&dom, &allowance("")).is_empty());
}

/// A class the contract cannot produce is caught, and so is a role class
/// written where the element's own class belongs.
#[test]
fn a_class_outside_the_contract_is_caught() {
    let (dom, _) = parse("<p class=\"prose\">a</p>", Shape::Fragment);
    assert_eq!(structural_problems(&dom, &allowance("")).len(), 1);
    let (dom, _) = parse("<p class=\"flm-heading flm-pres-par\">a</p>", Shape::Fragment);
    assert_eq!(structural_problems(&dom, &allowance("")).len(), 1);
    // The two in their contract order are no problem.
    let (dom, _) = parse("<p class=\"flm-pres-par flm-heading\">a</p>", Shape::Fragment);
    assert!(structural_problems(&dom, &allowance("")).is_empty());
}

/// An element and an attribute this backend does not write are caught.
#[test]
fn a_name_outside_the_contract_is_caught() {
    let (dom, _) = parse("<p class=\"flm-pres-par\"><aside>a</aside></p>", Shape::Fragment);
    // The `<aside>` is not this backend's, and the `</p>` the parser met
    // after it closes nothing — one structural problem and one parse error.
    assert_eq!(structural_problems(&dom, &allowance("")).len(), 1);
    let (dom, _) = parse("<p class=\"flm-pres-par\" onclick=\"x\">a</p>", Shape::Fragment);
    assert_eq!(structural_problems(&dom, &allowance("")).len(), 1);
}

/// A `role` attribute naming no document role the backend writes is caught;
/// the two roles of the list of cited works and of a link to one pass.
#[test]
fn a_role_outside_the_contract_is_caught() {
    let (dom, _) = parse(
        "<div class=\"flm-pres-block\" role=\"doc-bibliography\">\
         <a class=\"flm-pres-link\" href=\"#x\" id=\"x\" role=\"doc-biblioref\">1</a></div>",
        Shape::Fragment,
    );
    assert_eq!(structural_problems(&dom, &allowance("")), Vec::<String>::new());
    let (dom, _) =
        parse("<div class=\"flm-pres-block\" role=\"doc-biblioentry\">a</div>", Shape::Fragment);
    assert_eq!(structural_problems(&dom, &allowance("")).len(), 1);
}

/// A `style` attribute holding anything but a length field's custom property
/// — a CSS property, a property of no length field, a value that is no
/// length, a property set twice — is caught; the backend's own spelling
/// passes.
#[test]
fn a_style_outside_the_contract_is_caught() {
    let problems = |style: &str| {
        let text = format!("<div class=\"flm-pres-line\" style=\"{style}\">a</div>");
        let (dom, _) = parse(&text, Shape::Fragment);
        structural_problems(&dom, &allowance(""))
    };
    for good in [
        "--flm-element-indent-left: calc(2 * var(--flm-tab))",
        "--flm-element-indent-left: 1.5em; --flm-element-indent-right: 10pt",
        "--flm-element-indent-right: calc(-0.5 * var(--flm-tab))",
        "--flm-element-width: 5cm; --flm-element-height: 2in; --flm-element-aspect-ratio: 141.73 / 144",
    ] {
        assert_eq!(problems(good), Vec::<String>::new(), "{good}");
    }
    for bad in [
        "padding-left: 2em",
        "--flm-element-color: red",
        "--flm-tab: 3em",
        "--flm-element-indent-left: red",
        "--flm-element-indent-left: 2tab",
        "--flm-element-indent-left: 1em; --flm-element-indent-left: 2em",
        "--flm-element-indent-left: 1em; color: red",
        "--flm-element-aspect-ratio: 2cm",
        "--flm-element-aspect-ratio: 0 / 1",
        "--flm-element-width: 2 / 1",
    ] {
        assert!(!problems(bad).is_empty(), "`{bad}` passed");
    }
}

/// A golden's allowance exempts a construct node's subtree from the class and
/// the name checks, and nothing else.
#[test]
fn an_allowance_exempts_a_construct_subtree_and_no_more() {
    let text = "<aside class=\"picture-aside\" data-picture=\"v\">x</aside>\
                <p class=\"prose\">a</p>";
    let allowed = Allowance { golden: "", construct_roots: &["aside"], roles: &[], template_tags: &[] };
    // The `<aside>` and everything about it is the staging rule's; the
    // paragraph's class is still judged.
    let (dom, _) = parse(text, Shape::Fragment);
    assert_eq!(structural_problems(&dom, &allowed).len(), 1);
    // Without the allowance the `<aside>`, its class and its attribute are
    // three problems more.
    assert_eq!(structural_problems(&dom, &allowance("")).len(), 4);
}

/// A role a golden declares in its allowance is a class; one it does not
/// declare is not.
#[test]
fn a_declared_role_is_a_class_and_an_undeclared_one_is_not() {
    let (dom, _) = parse("<div class=\"flm-pres-block flm-theorem\">a</div>", Shape::Fragment);
    let allowed = Allowance { golden: "", construct_roots: &[], roles: &["flm.theorem"], template_tags: &[] };
    assert!(structural_problems(&dom, &allowed).is_empty());
    assert_eq!(structural_problems(&dom, &allowance("")).len(), 1);
}

/// The walk marks a construct node's subtree and nothing else.
#[test]
fn the_walk_marks_a_construct_subtree() {
    let (dom, _) = parse("<p class=\"flm-pres-par\"><figure><b>x</b></figure></p>", Shape::Page);
    let found = elements(&dom, allowance("emit-construct.html").construct_roots);
    let inside: BTreeSet<&str> = found
        .iter()
        .filter(|element| element.in_construct)
        .map(|element| element.tag.as_str())
        .collect();
    assert_eq!(inside, ["b", "figure"].into_iter().collect::<BTreeSet<_>>());
}

/// A page carrying the style and script definitions holds nothing outside the
/// sets, the attribute written with no value included.
///
/// A **valueless attribute** is one whose name alone says something, with no
/// `="…"` after it: a browser reads `defer` on a script that way, and the
/// parser hands it back with an empty value. The checks read attribute names,
/// so such an attribute is judged like any other.
#[test]
fn the_definitions_a_page_carries_hold_what_the_sets_allow() {
    let (dom, errors) = parse(
        "<style data-flm-def=\"flm.html.style.core\">.flm-pres-par { margin: 0; }</style>\
         <link rel=\"stylesheet\" href=\"sheet.css\" data-flm-def=\"flm.html.style.core\">\
         <script src=\"formulas.js\" defer data-flm-def=\"flm.html.script.math\"></script>",
        Shape::Fragment,
    );
    assert!(errors.is_empty(), "the definitions were not read as written: {errors:?}");
    let problems = structural_problems(&dom, &allowance(""));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // The valueless attribute really did reach the walk as a name with an
    // empty value, rather than being dropped before the checks saw it.
    let script = elements(&dom, &[])
        .into_iter()
        .find(|element| element.tag == "script")
        .expect("the script");
    assert_eq!(script.attribute("defer"), Some(""));
}

/// An element's attributes and classes are read as they were written.
#[test]
fn an_elements_attributes_are_read_as_written() {
    let (dom, _) =
        parse("<p class=\"flm-pres-par flm-heading\" id=\"x\">t</p>", Shape::Fragment);
    let paragraph = elements(&dom, &[])
        .into_iter()
        .find(|element| element.tag == "p")
        .expect("the paragraph");
    assert_eq!(paragraph.attribute("id"), Some("x"));
    assert_eq!(paragraph.classes(), vec!["flm-pres-par", "flm-heading"]);
}

// ============================================================================
// the stylesheet scanner
// ============================================================================

/// Where the default stylesheet's chunks live, relative to this crate's root.
///
/// Each file is one **style definition**: a complete piece of CSS a page
/// carries when it holds an element that needs it. The directory does not
/// exist until the default stylesheet is delivered, and an empty or absent
/// directory holds no chunk, which the scanner says and passes.
const STYLESHEET_DIRECTORY: &str = "src/stdcore/backends/html/css";

/// The one cascade layer everything this backend styles sits in.
///
/// A **cascade layer** is CSS's own way of saying "these rules lose to the
/// host page's own": a page that embeds FLM output can restyle it without
/// fighting specificity, and nothing here reaches the host's markup.
const LAYER: &str = "flm";

/// What every style definition's name begins with; a chunk names itself with
/// one on its first line.
const STYLE_PREFIX: &str = "flm.html.style.";

/// The chunk that styles the page itself — its typography and its measure, on
/// the `<body>` and on the content container.
///
/// It is the one chunk whose selectors are not classes of the contract,
/// because it styles the page skeleton rather than any node of a document; it
/// is required by the page assembly alone. It is told from the others by the
/// name it gives itself on its first line, not by the name of its file.
const PAGE_CHUNK: &str = "flm.html.style.page";

/// One chunk of the default stylesheet.
#[derive(Debug, Clone)]
struct Chunk {
    /// The file's name (`headings.css`).
    name: String,
    /// The file's name without its extension, which must be the last segment
    /// of the style definition's name the chunk gives itself.
    stem: String,
    /// Its text.
    text: String,
}

/// Every chunk of the default stylesheet, in name order; the empty list where
/// the directory does not exist yet.
fn chunks() -> Vec<Chunk> {
    let path = crate_path(STYLESHEET_DIRECTORY);
    let Ok(entries) = fs::read_dir(&path) else { return Vec::new() };
    let mut found: BTreeMap<String, Chunk> = BTreeMap::new();
    for entry in entries {
        let entry = entry.expect("a directory entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("css") {
            continue;
        }
        let name =
            path.file_name().and_then(|n| n.to_str()).expect("a chunk's name is text").to_string();
        let stem =
            path.file_stem().and_then(|n| n.to_str()).expect("a chunk's stem is text").to_string();
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read the chunk {}: {e}", path.display()));
        found.insert(name.clone(), Chunk { name, stem, text });
    }
    found.into_values().collect()
}

/// `text` with every comment replaced by one space, so that nothing inside a
/// comment is read as CSS.
///
/// An unterminated comment is a problem of its own: everything after it would
/// silently vanish.
fn without_comments(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        out.push(' ');
        let after = &rest[start + 2..];
        let Some(end) = after.find("*/") else {
            return Err(String::from("a comment is never closed"));
        };
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// The style definition a chunk names itself with on its first line, or the
/// reason it names none.
///
/// A chunk begins with a comment holding nothing but the definition's name —
/// `/* flm.html.style.core */` — and the name is read from the raw text,
/// before comments are removed, so that the chunk carries its own identity
/// rather than borrowing it from the name of its file. The name's shape is
/// fixed: the prefix every style definition of this backend shares, then one
/// lowercase segment.
fn marker(text: &str) -> Result<String, String> {
    let missing = format!(
        "the chunk does not begin with its own name in a comment, `/* {STYLE_PREFIX}<name> */`"
    );
    let Some(rest) = text.trim_start().strip_prefix("/*") else { return Err(missing) };
    let Some(end) = rest.find("*/") else {
        return Err(String::from("the chunk's first comment is never closed"));
    };
    let identifier = rest[..end].trim();
    if identifier.contains('\n') || !is_style_name(identifier) {
        return Err(format!(
            "the chunk begins with `/*{}*/`, which is not a style definition's name of the \
             form `{STYLE_PREFIX}<name>`",
            &rest[..end]
        ));
    }
    Ok(String::from(identifier))
}

/// Whether `identifier` is a style definition's name: the prefix every one of
/// them shares, then one segment of lowercase letters, digits and hyphens
/// beginning with a letter.
fn is_style_name(identifier: &str) -> bool {
    let Some(segment) = identifier.strip_prefix(STYLE_PREFIX) else { return false };
    segment.starts_with(|ch: char| ch.is_ascii_lowercase())
        && segment.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

/// The last segment of a style definition's name, which is what the chunk's
/// file is named after.
fn last_segment(identifier: &str) -> &str {
    identifier.rsplit('.').next().unwrap_or(identifier)
}

/// The problems of one chunk: everything the scanner checks.
///
/// The chunk is **scanned, not parsed**: there is no CSS parser here, and none
/// is wanted. What is checked is what this backend promises about the sheets
/// it ships, and each check reads the text for one thing.
fn chunk_problems(chunk: &Chunk) -> Vec<String> {
    let identifier = match marker(&chunk.text) {
        Ok(identifier) => identifier,
        Err(problem) => return vec![problem],
    };
    let text = match without_comments(&chunk.text) {
        Ok(text) => text,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    if last_segment(&identifier) != chunk.stem {
        problems.push(format!(
            "the chunk names itself `{identifier}` and is filed as `{}`: a chunk's file is \
             named after the last segment of its definition's name",
            chunk.name
        ));
    }
    if let Err(problem) = braces_balance(&text) {
        // Nothing below can be read from a chunk whose braces do not balance.
        return vec![problem];
    }
    let body = match one_layer(&text) {
        Ok(body) => body,
        Err(problem) => return vec![problem],
    };
    let classes: BTreeSet<String> = contract::element_classes()
        .into_iter()
        .chain(contract::auxiliary_classes())
        .chain(contract::role_classes())
        .collect();
    let attributes = contract::attributes();
    let is_page = identifier == PAGE_CHUNK;
    for selector in selectors(body) {
        if !is_page {
            for class in class_tokens(&selector) {
                if !classes.contains(&class) {
                    problems.push(format!(
                        "the selector `{}` names the class `{class}`, which the contract cannot \
                         produce",
                        selector.trim()
                    ));
                }
            }
            // Every selector begins with a class of the contract, so that a
            // page embedding this output keeps its own `p`, `a` and `table`
            // rules. The check is that the first compound carries a class at
            // all; the loop above is what makes that class the contract's.
            for complex in complex_selectors(&selector) {
                let compound = first_compound(complex);
                if compound.starts_with(":root") || !class_tokens(compound).is_empty() {
                    continue;
                }
                problems.push(format!(
                    "the selector `{complex}` does not begin with a class of the contract: a \
                     rule that begins with an element reaches a host page's own markup"
                ));
            }
        }
        for attribute in data_attributes(&selector) {
            if !attributes.contains(&attribute) && !contract::writes_attribute(&attribute) {
                problems.push(format!(
                    "the selector `{}` names the attribute `{attribute}`, which this backend \
                     does not write",
                    selector.trim()
                ));
            }
        }
    }
    // A property the page writes in an element's `style` attribute is set
    // there, per element, and a chunk applies it with a fallback of its own.
    let declared: BTreeSet<String> =
        declared_properties(body).into_iter().chain(contract::style_properties()).collect();
    for used in used_properties(body) {
        if !declared.contains(&used) {
            problems.push(format!(
                "the custom property `{used}` is used but is not declared in this chunk"
            ));
        }
    }
    problems
}

/// Whether the braces of `text` balance and never close one that was not
/// opened.
fn braces_balance(text: &str) -> Result<(), String> {
    let mut depth = 0i32;
    for (index, ch) in scan(text) {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return Err(format!("a `}}` at byte {index} closes nothing"));
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(format!("{depth} brace(s) are left open"));
    }
    Ok(())
}

/// The body of the chunk's one cascade layer, or the reason it has none.
///
/// A chunk is exactly one `@layer flm { … }` block and nothing outside it, so
/// that everything this backend styles loses to a host page's own rules.
fn one_layer(text: &str) -> Result<&str, String> {
    let opening = format!("@layer {LAYER}");
    let trimmed = text.trim();
    let Some(after) = trimmed.strip_prefix(&opening) else {
        return Err(format!("the chunk does not begin with `{opening} {{`"));
    };
    let after = after.trim_start();
    let Some(body) = after.strip_prefix('{') else {
        return Err(format!("the chunk does not begin with `{opening} {{`"));
    };
    let Some(body) = body.strip_suffix('}') else {
        return Err(String::from("the chunk holds something after its layer's closing brace"));
    };
    // The layer's own brace is the outermost one: a `}` that would close it
    // from inside the body means something stands beside the layer.
    let mut depth = 0i32;
    for (_, ch) in scan(body) {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return Err(String::from(
                        "the chunk holds something outside its `@layer` block",
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(body)
}

/// The characters of `text` that are not inside a quoted string, each with its
/// byte position.
///
/// A CSS string may hold a brace or a `.`, which would otherwise be read as
/// structure.
fn scan(text: &str) -> Vec<(usize, char)> {
    let mut out = Vec::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        match quote {
            Some(open) => {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == open {
                    quote = None;
                }
            }
            None => {
                if ch == '"' || ch == '\'' {
                    quote = Some(ch);
                } else {
                    out.push((index, ch));
                }
            }
        }
    }
    out
}

/// The selector of every rule of `body`, in the order they stand.
///
/// A **selector** here is what stands before a `{`: the selector list of an
/// ordinary rule, or the prelude of a nested at-rule (`@media …`), which is
/// handed back as it stands and holds no class of its own.
fn selectors(body: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut current = String::new();
    for (_, ch) in scan(body) {
        match ch {
            '{' => {
                let selector = std::mem::take(&mut current);
                let selector = selector.trim().to_string();
                if !selector.is_empty() && !selector.starts_with('@') {
                    found.push(selector);
                }
            }
            '}' | ';' => current.clear(),
            _ => current.push(ch),
        }
    }
    found
}

/// The complex selectors of a selector list: what stands between the commas
/// that are not inside brackets or parentheses.
///
/// A **complex selector** is one selector of the list — `.flm-pres-list > li`
/// — and what a browser matches on its own.
fn complex_selectors(list: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (index, ch) in list.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                found.push(list[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    found.push(list[start..].trim());
    found.into_iter().filter(|selector| !selector.is_empty()).collect()
}

/// The first compound of a complex selector: everything before its first
/// combinator.
///
/// A **compound selector** is the part that matches one element —
/// `.flm-pres-span[data-flm-style="small-caps"]` — and a **combinator** is
/// what stands between two of them: a space, `>`, `+` or `~`. A `~` or a space
/// inside brackets or parentheses belongs to an attribute or to a functional
/// selector and is not a combinator.
fn first_compound(selector: &str) -> &str {
    let mut depth = 0i32;
    for (index, ch) in selector.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            _ if depth == 0 && (ch.is_whitespace() || ch == '>' || ch == '+' || ch == '~') => {
                return selector[..index].trim_end();
            }
            _ => {}
        }
    }
    selector
}

/// The class names a selector list mentions: every `.` followed by a name.
///
/// A class named through an attribute selector (`[class~="flm-pres-par"]`) is
/// not read as a class here, by either rule that reads classes. No chunk is
/// written that way, and a chunk that were would escape both.
fn class_tokens(selector: &str) -> Vec<String> {
    let chars: Vec<char> = selector.chars().collect();
    let mut found = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '.' {
            let mut name = String::new();
            let mut at = index + 1;
            while at < chars.len()
                && (chars[at].is_ascii_alphanumeric() || chars[at] == '-' || chars[at] == '_')
            {
                name.push(chars[at]);
                at += 1;
            }
            if !name.is_empty() {
                found.push(name);
            }
            index = at;
        } else {
            index += 1;
        }
    }
    found
}

/// The `data-flm-…` attribute names a selector list mentions.
fn data_attributes(selector: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = selector;
    while let Some(start) = rest.find('[') {
        let after = &rest[start + 1..];
        let end = after.find(']').unwrap_or(after.len());
        let inside = &after[..end];
        let name: String = inside
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .collect();
        if name.starts_with("data-flm-") {
            found.push(name);
        }
        rest = &after[end.min(after.len())..];
        if rest.is_empty() {
            break;
        }
        rest = &rest[1.min(rest.len())..];
    }
    found
}

/// The custom properties `body` declares: a `--name` followed by a colon.
fn declared_properties(body: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let bytes: Vec<char> = body.chars().collect();
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == '-' && bytes[index + 1] == '-' {
            let mut name = String::from("--");
            let mut at = index + 2;
            while at < bytes.len()
                && (bytes[at].is_ascii_alphanumeric() || bytes[at] == '-' || bytes[at] == '_')
            {
                name.push(bytes[at]);
                at += 1;
            }
            let mut after = at;
            while after < bytes.len() && bytes[after].is_whitespace() {
                after += 1;
            }
            if name.len() > 2 && after < bytes.len() && bytes[after] == ':' {
                found.insert(name);
            }
            index = at;
        } else {
            index += 1;
        }
    }
    found
}

/// The custom properties `body` uses: the name inside every `var(…)`.
fn used_properties(body: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = body;
    while let Some(start) = rest.find("var(") {
        let after = rest[start + 4..].trim_start();
        let name: String = after
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .collect();
        if name.starts_with("--") && name.len() > 2 {
            found.insert(name);
        }
        rest = &rest[start + 4..];
    }
    found
}

/// Every chunk of the default stylesheet follows the rules this backend
/// promises: one cascade layer, contract selectors, and every custom property
/// it uses declared in it.
#[test]
fn every_stylesheet_chunk_is_written_to_the_contract() {
    let chunks = chunks();
    if chunks.is_empty() {
        println!(
            "no stylesheet chunk under {STYLESHEET_DIRECTORY}: the scanner has nothing to read \
             until the definitions are delivered"
        );
        return;
    }
    let mut problems: Vec<String> = Vec::new();
    for chunk in &chunks {
        let found = chunk_problems(chunk);
        if !found.is_empty() {
            problems.push(format!("{}:\n  {}", chunk.name, found.join("\n  ")));
        }
    }
    assert!(problems.is_empty(), "the stylesheet chunks:\n{}", problems.join("\n"));
}

/// A chunk written to the rules passes the scanner, and each rule broken is
/// caught: these are the scanner's own fixtures, since the chunks themselves
/// arrive with the default stylesheet.
#[test]
fn the_scanner_catches_a_chunk_that_breaks_a_rule() {
    // Every chunk here names itself on its first line, as a chunk must; the
    // marker's own fixtures are the test below.
    let chunk = |text: &str| Chunk {
        name: String::from("headings.css"),
        stem: String::from("headings"),
        text: format!("/* {STYLE_PREFIX}headings */\n{text}"),
    };
    // A chunk written to the rules.
    let good = chunk(
        "@layer flm {\n  :root { --flm-heading-gap: 0.5rem; }\n  \
         .flm-pres-block--heading[data-flm-level=\"1\"] { margin-top: var(--flm-heading-gap); }\n  \
         .flm-pres-list > li, .flm-pres-deco-margin::before { color: red; }\n}",
    );
    assert!(chunk_problems(&good).is_empty(), "{:?}", chunk_problems(&good));

    // Braces that do not balance.
    let problems = chunk_problems(&chunk("@layer flm {\n  .flm-pres-par { color: red;\n}"));
    assert_eq!(problems.len(), 1, "{problems:?}");

    // A comment that is never closed.
    let problems = chunk_problems(&chunk("/* open\n@layer flm { .flm-pres-par { color: red; } }"));
    assert_eq!(problems.len(), 1, "{problems:?}");

    // Rules outside the one cascade layer.
    let problems = chunk_problems(&chunk(".flm-pres-par { color: red; }"));
    assert_eq!(problems.len(), 1, "{problems:?}");
    let problems = chunk_problems(&chunk(
        "@layer flm { .flm-pres-par { color: red; } }\n.flm-pres-span { color: blue; }",
    ));
    assert_eq!(problems.len(), 1, "{problems:?}");

    // A class the contract cannot produce.
    let problems = chunk_problems(&chunk("@layer flm { .prose { color: red; } }"));
    assert_eq!(problems.len(), 1, "{problems:?}");

    // An attribute this backend does not write.
    let problems =
        chunk_problems(&chunk("@layer flm { .flm-pres-par[data-flm-mood=\"glad\"] { color: red; } }"));
    assert_eq!(problems.len(), 1, "{problems:?}");

    // A custom property used but not declared in this chunk.
    let problems =
        chunk_problems(&chunk("@layer flm { .flm-pres-par { margin: var(--flm-gap); } }"));
    assert_eq!(problems.len(), 1, "{problems:?}");

    // A selector that does not begin with a class of the contract: exactly
    // what a host page's own markup would be restyled by.
    let problems = chunk_problems(&chunk(
        "@layer flm { p { margin: 0; } a { color: blue; } table { width: 100%; } }",
    ));
    assert_eq!(problems.len(), 3, "{problems:?}");

    // The element a formula renderer writes is scoped under a contract class,
    // never selected on its own: a host page running the same renderer has
    // such elements too.
    let problems = chunk_problems(&chunk("@layer flm { tex-html { display: none; } }"));
    assert_eq!(problems.len(), 1, "{problems:?}");
    let problems =
        chunk_problems(&chunk("@layer flm { .flm-pres-math--passthrough tex-html { display: none; } }"));
    assert!(problems.is_empty(), "{problems:?}");

    // A selector whose first compound names an element and a contract class is
    // scoped and passes; one whose class stands only after a combinator does
    // not.
    let problems = chunk_problems(&chunk("@layer flm { p.flm-pres-par { margin: 0; } }"));
    assert!(problems.is_empty(), "{problems:?}");
    let problems = chunk_problems(&chunk("@layer flm { div > .flm-pres-par { margin: 0; } }"));
    assert_eq!(problems.len(), 1, "{problems:?}");
}

/// A chunk names itself on its first line, and its file is named after the
/// last segment of that name.
#[test]
fn a_chunk_names_itself_on_its_first_line() {
    let chunk = |text: &str| Chunk {
        name: String::from("headings.css"),
        stem: String::from("headings"),
        text: String::from(text),
    };
    let body = "@layer flm { .flm-pres-block--heading { margin-top: 0; } }";

    // The marker as a chunk carries it.
    assert!(
        chunk_problems(&chunk(&format!("/* {STYLE_PREFIX}headings */\n{body}"))).is_empty(),
        "a chunk that names itself was rejected"
    );

    // No marker at all, and a first comment that is not a definition's name.
    assert_eq!(chunk_problems(&chunk(body)).len(), 1);
    assert_eq!(chunk_problems(&chunk(&format!("/* the headings */\n{body}"))).len(), 1);
    assert_eq!(chunk_problems(&chunk(&format!("/* flm.html.headings */\n{body}"))).len(), 1);
    assert_eq!(chunk_problems(&chunk(&format!("/* {STYLE_PREFIX}Headings */\n{body}"))).len(), 1);

    // A name whose last segment is not the file's name: the file and the
    // definition have drifted apart.
    let problems = chunk_problems(&chunk(&format!("/* {STYLE_PREFIX}titles */\n{body}")));
    assert_eq!(problems.len(), 1, "{problems:?}");

    // The identifier, not the file's name, is what excepts the page chunk: a
    // chunk that selects the page itself passes under the page definition's
    // name and fails under another, whatever its file is called.
    let page = |identifier: &str| Chunk {
        name: String::from("page.css"),
        stem: String::from("page"),
        text: format!("/* {identifier} */\n@layer flm {{ body {{ margin: 0; }} }}"),
    };
    assert!(chunk_problems(&page(PAGE_CHUNK)).is_empty());
    // Under another definition's name the same text is a chunk filed wrongly
    // and a selector that does not begin with a contract class.
    assert_eq!(chunk_problems(&page(&format!("{STYLE_PREFIX}core"))).len(), 2);
}

/// The page chunk is the one whose selectors are not the contract's: it styles
/// the page skeleton, not any node of a document.
#[test]
fn the_page_chunk_may_select_the_page_itself() {
    let page = Chunk {
        name: String::from("page.css"),
        stem: String::from("page"),
        text: format!(
            "/* {PAGE_CHUNK} */\n@layer flm {{\n  :root {{ --flm-measure: 34rem; }}\n  \
             body {{ margin: 0; }}\n  \
             #flm-content {{ max-width: var(--flm-measure); }}\n}}"
        ),
    };
    assert!(chunk_problems(&page).is_empty(), "{:?}", chunk_problems(&page));
    // A class the page chunk invents for the page's own frame is allowed here
    // and caught in any other chunk, where it is a class the contract cannot
    // produce.
    let invented = "@layer flm { .page-frame { margin: 0; } }";
    let in_page =
        Chunk { text: format!("/* {PAGE_CHUNK} */\n{invented}"), ..page.clone() };
    assert!(chunk_problems(&in_page).is_empty());
    let elsewhere = Chunk {
        name: String::from("core.css"),
        stem: String::from("core"),
        text: format!("/* {STYLE_PREFIX}core */\n{invented}"),
    };
    assert_eq!(chunk_problems(&elsewhere).len(), 1);
}

/// A comment and a quoted string hide nothing and say nothing: a brace inside
/// one is not structure, and a class named inside one is not a selector.
#[test]
fn the_scanner_reads_neither_comments_nor_strings_as_css() {
    let chunk = Chunk {
        name: String::from("errors.css"),
        stem: String::from("errors"),
        text: format!(
            "/* {STYLE_PREFIX}errors */\n@layer flm {{\n  /* .prose {{ */\n  \
             .flm-pres-unsupported::after {{ content: \".prose {{\"; }}\n}}"
        ),
    };
    assert!(chunk_problems(&chunk).is_empty(), "{:?}", chunk_problems(&chunk));
}
