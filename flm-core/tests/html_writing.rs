//! The HTML writing layer (`flm_core::stdcore::backends::html`) as the
//! standard package registers it: the construct element that mirrors one HTML
//! tag and its payload through the serialization; the escaping of text and of
//! attribute values, in double and in single quotation marks; the typed sink —
//! its entry points, the shape of the bytes
//! it lays out, and what it refuses; the naming functions of the contract; the
//! address policy; and the walk over the library's own sources that keeps the
//! places writing unescaped content the listed ones.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use flm_core::core::registry::FlmRegistry;
use flm_core::dump::session::PayloadReaders;
use flm_core::lang::{FlmNodeExt, PresPayload};
use flm_core::model::options::TypeKey;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::html::address::{AddressUse, check_address};
use flm_core::stdcore::backends::html::construct::{self, payload_readers};
use flm_core::stdcore::backends::html::escape::{
    SPACE_REFERENCES, escape_attr_into, escape_attr_single_quoted_into, escape_text_into,
};
use flm_core::stdcore::backends::html::names::{
    class_of_element, class_of_role, data_attribute, is_valid_attribute_name, is_valid_tag_name,
};
use flm_core::stdcore::backends::html::{FEATURE_NAME, HtmlSink, HtmlTagPayload, Markup, TagBody};
use flm_core::stdcore::backends::pres;

use common::dump::round_trip_value;
use common::stdcore::registry;

fn package() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

// --- registration ----------------------------------------------------------------------------

#[test]
fn the_standard_package_registers_the_tag_element_under_html() {
    let registry = package();
    assert!(registry.has_feature(FEATURE_NAME));
    let entry = registry.presentation_element(construct::TAG).expect("the tag element");
    assert_eq!(entry.owner.as_str(), FEATURE_NAME);
    assert_eq!(entry.payload, TypeKey::of::<HtmlTagPayload>());
    assert!(!registry.elements_named(construct::TAG).is_empty(), "the tag is not an element");
    // It is the HTML backend's own, not part of the shared vocabulary: no
    // other backend can be asked to write it.
    assert!(!pres::ALL.contains(&construct::TAG));
}

#[test]
fn the_payload_reader_covers_exactly_the_tag_element() {
    let readers: PayloadReaders = payload_readers();
    let elements: Vec<String> = readers.elements().map(|e| e.as_str().to_string()).collect();
    assert_eq!(elements, [construct::TAG]);
}

// --- the payload through the serialization ----------------------------------------------------

fn round_trip(payload: HtmlTagPayload) {
    let registry = package();
    let lang = registry.lang();
    let spec =
        registry.presentation_element(construct::TAG).expect("registered").element.clone();
    let projection = payload.project();
    let ext = FlmNodeExt::new(Arc::clone(&spec)).with_payload(Arc::new(payload));
    let (written, read) = round_trip_value(lang, vec![spec], payload_readers(), ext);
    assert_eq!(common::dump::field(&written, "payload"), Some(&projection));
    let read = read.expect("the extension reads back").payload.expect("with its payload");
    assert!(read.downcast_ref::<HtmlTagPayload>().is_some(), "the payload reads back as its type");
    assert_eq!(read.project(), projection);
}

#[test]
fn every_shape_of_the_tag_payload_round_trips_through_its_registered_reader() {
    round_trip(HtmlTagPayload::new("span"));
    round_trip(HtmlTagPayload::new("div").block());
    round_trip(
        HtmlTagPayload::new("div")
            .attributes([("class", "mine-figure"), ("data-mine-kind", "plot")])
            .block(),
    );
    round_trip(
        HtmlTagPayload::new("tex-html")
            .attributes([("id", "eq-1")])
            .markup(Markup::trusted("<span class=\"mine\">x</span>")),
    );
    // A value that would be markup if it were not escaped survives as text.
    round_trip(HtmlTagPayload::new("span").attributes([("title", "a <b> & \"c\"")]));
}

// --- escaping ---------------------------------------------------------------------------------

fn text(input: &str) -> String {
    let mut out = String::new();
    escape_text_into(&mut out, input);
    out
}

fn attr(input: &str) -> String {
    let mut out = String::new();
    escape_attr_into(&mut out, input);
    out
}

fn attr_single(input: &str) -> String {
    let mut out = String::new();
    escape_attr_single_quoted_into(&mut out, input);
    out
}

#[test]
fn text_and_attribute_values_escape_the_characters_each_of_them_must() {
    let table: [(&str, &str, &str, &str); 7] = [
        // input, as text, as a double-quoted value, as a single-quoted one
        ("a & b", "a &amp; b", "a &amp; b", "a &amp; b"),
        ("<em>", "&lt;em&gt;", "&lt;em&gt;", "<em>"),
        ("\"quoted\"", "\"quoted\"", "&quot;quoted&quot;", "\"quoted\""),
        ("it's", "it's", "it's", "it&#39;s"),
        ("a\u{00A0}b", "a&#xA0;b", "a\u{00A0}b", "a\u{00A0}b"),
        ("Café — ½ ≤ α", "Café — ½ ≤ α", "Café — ½ ≤ α", "Café — ½ ≤ α"),
        ("x\ny\tz", "x\ny\tz", "x\ny\tz", "x\ny\tz"),
    ];
    for (input, as_text, as_attr, as_single) in table {
        assert_eq!(text(input), as_text, "{input:?} as text");
        assert_eq!(attr(input), as_attr, "{input:?} in double quotation marks");
        assert_eq!(attr_single(input), as_single, "{input:?} in single quotation marks");
    }
}

#[test]
fn a_string_that_already_holds_a_character_reference_is_escaped_again() {
    // No entry point takes an already-escaped string: what a document wrote is
    // text, and escaping it twice is the correct answer, not a defect.
    assert_eq!(text("&amp;"), "&amp;amp;");
    assert_eq!(attr("&amp;"), "&amp;amp;");
    assert_eq!(attr_single("&amp;"), "&amp;amp;");
    assert_eq!(text("&lt;p&gt;"), "&amp;lt;p&amp;gt;");
}

#[test]
fn the_eight_spaces_are_numeric_in_text_and_left_alone_in_an_attribute_value() {
    let expected = [
        ('\u{00A0}', "&#xA0;"),
        ('\u{200A}', "&#x200A;"),
        ('\u{2009}', "&#x2009;"),
        ('\u{2008}', "&#x2008;"),
        ('\u{2002}', "&#x2002;"),
        ('\u{2003}', "&#x2003;"),
        ('\u{2007}', "&#x2007;"),
        ('\u{202F}', "&#x202F;"),
    ];
    assert_eq!(SPACE_REFERENCES, expected);
    for (space, reference) in expected {
        assert_eq!(text(&space.to_string()), reference);
        assert_eq!(attr(&space.to_string()), space.to_string());
    }
}

// --- the sink ---------------------------------------------------------------------------------

#[test]
fn document_text_reaches_the_page_through_the_three_escaping_entry_points() {
    let mut sink = HtmlSink::new();
    sink.open_block("p")
        .class(["flm-pres-par"])
        .attr("title", "a <\"b\"> & c")
        .attr_json("data-flm-source-span", "[\"it's & <that>\",0,8]")
        .finish();
    sink.text("1 < 2 & \"three\"");
    sink.close();
    assert_eq!(
        sink.finish(),
        "<p class=\"flm-pres-par\" title=\"a &lt;&quot;b&quot;&gt; &amp; c\" \
         data-flm-source-span='[\"it&#39;s &amp; <that>\",0,8]'>\
         1 &lt; 2 &amp; \"three\"</p>"
    );
}

#[test]
fn a_markup_value_is_the_one_path_that_writes_bytes_unescaped() {
    let mut sink = HtmlSink::new();
    sink.open_inline("span").finish();
    sink.text("<not markup>");
    sink.markup(&Markup::trusted("<svg viewBox=\"0 0 1 1\"></svg>"));
    sink.close();
    assert_eq!(
        sink.finish(),
        "<span>&lt;not markup&gt;<svg viewBox=\"0 0 1 1\"></svg></span>"
    );
}

#[test]
fn block_and_inline_elements_are_laid_out_by_the_call_that_opened_them() {
    let mut sink = HtmlSink::new();
    // A tag a rule staged decides for itself: the same `<span>` name, opened
    // as a block, is laid out as a block.
    sink.open_block("section").class(["flm-pres-block", "flm-heading"]).finish();
    sink.open_block("h2").attr("data-flm-level", "2").finish();
    sink.text("Title ");
    sink.open_inline("em").finish();
    sink.text("here");
    sink.close();
    sink.close();
    sink.open_block("span").class(["staged-by-a-rule"]).finish();
    sink.text("a block whose tag is usually inline");
    sink.close();
    sink.close();
    assert_eq!(
        sink.finish(),
        "<section class=\"flm-pres-block flm-heading\">\n  \
         <h2 data-flm-level=\"2\">Title <em>here</em></h2>\n  \
         <span class=\"staged-by-a-rule\">a block whose tag is usually inline</span>\n\
         </section>"
    );
}

#[test]
fn a_preformatted_region_gets_no_introduced_whitespace_and_is_still_escaped() {
    let mut sink = HtmlSink::new();
    sink.open_block("div").class(["flm-pres-block"]).finish();
    sink.open_block("pre").class(["flm-pres-verbatim"]).preformatted().finish();
    sink.open_block("code").attr("data-flm-lang", "python").finish();
    sink.text("if a < b:\n    return \"&\"");
    sink.close();
    sink.close();
    sink.open_block("p").finish();
    sink.text("after");
    sink.close();
    sink.close();
    // Between tags a quotation mark is written as itself; the ampersand is not.
    assert_eq!(
        sink.finish(),
        "<div class=\"flm-pres-block\">\n  \
         <pre class=\"flm-pres-verbatim\"><code data-flm-lang=\"python\">\
         if a &lt; b:\n    return \"&amp;\"</code></pre>\n  \
         <p>after</p>\n\
         </div>"
    );
}

#[test]
fn the_content_of_a_style_or_script_element_is_written_as_it_stands() {
    let mut sink = HtmlSink::new();
    sink.raw_text_element("style", &[], "@layer flm { .flm-pres-par > a { color: blue } }\n")
        .expect("no closing tag inside");
    sink.raw_text_element("script", &[("type", "module")], "if (1 < 2) { load(); }\n")
        .expect("no closing tag inside");
    assert_eq!(
        sink.finish(),
        "<style>@layer flm { .flm-pres-par > a { color: blue } }\n</style>\n\
         <script type=\"module\">if (1 < 2) { load(); }\n</script>"
    );
}

#[test]
fn a_style_or_script_holding_its_own_closing_tag_is_refused_in_any_letter_case() {
    let mut sink = HtmlSink::new();
    for content in ["</script>", "x = \"</ScRiPt >\"", "</SCRIPT"] {
        let refused =
            sink.raw_text_element("script", &[], content).expect_err("it would end the element");
        assert_eq!(refused.tag, "script");
        assert!(refused.to_string().contains("closing tag"), "{refused}");
    }
    // The closing tag of another element is ordinary content there.
    sink.raw_text_element("script", &[], "x = \"</style>\"").expect("not its own closing tag");
    assert_eq!(sink.finish(), "<script>x = \"</style>\"</script>");
}

#[test]
#[should_panic(expected = "not a <textarea>")]
fn a_raw_text_element_is_a_style_or_a_script_and_no_other_tag() {
    // Those two are the elements whose content a browser reads as raw text.
    // `<textarea>`'s content is read as markup, so content written into it
    // unescaped would be markup; asking for it is a mistake in the calling
    // code, and it panics in every build.
    let mut sink = HtmlSink::new();
    let _ = sink.raw_text_element("textarea", &[], "1 < 2");
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "tag name is not writable")]
fn a_tag_name_the_caller_did_not_check_is_caught_while_testing() {
    // The sink's precondition is that the caller validated the name with
    // `is_valid_tag_name`; the emitter does that and reports the node it
    // refused. The sink checks it again in a debug assertion, which runs here
    // because a test binary is built in debug mode, and not in a release
    // build, where the name would be written as it stands.
    let mut sink = HtmlSink::new();
    sink.open_block("p><script>alert(1)</script").finish();
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "attribute whose name is not writable")]
fn an_attribute_name_the_caller_did_not_check_is_caught_while_testing() {
    // The same debug assertion over the other kind of name; a test binary is
    // built in debug mode, so it runs here.
    let mut sink = HtmlSink::new();
    sink.void_inline("img").attr("src\" onerror=\"alert(1)", "x.png").finish();
}

#[test]
fn a_sink_shows_its_state_and_never_its_bytes_before_it_is_finished() {
    // The bytes leave the sink through `finish` and by no other way, so a
    // sink printed for debugging shows how much it holds, not what.
    let mut sink = HtmlSink::new();
    sink.open_block("p").finish();
    sink.text("a document's own words");
    let shown = format!("{sink:?}");
    assert!(!shown.contains("document"), "{shown}");
    assert!(!shown.contains("<p>"), "{shown}");
    assert!(shown.contains("bytes_written: 25"), "{shown}");
    assert!(shown.contains("open: [\"p\"]"), "{shown}");
    sink.close();
}

// --- the accessibility fields of the shared vocabulary ----------------------------------------

#[test]
fn a_links_document_role_and_accessible_name_round_trip_through_the_serialization() {
    use flm_core::model::names::Name;
    use flm_core::stdcore::backends::pres::{DocRole, LinkPayload, SpanPayload};

    let registry = package();
    let lang = registry.lang();
    let link = LinkPayload {
        href: String::from("#fnref-1"),
        kind: Name::from("xnote-back"),
        doc_role: Some(DocRole::BackLink),
        label: Some(String::from("Back to \"1\" <here>")),
        shows_target: false,
    };
    let span = SpanPayload { doc_role: Some(DocRole::Footnote), ..SpanPayload::default() };
    let payloads: [(&str, Arc<dyn PresPayload>); 2] =
        [(pres::LINK, Arc::new(link)), (pres::SPAN, Arc::new(span))];
    for (element, payload) in payloads {
        let spec = registry.presentation_element(element).expect("registered").element.clone();
        let projection = payload.project();
        let ext = FlmNodeExt::new(Arc::clone(&spec)).with_payload(payload);
        let (written, read) = round_trip_value(lang, vec![spec], pres::payload_readers(), ext);
        assert_eq!(common::dump::field(&written, "payload"), Some(&projection));
        let read = read.expect("the extension reads back").payload.expect("with its payload");
        assert_eq!(read.project(), projection, "`{element}` reads back as it was written");
    }
}

#[test]
fn the_two_accessibility_attributes_are_writable_names() {
    use flm_core::stdcore::backends::html::conventions::row;

    let link = row(pres::LINK).expect("a row");
    for (field, attribute) in [("doc_role", "role"), ("label", "aria-label")] {
        let written = link.field(field).and_then(|field| field.attribute());
        assert_eq!(written.as_deref(), Some(attribute));
        assert!(is_valid_attribute_name(attribute), "{attribute}");
    }
    for element in [pres::BLOCK, pres::SPAN] {
        let row = row(element).expect("a row");
        assert_eq!(
            row.field("doc_role").and_then(|field| field.attribute()).as_deref(),
            Some("role")
        );
    }
}

// --- the naming functions ---------------------------------------------------------------------

#[test]
fn the_contract_s_names_are_derived_from_the_vocabulary_s_own() {
    assert_eq!(class_of_element(pres::PAR), "flm-pres-par");
    assert_eq!(class_of_element(pres::BLOCK_HEADING), "flm-pres-block--heading");
    assert_eq!(class_of_element(construct::TAG), "flm-pres-html-tag");
    assert_eq!(class_of_role("flm.heading"), "flm-heading");
    assert_eq!(data_attribute("kind"), "data-flm-kind");
    assert_eq!(data_attribute("indent_left"), "data-flm-indent-left");
    // Every class the scheme derives is a name that may be written.
    for element in pres::ALL {
        assert!(is_valid_attribute_name(&class_of_element(element)), "{element}");
    }
}

#[test]
fn a_tag_or_attribute_name_that_could_break_out_of_its_tag_is_refused() {
    for name in ["div", "h1", "tex-html"] {
        assert!(is_valid_tag_name(name), "{name}");
    }
    for name in ["data-flm-level", "href", "xml:lang"] {
        assert!(is_valid_attribute_name(name), "{name}");
    }
    for name in [
        "",
        "div onclick=\"x\"",
        "div>",
        "<div",
        "a/",
        "a b",
        "1a",
        "-a",
        "à",
        "a\"",
        "a'",
        "a=",
    ] {
        assert!(!is_valid_tag_name(name), "{name:?} as a tag name");
        assert!(!is_valid_attribute_name(name), "{name:?} as an attribute name");
    }
    // The name a construct payload carries is checked before it is written; a
    // payload can hold anything, which is why the check exists.
    let payload = HtmlTagPayload::new("script>alert(1)</script");
    assert!(!is_valid_tag_name(&payload.tag));
}

// --- the address policy -----------------------------------------------------------------------

#[test]
fn every_address_the_policy_writes() {
    let written = [
        "#section-2",
        "",
        "figures/plot.png",
        "../index.html",
        "/absolute",
        "?page=2",
        "a/b:c",
        "#see:this",
        "http://example.org/",
        "https://example.org/p?a=1&b=2",
        "mailto:someone@example.org",
        "ftp://example.org/f",
        "tel:+41000000000",
        "HTTPS://EXAMPLE.ORG/",
    ];
    for address in written {
        assert!(check_address(address, AddressUse::Link).is_ok(), "{address:?} as a link");
        assert!(check_address(address, AddressUse::Image).is_ok(), "{address:?} as an image");
    }
    // A picture written into the page is an image's address and no link's.
    let png = "data:image/png;base64,iVBORw0KGgo=";
    assert!(check_address(png, AddressUse::Image).is_ok());
    assert_eq!(check_address(png, AddressUse::Link).expect_err("not a link").scheme, "data");
}

#[test]
fn a_scheme_that_hides_behind_case_or_whitespace_is_refused_all_the_same() {
    let hidden = [
        "javascript:alert(1)",
        "JaVaScRiPt:alert(1)",
        "  javascript:alert(1)",
        "\u{1}javascript:alert(1)",
        "java\nscript:alert(1)",
        "java\tscript:alert(1)",
        "java\r\nscript:alert(1)",
        "\u{0}\u{1}\u{2} JAVASCRIPT:alert(1)",
    ];
    for address in hidden {
        for used_as in [AddressUse::Link, AddressUse::Image] {
            let refused = check_address(address, used_as).expect_err("a scheme not written");
            assert_eq!(refused.scheme, "javascript", "{address:?}");
        }
    }
    // A scheme written with character references is not decoded, and needs no
    // decoding: the sink escapes the `&`, so the page holds `&amp;#106;…`,
    // which is a relative address and not a scheme at all.
    assert!(check_address("&#106;avascript:alert(1)", AddressUse::Link).is_ok());
    // Other schemes a page must not carry.
    for (address, scheme) in [
        ("vbscript:x", "vbscript"),
        ("data:text/html,<script>x</script>", "data"),
        ("file:///etc/passwd", "file"),
        ("blob:https://example.org/x", "blob"),
    ] {
        let refused = check_address(address, AddressUse::Image).expect_err("a scheme not written");
        assert_eq!(refused.scheme, scheme, "{address:?}");
    }
}

// --- the places that write unescaped content ---------------------------------------------------

/// The three entry points through which bytes reach a page without being
/// escaped, each written as the text a call of it holds.
///
/// `Markup::trusted` is the public one, the deliberate act an extension takes
/// for the markup leaf of a tag construct; `Markup::from_library` is its
/// crate-private twin, for bytes this library itself wrote; and
/// `HtmlSink::raw_text_element` is where the content of a style or a script
/// definition, and whatever a host put in its place, is written.
const UNESCAPED_ENTRY_POINTS: &[&str] =
    &["Markup::trusted(", "from_library(", "raw_text_element("];

/// Every call of one of those entry points in the library's own code: the file
/// it stands in, as a path under `flm-core/src/` written with `/` whatever the
/// platform's separator, and the entry point it calls.
///
/// The standard package calls `Markup::trusted` nowhere; the page assembly
/// writes the trees' own HTML and the document type line through
/// `Markup::from_library`, and the content of a style or script definition
/// through `raw_text_element`. A call site that is not listed here is one this
/// test makes visible, so that adding one is a deliberate act.
const UNESCAPED_CALL_SITES: &[(&str, &str)] = &[
    // The markup body of a tag construct node, rebuilt from the serialized
    // form this library itself wrote.
    ("stdcore/backends/html/construct.rs", "from_library("),
    // The content of a `<style>` or a `<script>`, once the refusal check has
    // passed, written through the one entry point for unescaped bytes.
    ("stdcore/backends/html/sink.rs", "from_library("),
    // The page assembly: the document type line, which is not a tag, and each
    // tree's HTML, which this backend's own emitter wrote through this same
    // sink with every piece of document text already escaped.
    ("stdcore/backends/html/page.rs", "from_library("),
    // The page assembly again: the text of a style or script definition, and
    // whatever the host's settings put in its place. It is a feature author's
    // or a host's, never a document's, and a browser reads the content of
    // those two elements as raw characters, so it cannot be escaped at all.
    ("stdcore/backends/html/page.rs", "raw_text_element("),
];

/// Whether a line of code declares a function rather than calling one: its
/// first words are `fn`, or a visibility or a qualifier and then `fn`.
fn declares_a_function(code: &str) -> bool {
    let mut words = code.split_whitespace();
    loop {
        match words.next() {
            Some("fn") => return true,
            Some(word)
                if word == "pub"
                    || word.starts_with("pub(")
                    || word == "const"
                    || word == "async"
                    || word == "unsafe"
                    || word == "extern" => {}
            _ => return false,
        }
    }
}

/// The path of the crate's source directory.
fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every `.rs` file under `dir`, in a deterministic order.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .expect("the crate's source directory is readable")
        .map(|entry| entry.expect("a directory entry of the source tree is readable").path())
        .collect();
    entries.sort();
    let mut files = Vec::new();
    for entry in entries {
        if entry.is_dir() {
            files.extend(rust_files(&entry));
        } else if entry.extension().is_some_and(|ext| ext == "rs") {
            files.push(entry);
        }
    }
    files
}

#[test]
fn the_call_sites_that_write_unescaped_bytes_are_the_listed_ones() {
    // The library's shipped code is what is walked: the text before the
    // file's own unit tests, with each line's `//` comment cut off, so that a
    // documentation example naming an entry point is not read as a call of
    // it, and with the line that declares an entry point left out, since a
    // declaration is not a call. The walk reads text, so it has the limits
    // text has: a call written after a `//` inside a string literal, or in
    // shipped code standing after an early `#[cfg(test)]` module, would not
    // be seen. The LaTeX family's walk over its own prefix has the same
    // shape.
    let src = src_dir();
    let mut found: BTreeSet<(String, String)> = BTreeSet::new();
    for file in rust_files(&src) {
        let text = fs::read_to_string(&file).expect("a source file is readable");
        let shipped = match text.find("#[cfg(test)]") {
            Some(at) => &text[..at],
            None => text.as_str(),
        };
        // The platform's separator is not what the list is written with.
        let relative = file
            .strip_prefix(&src)
            .expect("under src")
            .to_string_lossy()
            .replace('\\', "/");
        for line in shipped.lines() {
            let code = match line.find("//") {
                Some(at) => &line[..at],
                None => line,
            };
            // A line that *declares* an entry point is not a call of it. The
            // declaration is what carries a visibility and a parameter list,
            // so it is recognized by the `fn` keyword standing at the start of
            // the line's code, after an optional `pub`, `const` or `unsafe`.
            if declares_a_function(code) {
                continue;
            }
            for entry_point in UNESCAPED_ENTRY_POINTS {
                if code.contains(entry_point) {
                    found.insert((relative.clone(), (*entry_point).to_string()));
                }
            }
        }
    }
    let listed: BTreeSet<(String, String)> = UNESCAPED_CALL_SITES
        .iter()
        .map(|(path, door)| ((*path).to_string(), (*door).to_string()))
        .collect();
    assert_eq!(
        found, listed,
        "the places the library writes unescaped bytes have changed; every one of them is a \
         deliberate act and belongs in UNESCAPED_CALL_SITES with the reason"
    );
}

#[test]
fn a_tag_body_is_children_or_markup_and_never_both() {
    // The payload has one body, so a node either carries a content slot or a
    // piece of markup: there is no shape in which the emitter would have to
    // decide where markup stands among children.
    let children = HtmlTagPayload::new("div").block();
    assert_eq!(children.body, TagBody::Children);
    let markup = HtmlTagPayload::new("div").markup(Markup::trusted("<svg></svg>"));
    assert_eq!(markup.body, TagBody::Markup(Markup::trusted("<svg></svg>")));
    // Asking for markup after children replaces the body rather than adding to it.
    let replaced = HtmlTagPayload::new("div").markup(Markup::trusted("<b>x</b>"));
    assert!(matches!(replaced.body, TagBody::Markup(_)));
}
