//! The extension `flmdoc`: the markup of documentation pages, such as the pages the command
//! `flm info` shows. It gives a document a code span written between backticks, the
//! constructs that lay out the documentation of one item (a command, an environment, a
//! feature, …), lists of named things and inline lists, the type a JSON Schema describes, and
//! a link to another documentation page, each lowered to the presentation elements that the
//! `text` and `html` backends write.
//!
//! The extension is written against the guest client `flm_ext::guest`. The `flm` program
//! carries it compiled in (the engine `native`); `extensions/rust/build-wasm.sh` builds the
//! same code as a WebAssembly module (the engine `wasm`). A document uses it by listing
//! `flmdoc` in its `use` list.
//!
//! # What an author writes
//!
//! The extension's package `_` (the package a `use` entry `flmdoc` gives) holds the feature
//! `flmdoc`, which declares these commands, each available in text (not in mathematics):
//!
//! | written | element | what it is |
//! |---|---|---|
//! | `` `code` ``, ``` ``a`b`` ``` | `flmdoc.code` | a code span: raw text shown as inline code |
//! | `\begin{docitem}[facts]{kind}{name} … \end{docitem}` | `flmdoc.item` | the documentation of one item |
//! | `\docsignature{…}` | `flmdoc.signature` | how the item is written, as a block of code |
//! | `\docsummary{…}` | `flmdoc.summary` | the summary of a documentation text, as a paragraph |
//! | `\begin{docdetails} … \end{docdetails}` | `flmdoc.details` | the rest of a documentation text, as blocks |
//! | `\begin{docparams}[title] … \end{docparams}` | `flmdoc.params` | the list of the item's parameters |
//! | `\docparam[facts]{form}{name}{role}{text}` | `flmdoc.param` | one parameter, in the body of `docparams` |
//! | `\begin{docentries}[options] … \end{docentries}` | `flmdoc.entries` | a list of named things, under a heading |
//! | `\docentry[facts]{name}{summary}` | `flmdoc.entry` | one listed thing, in the body of `docentries` |
//! | `\begin{docinlinelist} \item[facts] … \end{docinlinelist}` | `flmdoc.inline-list` | a list shown inline |
//! | `\item[facts]` | `flmdoc.inline-item` | the beginning of an item, in the body of `docinlinelist` |
//! | `\docjsontype{schema}` | `flmdoc.json-type` | the type a JSON Schema describes, in words |
//! | `\docfield{key}{value}` | `flmdoc.field` | one labeled fact, as a paragraph |
//! | `\docref{path}{text}` | `flmdoc.ref` | a link to another documentation page |
//!
//! A **code span** begins with a run of backticks and ends at the next run of as many
//! backticks: `` `x` `` holds `x`, and ``` ``a`b`` ``` holds ``a`b``. Its content is raw
//! text, in which no markup character means anything, and it may not hold the run that
//! opened it. The content is then adjusted in two steps, as in CommonMark: each line ending
//! becomes a space, so that a span may be wrapped across lines of the source; then, when the
//! content both begins and ends with a space and is not all spaces, one space is removed
//! from each end, so that ``` `` `a` `` ``` holds `` `a` ``. The special character sequence
//! reading the first backtick takes one argument of FLM's argument code ``` F`` ```, which
//! reads the rest of the run, the content and the closing run.
//!
//! In a document that uses the extension, every backtick in text opens a code span. The
//! quotation marks that FLM otherwise writes with backticks, ``` ``…'' ``` and `` `…' ``,
//! are therefore not available: write “…” and ‘…’ directly. A backtick that no later run of
//! as many backticks closes is an error. Since a span runs on to the next such run, even
//! across paragraphs, the error of a stray backtick may be reported at a later backtick of
//! the document rather than at the stray one.
//!
//! The arguments `kind`, `name` of `docitem`, the signature, the arguments `form`, `name`
//! and `role` of `\docparam`, the schema of `\docjsontype` and the path of `\docref` are
//! read as raw text between a pair of delimiters (FLM's argument code `v`): usually braces,
//! `{…}`, inside which braces pair up; another delimiter, such as `|…|`, lets the text hold
//! a brace alone, as in `\docsignature|\{|`. The others hold document content. The body of
//! `docparams` holds `\docparam` commands, the body of `docentries` holds `\docentry`
//! commands, and the body of `docinlinelist` holds `\item` commands and the content of the
//! items: each command is declared in a scoped package (`flmdoc.params.body`,
//! `flmdoc.entries.body`, `flmdoc.inline-list.body`) which the environment's body makes
//! available, and nowhere else. The body of `docinlinelist` is inline content, so that the
//! list may stand in a sentence or in a value; its items are its `\item` commands, each
//! with the content up to the next `\item` or the end of the list.
//!
//! # Facts as key–values
//!
//! Facts that the documentation composes from structure (who declared an item, where a
//! command is available, a package's origin, a stage's status in a plan, the notes of a
//! parameter, an entry or an item of an inline list) are given as key–values in the
//! construct's optional argument, `[key=value, flag, …]`; `flmdoc` writes their labels and
//! their words. A value is document content (a link, code, an inline list, text), and a
//! value written as one group, `{…}`, is the group's content, so that a value may hold a
//! comma or an `=` between braces. Some keys take one word, with no whitespace, and a flag
//! takes no value. A key given twice is read once, its last value in effect. A fact with
//! one plain value and a label of the page's own is a `\docfield{label}{value}` instead.
//!
//! The keys of each construct, what each accepts and the words `flmdoc` writes for it
//! (`X` stands for the value; a key in parentheses is read only together with the key
//! before it):
//!
//! **`docitem` and `\docentry`**, written on `docitem` as labeled facts at the end of its
//! body, and on `\docentry` as labeled facts after its first paragraph, in this order:
//!
//! | key | value | words |
//! |---|---|---|
//! | `anchor` (`docitem` only) | one word | none: the identifier a link to the item points at |
//! | `declared-by` | content | “Declared by: the feature X” |
//! | (`declarer`) | `feature` (the default), `language`, `program`, `package` or `data` | “the feature X”, “the language itself (X)”, “the program, as data (X)”, “the package X”, “written as data by X” |
//! | (`within`) | content | “, in the scoped package X” after who declared it |
//! | (`switched-off`) | content | “, which is switched off” after who declared it, and “Switched off: in X” |
//! | (`page`) | content | “; see X” after who declared it |
//! | `modes` | content: one mode, or an inline list of modes | “Available in: X mode”, or “X modes” for an inline list of several |
//! | `places` | content: an inline list of places (below) | “Available in: inside X”, after the modes and a comma when both are given |
//! | `origin` | `compiled`, `extension` or `bundled` | on `docitem`: “Origin: compiled into the program”, “from the extension E”, “from the extension E, bundled into the program” |
//! | (`extension`) | content, needed by `origin=extension` and `origin=bundled` | E above |
//! | `plan` (`docitem` only) | `scheduled`, `ruled-out` or `not-scheduled` | “In the plan in view: scheduled, in batch N”, “scheduled, as a rewrite stage” (no batch), “ruled out by B, which provides O as well”, “not scheduled: nothing the plan needs requires it” |
//! | (`workflow`) | content | the label “In the plan of the workflow X” in place of “In the plan in view” |
//! | (`batch`) | a whole number, with `plan=scheduled` | N above |
//! | (`by`), (`over`) | content, needed by `plan=ruled-out` | B and O above |
//!
//! **`\docentry` alone**, written in its first paragraph: the name, then the notes in
//! parentheses, separated by `; `, then the clauses, then `: ` and the rule, the summary and
//! the sentences:
//!
//! | key | value | words |
//! |---|---|---|
//! | `origin`, `extension` | as above | a note: “compiled into the program”, … |
//! | `owner` | content | a note: X (the declaring feature) |
//! | `data` | flag; not with `owner` | a note: “given as data” |
//! | `outputs` | content | a note: “outputs: X” |
//! | `from` | content | a note: X (where a folder was set) |
//! | `absent` | flag | a clause: “, which does not exist” |
//! | `by`, `over` | content, each needing the other | a clause: “, by B, which provides O as well” (a ruled-out stage) |
//! | `profile` | content | a clause: “ provides the profile X” |
//! | `rule` | content | “: X” after the name (the lowering rule chosen for an element) |
//! | `written` | content | a sentence after the summary: “Written as X.” |
//! | `unwritten` | flag; not with `written` | a sentence: “No command is written for this value.” |
//! | `aspects` | content | a sentence: “Aspects: X.” |
//!
//! **`\docparam`**, written after the text as notes in parentheses, separated by `; `, then
//! the sentence of `repeatable`; after a text of paragraphs, in a paragraph of their own:
//!
//! | key | value | words |
//! |---|---|---|
//! | `layout` | one word | “laid out as `X`” |
//! | `mode` | one word | “read in `X` mode” |
//! | `scoped` | content: one package, or an inline list | “with the scoped package X available inside”, “packages” for several |
//! | `kept` | flag | “content kept as written” |
//! | `field` | content | “an argument of the field X” |
//! | `default` | content | “default: X” |
//! | `reader` | content | “read as X” |
//! | `repeatable` | flag | the sentence “It may be written more than once.” |
//!
//! **`\item` of `docinlinelist`**: what the item names and the part of a command an item
//! of `places` names, before its content, then its notes in parentheses, separated by `; `:
//!
//! | key | value | words |
//! |---|---|---|
//! | `package` | flag | before the content: “the package ” |
//! | `body` | flag | before the content: “the body of ” |
//! | `argument` | one word: a position, or a name; not with `body` | “argument N of ”, or “the argument `X` of ” |
//! | `field` | content | after the above: “the field X of ” |
//! | `writer` | content | a note: “written by X” |
//! | `optional` | flag | a note: “may be absent” |
//! | `missing` | flag | a note: “not in the registry” |
//!
//! **`docentries`** takes options, not facts: `title` (content), the heading of the page's
//! part that the list is; `intro` (content), a paragraph under it; `level` (a whole number
//! from 0 to 255, 1 by default), the heading's level.
//!
//! A key that a construct does not read, a value that a key does not accept, a flag given a
//! value, a key given without a value it needs, a key given without the key it is read with,
//! a value missing a key it needs and two keys that may not be given together are each an
//! Error at the construct (`flmdoc.unknown-key`, `flmdoc.key-value`,
//! `flmdoc.key-dependency`; a bracket whose keys are not plain text, `flmdoc.keyvals`); the
//! key concerned is not read, and for a value missing a key it needs, nor is that value.
//!
//! # What each command becomes
//!
//! Each element has one lowering rule, its default on every backend, which stages
//! presentation nodes (paragraphs, blocks, headings, spans, a list, code, a link) whose roles
//! are `flmdoc.<construct>` and `flmdoc.<construct>.<part>`; the words of each composed fact
//! are in a span of the role `flmdoc.fact.<key>` (`flmdoc.fact.declared-by`,
//! `flmdoc.fact.available-in` and `flmdoc.fact.plan` for the labeled facts that several keys
//! compose). The `html` backend writes a role as a class and a kind as the attribute
//! `data-flm-kind`; the `text` backend prints the content. The rules:
//!
//! - a code span: a `flm.pres.verbatim` node of the kind `code`, inline, holding the
//!   characters, as `\verbcode` gives;
//! - `docitem`: a `flm.pres.block` of the role `flmdoc.item` whose kind is the item's kind
//!   and whose anchor is its key `anchor`, holding a paragraph with the kind word (a span of
//!   the role `flmdoc.item.kind`) and the name as inline code (a bold span of the role
//!   `flmdoc.item.name`), then the body, then the item's labeled facts, each a paragraph like
//!   that of `\docfield`;
//! - `\docsignature`: a block of the role `flmdoc.signature` holding the signature as a
//!   block of code;
//! - `\docsummary`: a paragraph holding a span of the role `flmdoc.summary`;
//! - `docdetails`: a block of the role `flmdoc.details` holding the body;
//! - `docparams`: a block of the role `flmdoc.params` holding a paragraph with the title (a
//!   bold span of the role `flmdoc.params.title`, `Arguments` when no title is given), then
//!   a `flm.pres.list` of the kind `flmdoc.params` with one entry per block of the body
//!   (the `text` backend prints each entry as a paragraph of its own, with no marker);
//! - `\docparam`: the entry's content: a paragraph with the form (a monospace span of the
//!   role `flmdoc.param.form`), the name when given (a bold span of the role
//!   `flmdoc.param.name`), the role when given (a span of the role `flmdoc.param.role`, in
//!   parentheses on the `text` backend), then `: ` and the text and the facts; text that
//!   holds paragraphs follows that first paragraph instead, the facts in a paragraph after
//!   it;
//! - `docentries`: a block of the role `flmdoc.entries` holding the title as a
//!   `flm.pres.block-heading` named `flmdoc.entries` of the level `level`, not in a table
//!   of contents, the introduction as a paragraph holding a span of the role
//!   `flmdoc.entries.intro`, then a `flm.pres.list` of the kind `flmdoc.entries` with one
//!   entry per block of the body, each marked `•`;
//! - `\docentry`: the entry's content: a paragraph with the name (a span of the role
//!   `flmdoc.entry.name`), its notes and clauses, then `: ` and its rule, the summary (a
//!   span of the role `flmdoc.entry.summary`) and its sentences, then its labeled facts;
//! - `docinlinelist`: a span of the role `flmdoc.inline-list` holding its items, separated
//!   by `, `, each a span of the role `flmdoc.inline-item`; content before the first `\item`
//!   is the Error `flmdoc.inline-list`, and is shown as an item of its own. An `\item` that
//!   does not stand directly in the body of `docinlinelist` is the same Error, and writes
//!   nothing;
//! - `\docjsontype`: a span of the role `flmdoc.json-type` holding, as inline code, the
//!   type the schema describes, in words: “one of "a", "b"” (`enum`), the value (`const`),
//!   the name of the definition (`$ref`), “A or B” (`anyOf`, `oneOf`, a list of types),
//!   “list of X” (`items`), “map to X” (`additionalProperties`), the type's name, or “any
//!   value”. Text that is not JSON is the Error `flmdoc.json-type`, and is shown as inline
//!   code as written;
//! - `\docfield`: a paragraph with the key (a bold span of the role `flmdoc.field.key`),
//!   `: `, and the value (a span of the role `flmdoc.field.value`);
//! - `\docref`: a `flm.pres.link` of the kind `flmdoc.page` holding the text, whose address
//!   is written from the path as the render option `link_style` says (below).
//!
//! An argument read as raw text whose content is not characters (possible only where a
//! user definition's template places one of its arguments there) is lowered as the
//! library lowers any such raw text: it is reported, and shown as the source text it holds.
//!
//! The extension declares no rule of its own for a LaTeX backend. The `latex-rendered`
//! backend writes the presentation nodes the rules stage, as it writes every presentation
//! node, so a document using the extension renders there (and to PDF, through the profile
//! `pdf-latex`); the `latex-recomposed` backend does not write them, and the plan of a
//! document using the extension stops there, listing the elements the backend cannot render.
//!
//! # The setting `link_style`
//!
//! The feature's option type `flmdoc.links`, set on a render request (`-O
//! flmdoc.link_style=path` on the command line), holds the key `link_style`: how
//! `\docref{features/headings}{…}` writes its address. `command` (the default) writes the
//! command that shows the page, `flm info features/headings`; `path` writes the page's path
//! followed by `.html`, `features/headings.html`. That address is counted from the root of
//! the page tree, not from the page holding the link (the extension does not know the path
//! of the page being rendered): it reaches the page from a page at the root of the tree, and
//! not from a page in a folder below it. Any other value is the Error `flmdoc.link-style`,
//! reported at each `\docref`, and the address is written as for `command`.

mod facts;
mod schema;
mod words;

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::api::model::aspects::FixedBlockRole;
use flm_ext::guest::core::lowering::{
    DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule, Staged,
};
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::lang::mint::{ArgDef, ArgLayout, BodyDef, ElementDef, ScopedPackageDef, SyntaxDef};
use flm_ext::guest::lang::rewrites::blocks::argument_holds_blocks;
use flm_ext::guest::lang::rewrites::userdefs::StagedNodes;
use flm_ext::guest::lang::text::chars_text;
use flm_ext::guest::model::aspects::BlockRole;
use flm_ext::guest::model::element::{Aspect, ElementSpec};
use flm_ext::guest::stdlib::backends::lowering::{is_blank, stage_trimmed};
use flm_ext::guest::stdlib::backends::pres;
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::techy::node::{Node, Nodes};
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::{Map, Value, json};

use facts::{Accepts, Companion, Facts, Key, Required, Vocabulary, key};

/// The feature's name.
const FEATURE: &str = "flmdoc";

/// The element of a code span, `` `…` ``.
const CODE: &str = "flmdoc.code";
/// The element of the environment `docitem`.
const ITEM: &str = "flmdoc.item";
/// The element of `\docsignature`.
const SIGNATURE: &str = "flmdoc.signature";
/// The element of `\docsummary`.
const SUMMARY: &str = "flmdoc.summary";
/// The element of the environment `docparams`.
const PARAMS: &str = "flmdoc.params";
/// The element of `\docparam`.
const PARAM: &str = "flmdoc.param";
/// The element of `\docfield`.
const FIELD: &str = "flmdoc.field";
/// The element of `\docref`.
const REF: &str = "flmdoc.ref";
/// The element of the environment `docdetails`.
const DETAILS: &str = "flmdoc.details";
/// The element of the environment `docentries`.
const ENTRIES: &str = "flmdoc.entries";
/// The element of `\docentry`.
const ENTRY: &str = "flmdoc.entry";
/// The element of the environment `docinlinelist`.
pub(crate) const INLINE_LIST: &str = "flmdoc.inline-list";
/// The element of `\item` in the body of `docinlinelist`.
pub(crate) const INLINE_ITEM: &str = "flmdoc.inline-item";
/// The element of `\docjsontype`.
const JSON_TYPE: &str = "flmdoc.json-type";

/// The scoped package holding `\docparam`, which the body of `docparams` makes available.
const PARAMS_BODY: &str = "flmdoc.params.body";
/// The scoped package holding `\docentry`, which the body of `docentries` makes available.
const ENTRIES_BODY: &str = "flmdoc.entries.body";
/// The scoped package holding `\item`, which the body of `docinlinelist` makes available.
const INLINE_LIST_BODY: &str = "flmdoc.inline-list.body";

/// The option type holding the key `link_style`, set on a render request.
const LINKS: &str = "flmdoc.links";
/// The key of [`LINKS`] that says how `\docref` writes its address.
const LINK_STYLE: &str = "link_style";
/// The value of `link_style` that writes the command showing the page (the default).
const LINK_STYLE_COMMAND: &str = "command";
/// The value of `link_style` that writes the path of the page's HTML file.
const LINK_STYLE_PATH: &str = "path";
/// The Error reported for a value of `link_style` that is neither of the two.
const BAD_LINK_STYLE: &str = "flmdoc.link-style";

/// The Error reported for the text of `\docjsontype` that is not JSON.
const BAD_JSON: &str = "flmdoc.json-type";
/// The Error reported for content of `docinlinelist` before its first `\item`, and for an
/// `\item` that does not stand directly in the body of `docinlinelist`.
const BAD_INLINE_LIST: &str = "flmdoc.inline-list";

/// The kind of the link `\docref` stages.
const PAGE_LINK_KIND: &str = "flmdoc.page";

/// The title of `docparams` when its optional argument is not given.
const DEFAULT_PARAMS_TITLE: &str = "Arguments";

/// The backend on which the role of a parameter is printed in parentheses.
const TEXT_BACKEND: &str = "text";

/// The separator `flmdoc` writes between the items of an inline list.
const INLINE_SEPARATOR: &str = ", ";
/// The label `flmdoc` writes before each entry of `docentries`.
const ENTRY_MARKER: &str = "•";

// The arguments, by name.
/// The content of a code span.
const CODE_ARG: &str = "code";
/// The key–values of the facts of `docitem`, `\docparam`, `\docentry` and `\item`.
const FACTS_ARG: &str = "facts";
/// The key–values of the options of `docentries`.
const OPTIONS_ARG: &str = "options";
/// The schema of `\docjsontype`.
const SCHEMA_ARG: &str = "schema";
/// The summary of `\docentry`.
const SUMMARY_ARG: &str = "summary";
/// The kind of `docitem`.
const KIND_ARG: &str = "kind";
/// The name of `docitem`, of `\docparam` and of `\docentry`.
const NAME_ARG: &str = "name";
/// The signature of `\docsignature`.
const SIGNATURE_ARG: &str = "signature";
/// The content of `\docsummary`, the text of `\docparam` and the text of `\docref`.
const TEXT_ARG: &str = "text";
/// The title of `docparams`.
const TITLE_ARG: &str = "title";
/// The form of `\docparam`.
const FORM_ARG: &str = "form";
/// The role of `\docparam`.
const ROLE_ARG: &str = "role";
/// The key of `\docfield`.
const KEY_ARG: &str = "key";
/// The value of `\docfield`.
const VALUE_ARG: &str = "value";
/// The path of `\docref`.
const PATH_ARG: &str = "path";

/// The argument code of the content of a code span: raw text after a run of backticks, up
/// to the same run followed by one more backtick (the special character sequence reads the
/// first backtick).
const CODE_SPAN_CODE: &str = "F``";
/// The argument code of a raw-text argument between a pair of delimiters: usually braces,
/// or another delimiter when the text holds a brace alone (`\docsignature|\{|`).
const RAW_TEXT_CODE: &str = "v";

/// The extension's code, which its row of the table of code of `flm-extensions` holds: the
/// function that declares its packages, and the extension's name and version.
pub static FLM_EXTENSION: flm_ext::guest::CompiledExtension =
    flm_ext::guest::CompiledExtension::new(extension, "flmdoc", env!("CARGO_PKG_VERSION"));

fn extension(ext: &mut Extension) {
    ext.package(
        "_",
        Package::new().feature(
            FEATURE,
            Feature::new()
                .title("Documentation markup")
                .docs(
                    "The markup of documentation pages: code spans between backticks, the documentation of an \
                     item with its signature, summary, parameters and facts, and links to other pages.",
                )
                .settings(settings())
                .register(register),
        ),
    );
}

/// The feature's settings, declared as data: the option type [`LINKS`], set on a render
/// request, with its key `link_style`.
fn settings() -> Value {
    json!({
        "options": {
            LINKS: {
                "bind": "render",
                "docs": "How a link to another documentation page is written.",
                "schema": {
                    "type": "object",
                    "properties": {
                        LINK_STYLE: {
                            "type": "string",
                            "enum": [LINK_STYLE_COMMAND, LINK_STYLE_PATH],
                            "default": LINK_STYLE_COMMAND,
                            "description": "How `\\docref` writes the address of a page: `command` writes the \
                                            command that shows the page (`flm info features/headings`), `path` \
                                            the page's path followed by `.html`, counted from the root of the \
                                            page tree (`features/headings.html`).",
                        },
                    },
                },
            },
        },
    })
}

// The constructs, their arguments and their presentation: each construct lowers to standard
// presentation nodes with roles of its own, so that every backend renders the parts of an
// item exactly and keeps their meaning for a stylesheet.
fn register(b: &mut FeatureBuilder, _settings: &Value) -> Result<(), ExtError> {
    let owner = b.feature()?;
    for element in [pres::PAR, pres::BLOCK, pres::BLOCK_HEADING, pres::SPAN, pres::LIST, pres::VERBATIM, pres::LINK] {
        b.peer_element(element, "A presentation element the documentation markup is lowered to.")?;
    }

    // The documentation markup is FLM with one construct borrowed from Markdown, the code
    // span, the one Markdown habit every documentation text already uses. Markdown's `*…*`
    // is not borrowed: a bare `*` in prose would open an emphasis that never closes.
    b.element(
        ElementDef::new(
            &owner,
            Vec::<String>::new(),
            SyntaxDef::Specials {
                trigger: "`".to_string(),
                args: vec![ArgDef::named_from_code(CODE_ARG, CODE_SPAN_CODE)?.docs(
                    "The code: raw text up to the next run of as many backticks as opened it; a line ending \
                     in it counts as a space, and one space is removed from each end when both ends hold one.",
                )],
            },
            element(CODE, "A code span: raw text shown as inline code.", BlockRole::DEFAULT)?,
        )
        .docs(
            "Shows raw text as inline code: a run of backticks opens it and the next run of as many \
             backticks closes it, as in ``` `\\section` ``` or ``` `` a`b `` ```.",
        )?,
    )?;

    b.element(
        ElementDef::new(
            &owner,
            ["docitem"],
            SyntaxDef::Environment {
                args: vec![
                    ArgDef::named_from_code(FACTS_ARG, "[")?.docs(
                        "The item's facts, as key–values: `anchor`, the identifier a link to the item points \
                         at, and the facts `flmdoc` writes at the end of the body.",
                    ),
                    ArgDef::named_from_code(KIND_ARG, RAW_TEXT_CODE)?
                        .docs("The kind of the item, as one word: `macro`, `environment`, `feature`, `stage`, …"),
                    ArgDef::named_from_code(NAME_ARG, RAW_TEXT_CODE)?.docs(
                        "The item's name as written, such as `\\section`, shown as code; a name holding a brace \
                         alone is written between other delimiters, as in `|\\{|`.",
                    ),
                ],
                body: BodyDef::new().blocks(),
            },
            element(ITEM, "The documentation of one item, headed by its kind and its name.", BlockRole::Block)?,
        )
        .docs(
            "Documents one item, such as a command, an environment or a feature: a line with its kind and \
             its name, then the body's paragraphs and blocks, then the facts its key–values give.",
        )?,
    )?;

    b.element(
        ElementDef::new(
            &owner,
            ["docsignature"],
            SyntaxDef::Macro {
                args: vec![ArgDef::named_from_code(SIGNATURE_ARG, RAW_TEXT_CODE)?.docs(
                    "The signature, exactly as written; one holding a brace alone is written between other \
                     delimiters, as in `|\\{|`.",
                )],
            },
            element(SIGNATURE, "How an item is written, as a block of code.", BlockRole::Block)?,
        )
        .docs("Shows how the item is written, as a block of code.")?,
    )?;

    b.element(
        ElementDef::new(
            &owner,
            ["docsummary"],
            SyntaxDef::Macro { args: vec![ArgDef::named_from_code(TEXT_ARG, "{")?.docs("The summary.")] },
            element(SUMMARY, "The summary of an item, as a paragraph.", BlockRole::Block)?,
        )
        .docs("States the item's summary, as a paragraph of its own.")?,
    )?;

    let param = ElementDef::new(
        &owner,
        ["docparam"],
        SyntaxDef::Macro {
            args: vec![
                ArgDef::named_from_code(FACTS_ARG, "[")?
                    .docs("The parameter's facts, as key–values, written after its text."),
                ArgDef::named_from_code(FORM_ARG, RAW_TEXT_CODE)?
                    .docs("How the parameter is written, such as `{title}`, `[options]` or `*`."),
                ArgDef::named_from_code(NAME_ARG, RAW_TEXT_CODE)?.docs("The parameter's name; it may be empty."),
                ArgDef::named_from_code(ROLE_ARG, RAW_TEXT_CODE)?
                    .docs("The registered name of what the parameter holds; it may be empty."),
                ArgDef::named_from_code(TEXT_ARG, "{")?
                    .layout(ArgLayout::InlineOrBlocks)
                    .docs("What the parameter holds: a line of text, or paragraphs."),
            ],
        },
        element(PARAM, "One parameter of an item, an entry of the list of parameters.", BlockRole::Block)?,
    )
    .docs("Describes one parameter of the item: how it is written, its name, its role, what it holds and its facts.")?;
    b.scoped_package(ScopedPackageDef::new(
        PARAMS_BODY,
        vec![param],
        "The command `\\docparam`, in the body of the environment `docparams`.",
    ))?;

    b.element(
        ElementDef::new(
            &owner,
            ["docparams"],
            SyntaxDef::Environment {
                args: vec![ArgDef::named_from_code(TITLE_ARG, "[")?.docs("The title shown above the list; `Arguments` by default.")],
                body: BodyDef::new().blocks().scoped(PARAMS_BODY),
            },
            element(PARAMS, "The list of the parameters of an item, under a title.", BlockRole::Block)?,
        )
        .docs("Lists the parameters of the item under a title, one `\\docparam` each.")?,
    )?;

    b.element(
        ElementDef::new(
            &owner,
            ["docfield"],
            SyntaxDef::Macro {
                args: vec![
                    ArgDef::named_from_code(KEY_ARG, "{")?.docs("What the fact is about, such as `Declared by`."),
                    ArgDef::named_from_code(VALUE_ARG, "{")?.docs("The fact."),
                ],
            },
            element(FIELD, "One labeled fact about an item, as a paragraph.", BlockRole::Block)?,
        )
        .docs("States one labeled fact about the item, such as the feature that declares it.")?,
    )?;

    b.element(
        ElementDef::new(
            &owner,
            ["docref"],
            SyntaxDef::Macro {
                args: vec![
                    ArgDef::named_from_code(PATH_ARG, RAW_TEXT_CODE)?.docs("The path of the page, such as `features/headings`."),
                    ArgDef::named_from_code(TEXT_ARG, "{")?.docs("The text of the link."),
                ],
            },
            element(REF, "A link to another documentation page.", BlockRole::DEFAULT)?,
        )
        .docs("Links to another documentation page, specified by its path.")?,
    )?;

    b.element(
        ElementDef::new(
            &owner,
            ["docdetails"],
            SyntaxDef::Environment { args: Vec::new(), body: BodyDef::new().blocks() },
            element(DETAILS, "The rest of a documentation text after its summary, as blocks.", BlockRole::Block)?,
        )
        .docs("Holds the rest of a documentation text after its summary: its paragraphs and blocks.")?,
    )?;

    let entry = ElementDef::new(
        &owner,
        ["docentry"],
        SyntaxDef::Macro {
            args: vec![
                ArgDef::named_from_code(FACTS_ARG, "[")?.docs("The entry's facts, as key–values."),
                ArgDef::named_from_code(NAME_ARG, "{")?
                    .docs("The name of the listed thing, usually a link to its page; it may be empty."),
                ArgDef::named_from_code(SUMMARY_ARG, "{")?.docs("The summary of the listed thing; it may be empty."),
            ],
        },
        element(ENTRY, "One listed thing, an entry of a list of entries.", BlockRole::Block)?,
    )
    .docs("Lists one thing: its name, its summary and its facts.")?;
    b.scoped_package(ScopedPackageDef::new(
        ENTRIES_BODY,
        vec![entry],
        "The command `\\docentry`, in the body of the environment `docentries`.",
    ))?;

    b.element(
        ElementDef::new(
            &owner,
            ["docentries"],
            SyntaxDef::Environment {
                args: vec![ArgDef::named_from_code(OPTIONS_ARG, "[")?.docs(
                    "The list's options, as key–values: `title`, the heading of the page's part the list is, \
                     `intro`, a paragraph under it, and `level`, the heading's level (1 by default).",
                )],
                body: BodyDef::new().blocks().scoped(ENTRIES_BODY),
            },
            element(ENTRIES, "A list of named things, each with its summary, under a heading.", BlockRole::Block)?,
        )
        .docs("Lists named things, one `\\docentry` each, under a heading and an introduction.")?,
    )?;

    let item = ElementDef::new(
        &owner,
        ["item"],
        SyntaxDef::Macro { args: vec![ArgDef::named_from_code(FACTS_ARG, "[")?.docs("The item's facts, as key–values.")] },
        element(INLINE_ITEM, "The beginning of one item of an inline list.", BlockRole::DEFAULT)?,
    )
    .docs("Begins one item of the inline list: the content up to the next `\\item` or the end of the list.")?;
    b.scoped_package(ScopedPackageDef::new(
        INLINE_LIST_BODY,
        vec![item],
        "The command `\\item`, in the body of the environment `docinlinelist`.",
    ))?;

    b.element(
        ElementDef::new(
            &owner,
            ["docinlinelist"],
            SyntaxDef::Environment { args: Vec::new(), body: BodyDef::new().scoped(INLINE_LIST_BODY) },
            element(INLINE_LIST, "A list shown inline, its items separated by commas.", BlockRole::DEFAULT)?,
        )
        .docs("Lists things inline, one `\\item` each, separated by commas.")?,
    )?;

    b.element(
        ElementDef::new(
            &owner,
            ["docjsontype"],
            SyntaxDef::Macro {
                args: vec![ArgDef::named_from_code(SCHEMA_ARG, RAW_TEXT_CODE)?.docs(
                    "A JSON Schema, as JSON: its keys `type`, `enum`, `const`, `items`, \
                     `additionalProperties`, `anyOf`, `oneOf` and `$ref` are read.",
                )],
            },
            element(JSON_TYPE, "The type a JSON Schema describes, in words.", BlockRole::DEFAULT)?,
        )
        .docs("Shows the type a JSON Schema describes, in words, as inline code.")?,
    )?;

    for (name, docs) in [
        (CODE, "Raw text shown as inline code, as written."),
        (ITEM, "The documentation of one item of a page: its kind, its name, its body and its facts."),
        (SIGNATURE, "How an item is written, shown as a block of code."),
        (SUMMARY, "The summary of a documentation text: its first paragraph, shown on its own."),
        (PARAMS, "The parameters of an item, listed under a title."),
        (PARAM, "One parameter of an item: its form, name, role, text and facts."),
        (FIELD, "A fact about an item, under a label that the page gives."),
        (REF, "A link to a page of the documentation, by its path."),
        (DETAILS, "The paragraphs and blocks of a documentation text after its summary."),
        (ENTRIES, "A part of a page that lists named things, under its heading."),
        (ENTRY, "One named thing of a list: its name, its summary and its facts."),
        (INLINE_LIST, "Names or values listed inline, within a sentence or the value of a fact."),
        (INLINE_ITEM, "The beginning of one item of an inline list, with the item's facts."),
        (JSON_TYPE, "The type of a value, given as a JSON Schema and written in words."),
    ] {
        b.element_type_text(name, docs)?;
    }

    let produces_par = [pres::PAR, pres::SPAN];
    rule(b, CODE, "Lowers a code span to inline code.", &[pres::VERBATIM], None, Arc::new(CodeRule))?;
    rule(
        b,
        ITEM,
        "Lowers the documentation of an item to a block headed by its kind and its name, its facts at the end.",
        &[pres::BLOCK, pres::PAR, pres::SPAN, pres::VERBATIM],
        None,
        Arc::new(ItemRule),
    )?;
    rule(
        b,
        SIGNATURE,
        "Lowers a signature to a block of code.",
        &[pres::BLOCK, pres::VERBATIM],
        None,
        Arc::new(SignatureRule),
    )?;
    rule(b, SUMMARY, "Lowers a summary to a paragraph.", &produces_par, None, Arc::new(SummaryRule))?;
    rule(
        b,
        PARAMS,
        "Lowers a list of parameters to a block holding its title and a list.",
        &[pres::BLOCK, pres::PAR, pres::SPAN, pres::LIST],
        None,
        Arc::new(ParamsRule),
    )?;
    rule(
        b,
        PARAM,
        "Lowers a parameter to the content of its entry, its facts after its text.",
        &[pres::PAR, pres::SPAN, pres::VERBATIM],
        None,
        Arc::new(ParamRule),
    )?;
    rule(b, FIELD, "Lowers a labeled fact to a paragraph.", &produces_par, None, Arc::new(FieldRule))?;
    rule(
        b,
        REF,
        "Lowers a link to another documentation page, its address written as `link_style` says.",
        &[pres::LINK],
        Some(LINKS),
        Arc::new(RefRule),
    )?;
    rule(b, DETAILS, "Lowers the rest of a documentation text to a block.", &[pres::BLOCK], None, Arc::new(DetailsRule))?;
    rule(
        b,
        ENTRIES,
        "Lowers a list of entries to a block holding its heading, its introduction and a list.",
        &[pres::BLOCK, pres::BLOCK_HEADING, pres::PAR, pres::SPAN, pres::LIST],
        None,
        Arc::new(EntriesRule),
    )?;
    rule(b, ENTRY, "Lowers an entry to the content of its item of the list.", &produces_par, None, Arc::new(EntryRule))?;
    rule(
        b,
        INLINE_LIST,
        "Lowers an inline list to its items, separated by commas, with their facts.",
        &[pres::SPAN, pres::VERBATIM],
        None,
        Arc::new(InlineListRule),
    )?;
    rule(
        b,
        INLINE_ITEM,
        "Reports an `\\item` that does not begin an item of an inline list; the list's rule writes the items.",
        &[],
        None,
        Arc::new(InlineItemRule),
    )?;
    rule(
        b,
        JSON_TYPE,
        "Lowers a JSON Schema to the words of its type, as inline code.",
        &[pres::SPAN, pres::VERBATIM],
        None,
        Arc::new(JsonTypeRule),
    )
}

/// The element `name`, with the documentation `docs` and the block role `role`.
fn element(name: &str, docs: &str, role: BlockRole) -> Result<ElementSpec, ExtError> {
    ElementSpec::new(name).docs(docs).with(Aspect::BlockRoleAspect(FixedBlockRole(role)))
}

/// Registers the lowering rule of `element`, named `<element>.lower`, the default on every
/// backend, which produces the presentation elements `produces` and reads the option type
/// `option` when one is given. `produces` lists what the rule itself builds; what the rules
/// of its arguments' content build, such as inline code inside a summary, those rules
/// declare.
fn rule(
    b: &mut FeatureBuilder,
    element: &str,
    docs: &str,
    produces: &[&str],
    option: Option<&str>,
    body: Arc<dyn Lower>,
) -> Result<(), ExtError> {
    let mut decl = LoweringDecl::new();
    for produced in produces {
        decl = decl.produces(*produced);
    }
    if let Some(option) = option {
        decl = decl.option(option);
    }
    // The rules stage presentation nodes, which every backend that writes those nodes
    // renders: each is the default on every backend, and a backend that does not write them
    // refuses the plan. Only the parentheses around a parameter's role depend on the
    // backend.
    let rule = LoweringRule::new(format!("{element}.lower"), element, decl, body, DefaultMark::Everywhere)?;
    b.lowering_rule(rule.docs(docs)?)
}

// --- reading the arguments -------------------------------------------------------------------

/// The content of an argument read as raw text: its characters, or, when it holds
/// something else, the argument lowered through the rules.
enum RawText {
    /// The characters.
    Chars(String),
    /// The nodes the argument became: the library reports content that is not characters,
    /// and shows the source text it holds.
    Lowered(Vec<Staged>),
}

/// Reads the raw-text argument `name` of `node`. Its content is characters unless a user
/// definition's template placed one of its arguments there; then the argument (its group
/// of the verbatim class) is lowered through the rules, as any raw text whose content is
/// not characters.
fn raw_text(node: &Node, name: &str, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<RawText, ExtError> {
    let content = node.argument_content_nodes_named(name)?.unwrap_or_default();
    if let Some(text) = chars_text(&content)? {
        return Ok(RawText::Chars(text));
    }
    let group = node.argument_nodes_named(name)?.unwrap_or_default();
    Ok(RawText::Lowered(lw.lower_nodes(cx, &group)?.to_vec()?))
}

/// The argument `name` of `node` lowered through the rules; nothing when it is not given.
fn lowered_argument(node: &Node, name: &str, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Staged>, ExtError> {
    if node.argument_is_provided(name)? { lw.lower_arg_named(cx, node, name)?.to_vec() } else { Ok(Vec::new()) }
}

/// The text a code span shows, by CommonMark's rule: each line ending (`\r\n`, `\n` or
/// `\r`) becomes a space; then, when the result both begins and ends with a space and is not
/// all spaces, one space is removed from each end. `" `a` "` gives `` "`a`" ``, `"a\nb"`
/// gives `"a b"`, `"\nx\n"` gives `"x"`; `"  "` and `" a"` are kept.
fn code_span_text(content: &str) -> String {
    let spaced = content.replace("\r\n", " ").replace(['\n', '\r'], " ");
    match spaced.strip_prefix(' ').and_then(|rest| rest.strip_suffix(' ')) {
        Some(inner) if !spaced.bytes().all(|byte| byte == b' ') => inner.to_string(),
        _ => spaced,
    }
}

// --- staging presentation nodes --------------------------------------------------------------

/// A text node holding `text`; nothing for an empty text.
fn text(cx: &mut LowerCx, lw: &mut Lowerer, text: &str) -> Result<Vec<Staged>, ExtError> {
    if text.is_empty() { Ok(Vec::new()) } else { Ok(vec![lw.text(cx, text)?]) }
}

/// A `flm.pres.verbatim` node of the kind `code` holding `code`, displayed `display`
/// (`inline` or `block`).
pub(crate) fn code(cx: &mut LowerCx, lw: &mut Lowerer, code: &str, display: &str) -> Result<Staged, ExtError> {
    let content = text(cx, lw, code)?;
    lw.pres(cx, pres::VERBATIM, &json!({ "kind": "code", "display": display }), content.into())
}

/// A `flm.pres.span` of the role `role`, with the style `style` when one is given.
fn span(
    cx: &mut LowerCx,
    lw: &mut Lowerer,
    role: &str,
    style: Option<&str>,
    content: Vec<Staged>,
) -> Result<Staged, ExtError> {
    let mut payload = Map::new();
    if let Some(style) = style {
        payload.insert("style".to_string(), Value::from(style));
    }
    payload.insert("role".to_string(), Value::from(role));
    lw.pres(cx, pres::SPAN, &Value::Object(payload), content.into())
}

/// A `flm.pres.par` holding `content`.
fn par(cx: &mut LowerCx, lw: &mut Lowerer, content: Vec<Staged>) -> Result<Staged, ExtError> {
    lw.pres(cx, pres::PAR, &json!({}), content.into())
}

/// A `flm.pres.block` of the role `role`, with the kind tokens `kinds` and the anchor
/// `anchor`.
fn block(
    cx: &mut LowerCx,
    lw: &mut Lowerer,
    role: &str,
    kinds: &[String],
    anchor: Option<String>,
    content: Vec<Staged>,
) -> Result<Staged, ExtError> {
    let mut payload = Map::new();
    payload.insert("role".to_string(), Value::from(role));
    if !kinds.is_empty() {
        payload.insert("kinds".to_string(), json!(kinds));
    }
    if let Some(anchor) = anchor {
        payload.insert("anchor".to_string(), Value::from(anchor));
    }
    lw.pres(cx, pres::BLOCK, &Value::Object(payload), content.into())
}

// --- the rules -------------------------------------------------------------------------------

/// The rule of a code span: inline code holding its characters, adjusted by
/// [`code_span_text`].
struct CodeRule;

impl Lower for CodeRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        Ok(Lowering::Emit(match raw_text(node, CODE_ARG, cx, lw)? {
            RawText::Chars(content) => vec![code(cx, lw, &code_span_text(&content), "inline")?].into(),
            RawText::Lowered(nodes) => nodes.into(),
        }))
    }
}

/// The rule of `docitem`: a block of the role `flmdoc.item`, of the item's kind, anchored at
/// its key `anchor`, holding a paragraph with the kind and the name, then the body, then the
/// item's facts ([`item_facts`]).
struct ItemRule;

impl Lower for ItemRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let facts = facts::read(node, FACTS_ARG, &ITEM_FACTS, lw)?;
        let anchor = facts.word("anchor").filter(|anchor| !anchor.is_empty()).map(str::to_string);
        let mut kinds = Vec::new();
        let mut head = Vec::new();
        let kind = match raw_text(node, KIND_ARG, cx, lw)? {
            RawText::Chars(kind) => {
                let kind = kind.trim().to_string();
                let content = text(cx, lw, &kind)?;
                if !kind.is_empty() {
                    kinds.push(kind);
                }
                content
            }
            RawText::Lowered(nodes) => nodes,
        };
        if !kind.is_empty() {
            head.push(span(cx, lw, &format!("{ITEM}.kind"), None, kind)?);
        }
        let name = match raw_text(node, NAME_ARG, cx, lw)? {
            RawText::Chars(name) if name.trim().is_empty() => Vec::new(),
            RawText::Chars(name) => vec![code(cx, lw, name.trim(), "inline")?],
            RawText::Lowered(nodes) => nodes,
        };
        if !name.is_empty() {
            if !head.is_empty() {
                head.extend(text(cx, lw, " ")?);
            }
            head.push(span(cx, lw, &format!("{ITEM}.name"), Some("bold"), name)?);
        }
        let mut content = Vec::new();
        if !head.is_empty() {
            content.push(par(cx, lw, head)?);
        }
        content.extend(lw.lower_body(cx, node)?.to_vec()?);
        content.extend(item_facts(&facts, true, cx, lw)?);
        Ok(Lowering::Emit(vec![block(cx, lw, ITEM, &kinds, anchor, content)?].into()))
    }
}

/// The rule of `\docsignature`: a block of the role `flmdoc.signature` holding the
/// signature as a block of code.
struct SignatureRule;

impl Lower for SignatureRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let content = match raw_text(node, SIGNATURE_ARG, cx, lw)? {
            RawText::Chars(signature) => vec![code(cx, lw, &signature, "block")?],
            RawText::Lowered(nodes) => nodes,
        };
        Ok(Lowering::Emit(vec![block(cx, lw, SIGNATURE, &[], None, content)?].into()))
    }
}

/// The rule of `\docsummary`: a paragraph holding a span of the role `flmdoc.summary`.
struct SummaryRule;

impl Lower for SummaryRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let summary = lowered_argument(node, TEXT_ARG, cx, lw)?;
        let span = span(cx, lw, SUMMARY, None, summary)?;
        Ok(Lowering::Emit(vec![par(cx, lw, vec![span])?].into()))
    }
}

/// The rule of `docparams`: a block of the role `flmdoc.params` holding a paragraph with
/// the title, then a list with one entry per block of the body (each `\docparam`, whose
/// own rule gives the entry's content).
struct ParamsRule;

impl Lower for ParamsRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let title = if node.argument_is_provided(TITLE_ARG)? {
            lw.lower_arg_named(cx, node, TITLE_ARG)?.to_vec()?
        } else {
            text(cx, lw, DEFAULT_PARAMS_TITLE)?
        };
        let title = span(cx, lw, &format!("{PARAMS}.title"), Some("bold"), title)?;
        let title = par(cx, lw, vec![title])?;
        let mut slots = Vec::new();
        for child in node.body()?.unwrap_or_default().to_vec()? {
            let entry = lw.lower_nodes(cx, &Nodes::from(vec![child]))?;
            // A node that lowers to nothing (a comment) makes no entry.
            if !entry.is_empty() {
                slots.push((pres::LABEL_SLOT.to_string(), StagedNodes::default()));
                slots.push((pres::ITEM_SLOT.to_string(), entry));
            }
        }
        let list = lw.pres_slots(cx, pres::LIST, &json!({ "kinds": [PARAMS] }), slots)?;
        Ok(Lowering::Emit(vec![block(cx, lw, PARAMS, &[], None, vec![title, list])?].into()))
    }
}

/// The rule of `\docparam`: the content of its entry in the list of `docparams`, a
/// paragraph with the form, the name and the role, then the text (in that paragraph after
/// `: ` when it is inline, after it when it holds paragraphs), then the parameter's notes in
/// parentheses and the sentence of `repeatable` (after the text, or in a paragraph of their
/// own after a text of paragraphs).
struct ParamRule;

impl Lower for ParamRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let facts = facts::read(node, FACTS_ARG, &PARAM_FACTS, lw)?;
        let form = match raw_text(node, FORM_ARG, cx, lw)? {
            RawText::Chars(form) => text(cx, lw, form.trim())?,
            RawText::Lowered(nodes) => nodes,
        };
        let mut head = vec![span(cx, lw, &format!("{PARAM}.form"), Some("monospace"), form)?];
        let name = match raw_text(node, NAME_ARG, cx, lw)? {
            RawText::Chars(name) => text(cx, lw, name.trim())?,
            RawText::Lowered(nodes) => nodes,
        };
        if !name.is_empty() {
            head.extend(text(cx, lw, " ")?);
            head.push(span(cx, lw, &format!("{PARAM}.name"), Some("bold"), name)?);
        }
        let parenthesized = lw.backend()? == TEXT_BACKEND;
        let role = match raw_text(node, ROLE_ARG, cx, lw)? {
            RawText::Chars(role) if role.trim().is_empty() => Vec::new(),
            RawText::Chars(role) if parenthesized => text(cx, lw, &format!("({})", role.trim()))?,
            RawText::Chars(role) => text(cx, lw, role.trim())?,
            RawText::Lowered(nodes) => nodes,
        };
        if !role.is_empty() {
            head.extend(text(cx, lw, " ")?);
            head.push(span(cx, lw, &format!("{PARAM}.role"), None, role)?);
        }
        let notes = words::param_notes(&facts, cx, lw)?;
        let mut sentences = Vec::new();
        if facts.has("repeatable") {
            sentences.push(("repeatable", text(cx, lw, words::REPEATABLE)?));
        }
        // The text is the argument at index 4, after the facts, the form, the name and the
        // role.
        if argument_holds_blocks(node, 4)? {
            let blocks = lowered_argument(node, TEXT_ARG, cx, lw)?;
            let mut content = vec![par(cx, lw, head)?];
            content.extend(blocks);
            let tail = tail(notes, sentences, Before::Nothing, cx, lw)?;
            if !tail.is_empty() {
                content.push(par(cx, lw, tail)?);
            }
            return Ok(Lowering::Emit(content.into()));
        }
        let inline = lowered_argument(node, TEXT_ARG, cx, lw)?;
        let before = match inline.is_empty() {
            true => Before::Nothing,
            false => Before::Text { ends_sentence: ends_with_punctuation(&argument_source(node, TEXT_ARG)?) },
        };
        if !inline.is_empty() || !notes.is_empty() || !sentences.is_empty() {
            head.extend(text(cx, lw, ": ")?);
            head.extend(inline);
        }
        head.extend(tail(notes, sentences, before, cx, lw)?);
        Ok(Lowering::Emit(vec![par(cx, lw, head)?].into()))
    }
}

/// The rule of `\docfield`: a paragraph with the key, `: ` and the value.
struct FieldRule;

impl Lower for FieldRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let key = lowered_argument(node, KEY_ARG, cx, lw)?;
        let key = span(cx, lw, &format!("{FIELD}.key"), Some("bold"), key)?;
        let mut content = vec![key];
        content.extend(text(cx, lw, ": ")?);
        let value = lowered_argument(node, VALUE_ARG, cx, lw)?;
        content.push(span(cx, lw, &format!("{FIELD}.value"), None, value)?);
        Ok(Lowering::Emit(vec![par(cx, lw, content)?].into()))
    }
}

/// How `\docref` writes the address of a page, the value of `link_style`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkStyle {
    /// The command that shows the page: `flm info PATH`.
    Command,
    /// The page's path followed by `.html`, `PATH.html`: an address counted from the root of
    /// the page tree, not relative to the page holding the link.
    Path,
}

impl LinkStyle {
    /// The address of the page at `path`.
    fn address(self, path: &str) -> String {
        match self {
            LinkStyle::Command => format!("flm info {path}"),
            LinkStyle::Path => format!("{path}.html"),
        }
    }
}

/// The rule of `\docref`: a link of the kind `flmdoc.page` holding the text, whose address
/// is written from the path as `link_style` says.
struct RefRule;

impl RefRule {
    /// The value of `link_style` for this render: the default when it is not set; for a
    /// value that is neither of the two, the default, after reporting it.
    fn link_style(lw: &mut Lowerer) -> Result<LinkStyle, ExtError> {
        let options = lw.option(LINKS)?;
        match options.get(LINK_STYLE) {
            None | Some(Value::Null) => Ok(LinkStyle::Command),
            Some(Value::String(style)) if style == LINK_STYLE_COMMAND => Ok(LinkStyle::Command),
            Some(Value::String(style)) if style == LINK_STYLE_PATH => Ok(LinkStyle::Path),
            Some(other) => {
                let given = other.as_str().map_or_else(|| other.to_string(), str::to_string);
                lw.diagnose(Diagnostic::error(
                    BAD_LINK_STYLE,
                    format!(
                        "the setting `{FEATURE}.{LINK_STYLE}` must be `{LINK_STYLE_COMMAND}` or `{LINK_STYLE_PATH}`, \
                         not `{given}`"
                    ),
                ))?;
                Ok(LinkStyle::Command)
            }
        }
    }
}

impl Lower for RefRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let path = raw_text(node, PATH_ARG, cx, lw)?;
        let mut content = lowered_argument(node, TEXT_ARG, cx, lw)?;
        match path {
            RawText::Chars(path) => {
                let href = RefRule::link_style(lw)?.address(path.trim());
                let payload = json!({ "href": href, "kind": PAGE_LINK_KIND });
                Ok(Lowering::Emit(vec![lw.pres(cx, pres::LINK, &payload, content.into())?].into()))
            }
            // A path that is not characters has no address: the text is shown, then the
            // path's source text.
            RawText::Lowered(nodes) => {
                content.extend(text(cx, lw, " ")?);
                content.extend(nodes);
                Ok(Lowering::Emit(content.into()))
            }
        }
    }
}

// --- the vocabulary of facts -----------------------------------------------------------------

/// The words a declarer may be (`declarer`).
const DECLARERS: &[&str] = &["feature", "language", "program", "package", "data"];
/// The words an origin may be (`origin`).
const ORIGINS: &[&str] = &["compiled", "extension", "bundled"];
/// The words a stage's status in a plan may be (`plan`).
const PLAN_STATUSES: &[&str] = &["scheduled", "ruled-out", "not-scheduled"];
/// Any word.
const ANY_WORD: &[&str] = &[];

/// The keys of who declared an item, read on `docitem` and `\docentry`.
const DECLARED_KEYS: [Key; 5] = [
    key("declared-by", Accepts::Content),
    key("declarer", Accepts::Word(DECLARERS)),
    key("within", Accepts::Content),
    key("switched-off", Accepts::Content),
    key("page", Accepts::Content),
];
/// The keys read only with `declared-by`.
const DECLARED_COMPANIONS: [Companion; 4] = [
    Companion { key: "declarer", with: "declared-by", values: &[] },
    Companion { key: "within", with: "declared-by", values: &[] },
    Companion { key: "switched-off", with: "declared-by", values: &[] },
    Companion { key: "page", with: "declared-by", values: &[] },
];
/// The origins that need the key `extension`.
const ORIGIN_REQUIRED: [Required; 2] = [
    Required { key: "origin", value: "extension", needs: &["extension"] },
    Required { key: "origin", value: "bundled", needs: &["extension"] },
];
/// The key `extension`, read only with an origin that names one.
const EXTENSION_COMPANION: Companion = Companion { key: "extension", with: "origin", values: &["extension", "bundled"] };

/// The keys of `docitem`.
const ITEM_KEYS: &[Key] = &[
    key("anchor", Accepts::Word(ANY_WORD)),
    DECLARED_KEYS[0],
    DECLARED_KEYS[1],
    DECLARED_KEYS[2],
    DECLARED_KEYS[3],
    DECLARED_KEYS[4],
    key("modes", Accepts::Content),
    key("places", Accepts::Content),
    key("origin", Accepts::Word(ORIGINS)),
    key("extension", Accepts::Content),
    key("plan", Accepts::Word(PLAN_STATUSES)),
    key("workflow", Accepts::Content),
    key("batch", Accepts::Number(None)),
    key("by", Accepts::Content),
    key("over", Accepts::Content),
];
/// The facts of `docitem`.
const ITEM_FACTS: Vocabulary = Vocabulary {
    construct: "docitem",
    keys: ITEM_KEYS,
    companions: &[
        DECLARED_COMPANIONS[0],
        DECLARED_COMPANIONS[1],
        DECLARED_COMPANIONS[2],
        DECLARED_COMPANIONS[3],
        EXTENSION_COMPANION,
        Companion { key: "workflow", with: "plan", values: &[] },
        Companion { key: "batch", with: "plan", values: &["scheduled"] },
        Companion { key: "by", with: "plan", values: &["ruled-out"] },
        Companion { key: "over", with: "plan", values: &["ruled-out"] },
    ],
    required: &[ORIGIN_REQUIRED[0], ORIGIN_REQUIRED[1], Required { key: "plan", value: "ruled-out", needs: &["by", "over"] }],
    exclusive: &[],
};

/// The keys of `\docentry`.
const ENTRY_KEYS: &[Key] = &[
    DECLARED_KEYS[0],
    DECLARED_KEYS[1],
    DECLARED_KEYS[2],
    DECLARED_KEYS[3],
    DECLARED_KEYS[4],
    key("modes", Accepts::Content),
    key("places", Accepts::Content),
    key("origin", Accepts::Word(ORIGINS)),
    key("extension", Accepts::Content),
    key("owner", Accepts::Content),
    key("data", Accepts::Nothing),
    key("outputs", Accepts::Content),
    key("from", Accepts::Content),
    key("absent", Accepts::Nothing),
    key("by", Accepts::Content),
    key("over", Accepts::Content),
    key("profile", Accepts::Content),
    key("rule", Accepts::Content),
    key("written", Accepts::Content),
    key("unwritten", Accepts::Nothing),
    key("aspects", Accepts::Content),
];
/// The facts of `\docentry`.
const ENTRY_FACTS: Vocabulary = Vocabulary {
    construct: "\\docentry",
    keys: ENTRY_KEYS,
    companions: &[
        DECLARED_COMPANIONS[0],
        DECLARED_COMPANIONS[1],
        DECLARED_COMPANIONS[2],
        DECLARED_COMPANIONS[3],
        EXTENSION_COMPANION,
        Companion { key: "by", with: "over", values: &[] },
        Companion { key: "over", with: "by", values: &[] },
    ],
    required: &[ORIGIN_REQUIRED[0], ORIGIN_REQUIRED[1]],
    exclusive: &[("owner", "data"), ("written", "unwritten")],
};

/// The facts of `\docparam`.
const PARAM_FACTS: Vocabulary = Vocabulary {
    construct: "\\docparam",
    keys: &[
        key("layout", Accepts::Word(ANY_WORD)),
        key("mode", Accepts::Word(ANY_WORD)),
        key("scoped", Accepts::Content),
        key("kept", Accepts::Nothing),
        key("field", Accepts::Content),
        key("default", Accepts::Content),
        key("reader", Accepts::Content),
        key("repeatable", Accepts::Nothing),
    ],
    companions: &[],
    required: &[],
    exclusive: &[],
};

/// The facts of `\item` in the body of `docinlinelist`.
const INLINE_ITEM_FACTS: Vocabulary = Vocabulary {
    construct: "\\item",
    keys: &[
        key("package", Accepts::Nothing),
        key("writer", Accepts::Content),
        key("optional", Accepts::Nothing),
        key("missing", Accepts::Nothing),
        key("body", Accepts::Nothing),
        key("argument", Accepts::Word(ANY_WORD)),
        key("field", Accepts::Content),
    ],
    companions: &[],
    required: &[],
    exclusive: &[("body", "argument")],
};

/// The highest level of the heading of `docentries` (`level`).
const LEVEL_MAX: u32 = 255;

/// The options of `docentries`.
const ENTRIES_OPTIONS: Vocabulary = Vocabulary {
    construct: "docentries",
    keys: &[key("title", Accepts::Content), key("intro", Accepts::Content), key("level", Accepts::Number(Some(LEVEL_MAX)))],
    companions: &[],
    required: &[],
    exclusive: &[],
};

// --- writing facts ---------------------------------------------------------------------------

/// The facts of an item written as labeled facts, in order, each a paragraph like that of
/// `\docfield`, its words in a span of the role `flmdoc.fact.<fact>`: “Declared by”
/// ([`words::declared_by`]), “Switched off” ([`words::switched_off`]), “Available in”
/// ([`words::available`]), and, on `docitem` (`item`), “Origin” ([`words::origin`]) and the
/// stage's status in a plan ([`words::plan_label`], [`words::plan`]).
fn item_facts(facts: &Facts, item: bool, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Staged>, ExtError> {
    let mut paragraphs = Vec::new();
    let declared = words::declared_by(facts, cx, lw)?;
    let switched = words::switched_off(facts, cx, lw)?;
    let available = words::available(facts, cx, lw)?;
    let mut labeled = vec![
        ("Declared by", "declared-by", declared),
        ("Switched off", "switched-off", switched),
        ("Available in", "available-in", available),
    ];
    if item {
        labeled.push(("Origin", "origin", words::origin(facts, cx, lw)?));
    }
    for (label, fact, value) in labeled {
        if !value.is_empty() {
            let label = text(cx, lw, label)?;
            paragraphs.push(fact_paragraph(label, fact, value, cx, lw)?);
        }
    }
    if item {
        let status = words::plan(facts, cx, lw)?;
        if !status.is_empty() {
            let label = words::plan_label(facts, cx, lw)?;
            paragraphs.push(fact_paragraph(label, "plan", status, cx, lw)?);
        }
    }
    Ok(paragraphs)
}

/// A labeled fact: a paragraph with `label` (a bold span of the role `flmdoc.field.key`),
/// `: `, and `value` (a span of the role `flmdoc.fact.<fact>`).
fn fact_paragraph(label: Vec<Staged>, fact: &str, value: Vec<Staged>, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Staged, ExtError> {
    let mut content = vec![span(cx, lw, &format!("{FIELD}.key"), Some("bold"), label)?];
    content.extend(text(cx, lw, ": ")?);
    content.push(span(cx, lw, &format!("flmdoc.fact.{fact}"), None, value)?);
    par(cx, lw, content)
}

/// What precedes the notes and the sentences of a construct ([`tail`]).
#[derive(Debug, Clone, Copy)]
enum Before {
    /// Nothing: they begin the paragraph or the content.
    Nothing,
    /// Text, which ends a sentence (with `.`, `!` or `?`) or not.
    Text { ends_sentence: bool },
}

/// The notes of a construct in parentheses, separated by `; `, then its sentences, each
/// note's and each sentence's words in a span of the role `flmdoc.fact.<key>`: a space
/// before the parentheses after text; before a sentence, a space after text that ends a
/// sentence, `. ` after other text, nothing at the beginning.
fn tail(
    notes: Vec<words::Note>,
    sentences: Vec<words::Note>,
    before: Before,
    cx: &mut LowerCx,
    lw: &mut Lowerer,
) -> Result<Vec<Staged>, ExtError> {
    let mut staged = Vec::new();
    let mut before = before;
    if !notes.is_empty() {
        if let Before::Text { .. } = before {
            staged.extend(text(cx, lw, " ")?);
        }
        staged.extend(text(cx, lw, "(")?);
        for (index, (key, note)) in notes.into_iter().enumerate() {
            if index > 0 {
                staged.extend(text(cx, lw, "; ")?);
            }
            staged.push(span(cx, lw, &format!("flmdoc.fact.{key}"), None, note)?);
        }
        staged.extend(text(cx, lw, ")")?);
        before = Before::Text { ends_sentence: false };
    }
    for (key, sentence) in sentences {
        match before {
            Before::Nothing => {}
            Before::Text { ends_sentence: true } => staged.extend(text(cx, lw, " ")?),
            Before::Text { ends_sentence: false } => staged.extend(text(cx, lw, ". ")?),
        }
        staged.push(span(cx, lw, &format!("flmdoc.fact.{key}"), None, sentence)?);
        before = Before::Text { ends_sentence: true };
    }
    Ok(staged)
}

/// Whether `text` ends a sentence: its last character, braces and spaces left aside, is
/// `.`, `!` or `?`.
fn ends_with_punctuation(text: &str) -> bool {
    text.trim_end_matches(|c: char| c == '}' || c.is_whitespace()).ends_with(['.', '!', '?'])
}

/// The source text of the content of the argument `name` of `node`; empty when it is not
/// given.
fn argument_source(node: &Node, name: &str) -> Result<String, ExtError> {
    let mut source = String::new();
    for node in node.argument_content_nodes_named(name)?.unwrap_or_default().to_vec()? {
        source.push_str(&node.span_content()?);
    }
    Ok(source)
}

// --- the rules of the lists ------------------------------------------------------------------

/// The rule of `docdetails`: a block of the role `flmdoc.details` holding the body.
struct DetailsRule;

impl Lower for DetailsRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let body = lw.lower_body(cx, node)?;
        Ok(Lowering::Emit(vec![block(cx, lw, DETAILS, &[], None, body.to_vec()?)?].into()))
    }
}

/// The rule of `docentries`: a block of the role `flmdoc.entries` holding the title as a
/// heading of the level `level` (a `flm.pres.block-heading` named `flmdoc.entries`, not in a
/// table of contents), the introduction as a paragraph (a span of the role
/// `flmdoc.entries.intro`), then a list of the kind `flmdoc.entries` with one entry per
/// block of the body, each marked `•`.
struct EntriesRule;

impl Lower for EntriesRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let options = facts::read(node, OPTIONS_ARG, &ENTRIES_OPTIONS, lw)?;
        let mut content = Vec::new();
        if let Some(title) = options.content("title") {
            let title = words::lower_pieces(cx, lw, title)?;
            // A level that was read is a whole number from 0 to `LEVEL_MAX`.
            let level = options.word("level").and_then(|level| level.parse::<u8>().ok()).unwrap_or(1);
            let payload = json!({ "name": ENTRIES, "level": level, "in_toc": false });
            content.push(lw.pres(cx, pres::BLOCK_HEADING, &payload, title.into())?);
        }
        if let Some(intro) = options.content("intro") {
            let intro = words::lower_pieces(cx, lw, intro)?;
            let intro = span(cx, lw, &format!("{ENTRIES}.intro"), None, intro)?;
            content.push(par(cx, lw, vec![intro])?);
        }
        let mut slots = Vec::new();
        for child in node.body()?.unwrap_or_default().to_vec()? {
            let entry = lw.lower_nodes(cx, &Nodes::from(vec![child]))?;
            // A node that lowers to nothing (a comment) makes no entry.
            if !entry.is_empty() {
                slots.push((pres::LABEL_SLOT.to_string(), text(cx, lw, ENTRY_MARKER)?.into()));
                slots.push((pres::ITEM_SLOT.to_string(), entry));
            }
        }
        content.push(lw.pres_slots(cx, pres::LIST, &json!({ "kinds": [ENTRIES] }), slots)?);
        Ok(Lowering::Emit(vec![block(cx, lw, ENTRIES, &[], None, content)?].into()))
    }
}

/// The rule of `\docentry`: the content of its entry in the list of `docentries`, a
/// paragraph with the name (a span of the role `flmdoc.entry.name`), the entry's notes in
/// parentheses ([`words::entry_notes`]) and its clauses ([`words::entry_clauses`]), then,
/// after `: `, the rule (`rule`), the summary (a span of the role `flmdoc.entry.summary`)
/// and the entry's sentences ([`words::entry_sentences`]); then the entry's labeled facts
/// ([`item_facts`]).
struct EntryRule;

impl Lower for EntryRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let facts = facts::read(node, FACTS_ARG, &ENTRY_FACTS, lw)?;
        let mut head = Vec::new();
        let name = lowered_argument(node, NAME_ARG, cx, lw)?;
        if !name.is_empty() {
            head.push(span(cx, lw, &format!("{ENTRY}.name"), None, name)?);
        }
        let notes = words::entry_notes(&facts, cx, lw)?;
        let before = if head.is_empty() { Before::Nothing } else { Before::Text { ends_sentence: false } };
        head.extend(tail(notes, Vec::new(), before, cx, lw)?);
        for (key, separator, clause) in words::entry_clauses(&facts, cx, lw)? {
            head.extend(text(cx, lw, separator)?);
            head.push(span(cx, lw, &format!("flmdoc.fact.{key}"), None, clause)?);
        }
        let mut rest = Vec::new();
        let mut before = Before::Nothing;
        if let Some(rule) = facts.content("rule") {
            let rule = words::lower_pieces(cx, lw, rule)?;
            rest.push(span(cx, lw, "flmdoc.fact.rule", None, rule)?);
            before = Before::Text { ends_sentence: false };
        }
        let summary = lowered_argument(node, SUMMARY_ARG, cx, lw)?;
        if !summary.is_empty() {
            if let Before::Text { .. } = before {
                rest.extend(text(cx, lw, "; ")?);
            }
            rest.push(span(cx, lw, &format!("{ENTRY}.summary"), None, summary)?);
            before = Before::Text { ends_sentence: ends_with_punctuation(&argument_source(node, SUMMARY_ARG)?) };
        }
        let sentences = words::entry_sentences(&facts, cx, lw)?;
        rest.extend(tail(Vec::new(), sentences, before, cx, lw)?);
        if !head.is_empty() && !rest.is_empty() {
            head.extend(text(cx, lw, ": ")?);
        }
        head.extend(rest);
        let mut content = Vec::new();
        if !head.is_empty() {
            content.push(par(cx, lw, head)?);
        }
        content.extend(item_facts(&facts, false, cx, lw)?);
        Ok(Lowering::Emit(content.into()))
    }
}

/// The rule of `docinlinelist`: its items, separated by `, `, in a span of the role
/// `flmdoc.inline-list`; each item a span of the role `flmdoc.inline-item` holding the part
/// of a command or the package it names ([`words::place`]), its content without the whitespace at its
/// edges ([`stage_trimmed`]), and its notes in parentheses ([`words::item_notes`]). Content
/// before the first `\item`, other than comments and whitespace, is reported, and shown as
/// an item of its own.
struct InlineListRule;

impl Lower for InlineListRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        // The items: each `\item` with the run of nodes after it, up to the next.
        let mut items: Vec<(Option<Node>, Vec<Node>)> = vec![(None, Vec::new())];
        for child in node.body()?.unwrap_or_default().to_vec()? {
            if child.is_callable()? && child.element_name()? == INLINE_ITEM {
                items.push((Some(child), Vec::new()));
            } else if let Some((_, run)) = items.last_mut() {
                run.push(child);
            }
        }
        let mut staged = Vec::new();
        for (item, run) in items {
            let facts = match &item {
                Some(item) => facts::read(item, FACTS_ARG, &INLINE_ITEM_FACTS, lw)?,
                None if is_blank_run(&run)? => continue,
                None => {
                    let written: String = run.iter().map(|node| node.span_content().unwrap_or_default()).collect();
                    let message = format!("Missing ‘\\item’ before ‘{}’ in ‘docinlinelist’", written.trim());
                    facts::report(lw, node, BAD_INLINE_LIST, message, json!({ "content": written.trim() }))?;
                    Facts::default()
                }
            };
            if !staged.is_empty() {
                staged.extend(text(cx, lw, INLINE_SEPARATOR)?);
            }
            let mut parts = words::place(&facts, cx, lw)?;
            parts.extend(stage_trimmed(cx, lw, &Nodes::from(run))?.to_vec()?);
            let notes = words::item_notes(&facts, cx, lw)?;
            let before = if parts.is_empty() { Before::Nothing } else { Before::Text { ends_sentence: false } };
            parts.extend(tail(notes, Vec::new(), before, cx, lw)?);
            staged.push(span(cx, lw, INLINE_ITEM, None, parts)?);
        }
        Ok(Lowering::Emit(vec![span(cx, lw, INLINE_LIST, None, staged)?].into()))
    }
}

/// Whether every node of `run` is a comment or blank ([`is_blank`]).
fn is_blank_run(run: &[Node]) -> Result<bool, ExtError> {
    for node in run {
        if !(node.is_comment()? || is_blank(node)?) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The rule of `\item` in the body of `docinlinelist`, which the list's rule never lowers: an
/// `\item` the lowering reaches on its own does not stand directly in the list's body. It is
/// reported, and writes nothing.
struct InlineItemRule;

impl Lower for InlineItemRule {
    fn lower(&self, node: &Node, _cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let message = String::from("‘\\item’ must stand directly in the body of ‘docinlinelist’");
        facts::report(lw, node, BAD_INLINE_LIST, message, json!({}))?;
        Ok(Lowering::Emit(Vec::new().into()))
    }
}

/// The rule of `\docjsontype`: the words of the type the schema describes
/// ([`schema::type_text`]), as inline code in a span of the role `flmdoc.json-type`. Text
/// that is not JSON is reported, and shown as inline code as written.
struct JsonTypeRule;

impl Lower for JsonTypeRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let content = match raw_text(node, SCHEMA_ARG, cx, lw)? {
            RawText::Chars(written) => match serde_json::from_str::<Value>(&written) {
                Ok(schema) => vec![code(cx, lw, &schema::type_text(&schema), "inline")?],
                Err(error) => {
                    let message = format!("Invalid JSON in ‘\\docjsontype’: {error}");
                    facts::report(lw, node, BAD_JSON, message, json!({ "schema": written }))?;
                    vec![code(cx, lw, written.trim(), "inline")?]
                }
            },
            RawText::Lowered(nodes) => nodes,
        };
        Ok(Lowering::Emit(vec![span(cx, lw, JSON_TYPE, None, content)?].into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_span_loses_one_space_at_each_end_when_both_ends_hold_one() {
        assert_eq!(code_span_text(" `a` "), "`a`");
        assert_eq!(code_span_text("  a  "), " a ");
        assert_eq!(code_span_text(" a "), "a");
    }

    #[test]
    fn a_code_span_keeps_its_spaces_when_one_end_holds_none_or_it_is_all_spaces() {
        assert_eq!(code_span_text(" a"), " a");
        assert_eq!(code_span_text("a "), "a ");
        assert_eq!(code_span_text("a"), "a");
        assert_eq!(code_span_text(" "), " ");
        assert_eq!(code_span_text("   "), "   ");
        assert_eq!(code_span_text(""), "");
    }

    #[test]
    fn a_code_span_counts_only_spaces_not_other_whitespace() {
        assert_eq!(code_span_text("\ta\t"), "\ta\t");
        assert_eq!(code_span_text(" \t "), "\t");
    }

    #[test]
    fn a_line_ending_in_a_code_span_becomes_a_space() {
        assert_eq!(code_span_text("over\ntwo lines"), "over two lines");
        assert_eq!(code_span_text("a\r\nb"), "a b");
        assert_eq!(code_span_text("a\rb"), "a b");
        assert_eq!(code_span_text("a \n b"), "a   b");
    }

    #[test]
    fn line_endings_are_spaces_before_one_space_is_removed_from_each_end() {
        assert_eq!(code_span_text("\nx\n"), "x");
        assert_eq!(code_span_text("\r\nx\r\n"), "x");
        assert_eq!(code_span_text("\n x\n"), " x");
        assert_eq!(code_span_text("\n"), " ");
        assert_eq!(code_span_text("\n\n"), "  ");
    }

    #[test]
    fn the_address_of_a_page_follows_the_link_style() {
        assert_eq!(LinkStyle::Command.address("features/headings"), "flm info features/headings");
        assert_eq!(LinkStyle::Path.address("features/headings"), "features/headings.html");
    }
}
