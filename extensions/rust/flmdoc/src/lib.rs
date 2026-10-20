//! The extension `flmdoc`: the markup of documentation pages, such as the pages the command
//! `flm info` shows. It gives a document a code span written between backticks, the
//! constructs that lay out the documentation of one item (a command, an environment, a
//! feature, …), and a link to another documentation page, each lowered to the presentation
//! elements that the `text` and `html` backends write.
//!
//! The extension is written against the guest client `flm_ext::guest`. The `flm` program
//! carries it compiled in (the engine `native`); `extensions/rust/build-wasm.sh` builds the
//! same crate as a WebAssembly module (the engine `wasm`). A document uses it by listing
//! `flmdoc` in its `use` list.
//!
//! # What an author writes
//!
//! The extension's package `_` (the package a `use` entry `flmdoc` gives) uses `stdcore`
//! and holds the feature `flmdoc`, which declares these commands, each available in text
//! (not in mathematics):
//!
//! | written | element | what it is |
//! |---|---|---|
//! | `` `code` ``, ``` ``a`b`` ``` | `flmdoc.code` | a code span: raw text shown as inline code |
//! | `\begin{docitem}[anchor]{kind}{name} … \end{docitem}` | `flmdoc.item` | the documentation of one item |
//! | `\docsignature{…}` | `flmdoc.signature` | how the item is written, as a block of code |
//! | `\docsummary{…}` | `flmdoc.summary` | the item's summary, as a paragraph |
//! | `\begin{docparams}[title] … \end{docparams}` | `flmdoc.params` | the list of the item's parameters |
//! | `\docparam{form}{name}{role}{text}` | `flmdoc.param` | one parameter, in the body of `docparams` |
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
//! and `role` of `\docparam`, and the path of `\docref` are read as raw text between a pair
//! of delimiters (FLM's argument code `v`): usually braces, `{…}`, inside which braces pair
//! up; another delimiter, such as `|…|`, lets the text hold a brace alone, as in
//! `\docsignature|\{|`. The others hold document content. The body of `docparams`
//! holds `\docparam` commands: the command is declared in the scoped package
//! `flmdoc.params.body`, which the environment's body makes available, and nowhere else.
//!
//! # What each command becomes
//!
//! Each element has one lowering rule, its default on every backend, which stages
//! presentation nodes (paragraphs, blocks, spans, a list, code, a link) whose roles are
//! `flmdoc.<construct>` and `flmdoc.<construct>.<part>`. The `html` backend writes a role as
//! a class and a kind as the attribute `data-flm-kind`; the `text` backend prints the
//! content. The rules:
//!
//! - a code span: a `flm.pres.verbatim` node of the kind `code`, inline, holding the
//!   characters, as `\verbcode` gives;
//! - `docitem`: a `flm.pres.block` of the role `flmdoc.item` whose kind is the item's kind
//!   and whose anchor is the optional argument, holding a paragraph with the kind word (a
//!   span of the role `flmdoc.item.kind`) and the name as inline code (a bold span of the
//!   role `flmdoc.item.name`), then the body;
//! - `\docsignature`: a block of the role `flmdoc.signature` holding the signature as a
//!   block of code;
//! - `\docsummary`: a paragraph holding a span of the role `flmdoc.summary`;
//! - `docparams`: a block of the role `flmdoc.params` holding a paragraph with the title (a
//!   bold span of the role `flmdoc.params.title`, `Arguments` when no title is given), then
//!   a `flm.pres.list` of the kind `flmdoc.params` with one entry per block of the body
//!   (the `text` backend prints each entry as a paragraph of its own, with no marker);
//! - `\docparam`: the entry's content: a paragraph with the form (a monospace span of the
//!   role `flmdoc.param.form`), the name when given (a bold span of the role
//!   `flmdoc.param.name`), the role when given (a span of the role `flmdoc.param.role`, in
//!   parentheses on the `text` backend), then `: ` and the text; text that holds paragraphs
//!   follows that first paragraph instead;
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

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::api::model::aspects::FixedBlockRole;
use flm_ext::guest::core::lowering::{
    DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule, Staged,
};
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::lang::mint::{ArgDef, ArgLayout, BodyDef, ElementDef, ScopedPackageDef, SyntaxDef};
use flm_ext::guest::lang::rewrites::blocks::argument_holds_blocks;
use flm_ext::guest::lang::text::chars_text;
use flm_ext::guest::model::aspects::BlockRole;
use flm_ext::guest::model::element::{Aspect, ElementSpec};
use flm_ext::guest::stdcore::backends::pres;
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::techy::node::Node;
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::{Map, Value, json};

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

/// The scoped package holding `\docparam`, which the body of `docparams` makes available.
const PARAMS_BODY: &str = "flmdoc.params.body";

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

/// The kind of the link `\docref` stages.
const PAGE_LINK_KIND: &str = "flmdoc.page";

/// The title of `docparams` when its optional argument is not given.
const DEFAULT_PARAMS_TITLE: &str = "Arguments";

/// The backend on which the role of a parameter is printed in parentheses.
const TEXT_BACKEND: &str = "text";

// The arguments, by name.
/// The content of a code span.
const CODE_ARG: &str = "code";
/// The anchor of `docitem`.
const ANCHOR_ARG: &str = "anchor";
/// The kind of `docitem`.
const KIND_ARG: &str = "kind";
/// The name of `docitem`, and the name of `\docparam`.
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

// The static `FLM_EXTENSION`, through which a program carrying the crate compiled in reaches
// it, and the module's exports in a build of the WebAssembly form.
flm_ext::guest::extension!(extension);

fn extension(ext: &mut Extension) {
    ext.package(
        "_",
        Package::new().uses(["stdcore"]).feature(
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

// The constructs, their arguments and their presentation: see decision rationale at
// [§flm-dd-dr:flmdoc-constructs].
fn register(b: &mut FeatureBuilder, _settings: &Value) -> Result<(), ExtError> {
    let owner = b.feature()?;
    for element in [pres::PAR, pres::BLOCK, pres::SPAN, pres::LIST, pres::VERBATIM, pres::LINK] {
        b.peer_element(element, "A presentation element the documentation markup is lowered to.")?;
    }

    // The documentation markup is FLM with one construct borrowed from Markdown, the code
    // span: see decision rationale at [§flm-dd-dr:docs-markup].
    b.element(
        ElementDef::new(
            &owner,
            Vec::<String>::new(),
            SyntaxDef::Specials {
                trigger: "`".to_string(),
                args: vec![ArgDef::named(CODE_ARG, CODE_SPAN_CODE).docs(
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
                    ArgDef::named(ANCHOR_ARG, "[").docs("The identifier a link to the item points at."),
                    ArgDef::named(KIND_ARG, RAW_TEXT_CODE)
                        .docs("The kind of the item, as one word: `macro`, `environment`, `feature`, `stage`, …"),
                    ArgDef::named(NAME_ARG, RAW_TEXT_CODE).docs(
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
             its name, then the body's paragraphs and blocks.",
        )?,
    )?;

    b.element(
        ElementDef::new(
            &owner,
            ["docsignature"],
            SyntaxDef::Macro {
                args: vec![ArgDef::named(SIGNATURE_ARG, RAW_TEXT_CODE).docs(
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
            SyntaxDef::Macro { args: vec![ArgDef::named(TEXT_ARG, "{").docs("The summary.")] },
            element(SUMMARY, "The summary of an item, as a paragraph.", BlockRole::Block)?,
        )
        .docs("States the item's summary, as a paragraph of its own.")?,
    )?;

    let param = ElementDef::new(
        &owner,
        ["docparam"],
        SyntaxDef::Macro {
            args: vec![
                ArgDef::named(FORM_ARG, RAW_TEXT_CODE)
                    .docs("How the parameter is written, such as `{title}`, `[options]` or `*`."),
                ArgDef::named(NAME_ARG, RAW_TEXT_CODE).docs("The parameter's name; it may be empty."),
                ArgDef::named(ROLE_ARG, RAW_TEXT_CODE)
                    .docs("The registered name of what the parameter holds; it may be empty."),
                ArgDef::named(TEXT_ARG, "{")
                    .layout(ArgLayout::InlineOrBlocks)
                    .docs("What the parameter holds: a line of text, or paragraphs."),
            ],
        },
        element(PARAM, "One parameter of an item, an entry of the list of parameters.", BlockRole::Block)?,
    )
    .docs("Describes one parameter of the item: how it is written, its name, its role and what it holds.")?;
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
                args: vec![ArgDef::named(TITLE_ARG, "[").docs("The title shown above the list; `Arguments` by default.")],
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
                    ArgDef::named(KEY_ARG, "{").docs("What the fact is about, such as `Declared by`."),
                    ArgDef::named(VALUE_ARG, "{").docs("The fact."),
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
                    ArgDef::named(PATH_ARG, RAW_TEXT_CODE).docs("The path of the page, such as `features/headings`."),
                    ArgDef::named(TEXT_ARG, "{").docs("The text of the link."),
                ],
            },
            element(REF, "A link to another documentation page.", BlockRole::DEFAULT)?,
        )
        .docs("Links to another documentation page, specified by its path.")?,
    )?;

    let produces_par = [pres::PAR, pres::SPAN];
    rule(b, CODE, "Lowers a code span to inline code.", &[pres::VERBATIM], None, Arc::new(CodeRule))?;
    rule(
        b,
        ITEM,
        "Lowers the documentation of an item to a block headed by its kind and its name.",
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
    rule(b, PARAM, "Lowers a parameter to the content of its entry.", &produces_par, None, Arc::new(ParamRule))?;
    rule(b, FIELD, "Lowers a labeled fact to a paragraph.", &produces_par, None, Arc::new(FieldRule))?;
    rule(
        b,
        REF,
        "Lowers a link to another documentation page, its address written as `link_style` says.",
        &[pres::LINK],
        Some(LINKS),
        Arc::new(RefRule),
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
    Ok(RawText::Lowered(lw.lower_nodes(cx, &group)?))
}

/// The argument `name` of `node` lowered through the rules; nothing when it is not given.
fn lowered_argument(node: &Node, name: &str, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Staged>, ExtError> {
    if node.argument_is_provided(name)? { lw.lower_arg_named(cx, node, name) } else { Ok(Vec::new()) }
}

/// The text of the argument `name` of `node`, trimmed, or `None` when it is not given or
/// blank: its characters, or the source text of its content when it holds markup.
fn plain_argument(node: &Node, name: &str) -> Result<Option<String>, ExtError> {
    let Some(content) = node.argument_content_nodes_named(name)? else { return Ok(None) };
    let text = match chars_text(&content)? {
        Some(text) => text,
        None => {
            let mut source = String::new();
            for node in &content {
                source.push_str(&node.span_content()?);
            }
            source
        }
    };
    let text = text.trim();
    Ok((!text.is_empty()).then(|| text.to_string()))
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
fn code(cx: &mut LowerCx, lw: &mut Lowerer, code: &str, display: &str) -> Result<Staged, ExtError> {
    let content = text(cx, lw, code)?;
    lw.pres(cx, pres::VERBATIM, &json!({ "kind": "code", "display": display }), content)
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
    lw.pres(cx, pres::SPAN, &Value::Object(payload), content)
}

/// A `flm.pres.par` holding `content`.
fn par(cx: &mut LowerCx, lw: &mut Lowerer, content: Vec<Staged>) -> Result<Staged, ExtError> {
    lw.pres(cx, pres::PAR, &json!({}), content)
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
    lw.pres(cx, pres::BLOCK, &Value::Object(payload), content)
}

// --- the rules -------------------------------------------------------------------------------

/// The rule of a code span: inline code holding its characters, adjusted by
/// [`code_span_text`].
struct CodeRule;

impl Lower for CodeRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        Ok(Lowering::Emit(match raw_text(node, CODE_ARG, cx, lw)? {
            RawText::Chars(content) => vec![code(cx, lw, &code_span_text(&content), "inline")?],
            RawText::Lowered(nodes) => nodes,
        }))
    }
}

/// The rule of `docitem`: a block of the role `flmdoc.item`, of the item's kind, holding a
/// paragraph with the kind and the name, then the body.
struct ItemRule;

impl Lower for ItemRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let anchor = plain_argument(node, ANCHOR_ARG)?;
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
        content.extend(lw.lower_body(cx, node)?);
        Ok(Lowering::Emit(vec![block(cx, lw, ITEM, &kinds, anchor, content)?]))
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
        Ok(Lowering::Emit(vec![block(cx, lw, SIGNATURE, &[], None, content)?]))
    }
}

/// The rule of `\docsummary`: a paragraph holding a span of the role `flmdoc.summary`.
struct SummaryRule;

impl Lower for SummaryRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let summary = lowered_argument(node, TEXT_ARG, cx, lw)?;
        let span = span(cx, lw, SUMMARY, None, summary)?;
        Ok(Lowering::Emit(vec![par(cx, lw, vec![span])?]))
    }
}

/// The rule of `docparams`: a block of the role `flmdoc.params` holding a paragraph with
/// the title, then a list with one entry per block of the body (each `\docparam`, whose
/// own rule gives the entry's content).
struct ParamsRule;

impl Lower for ParamsRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let title = if node.argument_is_provided(TITLE_ARG)? {
            lw.lower_arg_named(cx, node, TITLE_ARG)?
        } else {
            text(cx, lw, DEFAULT_PARAMS_TITLE)?
        };
        let title = span(cx, lw, &format!("{PARAMS}.title"), Some("bold"), title)?;
        let title = par(cx, lw, vec![title])?;
        let mut slots = Vec::new();
        for child in node.body()?.unwrap_or_default() {
            let entry = lw.lower_nodes(cx, std::slice::from_ref(&child))?;
            // A node that lowers to nothing (a comment) makes no entry.
            if !entry.is_empty() {
                slots.push((pres::LABEL_SLOT.to_string(), Vec::new()));
                slots.push((pres::ITEM_SLOT.to_string(), entry));
            }
        }
        let list = lw.pres_slots(cx, pres::LIST, &json!({ "kinds": [PARAMS] }), slots)?;
        Ok(Lowering::Emit(vec![block(cx, lw, PARAMS, &[], None, vec![title, list])?]))
    }
}

/// The rule of `\docparam`: the content of its entry in the list of `docparams`, a
/// paragraph with the form, the name and the role, then the text (in that paragraph after
/// `: ` when it is inline, after it when it holds paragraphs).
struct ParamRule;

impl Lower for ParamRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
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
        // The text is the argument at index 3, after the form, the name and the role.
        if argument_holds_blocks(node, 3)? {
            let blocks = lowered_argument(node, TEXT_ARG, cx, lw)?;
            let mut content = vec![par(cx, lw, head)?];
            content.extend(blocks);
            return Ok(Lowering::Emit(content));
        }
        let inline = lowered_argument(node, TEXT_ARG, cx, lw)?;
        if !inline.is_empty() {
            head.extend(text(cx, lw, ": ")?);
            head.extend(inline);
        }
        Ok(Lowering::Emit(vec![par(cx, lw, head)?]))
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
        Ok(Lowering::Emit(vec![par(cx, lw, content)?]))
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
                Ok(Lowering::Emit(vec![lw.pres(cx, pres::LINK, &payload, content)?]))
            }
            // A path that is not characters has no address: the text is shown, then the
            // path's source text.
            RawText::Lowered(nodes) => {
                content.extend(text(cx, lw, " ")?);
                content.extend(nodes);
                Ok(Lowering::Emit(content))
            }
        }
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
