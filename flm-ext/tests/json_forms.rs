//! The round trips of the JSON forms (`adapter/values.rs`): for every type row
//! of the mapping table whose crossing form is pure data, a sample of each
//! variant is written (`to_value`), read back (`from_value`) and written again,
//! and the two values written are equal; the scalars and the containers
//! (`Option`, `Vec`, pairs) likewise. A sample that is easier to state as JSON
//! is first read from it ([`sample`]); the round trip then starts from the
//! value read.

use std::fmt::Debug;

use serde_json::{json, Value};

use flm_core::lang::mint::{ArgLayout, BodyDef, BodyLayout, CallableKind, FallbackKind};
use flm_core::lang::rewrites::userdefs::{DefinitionKind, DefinitionModes, ExpandPolicy};
use flm_core::model::aspects::{BlockRole, InlineSpacing, Placement};
use flm_ext::adapter::values::JsonForm;
use flm_core::core::artifact::RelPath;
use flm_ext::api::core::artifact::{Artifact, ArtifactData, ArtifactId};
use flm_ext::api::core::lowering::{DefaultMark, LoweringDecl};
use flm_ext::api::core::registry::{DefContent, OutputDef, ScriptDef, StandardTable, StyleDef};
use flm_ext::api::core::services::Arity;
use flm_ext::api::core::stage::{Effect, Needs, RewriteDecl, Scope, StageDecl};
use flm_ext::api::host::FilesScope;
use flm_ext::api::lang::mint::Mode;
use flm_ext::api::lang::rewrites::userdefs::{PlaceholderVocabulary, TemplateSpec};
use flm_ext::api::lang::text::CharsContent;
use flm_ext::api::model::aspects::{AcceptedRefTypes, Label, MalformedLabel};
use flm_ext::api::stdcore::backends::latex::{
    LatexArgShape, LatexDefinitionKind, LatexDefinitionPayload, LatexEnvironmentPayload, LatexMacroPayload, LatexPackageDef,
    LatexPreambleDef, LatexRawPayload,
};
use flm_ext::api::stdcore::features::citations::{CitationProviderError, EntryPiece};
use flm_ext::api::stdcore::features::compose::{Accepts, PieceValue, TemplateContext, TemplateDescription, TemplateFile};
use flm_ext::api::stdcore::features::numbering::{
    CounterDecl, CounterStyleDecl, Delimiters, FormatOptions, NumberTemplate, ParentSetDecl, Parents, PluralWords,
    PrefixWords, Within, WithinJoinerSpec,
};
use flm_ext::api::stdcore::features::refs::LabelUse;
use flm_ext::api::techy::error::Severity;
use flm_ext::api::techy::node::{NodeKindName, Span};

/// `value` written, read back and written again: the two values written are
/// equal.
fn round_trip<T: JsonForm>(value: &T) {
    let written = value.to_value();
    let read = T::from_value(&written).unwrap_or_else(|error| panic!("`{written}` does not read back: {error}"));
    assert_eq!(read.to_value(), written, "the value read back from `{written}` is written otherwise");
}

/// The value `T` reads from `value`.
fn sample<T: JsonForm>(value: Value) -> T {
    T::from_value(&value).unwrap_or_else(|error| panic!("`{value}` does not read: {error}"))
}

/// The round trip of each sample, each read from its JSON first.
fn round_trips<T: JsonForm>(values: &[Value]) {
    for value in values {
        round_trip(&sample::<T>(value.clone()));
    }
}

/// A unit enum: each of its tokens reads, round-trips and is written as
/// itself.
fn tokens<T: JsonForm + Debug>(tokens: &[&str]) {
    for token in tokens {
        let value: T = sample(json!(token));
        round_trip(&value);
        assert_eq!(value.to_value(), json!(token), "{value:?}");
    }
}

#[test]
fn the_scalars_round_trip() {
    round_trip(&"a text".to_string());
    round_trip(&true);
    round_trip(&false);
    round_trip(&7usize);
    round_trip(&u32::MAX);
    round_trip(&json!({ "a": [1, "b", null, true], "c": { "d": -3 } }));
}

#[test]
fn the_containers_round_trip() {
    round_trip(&None::<String>);
    round_trip(&Some("x".to_string()));
    round_trip(&vec!["a".to_string(), "b".to_string()]);
    round_trip(&Vec::<usize>::new());
    round_trip(&("x".to_string(), 3usize));
    round_trip(&Some(("x".to_string(), "y".to_string())));
    round_trip(&vec![Mode::Text, Mode::Math]);
    round_trip(&vec![Some(true), None]);
    round_trip(&Some(Label::new("eq", "one")));
    round_trip(&None::<Label>);
}

#[test]
fn a_container_of_another_shape_is_refused() {
    let error = |result: Result<Vec<String>, flm_ext::runtime::ExtError>| result.expect_err("refused").to_string();
    assert_eq!(error(Vec::from_value(&json!("a"))), "the value must be a list, not `\"a\"`");
    assert_eq!(error(Vec::from_value(&json!([1]))), "the value must be a text, not `1`");
    let pair = <(String, String)>::from_value(&json!(["a"])).expect_err("refused").to_string();
    assert_eq!(pair, "the value must be a list of two items, not `[\"a\"]`");
}

#[test]
fn the_unit_enums_round_trip() {
    tokens::<Effect>(&["compute", "check", "io"]);
    tokens::<Scope>(&["per-tree", "document"]);
    tokens::<CallableKind>(&["macro", "environment", "specials"]);
    tokens::<FallbackKind>(&["macro", "environment"]);
    tokens::<DefinitionKind>(&["macro", "environment", "specials"]);
    tokens::<DefinitionModes>(&["both", "text", "math"]);
    tokens::<ArgLayout>(&["inline", "blocks", "inline-or-blocks"]);
    tokens::<BodyLayout>(&["inline", "blocks", "from-role"]);
    tokens::<ExpandPolicy>(&["always", "per-plan"]);
    tokens::<Placement>(&["leads", "trails", "free"]);
    tokens::<Severity>(&["error", "warning", "note"]);
    tokens::<NodeKindName>(&["chars", "group", "callable", "comment", "list"]);
    tokens::<Mode>(&["text", "math"]);
    tokens::<StandardTable>(&["lowered-trees", "lowered-pieces", "regions", "artifact-set"]);
    tokens::<Arity>(&["single", "ordered-many"]);
    tokens::<FilesScope>(&["extension", "document"]);
    tokens::<LatexArgShape>(&["star", "optional", "mandatory", "escverb"]);
    tokens::<LatexDefinitionKind>(&["macro", "environment", "declaration"]);
}

#[test]
fn the_declarations_of_stages_and_rules_round_trip() {
    round_trips::<StageDecl>(&[json!({
        "name": "x.s", "effect": "compute", "scope": "document", "provides": ["x.t"], "writes": ["x.w"], "docs": "D."
    })]);
    round_trips::<Needs>(&[json!({
        "requires": ["a"], "reads": ["b"], "reads_opt": ["c"], "after": ["d"], "before": ["e"], "options": ["f"]
    })]);
    round_trips::<RewriteDecl>(&[
        json!({ "name": "x.r", "publishes": "x.p", "refines": "x.q", "default": true, "docs": "D." }),
        json!({ "name": "x.r", "publishes": "x.p" }),
    ]);
    round_trips::<LoweringDecl>(&[json!({
        "requires": ["a"], "reads": ["b"], "reads_opt": ["c"], "options": ["d"], "produces": ["e"], "selection_requires": ["f"]
    })]);
    round_trips::<DefaultMark>(&[json!("everywhere"), json!("never"), json!({ "on_backends": ["html", "latex"] })]);
    round_trips::<OutputDef>(&[
        json!({
            "name": "tb", "capability": "tb.document", "table": "tb.files", "media_type": "application/x-typst",
            "extension": "typ", "docs": "Typst."
        }),
        json!({
            "name": "tp", "capability": "tp.document", "table": "tp.files", "media_type": "application/pdf",
            "extension": "pdf", "made_from": ["latex-rendered", "html"]
        }),
    ]);
    let pdf: OutputDef = sample(json!({
        "name": "tp", "capability": "tp.document", "table": "tp.files", "media_type": "application/pdf",
        "extension": "pdf", "made_from": ["latex-rendered"]
    }));
    assert_eq!(pdf, OutputDef::new("tp", "tp.document", "tp.files", "application/pdf", "pdf").made_from("latex-rendered"));
}

#[test]
fn the_values_a_node_answers_round_trip() {
    round_trip(&Span { start: 3, end: 9 });
    round_trips::<CharsContent>(&[json!({ "text": "abc", "has_placeholder": false }), json!({ "text": null, "has_placeholder": true })]);
    round_trip(&InlineSpacing { absorb_before: true, absorb_after: false });
    round_trips::<BlockRole>(&[
        json!("block"),
        json!({ "inline": { "absorb_before": false, "absorb_after": true } }),
        json!({ "block-decorator": "trails" }),
    ]);
}

#[test]
fn the_citation_values_round_trip() {
    round_trip(&CitationProviderError::new("no such key"));
    round_trips::<EntryPiece>(&[
        json!({ "text": "A." }),
        json!({ "source": "\\emph{B}" }),
        json!({ "emphasized": [{ "text": "C" }] }),
        json!({ "strong": [{ "text": "D" }, { "source": "E" }] }),
        json!({ "link": { "address": "https://example.org", "content": [{ "text": "F" }] } }),
    ]);
}

#[test]
fn the_compose_values_round_trip() {
    round_trip(&Accepts::All);
    round_trips::<Accepts>(&[json!(["body", "title"])]);
    round_trips::<TemplateDescription>(&[
        json!({ "output": "page", "accepts": "all", "requires": ["body"], "config": { "a": 1 } }),
        json!({ "output": "page", "accepts": ["body"] }),
    ]);
    round_trips::<TemplateFile>(&[
        json!({ "name": "style", "extension": "css", "media_type": "text/css", "text": "p {}" }),
        json!({ "name": "logo", "file": "logo.svg", "scope": "extension", "media_type": "image/svg+xml" }),
        json!({ "name": "notes", "file": "notes.txt", "scope": "document" }),
    ]);
    round_trip(&PieceValue {
        name: "body".to_string(),
        place: "main".to_string(),
        text: "<p>A</p>".to_string(),
        words: "A".to_string(),
    });
    round_trips::<TemplateContext>(&[json!({
        "html": true,
        "regions": [["main", "<p>A</p>"]],
        "pieces": [{ "name": "body", "place": "main", "text": "<p>A</p>", "words": "A" }],
        "meta": { "title": "T" },
        "config": { "lang": "en" },
        "artifacts": [["logo", "logo.svg"]],
    })]);
}

#[test]
fn a_wrong_field_of_the_data_of_a_compose_template_is_named() {
    let data = |field: &str, value: Value| {
        let mut data = json!({ "html": true, "regions": [], "pieces": [], "meta": {}, "config": {}, "artifacts": [] });
        data[field] = value;
        TemplateContext::from_value(&data).expect_err("refused").to_string()
    };
    assert_eq!(
        data("html", json!(3)),
        "in the field `html` of the data of a compose template, the value must be `true` or `false`, not `3`",
    );
    assert_eq!(
        data("regions", json!([["main", 1]])),
        "in the field `regions` of the data of a compose template, the value must be a text, not `1`",
    );
    assert_eq!(
        data("pieces", json!({})),
        "in the field `pieces` of the data of a compose template, the value must be a list, not `{}`",
    );
    assert_eq!(
        data("artifacts", json!([["logo"]])),
        "in the field `artifacts` of the data of a compose template, the value must be a list of two items, not `[\"logo\"]`",
    );
}

#[test]
fn the_style_and_script_definitions_round_trip() {
    round_trips::<DefContent>(&[
        json!({ "inline": "p {}" }),
        json!({ "file": "a.css" }),
        json!({ "url": "https://example.org/a.css" }),
        json!({ "table": "x.styles" }),
    ]);
    let definition = json!({
        "id": "x.style",
        "content": [{ "inline": "p {}" }, { "file": "a.css" }, { "url": "https://example.org" }, { "table": "x.t" }],
        "docs": "D."
    });
    round_trips::<StyleDef>(std::slice::from_ref(&definition));
    round_trips::<ScriptDef>(&[definition]);
}

#[test]
fn the_labels_round_trip() {
    round_trip(&Label::new("eq", "one"));
    round_trips::<LabelUse>(&[
        json!({ "text": "eq:one", "span": { "start": 0, "end": 6 }, "label": { "ref_type": "eq", "ref_label": "one" } }),
        json!({ "text": "one", "span": { "start": 2, "end": 5 }, "label": null }),
    ]);
    round_trips::<MalformedLabel>(&[json!({ "text": "a:b:c", "span": { "start": 1, "end": 6 }, "position": 2 })]);
    round_trip(&AcceptedRefTypes::Any);
    round_trip(&AcceptedRefTypes::These(vec!["eq".to_string(), "fig".to_string()]));
}

#[test]
fn the_artifacts_round_trip() {
    round_trip(&ArtifactId::new("x", "page"));
    round_trips::<ArtifactData>(&[
        json!({ "bytes": "text" }),
        json!({ "bytes": { "text": "text" } }),
        json!({ "bytes": { "base64": "AP8=" } }),
        json!({ "files": [["a/b.txt", "B"], ["c.txt", ""]] }),
        json!({ "files": [["a.bin", { "base64": "/wAB" }], ["b.txt", { "text": "B" }]] }),
    ]);
    round_trips::<Artifact>(&[
        json!({
            "id": { "owner": "x", "name": "page" },
            "media_type": "text/html",
            "entry": true,
            "data": { "bytes": "<p>A</p>" },
            "fixed_path": "index.html",
            "refs": [{ "owner": "x", "name": "style" }],
            "extension": "html"
        }),
        json!({
            "id": { "owner": "x", "name": "folder" },
            "media_type": "inode/directory",
            "data": { "files": [["a.txt", "A"]] }
        }),
    ]);
}

#[test]
fn a_content_is_written_as_text_when_it_is_utf_8_and_in_base64_otherwise() {
    assert_eq!(ArtifactData::Bytes(b"caf\xc3\xa9".to_vec()).to_value(), json!({ "bytes": { "text": "café" } }));
    assert_eq!(ArtifactData::Bytes(vec![0xff, 0x00]).to_value(), json!({ "bytes": { "base64": "/wA=" } }));
    let files = ArtifactData::Files(vec![(RelPath::new("a.png"), vec![0x89, b'P']), (RelPath::new("b.txt"), b"B".to_vec())]);
    assert_eq!(files.to_value(), json!({ "files": [["a.png", { "base64": "iVA=" }], ["b.txt", { "text": "B" }]] }));
    // The three forms a writer may give, and the bytes each stands for.
    for (content, bytes) in [
        (json!("é"), "é".as_bytes().to_vec()),
        (json!({ "text": "é" }), "é".as_bytes().to_vec()),
        (json!({ "base64": "/wA=" }), vec![0xff, 0x00]),
        (json!({ "base64": "" }), Vec::new()),
    ] {
        let read: ArtifactData = sample(json!({ "bytes": content }));
        assert_eq!(read, ArtifactData::Bytes(bytes), "{content}");
    }
}

#[test]
fn the_numbering_values_round_trip() {
    round_trip(&FormatOptions::new());
    round_trip(&FormatOptions::new().variant("short").without_word().without_delimiters());
    round_trips::<Within>(&[json!({ "parents": { "one": "x.p" }, "joiner": { "joined": "." }, "no_restart": false })]);
    round_trips::<Parents>(&[json!({ "one": "x.a" }), json!({ "any": ["x.a", "x.b"] }), json!({ "set": "x.s" })]);
    round_trips::<WithinJoinerSpec>(&[
        json!({ "joined": "." }),
        json!({ "template": "${parent}.${this}" }),
        json!("bare"),
        json!({ "named": "x.j" }),
    ]);
    round_trips::<PluralWords>(&[json!({ "one": "Equation", "other": "Equations" })]);
    round_trips::<PrefixWords>(&[json!({
        "base": { "one": "Equation", "other": "Equations" },
        "variants": { "short": { "one": "Eq.", "other": "Eqs." } }
    })]);
    round_trips::<Delimiters>(&[json!({ "open": "(", "close": ")" })]);
    round_trips::<NumberTemplate>(&[json!({ "formats": "${arabic}" }), json!({ "initial": "1" })]);
    round_trips::<CounterDecl>(&[
        json!({ "name": "x.c", "default_style": "x.s" }),
        json!({
            "name": "x.c", "default_style": "x.s", "docs": "D.",
            "within": { "parents": { "one": "x.p" }, "joiner": { "joined": "." }, "no_restart": true },
            "nesting": "bare"
        }),
    ]);
    round_trips::<CounterStyleDecl>(&[
        json!({ "name": "x.s" }),
        json!({
            "name": "x.s",
            "number_format": { "formats": "${arabic}" },
            "prefix_words": { "base": { "one": "Item", "other": "Items" } },
            "delimiters": { "open": "[", "close": "]" },
            "name_in_link": false,
            "repeat_parents_in_range": true,
            "ranges": false,
            "number_writer": "x.w"
        }),
    ]);
    round_trips::<ParentSetDecl>(&[json!({ "name": "x.p", "counters": ["x.a", "x.b"] })]);
}

#[test]
fn the_user_definition_templates_round_trip() {
    round_trip(&PlaceholderVocabulary::new(2));
    round_trip(&PlaceholderVocabulary::new(1).names(vec![Some("a".to_string()), None]).with_body().extra("n", true));
    round_trips::<TemplateSpec>(&[
        json!({
            "text": "#1 and #{n}", "mode": "text",
            "vocabulary": { "arity": 1, "names": ["a"], "body": false, "extras": [{ "name": "n", "substitution": false }] },
            "definition": "\\x", "label": "the template", "origin_label": "x.flm", "user_definition": true, "offset": 4
        }),
        json!({
            "text": "#1", "mode": "math", "vocabulary": { "arity": 1 },
            "definition": "\\y", "label": "l", "origin_label": "o"
        }),
    ]);
    round_trip(&BodyDef::new());
    round_trip(&BodyDef::new().layout(BodyLayout::Blocks).verbatim().mode(Mode::Math).scoped("x.pkg"));
}

#[test]
fn the_latex_values_round_trip() {
    round_trips::<LatexMacroPayload>(&[
        json!({ "name": "emph" }),
        json!({ "name": "section", "args": ["star", "optional", "mandatory"], "packed": true }),
    ]);
    round_trips::<LatexEnvironmentPayload>(&[
        json!({ "name": "quote", "verbatim": false }),
        json!({ "name": "lstlisting", "args": ["optional"], "verbatim": true, "packed": true, "body": true }),
    ]);
    round_trips::<LatexRawPayload>(&[json!({ "text": "\\relax" })]);
    round_trips::<LatexDefinitionPayload>(&[
        json!({ "kind": "macro", "name": "x" }),
        json!({ "kind": "macro", "name": "y", "source": "z", "args": 2, "packed": true, "robust": true, "feature": "x.f" }),
        json!({ "kind": "environment", "name": "v", "args": 1, "body": true }),
        json!({ "kind": "declaration", "name": "w", "feature": "x.f" }),
    ]);
    round_trips::<LatexPreambleDef>(&[json!({ "name": "x.p", "content": "\\newcommand{\\x}{X}", "docs": "D.", "requires": ["x.q"] })]);
    round_trips::<LatexPackageDef>(&[json!({ "id": "x.pkg", "package": "amsmath", "options": ["fleqn"], "docs": "D." })]);
}
