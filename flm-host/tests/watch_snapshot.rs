//! The document snapshot of the live preview (the module `watch::snapshot`): the headings, the
//! labels and the constructs at a place, over real builds of documents held in memory. It
//! performs no input or output, so these tests pass with the crate's default features off.

use std::sync::Arc;

use flm_core::lang::mint::CallableKind;
use flm_core::progress::Scope;
use flm_host::files::MemoryFiles;
use flm_host::output::{MemorySink, Naming, Sink};
use flm_host::session::{Input, Request, build, prepare};
use flm_host::watch::build_settings;
use flm_host::watch::snapshot::{Heading, LabelEntry, Processed};

/// Builds `doc/paper.flm` of `files` as the preview does (through its errors, with the
/// settings the loop adds to every build) and gives what its run knows of the document.
fn process(files: MemoryFiles) -> Processed {
    let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
    request.default_math = flm_host::seed::MathChoice::processor("passthrough");
    request.naming = Naming::File { name: String::from("paper.html") };
    request.sink = Some(Arc::new(MemorySink::new()) as Arc<dyn Sink>);
    request.continue_with_errors = true;
    request.options.extend(build_settings());
    let prepared = prepare(request, Arc::new(files), &[], &Scope::none()).expect("prepared");
    let built = build(&prepared, &Scope::none()).expect("built");
    Processed::of_run(&built.run)
}

/// The kind and the name of each construct at the byte `offset` of `file`, from the
/// innermost outward.
fn at(processed: &Processed, file: &str, offset: usize) -> Vec<(CallableKind, String)> {
    let at = processed.at(file, offset).expect("a place of the file");
    at.constructs.iter().map(|construct| (construct.kind, construct.name.clone())).collect()
}

const OUTLINED: &str = "---\nflm:\n  features:\n    headings: {numbering: all-numbered}\n---\n\
                        \\section{Introduction}\\label{sec:intro}\n\nSee \\ref{sec:intro}.\n\n\
                        \\subsection*{Aside}\n\nWords.\n\n\\section{Results}\n\nMore.\n";

#[test]
fn the_outline_lists_the_headings_in_document_order_with_their_numbers_and_identifiers() {
    let processed = process(MemoryFiles::new().with("doc/paper.flm", OUTLINED));
    let headings: Vec<(u32, Option<&str>, &str)> =
        processed.headings().iter().map(|heading| (heading.level, heading.number.as_deref(), heading.title.as_str())).collect();
    assert_eq!(headings, [(1, Some("1"), "Introduction"), (2, None, "Aside"), (1, Some("2"), "Results")]);
    let Heading { id, .. } = &processed.headings()[0];
    assert_eq!(id.as_deref(), Some("sec-intro"));
    assert!(processed.headings().iter().all(|heading| heading.id.is_some()), "{:?}", processed.headings());
}

#[test]
fn a_label_designates_what_a_reference_to_it_shows_and_names_the_element() {
    let processed = process(MemoryFiles::new().with("doc/paper.flm", OUTLINED));
    let LabelEntry { label, designation, id, .. } = &processed.labels()[0];
    assert_eq!((label.as_str(), id.as_deref()), ("sec:intro", Some("sec-intro")));
    // A numbered section shows its word and its number.
    assert!(designation.ends_with('1') && designation.len() > 1, "{designation}");
    assert_eq!(processed.references().get("sec-intro").map(String::as_str), Some("sec:intro"));

    let unnumbered = process(MemoryFiles::new().with("doc/paper.flm", "\\section{Introduction}\\label{sec:intro}\n\nHello.\n"));
    assert_eq!(unnumbered.labels()[0].designation, "Introduction");
    assert_eq!(unnumbered.headings()[0].number, None);
}

#[test]
fn a_label_gives_the_kind_of_its_point_and_the_heading_it_stands_under() {
    let text = "\\begin{equation}x\\label{eq:first}\\end{equation}\n\n\
                \\section{Introduction}\\label{sec:intro}\n\n\
                \\begin{defterm}{Entropy}\\aliasterm{Entropies}\\label{term:entropy}\nA measure.\n\\end{defterm}\n\n\
                \\subsection{Aside}\n\n\\begin{equation}y\\label{eq:second}\\end{equation}\n";
    let processed = process(MemoryFiles::new().with("doc/paper.flm", text));
    let labels: Vec<(&str, Option<&str>, Option<usize>)> = processed
        .labels()
        .iter()
        .map(|entry| (entry.label.as_str(), entry.kind.as_deref(), entry.heading))
        .collect();
    assert_eq!(
        labels,
        [
            // Before the first heading.
            ("eq:first", Some("equation"), None),
            // A heading's own label stands under that heading.
            ("sec:intro", Some("section"), Some(0)),
            // A term's labels, computed and written, each stand on their own.
            ("defterm:Entropies", Some("defterm"), Some(0)),
            ("defterm:Entropy", Some("defterm"), Some(0)),
            ("term:entropy", Some("defterm"), Some(0)),
            ("eq:second", Some("equation"), Some(1)),
        ]
    );
}

#[test]
fn the_constructs_at_a_place_go_from_the_innermost_outward() {
    let text = "Hello \\emph{a \\textbf{bold} word}.\n";
    let processed = process(MemoryFiles::new().with("doc/paper.flm", text));
    let offset = text.find("bold").unwrap();
    assert_eq!(
        at(&processed, "doc/paper.flm", offset),
        [(CallableKind::Macro, String::from("textbf")), (CallableKind::Macro, String::from("emph"))]
    );
    let place = processed.at("doc/paper.flm", offset).unwrap();
    assert_eq!((place.line, place.column), (1, offset + 1));
    let textbf = &place.constructs[0];
    assert_eq!(&text[textbf.start..textbf.end], "\\textbf{bold}");
    assert!(textbf.feature.is_some(), "{textbf:?}");
    // Outside every construct: the line and the column alone.
    assert_eq!(at(&processed, "doc/paper.flm", 2), []);
    assert_eq!(
        processed.at("doc/paper.flm", text.len() + 1).unwrap_err().to_string(),
        format!("Byte offset {} past the end of ‘doc/paper.flm’", text.len() + 1)
    );
    assert_eq!(
        processed.at("doc/other.flm", 0).unwrap_err().to_string(),
        "Missing source file ‘doc/other.flm’ in the document shown"
    );
}

#[test]
fn the_constructs_of_an_included_file_are_found_in_that_file() {
    let main = "Main \\input{part.flm}\n";
    let part = "Part \\emph{inside}.\n";
    let processed = process(MemoryFiles::new().with("doc/paper.flm", main).with("doc/part.flm", part));
    assert_eq!(at(&processed, "doc/part.flm", part.find("inside").unwrap()), [(CallableKind::Macro, String::from("emph"))]);
    let place = processed.at("doc/part.flm", part.find("inside").unwrap()).unwrap();
    assert_eq!((place.line, place.column), (1, part.find("inside").unwrap() + 1));
    assert!(processed.at("doc/paper.flm", main.find("part").unwrap()).is_ok());
}

#[test]
fn a_place_in_the_content_of_a_user_definition_answers_with_its_use() {
    let front = "---\nflm:\n  features:\n    userdefs:\n      macros:\n        \
                 see: {content: 'see #1', args: ['{']}\n---\n";
    // The offsets of the constructs count in the content below the front matter.
    let body = "Then \\see{\\emph{x}} here.\n";
    let processed = process(MemoryFiles::new().with("doc/paper.flm", &format!("{front}{body}")));
    // The page writes the place of the use for the content the definition makes.
    let use_start = body.find("\\see").unwrap();
    let place = processed.at("doc/paper.flm", use_start).unwrap();
    let used = &place.constructs[0];
    assert_eq!((used.kind, used.name.as_str()), (CallableKind::Macro, "see"));
    assert_eq!(&body[used.start..used.end], "\\see{\\emph{x}}");
    assert!(used.feature.is_some(), "{used:?}");
    assert_eq!(place.constructs.len(), 1, "{:?}", place.constructs);
    // Inside the argument: the construct written there, then the use.
    assert_eq!(
        at(&processed, "doc/paper.flm", body.find('x').unwrap()),
        [(CallableKind::Macro, String::from("emph")), (CallableKind::Macro, String::from("see"))]
    );
}

#[test]
fn a_command_of_a_scoped_package_names_its_feature() {
    let text = "\\begin{itemize}\n\\item One.\n\\end{itemize}\n";
    let processed = process(MemoryFiles::new().with("doc/paper.flm", text));
    let place = processed.at("doc/paper.flm", text.find("\\item").unwrap()).unwrap();
    let named: Vec<(&str, Option<&str>)> =
        place.constructs.iter().map(|construct| (construct.name.as_str(), construct.feature.as_deref())).collect();
    assert_eq!(named, [("item", Some("lists")), ("itemize", Some("lists"))]);
}
