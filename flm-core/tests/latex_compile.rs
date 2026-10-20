//! Compiles every LaTeX golden with `pdflatex` and `lualatex` and checks what
//! came out.
//!
//! A **golden** under `tests/goldens/latex/` (the backend's own suite, one
//! property per document) or `tests/goldens/spine/` (the two whole-pipeline
//! documents of `spine_latex.rs`) is one document the `latex-recomposed`
//! backend produced: the recomposed LaTeX, and — where the document needed one —
//! the style file, each under a marker line. This suite writes each golden
//! back out as the files LaTeX expects (the style file under its own name,
//! the recomposed LaTeX inside a minimal `article` document that loads it), runs
//! the engine twice, and asserts that the compilation succeeded and that the
//! log holds no undefined reference, no multiply-defined label and no lost
//! label. Every golden is compiled twice over: once plain and once with
//! `hyperref` loaded, since `hyperref` changes how labels and anchors are
//! made.
//!
//! **Every test runs once under each engine of [`ENGINES`]** — `pdflatex` and
//! `lualatex` — so that what the style file does is measured on an 8-bit engine
//! and on a Unicode one alike.
//!
//! **It is skipped unless the environment variable `FLM_TESTS_LATEX_COMPILE` is
//! set** (to anything but the empty string or `0`), with a printed message, so
//! that a plain `cargo test` stays fast and the library's own build needs no
//! TeX. Where the variable is set, an engine that cannot be run from the
//! `PATH` is a failure, not a skip. On a machine with a TeX installation, run
//! it with
//!
//! ```text
//! FLM_TESTS_LATEX_COMPILE=1 cargo test -p flm-core --test latex_compile
//! ```
//!
//! (On TeX Live 2025 on an Apple-silicon laptop, the suite takes about three
//! minutes under `pdflatex` alone and about nine under both engines.)
//!
//! The floor is a LaTeX system of 2022 or later with the packages the goldens
//! request — `amsmath`, `amssymb`, `zref-clever`, `fvextra` 1.5 or later,
//! `newunicodechar`, `dsfont`, `bbm`, `nicefrac`, `mathrsfs`, `tipa`, `tipx`,
//! `textcase`, `enumitem`, `amsthm`, `phfthm`, `phfcc`, `graphicx`, `adjustbox`, `caption`,
//! `tabularray`, `xcolor` — the Cyrillic font encodings `T1`, `T2A`, `T2B`, `T2C`, `X2`,
//! `OT2` and `T2D` with their fonts (the `lh` and `ucs` collections), and the
//! font families the style file declares math alphabets and symbol fonts from
//! without loading their packages: `bbold`, `dutchcal`, `wasy` and the STIX
//! families. `textcomp` is not among them: its symbols have been part of the
//! LaTeX kernel since 2020, so nothing requests it.
//!
//! # A new form of recomposed user definition is proved inside a heading
//!
//! A section title is a *moving argument*: LaTeX writes it to the table of
//! contents and to the auxiliary file and reads it back later. Every form a
//! user definition is recomposed as — written into the style file as one
//! LaTeX definition — has to work there, so this suite carries one golden
//! that uses a recomposed definition inside a heading title
//! (`userdefs-in-heading.tex`) and probes over its style file: a table of
//! contents, a caption and a list of figures, `\MakeUppercase` and `\edef`,
//! and the PDF bookmark `hyperref` builds. The same document written under the
//! render option `robust_names` (`userdefs-robust.tex`) is probed in the same
//! three places, where the answers are the opposite ones — the call in the
//! table of contents, the definition dropped from the bookmark with
//! `hyperref`'s warning, the definition's own words left alone by the case
//! change — so that the trade the option is stands measured on both sides.
//!
//! **Whoever adds a new form** — a
//! star argument, a flag for a question the template asks, the arguments of a
//! definition that takes more than nine handed over in one group, a definition
//! written for two parsing modes — **adds one golden that uses that form
//! inside a heading**, so that the form is proved in a moving argument as it
//! lands rather than once at the end. A form only an *environment* can take —
//! one that takes its body as an argument — cannot be used in a title at all,
//! an environment not being something a title can hold; the golden for such a
//! form puts a recomposed macro in the environment's argument and the same
//! macro in a heading, so that what travels into the table of contents is
//! proved all the same (`userdefs-body-argument.tex`).
//!
//! # The packages and the preamble block the author asks for
//!
//! `author-preamble.tex` is the golden of a document whose author asked,
//! through the render options, for a package loaded with an option
//! (`\RequirePackage[fleqn]{amsmath}`) and for all three pieces of their own
//! LaTeX. Compiling it proves the whole path at once: the request reaches the
//! package block with its option; the first piece stands above that block, so
//! its `\PassOptionsToPackage{leqno}{amsmath}` still reaches the package the
//! block requests; the second stands below the block and above every
//! definition the library makes, so its `\ProvideDocumentCommand\flmRef` is
//! the reference hook the document uses and the family's own guarded
//! definition finds the name taken; the third reaches the end of the file —
//! after everything the library itself put there, so it may use any of it —
//! and defines the command the document's own mathematics names, which
//! nothing in this library wrote.
//!
//! # The default font encoding of a merged `fontenc` request
//!
//! The style file requests `fontenc` once per engine, with every encoding its
//! characters need under that engine — beside `T1` under pdfLaTeX and beside
//! `TU` under LuaLaTeX and XeLaTeX, inside a test of the engine — and
//! `fontenc` makes the last encoding of its option list the one the
//! document's text is set in. So the file writes, right after the package
//! requests and in a test of the engine of its own, an explicit command naming
//! each engine's own encoding the default, and
//! `the_default_encoding_command_sets_latin_text_in_each_engine_s_own_encoding_after_a_merged_fontenc`
//! measures what that command does: it compiles the two goldens whose block
//! merges several `fontenc` requests and reads, out of the log, the encoding
//! LaTeX had selected where the body was set and the font file it picked —
//! `T1` with `ecrm1000` under `pdflatex`, `TU` with Latin Modern's Unicode
//! roman `lmroman10-regular` under `lualatex`. The Cyrillic text is
//! unaffected, each letter carrying its own switch to its encoding.
//!
//! # The hook of a reference showing the author's own words
//!
//! `\flmHyperref{<key>}{<text>}` shows `<text>` as a link to the place
//! `<key>` names, and making a link in a PDF takes the `hyperref` package.
//! Whether a document is compiled with that package is its author's decision,
//! so the hook's default asks whether the *document* loaded it: it links
//! through the package where it did and prints the text alone where it did
//! not. Both answers are measured over the `refs-hyperref` golden's style
//! file, by a reference to a key nothing labels — `hyperref` reports one it
//! cannot resolve, and that report exists exactly where the link was made —
//! and the text such a reference contributes to a PDF bookmark is measured
//! beside it.
//!
//! # The heading hook's key-value argument
//!
//! `\flmHeading` takes a key-value argument of its own with two keys: `toc`,
//! which the recomposed LaTeX writes on every heading, and `alt`, an alternative
//! title for the table of contents and the PDF bookmark, which the recomposed
//! LaTeX writes where a heading's options give `alt=`; one test compiles the
//! golden `headings-alt` that holds it. The other tests at the end of this file
//! write the argument by hand in a
//! document of their own over the `headings` golden's style file. One pins the
//! auxiliary file of a heading written *without* the argument against the bytes
//! measured before the argument existed; the next pins the golden as the
//! backend writes it, which is those bytes and one more table-of-contents line
//! for its starred heading; two measure what `toc` does, plain and under
//! `hyperref`. One of them loads two style files in one job, which is what a
//! document holding two recomposed documents does.
//!
//! # A reference to a labeled footnote
//!
//! `xnotes.tex` is the golden of a document holding three footnotes, the
//! third labeled — `\flmXNfootnote[{\flmLabels{\label{ref1}}}]{…}` — and a
//! reference to it. The style file's default definition of `\flmXNfootnote`
//! hands the labels to `\footnote` first inside its argument, where LaTeX
//! makes a label refer to the footnote. The test at the end of this file reads
//! the label's record out of the auxiliary file: it names the counter
//! `footnote` and the value 3, which is what `\zcref` prints ("footnote 3").
//!
//! # A footnote in what a reference prints
//!
//! `xnotes-in-display-text.tex` holds an unnumbered float whose caption holds
//! a footnote and a definition whose term holds one, each referred to twice,
//! a second unnumbered float whose caption's alternative text leaves its
//! footnote out, referred to twice, then a last footnote. What a reference
//! prints is the display text of `\flmLabelsWithDisplayText`, the caption or
//! the term as written: LaTeX sets the footnote again at each reference to the
//! first two, and prints the alternative text alone for the third.
//!
//! # A nested footnote
//!
//! `xnotes-nested.tex` holds a footnote inside a footnote, the inner one
//! labeled, and a reference to that label. Under the style file's default
//! `\flmXNfootnote`, which hands a note to LaTeX's own `\footnote`, LaTeX
//! leaves the inner footnote unset: its text and its label never reach the
//! page, under either engine, with or without `hyperref`, and the reference is
//! undefined. No short definition of `\flmXNfootnote` from LaTeX's public
//! footnote commands sets nested notes, so the author supplies one where the
//! PDF needs them. The golden is compiled by its own test, which pins that.
//!
//! # The hyperlink hooks
//!
//! `href.tex` holds each of the three hyperlink commands written as its hook —
//! `\flmHref`, `\flmUrl`, `\flmEmail` — an address holding `%`, `#`, `~`, `_`
//! and braces, and a `\flmUrl` in a heading's title; `href-userdefs-kept.tex`,
//! `href-userdefs-verbatim.tex` and `href-kept-display.tex` hold the use sites
//! of user definitions whose template puts a placeholder into an address, each
//! expanded. The style file requests `hyperref`, which
//! defines `\href`, so the goldens compile with and without the author's own
//! `\usepackage{hyperref}` like every golden. The test at the end of this file
//! compiles `href.tex` behind a table of contents: the hook is robust, so it
//! travels into the table of contents as its call, and it contributes its
//! display alone to the PDF bookmark, with no complaint in the log.
//!
//! `\flmUrl` and `\flmEmail` take the address alone and derive the display in
//! LaTeX, through the definition piece's internal helper `\flm@href@display`.
//! One test runs the helper over a range of addresses — each prefix, the
//! trailing run of `/`, `#` and `?`, `%`, `#`, `\`, `~`, `_`, braces, no scheme
//! — writing each result to the log with `\typeout`, and compares it with the
//! `href` feature's own display function; it also compiles each address's hook
//! and reads what the hook hands `\nolinkurl`. Another compiles
//! `href-kept-display.tex` and reads the same of its two hooks, `x.org` and
//! `example.com/a`.
//!
//! `href-skip-hyperref.tex` is the same document with `hyperref` named in the
//! render option `skip_packages`: its own test compiles it with
//! `\usepackage[colorlinks]{hyperref}` after the style file, which links and
//! fills the bookmark as the request would have, and pins that the same line
//! after `href.tex`'s style file, whose request loaded `hyperref` without
//! those options, stops on LaTeX's option clash. And one test reads the link
//! targets out of the PDF of `href.tex`, `href-skip-hyperref.tex`,
//! `href-userdefs-kept.tex`, `href-userdefs-verbatim.tex` and
//! `href-kept-display.tex`, compiled with
//! the PDF's objects uncompressed so that each target stands in the file as
//! plain text: every target is exactly the address written, which pins the
//! escaping of an address (`\#`, `\%`, `\\`) under both engines.
//!
//! # Defined terms
//!
//! `defterm-definition.tex` holds a definition written back as the
//! environment `defterm`, its body opening with `\phantomsection` and
//! `\flmLabelsWithDisplayText{Entropy}{…}` over four labels — the one computed
//! from the term, the alias's, a `topic:` and a `term:` label — and a `\ref`
//! to each written label; its style file defines the environment from the
//! lead-in. The test at the end of this file reads each label's record out of
//! the auxiliary file: the text a reference prints is the term, under the
//! reference type whose names are empty, so `\zcref` to a `topic:` label
//! prints "Entropy".
//!
//! `defterm-terms.tex` holds term links written as `\flmTerm{<key>}{<shown>}`,
//! one in a heading's title, and a term inside its own definition written as
//! `\flmTermInDefterm{<shown>}`; `defterm-userdefs-kept.tex` a user
//! definition kept as a LaTeX definition holding a term link with a fixed
//! term, and the use sites of three definitions whose term link holds a
//! placeholder, each expanded and linked under its definition's key. The test at the end of this file compiles
//! `defterm-terms.tex` behind a table of contents with the PDF's objects
//! uncompressed and counts the links to each definition's anchor: every
//! `\flmTerm`, and none for `\flmTermInDefterm`; the hook in the title reaches
//! the table of contents as its call and the bookmark as its text.
//!
//! `defterm-unresolved.tex` holds references that resolve to nothing — a
//! term, a term holding mathematics, an `\eqref` — each written under a key
//! allocated for its target; its own test compiles it and finds the
//! undefined references LaTeX warns of, and no error.
//!
//! # Theorems
//!
//! `theorems-authored.tex` and `theorems-resolved.tex` hold every kind of the
//! default configuration, a labeled theorem, a proof showing its
//! back-reference and one recording it only, `\noproofref`, a written title
//! on a proof and references to three statements, recomposed in the two forms
//! of a proof's back-reference: as the author wrote it, over the `phfthm`
//! package, and resolved into the proof's heading, over `amsthm` alone. Their
//! style files declare the kinds the documents use, each guarded on its
//! name. The tests at the end of this file compile each golden as it stands,
//! behind the author's own `\usepackage{amsthm}` and
//! `\newtheorem{theorem}{Theorem}`, which the guard lets stand, and, the
//! authored one, behind the author's own `\usepackage[thmset=empty]{phfthm}`,
//! the option the style file requests the package with — with a `\ref` and
//! a `\zcref` to the theorem printing its number; the author's own
//! `\usepackage{phfthm}` without that option stops the compilation with
//! LaTeX's option clash, as a second request of most packages with an option
//! their first request lacked does. They read the text of
//! the PDF, through the `pdftotext` program, for the headings of the proofs:
//! the one showing its back-reference, "Proof of Theorem 1" through `phfthm`,
//! which names the statement through `hyperref`'s `\autoref` where the
//! document loads `hyperref`, and "Proof (theorem 1)" through the proof title
//! hook `\flmProofTitle` around `\flmRef`, which prints what `\zcref`
//! prints; the one with a written title, "Proof (Sketch)" through the hook
//! in the resolved form, and "Proof of Sketch" in the authored form, whose
//! hook's default is the title alone, which `phfthm` sets after its own
//! "Proof of"; and, with the hook redefined by the author in the resolved
//! form, the title alone. `theorems-host-authored.tex` and
//! `theorems-host-resolved.tex` hold a host's kind counting in a counter of
//! its own beside a lemma; `theorems-own-authored.tex` and
//! `theorems-own-resolved.tex` hold the kind `theorem` counting on its own
//! beside a lemma and a proposition sharing the counter. A test reads the
//! text of the references to the kinds of all six, each named with its
//! kind's words — every kind declares a reference type of its own for
//! `zref-clever` — and numbered in its kind's counter.
//! `theorems-e2e-variants-authored.tex` and `theorems-e2e-variants-resolved.tex`
//! reference a lemma in the wordings its kind declares and a host's kind in
//! a variant of the host's own naming; a test reads what each reference
//! prints — the reference hook's default translating the shipped names into
//! the package's keys and dropping the host's.
//!
//! # Quotations
//!
//! `quote-tests-1-3.tex` holds a quotation of each shape: a passage and its
//! attribution, the four sections in one quotation — lines with an indented
//! one, a block of two paragraphs — a `blockquote` of two paragraphs, a block
//! of one line and an `address` of three lines, the second indented; its style
//! file provides the four section hooks, `\flmIndent` and the environments
//! `blockquote` and `address`, and declares the length `\flmTabWidth` guarded
//! on its name, while `quote` is LaTeX's own. A test reads the text of
//! the PDF for every section, the attribution after its dash.
//! `quote-lines-edges.tex` holds lines a bare `\\` would read as its own —
//! one beginning with `[`, one with `*`, a `[` after a comment — and an empty
//! first line, in a lines section and opening an `address`: a test reads each
//! line back from the PDF, the `[` and the `*` printed as written.
//! `quote-myquote.tex` holds a quotation of a host's environment, `myquote`,
//! which the style file leaves to the author: its test compiles it behind the
//! author's own `\newenvironment{myquote}` and an attribution hook the
//! author defined first, which the style file's `\ProvideDocumentCommand`
//! lets stand, and reads the author's attribution in the PDF.
//!
//! # Annotations
//!
//! `annotations.tex` holds every form of an annotator's command — a
//! highlight in braces and closed by its command, a comment in brackets —
//! each written back as the author wrote it, a comment and a highlight
//! crossing a paragraph break as one invocation, a list inside a passage,
//! and annotations in a heading's title, a list item and a footnote, over
//! the `phfcc` package, whose commands the style file makes, each guarded on
//! its name; `annotations-hidden.tex` is the same document hidden, which
//! needs no package. The test at the end of this file compiles the first
//! golden behind nothing, and behind the author's own `phfcc` making `\phf`
//! with initials of their own, which the guard lets stand: the PDF's text
//! then holds the author's initials and not the configured ones.
//!
//! # Images
//!
//! `graphics.tex` holds the image hook `\flmIncludeGraphics` bare, with a
//! width in centimeters, with a width of `10pt` written `10bp`, and with a
//! height, over the packages `graphicx` and `adjustbox` (with `export`), whose
//! key `max width` the hook's default gives. No image file is committed: every
//! compilation writes a picture of one pixel at the plain relative path of each
//! hook of its document ([`write_images`]).
//!
//! # Copies of shown content
//!
//! `copies-e2e.tex` is the end-to-end document of copies of shown content:
//! unnumbered headings, an unnumbered float, a defined term and a list item
//! whose words hold a footnote, each referred to, one footnote labeled and
//! referred to; a note of a configured kind; an image in a heading's title;
//! and references to a label nothing carries, in a heading's title and in a
//! user definition's template, on purpose. Its own test compiles it and finds
//! exactly those references undefined, the labeled footnote's reference
//! resolved, and no error.
//!
//! # Tables of cells
//!
//! `cells-simple.tex`, `cells-tokens.tex`, `cells-content.tex`,
//! `cells-float.tex`, `cells-wide.tex`, `cells-marks.tex` and
//! `cells-host-styles.tex` hold tables of cells, each one call of the hook
//! `\flmCellsTable` over `tabularray`: a header row, every style token, a cell
//! holding a formula, a reference, a footnote, two paragraphs and a nested
//! table, a table in a `table` float with a caption and references to it, a
//! table wider than the line, which the hook sets again with the fitted
//! columns, the comment lines of what the reader left out, and a host's
//! custom styles. Their tests read the PDF's text: the float's caption and
//! the references to it, the two paragraphs of one cell, the wide table's
//! text broken into lines; a cell's table commands composing with its entry
//! and with one another, the later of two setting `cmd=` winning, and an
//! author's keys defined before the style file is loaded standing; and the
//! author's own `\flmCellsTable`, defined before the style file is loaded,
//! setting the tables its own way. The two end-to-end documents of the tables
//! of cells, `cells-e2e-global.tex` and `cells-e2e-floats.tex`, compile as
//! every golden does, and a test of their own reads their floats' captions and
//! the references to them in the PDF.
//!
//! # Citations
//!
//! `citations-chunks.tex`, `citations-refcite.tex`, `citations-heading.tex`
//! and the end-to-end document `citations-e2e.tex` hold citations written
//! through the hooks of the `citations` feature: a
//! chunk with notes, keys holding `_` and `:`, a `\cite` written with and
//! without a space before it and one inside a kept definition; the `\R` forms;
//! a citation in a heading's title. The list of the cited works is LaTeX's own
//! and none of them defines it, so each is compiled by a test of its own that
//! appends a `thebibliography` environment defining the keys
//! ([`compile_with_bibliography`]), plain and with `hyperref`, and reads the
//! PDF's text: `text [1]` for both spellings, the words of `\R`, and, where the
//! page style sets the heading in the running head in capitals, the keys kept
//! as written and the note uppercased.
//!
//! # A style file named through the naming service
//!
//! One test does not read a golden: it runs the backend with a naming service
//! of the tests' own, which names the style file `job-flmrecomposed.<hash>.sty`,
//! writes the file under that name beside the document, and compiles it, so that
//! the package name the recomposed LaTeX loads is the file's.
//!
//! # The goldens that are not compiled
//!
//! [`NOT_COMPILED`] names them, each with its reason. Every one of them is a
//! document whose *input* is deliberately broken or whose options deliberately
//! leave something out, so that the recomposed LaTeX is expected not to
//! compile; it is still what the backend must write, which is what the golden
//! pins. Everything else compiles.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use std::sync::Arc;

use flm_core::core::artifact::ArtifactNamer;
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::stdcore::backends::latex::escape_url;
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::backends::lowering::NAMER_SLOT;
use flm_core::stdcore::features::href::display;
use flm_core::stdcore::{StdcoreConfig, package};

mod common;

use common::latex;
use common::naming::JobNamer;
use common::stdcore::run_workflow;

/// Distinguishes the working directories of two compilations in one process.
/// Together with the process id it makes each one unique: libtest runs the
/// tests of this file on parallel threads, two of them compile the same
/// golden, and `compile` opens by deleting the directory it is about to use —
/// so a shared path would let one run delete another's files. Two batteries
/// running side by side (a checkout and a worktree) need the process id for
/// the same reason.
static NEXT_COMPILE: AtomicUsize = AtomicUsize::new(0);

/// The goldens this suite does not compile, each with the reason. The
/// recomposed LaTeX of each is correct — it is the document, the options or the
/// input that make the result uncompilable — so the golden still pins the
/// bytes and only the compilation is skipped.
const NOT_COMPILED: &[(&str, &str)] = &[
    (
        "unknown",
        "the document holds a macro no feature attributes, recomposed as written beside the \
         error, so LaTeX meets an undefined control sequence",
    ),
    (
        "skip-packages",
        "the render option leaves out the packages the recomposed LaTeX needs, which is what the \
         option is for",
    ),
    (
        "math-inline-display",
        "the document's inline mathematics holds an alignment tab, which is a LaTeX error \
         wherever it is written",
    ),
    (
        "math-surround",
        "the `math` option surrounds every environment with text the test chose \
         (`\\mathbefore`), which nothing defines",
    ),
    (
        "text-declare",
        "the document holds a character no spelling is known for, reported and written \
         through, which LaTeX cannot set",
    ),
    (
        "defterm-unresolved",
        "the document's references name nothing on purpose, so the undefined-reference check \
         would reject the run; its own test compiles it and expects exactly that warning",
    ),
    (
        "refs-default",
        "the test document references `nope`, which nothing in it labels, so the \
         undefined-reference check would reject the run",
    ),
    (
        "refs-raw-types",
        "the same reference to `nope`. The raw label type writes `sec:a`, which the \
         document does define — that is not a reason",
    ),
    (
        "refs-protect",
        "the same reference to `nope`",
    ),
    (
        "features-recomposed",
        "the spine's features document references `sec:missing`, which nothing labels, on \
         purpose, so the undefined-reference check rejects the run",
    ),
    (
        "xnotes-nested",
        "the document references the label of a footnote inside a footnote, which LaTeX's \
         `\\footnote`, the default `\\flmXNfootnote` hands a note to, leaves unset, so the \
         reference is undefined; compiled by its own test, which expects exactly that",
    ),
    (
        "quote-annotations-e2e-reported",
        "the document holds the cases the quotations and annotations features report on \
         purpose: two passages of `\\phf` left open, which `phfcc` reads on to a closing \
         `\\endphf` that never comes, and a paragraph break inside a heading's title",
    ),
    (
        "xnotes-e2e-reported-in-place",
        "the document holds a footnote of two paragraphs in a heading's title and in \
         `\\text{…}`, which FLM reports (`flm.blocks.block-in-inline`) and the recomposed \
         LaTeX writes as the document wrote it, a paragraph break inside an argument LaTeX \
         reads as a short one (`Paragraph ended before \\flmHeading was complete`); and it \
         references `fn:nowhere`, which nothing labels, on purpose",
    ),
    (
        "xnotes-e2e-reported-default",
        "the same document under the default configuration, where the footnote's list entry \
         holds the two paragraphs and FLM reports nothing about them, while LaTeX stops on \
         the same paragraph break inside the heading's argument; and the same reference to \
         `fn:nowhere`",
    ),
    (
        "theorems-e2e-reported-authored",
        "a proof's back-reference names a label nothing carries, on purpose, so the \
         undefined-reference check would reject the run",
    ),
    (
        "theorems-e2e-reported-resolved",
        "the same back-reference to nothing, written through `\\flmRef`",
    ),
    (
        "links-e2e-reported",
        "the document defines a term inside a heading's title, which FLM reports \
         (`flm.blocks.block-in-inline`) and the recomposed LaTeX writes as the document wrote \
         it, an environment inside the heading's argument; and its term links name terms \
         nothing defines, on purpose, so the undefined-reference check would reject the run",
    ),
    (
        "userdefs-refs-placeholder-unresolved",
        "the use sites of the document complete reference targets that name nothing, on \
         purpose, so the undefined-reference check would reject the run; its own test compiles \
         it and expects exactly that warning",
    ),
    (
        "userdefs-refs-placeholder-verbatim",
        "the same: targets a verbatim argument completes, `x%y` and `u\\v` among them, that \
         name nothing; compiled by the same test",
    ),
    (
        "userdefs-refs-placeholder-terms",
        "one of its term links names a term nothing defines, on purpose; compiled by the same \
         test, which reads the one that resolves out of the auxiliary file",
    ),
    (
        "quote-myquote",
        "the environment `myquote` is a host's, which the style file leaves to the author's \
         LaTeX; compiled by its own test behind the author's `\\newenvironment`",
    ),
    (
        "floats-configured",
        "the environment `diagram` is a configured float type's, which the style file leaves to \
         the author's LaTeX; compiled by its own test behind the author's float environment",
    ),
    (
        "graphics-floats-e2e-shown",
        "the environments `diagram` and `sketch` are configured float types', which the style \
         file leaves to the author's LaTeX; compiled by its own test behind the author's float \
         environments",
    ),
    (
        "graphics-floats-e2e-reported",
        "the document holds a figure with two captions and an image with no path, on purpose; \
         what the backend writes for them is pinned, not compiled",
    ),
    (
        "copies-e2e",
        "the document refers to `sec:nowhere`, which nothing labels, on purpose, in a heading's \
         title and in a user definition's template, so the undefined-reference check would \
         reject the run; compiled by its own test, which expects exactly those references \
         undefined",
    ),
    (
        "citations-chunks",
        "the document cites keys no bibliography defines, the list being LaTeX's own, so the \
         undefined-citation check would reject the run; compiled by its own test with a \
         `thebibliography` appended",
    ),
    (
        "citations-refcite",
        "the document cites keys no bibliography defines; compiled by its own test with a \
         `thebibliography` appended",
    ),
    (
        "citations-heading",
        "the document cites a key no bibliography defines; compiled by its own test with a \
         `thebibliography` appended",
    ),
    (
        "citations-e2e",
        "the document cites keys no bibliography defines; compiled by its own test with a \
         `thebibliography` appended",
    ),
    (
        "href-skip-hyperref",
        "the render option `skip_packages` leaves out `hyperref`, which the hyperlink hooks \
         link through, because the document loads it itself; compiled by its own test with \
         `hyperref` loaded, with options, after the style file",
    ),
];

/// The marker line the recomposed LaTeX follows.
const DOCUMENT_MARKER: &str = "% ===== latex-recomposed:document =====";
/// The prefix of the marker line the style file follows; the rest of the
/// line is the style file's name.
const STYLE_MARKER: &str = "% ===== latex-recomposed:style ";

/// The directory the backend's own goldens live in.
fn goldens() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/latex")
}

/// Every directory holding LaTeX goldens: the backend's own, and the spine's,
/// whose two recomposed outputs (`*-recomposed.tex`) are the whole-pipeline ones.
fn golden_directories() -> [PathBuf; 2] {
    [goldens(), Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/spine")]
}

/// The environment variable that turns this suite on; see the module
/// documentation.
const ENABLE_VARIABLE: &str = "FLM_TESTS_LATEX_COMPILE";

/// The engines every test of this suite runs under, in turn. Each is run with
/// the same arguments and writes the same `doc.log`, `doc.aux` and so on.
const ENGINES: &[&str] = &["pdflatex", "lualatex"];

/// Whether [`ENABLE_VARIABLE`] turns the suite on: set, and neither empty nor
/// `0`. Where it does, every engine of [`ENGINES`] must run — a missing one is
/// a failure, not a skip, since whoever set the variable asked for the checks.
fn enabled() -> bool {
    let value = std::env::var(ENABLE_VARIABLE).unwrap_or_default();
    if matches!(value.trim(), "" | "0") {
        return false;
    }
    for engine in ENGINES {
        let runs =
            Command::new(engine).arg("--version").output().is_ok_and(|out| out.status.success());
        assert!(runs, "{ENABLE_VARIABLE} is set, but `{engine}` cannot be run from the PATH");
    }
    true
}

thread_local! {
    /// The engine [`compile_document`] runs. Every test runs on a thread of
    /// its own, and [`under_each_engine`] sets this before each pass.
    static ENGINE: std::cell::Cell<&'static str> = const { std::cell::Cell::new("pdflatex") };
}

/// Prints the engine a failing pass ran under, since the assertions of the
/// tests do not name it.
struct EngineReport(&'static str);

impl Drop for EngineReport {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("(the failure above happened under `{}`)", self.0);
        }
    }
}

/// Runs `test` once under each engine of [`ENGINES`], or skips it with a
/// printed message where [`ENABLE_VARIABLE`] does not turn the suite on.
fn under_each_engine(test: fn()) {
    if !enabled() {
        println!("skipped: set {ENABLE_VARIABLE} to run the LaTeX compilation checks");
        return;
    }
    for engine in ENGINES {
        ENGINE.set(engine);
        let _report = EngineReport(engine);
        test();
    }
}

/// One golden split into its parts: the recomposed LaTeX, and the style file with
/// its file name where there is one.
struct Golden {
    output: String,
    style: Option<(String, String)>,
}

/// Splits a golden's text at its marker lines.
fn split(text: &str) -> Golden {
    let after_marker =
        text.strip_prefix(DOCUMENT_MARKER).and_then(|rest| rest.strip_prefix('\n')).unwrap_or(text);
    match after_marker.find(STYLE_MARKER) {
        Some(at) => {
            let (output, rest) = after_marker.split_at(at);
            let (marker, style) = rest.split_once('\n').expect("a marker line ends");
            let name = marker[STYLE_MARKER.len()..].trim_end_matches(" =====").to_string();
            Golden { output: output.to_string(), style: Some((style.to_string(), name)) }
        }
        None => Golden { output: after_marker.to_string(), style: None },
    }
}

/// What one compilation produced: whether the engine succeeded, and the files
/// it wrote that a test reads. A file the document did not write is the empty
/// string.
struct Compiled {
    ok: bool,
    /// `doc.log`, the compilation's own account of itself.
    log: String,
    /// `doc.aux`, the auxiliary file: what a label recorded and what a
    /// reference reads back on the next run.
    aux: String,
    /// `doc.toc`, the table of contents, written by a heading and read back
    /// by `\tableofcontents`.
    toc: String,
    /// `doc.toc` as it stood after each run, in order: what a heading wrote to
    /// it the first time and what it wrote the second time, after the first
    /// run's file had been read back. The two differ where something in a
    /// title does not survive the journey through the file.
    toc_runs: Vec<String>,
    /// `doc.lof`, the list of figures, written by a caption.
    lof: String,
    /// `doc.out`, the PDF bookmarks `hyperref` writes.
    bookmarks: String,
    /// `doc.pdf`, as bytes: what [`link_targets`] reads the link targets out
    /// of, where the document turned the compression of objects off
    /// ([`uncompressed_objects`]).
    pdf: Vec<u8>,
}

/// One compilation: writes `styles` — each a style file's text and the file
/// name it takes, requested in the order given — and a document whose preamble
/// holds `document_preamble` and whose
/// body is `body` into a fresh directory under the system temporary
/// directory, runs the current engine ([`ENGINE`]) twice over it, and gives
/// back what it produced.
/// The directory is named for this process, a counter ([`NEXT_COMPILE`]) and
/// `variant`: the counter is what makes it unique, and the process and the
/// variant are there to make it readable while a run is under way. It is
/// deleted afterwards.
///
/// `styles` is a slice so that a test can load **two** style files in one job,
/// which is what a document holding two recomposed documents does: a style
/// file is named after a hash of its own content, so two of them meet under
/// two names and every declaration the family makes has to survive being made
/// twice. A golden's own single style file is passed as
/// `golden.style.as_slice()`.
///
/// Every compilation of this suite goes through here: [`compile`] for a
/// golden as the backend wrote it, and the probes of a recomposed user
/// definition for a document of their own over a golden's style file.
fn compile_document(
    name: &str,
    variant: &str,
    styles: &[(String, String)],
    document_preamble: &str,
    body: &str,
) -> Compiled {
    compile_document_after(name, variant, "", styles, document_preamble, body)
}

/// [`compile_document`] with `before_styles` written into the preamble
/// before the style files are loaded: where an author's own definitions
/// stand when the style file's guarded ones should find them made.
fn compile_document_after(
    name: &str,
    variant: &str,
    before_styles: &str,
    styles: &[(String, String)],
    document_preamble: &str,
    body: &str,
) -> Compiled {
    let unique = NEXT_COMPILE.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "flm-latex-compile-{}-{unique}-{name}-{variant}-{}",
        std::process::id(),
        ENGINE.get()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a temporary directory");

    let mut preamble = String::from(before_styles);
    for (style, file) in styles {
        fs::write(dir.join(file), style).expect("the style file");
        let package = file.strip_suffix(".sty").unwrap_or(file);
        preamble.push_str(&format!("\\usepackage{{{package}}}\n"));
    }
    preamble.push_str(document_preamble);
    let document =
        format!("\\documentclass{{article}}\n{preamble}\\begin{{document}}\n{body}\\end{{document}}\n");
    fs::write(dir.join("doc.tex"), &document).expect("the document");
    write_images(&dir, body);

    let side = |extension: &str| {
        fs::read_to_string(dir.join(format!("doc.{extension}"))).unwrap_or_default()
    };
    let mut ok = true;
    let mut toc_runs = Vec::new();
    for _ in 0..2 {
        let engine = ENGINE.get();
        let status = Command::new(engine)
            .current_dir(&dir)
            .args(["-interaction=nonstopmode", "-halt-on-error", "doc.tex"])
            .output()
            .unwrap_or_else(|error| panic!("{engine} runs: {error}"));
        if !status.status.success() {
            ok = false;
            break;
        }
        toc_runs.push(side("toc"));
    }
    let compiled = Compiled {
        ok,
        log: side("log"),
        aux: side("aux"),
        toc: side("toc"),
        lof: side("lof"),
        bookmarks: side("out"),
        pdf: fs::read(dir.join("doc.pdf")).unwrap_or_default(),
        toc_runs,
    };
    let _ = fs::remove_dir_all(&dir);
    compiled
}

/// A picture of one gray pixel, in PNG: what [`write_images`] writes for
/// every image a document shows, so that the document compiles with no image
/// file committed.
const ONE_PIXEL_PNG: [u8; 67] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x00, 0x00, 0x00, 0x00, 0x3a, 0x7e, 0x9b,
    0x55, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x68, 0x00, 0x00, 0x00,
    0x82, 0x00, 0x81, 0x77, 0xcd, 0x72, 0xb6, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// Writes [`ONE_PIXEL_PNG`] into `dir` at the path of every image hook
/// `\flmIncludeGraphics[…]{<path>}` of `body` whose path is a plain relative
/// one — letters, digits, `/`, `.`, `-` and `_`, no `..` — so that the image a
/// golden shows is there when it compiles.
fn write_images(dir: &Path, body: &str) {
    for (at, _) in body.match_indices("\\flmIncludeGraphics") {
        let rest = &body[at + "\\flmIncludeGraphics".len()..];
        let rest = match rest.strip_prefix('[') {
            Some(keys) => keys.split_once(']').map_or("", |(_, after)| after),
            None => rest,
        };
        let Some(path) = rest.strip_prefix('{').and_then(|inner| inner.split_once('}')).map(|(p, _)| p)
        else {
            continue;
        };
        let plain = path.chars().all(|ch| ch.is_ascii_alphanumeric() || "/.-_".contains(ch));
        if path.is_empty() || !plain || path.split('/').any(|part| part == "..") {
            continue;
        }
        let file = dir.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).expect("the image's folder");
        }
        fs::write(file, ONE_PIXEL_PNG).expect("the image");
    }
}

/// Writes `golden` out as the files LaTeX expects and compiles it, plain or
/// with `hyperref` loaded.
fn compile(name: &str, golden: &Golden, hyperref: bool) -> Compiled {
    compile_document(
        name,
        if hyperref { "hyperref" } else { "plain" },
        golden.style.as_slice(),
        if hyperref { "\\usepackage{hyperref}\n" } else { "" },
        &golden.output,
    )
}

/// The first line of a log that reports an error, for a failure message.
fn first_error(log: &str) -> String {
    log.lines()
        .find(|line| line.starts_with('!'))
        .map(String::from)
        .unwrap_or_else(|| String::from("(no `!` line in the log)"))
}

#[test]
fn every_golden_compiles_with_and_without_hyperref() {
    under_each_engine(every_golden_compiles_with_and_without_hyperref_under);
}

fn every_golden_compiles_with_and_without_hyperref_under() {
    let skipped: BTreeSet<&str> = NOT_COMPILED.iter().map(|(name, _)| *name).collect();
    let mut compiled = 0;
    let mut names: Vec<String> = Vec::new();
    let paths = golden_directories().into_iter().flat_map(|directory| {
        fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("the goldens directory {}: {error}", directory.display()))
            .map(|entry| entry.expect("a golden").path())
            .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("tex"))
            .collect::<Vec<_>>()
    });
    for path in paths {
        let name = path.file_stem().expect("a stem").to_string_lossy().to_string();
        names.push(name.clone());
        if skipped.contains(name.as_str()) {
            continue;
        }
        let golden = split(&fs::read_to_string(&path).expect("a golden's text"));
        for hyperref in [false, true] {
            let result = compile(&name, &golden, hyperref);
            assert!(
                result.ok,
                "{name} (hyperref: {hyperref}) did not compile: {}",
                first_error(&result.log)
            );
            for complaint in ["There were undefined references", "multiply defined", "Multiple \\label"]
            {
                assert!(
                    !result.log.contains(complaint),
                    "{name} (hyperref: {hyperref}): {complaint}"
                );
            }
            compiled += 1;
        }
    }
    assert!(compiled > 20, "only {compiled} compilations ran");
    // Every skipped golden is a golden that exists: a stale entry would
    // silently stop compiling nothing.
    for (name, _) in NOT_COMPILED {
        assert!(names.iter().any(|n| n == name), "{name} names no golden");
    }
}

// --- a recomposed user definition inside another command's argument ---------------------------

/// The golden whose document uses a recomposed user definition inside a
/// heading title, and whose style file defines it: `\flmUDhello`, one
/// mandatory argument, standing for `Hello, <argument>!`. Every probe below
/// compiles a document of its own over this style file.
fn in_heading() -> Golden {
    split(&fs::read_to_string(goldens().join("userdefs-in-heading.tex")).expect("the golden"))
}

#[test]
fn a_recomposed_definition_in_a_heading_reaches_the_table_of_contents_and_the_auxiliary_file() {
    under_each_engine(a_recomposed_definition_in_a_heading_reaches_the_table_of_contents_and_the_auxiliary_file_under);
}

fn a_recomposed_definition_in_a_heading_reaches_the_table_of_contents_and_the_auxiliary_file_under() {
    // A heading title is a moving argument: LaTeX writes it to the table of
    // contents and to the auxiliary file and reads it back on the next run. A
    // recomposed definition is a plain expandable macro, so it is expanded on
    // the way there and what lands in both files is the definition's own text.
    // That is what makes a call work in a moving argument, and it is why a
    // reference to the heading resolves to the same words the page shows.
    let golden = in_heading();
    let body = format!("\\tableofcontents\n{}", golden.output);
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result = compile_document("userdefs-in-heading-toc", "toc", golden.style.as_slice(), preamble, &body);
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        // The definition's text is in the table of contents, and the call is
        // not: it was expanded into the line rather than written to it.
        assert!(result.toc.contains("Greeting Hello, World!"), "{}", result.toc);
        assert!(!result.toc.contains("flmUDhello"), "{}", result.toc);
        // And in the auxiliary file inside the label's own record: the
        // reference text of the unnumbered heading is its title, which holds
        // the call, and that text goes through a `\protected@edef`
        // (`flm.latex.def.label-display-text`) — the harder case, and the one
        // the table-of-contents line above would not prove.
        assert!(
            result.aux.contains("\\newlabel{ref2}{{Unnumbered Hello, Carla!"),
            "{}",
            result.aux
        );
        assert!(
            !result.log.contains("There were undefined references"),
            "{}",
            result.log
        );
    }
}

#[test]
fn a_recomposed_definition_in_a_caption_reaches_the_list_of_figures() {
    under_each_engine(a_recomposed_definition_in_a_caption_reaches_the_list_of_figures_under);
}

fn a_recomposed_definition_in_a_caption_reaches_the_list_of_figures_under() {
    // A caption is the other moving argument an author meets, and no FLM
    // feature writes one yet, so the document is this test's own over the
    // golden's style file.
    let golden = in_heading();
    let body = "\\listoffigures\n\
                \\begin{figure}\n\
                A figure.\n\
                \\caption{A caption \\flmUDhello{World}}\\label{fig:one}\n\
                \\end{figure}\n\
                See Figure~\\ref{fig:one}.\n";
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result = compile_document("userdefs-in-caption", "caption", golden.style.as_slice(), preamble, body);
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        assert!(result.lof.contains("Hello, World!"), "{}", result.lof);
        assert!(
            !result.log.contains("There were undefined references"),
            "{}",
            result.log
        );
    }
}

#[test]
fn a_recomposed_definition_reaches_a_pdf_bookmark_with_its_text_and_no_warning() {
    under_each_engine(a_recomposed_definition_reaches_a_pdf_bookmark_with_its_text_and_no_warning_under);
}

fn a_recomposed_definition_reaches_a_pdf_bookmark_with_its_text_and_no_warning_under() {
    // The place a robust command could not work, and the reason every
    // recomposed definition is expandable: with `hyperref` loaded, a heading's
    // PDF bookmark is built by *expanding* the title into a string, never by
    // typesetting it. A plain expandable macro is expanded there like any
    // other text, so the bookmark holds the definition's own words and the log
    // holds no complaint. Under the earlier robust form `hyperref` dropped the
    // call and wrote "Token not allowed in a PDF string"; this test is that
    // one reversed.
    let golden = in_heading();
    let result = compile_document(
        "userdefs-in-heading-bookmarks",
        "bookmarks",
        golden.style.as_slice(),
        "\\usepackage{hyperref}\n",
        &golden.output,
    );
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    assert!(!result.log.contains("Token not allowed in a PDF string"), "{}", result.log);
    // The bookmark was written and holds the definition's text. Its string is
    // written as UTF-16 escapes, one `\000` before each ASCII character, so
    // the words are matched with those separators between their letters.
    assert!(result.bookmarks.contains("\\BOOKMARK"), "{}", result.bookmarks);
    assert!(!result.bookmarks.contains("flmUDhello"), "{}", result.bookmarks);
    // `hyperref` writes the bookmark string as UTF-16, each ASCII letter
    // preceded by `\000`, so the words are looked for in that spelling.
    let escaped = |word: &str| -> String {
        word.chars().map(|c| format!("\\000{c}")).collect()
    };
    for word in ["Hello", "World"] {
        assert!(
            result.bookmarks.contains(&escaped(word)) || result.bookmarks.contains(word),
            "the bookmark does not hold `{word}`:\n{}",
            result.bookmarks
        );
    }
    // Nothing worse: no error, and every reference resolved.
    assert!(!result.log.contains("There were undefined references"), "{}", result.log);
}

/// The golden of the macro written robustly on request: the document of
/// [`in_heading`] with the render option `robust_names` naming `hello`, and one
/// definition more — `\sig`, of no arguments, named in the option too, so that
/// the empty argument specification `\ProvideDocumentCommand\flmUDsig{}{…}` is
/// compiled here as well.
fn robust() -> Golden {
    split(&fs::read_to_string(goldens().join("userdefs-robust.tex")).expect("the golden"))
}

/// The golden of a packed and a flagged definition written robustly, both used
/// in a heading.
fn robust_packed() -> Golden {
    split(&fs::read_to_string(goldens().join("userdefs-robust-packed.tex")).expect("the golden"))
}

#[test]
fn a_robust_definition_in_a_heading_travels_into_the_table_of_contents_as_its_call() {
    under_each_engine(a_robust_definition_in_a_heading_travels_into_the_table_of_contents_as_its_call_under);
}

fn a_robust_definition_in_a_heading_travels_into_the_table_of_contents_as_its_call_under() {
    // The other half of the option: a macro the render option `robust_names`
    // names is written through the document command interface, which makes it
    // engine-protected. LaTeX then writes the *call* into the table of contents
    // and into the auxiliary file rather than what the call stands for, and
    // runs it when that file is read back — which is how a robust command
    // survives a moving argument. This is the same document as
    // `userdefs-in-heading.tex`, whose probe above finds the definition's text
    // in both files instead; the two together are the trade the option is.
    let golden = robust();
    let body = format!("\\tableofcontents\n{}", golden.output);
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result =
            compile_document("userdefs-robust-toc", "toc", golden.style.as_slice(), preamble, &body);
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        // The call itself is in the table of contents, and its own text is not.
        assert!(result.toc.contains("\\flmUDhello {World}"), "{}", result.toc);
        assert!(!result.toc.contains("Hello, World!"), "{}", result.toc);
        // And the definition still produces its text on the page, which the
        // table of contents proves indirectly: the line was read back and set
        // without error.
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
    }
}

#[test]
fn a_robust_definition_is_dropped_from_a_pdf_bookmark_with_hyperrefs_warning() {
    under_each_engine(a_robust_definition_is_dropped_from_a_pdf_bookmark_with_hyperrefs_warning_under);
}

fn a_robust_definition_is_dropped_from_a_pdf_bookmark_with_hyperrefs_warning_under() {
    // The cost of asking for robustness, pinned as the expected behavior. With
    // `hyperref` loaded a heading's PDF bookmark is built by *expanding* the
    // title into a string; a robust command cannot be expanded, so `hyperref`
    // removes it and says so in the log. The argument's text survives, the
    // definition's own words do not: the bookmark reads "Greeting World" where
    // the page reads "Greeting Hello, World!". Nothing worse happens — no
    // error, and every reference still resolves. The mirror of this test, for
    // the expandable form every definition takes by default, is
    // `a_recomposed_definition_reaches_a_pdf_bookmark_with_its_text_and_no_warning`
    // above.
    let golden = robust();
    let result = compile_document(
        "userdefs-robust-bookmarks",
        "bookmarks",
        golden.style.as_slice(),
        "\\usepackage{hyperref}\n",
        &golden.output,
    );
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    assert!(
        result.log.contains("Token not allowed in a PDF string"),
        "no bookmark warning in the log:\n{}",
        result.log
    );
    assert!(result.log.contains("removing `\\flmUDhello'"), "{}", result.log);
    assert!(result.bookmarks.contains("\\BOOKMARK"), "{}", result.bookmarks);
    // The bookmark string holds the argument's word and not the definition's.
    // `hyperref` writes it as UTF-16, each ASCII letter preceded by `\000`.
    let escaped = |word: &str| -> String { word.chars().map(|c| format!("\\000{c}")).collect() };
    assert!(
        result.bookmarks.contains(&escaped("World")) || result.bookmarks.contains("World"),
        "{}",
        result.bookmarks
    );
    assert!(
        !result.bookmarks.contains(&escaped("Hello")) && !result.bookmarks.contains("Hello,"),
        "{}",
        result.bookmarks
    );
    assert!(!result.log.contains("There were undefined references"), "{}", result.log);
}

#[test]
fn case_changing_leaves_a_robust_definitions_own_words_as_the_definition_wrote_them() {
    under_each_engine(case_changing_leaves_a_robust_definitions_own_words_as_the_definition_wrote_them_under);
}

fn case_changing_leaves_a_robust_definitions_own_words_as_the_definition_wrote_them_under() {
    // The second reason to ask for robustness, and the contrast that makes it
    // plain. `\MakeUppercase` expands what it is given before changing its
    // case, so an expandable definition's own body text is uppercased with the
    // argument (`HELLO, WORLD!`, pinned by
    // `case_changing_and_edef_expand_a_recomposed_call` above). A robust
    // command is not expanded there: the call travels through the case change
    // untouched and only its argument is changed, so the definition's own words
    // reach the page exactly as the definition wrote them.
    //
    // What LaTeX printed is not readable from a test, so the document writes
    // the case-changed tokens into the log through `\text_uppercase:n`, the
    // expandable function `\MakeUppercase` itself calls.
    let golden = robust();
    let body = "\\ExplSyntaxOn\n\
                \\newcommand\\probe{\\typeout{PROBE:\\text_uppercase:n {\\flmUDhello{World}}}}\n\
                \\ExplSyntaxOff\n\
                \\probe\n\
                \\MakeUppercase{\\flmUDhello{World}}\n";
    let result =
        compile_document("userdefs-robust-uppercase", "uppercase", golden.style.as_slice(), "", body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    // The call survives as itself and its argument alone is uppercased.
    assert!(result.log.contains("PROBE:\\flmUDhello {WORLD}"), "{}", result.log);
    assert!(!result.log.contains("PROBE:HELLO"), "{}", result.log);
}

#[test]
fn a_robust_packed_and_flagged_call_in_a_heading_reaches_the_table_of_contents_whole() {
    under_each_engine(a_robust_packed_and_flagged_call_in_a_heading_reaches_the_table_of_contents_whole_under);
}

fn a_robust_packed_and_flagged_call_in_a_heading_reaches_the_table_of_contents_whole_under() {
    // Nothing about a definition's shape stands in the option's way, and this
    // is the proof in the hardest place. A definition of more than nine
    // effective arguments hands them over in one group, and a definition the
    // template asks a question about carries a flag group per question; written
    // robustly, both travel into the table of contents as the call, group and
    // all, and are read back and set from there.
    let golden = robust_packed();
    let body = format!("\\tableofcontents\n{}", golden.output);
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result = compile_document(
            "userdefs-robust-packed-toc",
            "toc",
            golden.style.as_slice(),
            preamble,
            &body,
        );
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        assert!(
            result.toc.contains("\\flmUDbig {{A}{b}{c}{d}{e}{f}{G}{h}{i}{j}{k}{L}}"),
            "{}",
            result.toc
        );
        assert!(
            result.toc.contains("\\flmUDmymacro {Hello}{Ada}{\\BooleanFalse }"),
            "{}",
            result.toc
        );
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
    }
}

#[test]
fn case_changing_and_edef_expand_a_recomposed_call() {
    under_each_engine(case_changing_and_edef_expand_a_recomposed_call_under);
}

fn case_changing_and_edef_expand_a_recomposed_call_under() {
    // Two places that expand text rather than typeset it, and what an
    // expandable definition does in each. `\MakeUppercase` expands what it is
    // given and case-changes the characters it finds, so a recomposed call is
    // expanded and the whole of what it stands for is uppercased — the
    // definition's own body text as well as the argument:
    // `\MakeUppercase{\flmUDhello{World}}` prints `HELLO, WORLD!`. And
    // `\edef` fixes a macro's replacement text by expanding what it is given,
    // so a call inside one leaves the definition's text behind and no trace of
    // the call.
    //
    // What LaTeX printed is not readable from a test, so the document writes
    // both results into the log: the case change through `\text_uppercase:n`
    // — the function `\MakeUppercase` itself calls, and an expandable one —
    // and the `\edef` through `\meaning`.
    let golden = in_heading();
    let body = "\\ExplSyntaxOn\n\
                \\newcommand\\probe{\\typeout{PROBE:\\text_uppercase:n {\\flmUDhello{World}}}}\n\
                \\ExplSyntaxOff\n\
                \\probe\n\
                \\edef\\stored{\\flmUDhello{World}}\\typeout{EDEF:\\meaning\\stored}\n\
                \\MakeUppercase{\\flmUDhello{World}}\n";
    let result =
        compile_document("userdefs-uppercase", "uppercase", golden.style.as_slice(), "", body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    // The call is gone and everything it stood for is uppercased.
    assert!(result.log.contains("PROBE:HELLO, WORLD!"), "{}", result.log);
    assert!(!result.log.contains("PROBE:\\flmUDhello"), "{}", result.log);
    // `\edef` fixed the definition's text, with nothing left to expand.
    assert!(result.log.contains("EDEF:macro:->Hello, World!"), "{}", result.log);
}

#[test]
fn an_argument_holding_a_paragraph_break_compiles() {
    under_each_engine(an_argument_holding_a_paragraph_break_compiles_under);
}

fn an_argument_holding_a_paragraph_break_compiles_under() {
    // A recomposed definition is written with `\providecommand`, which makes a
    // **long** macro, so an argument may hold a paragraph break. This is the
    // defect the guard spelling fixes: neither `\DeclareDocumentCommand{m}`
    // nor `\ProvideExpandableDocumentCommand{m}` accepts a `\par` in an
    // argument, and a document whose own text put one there stopped the
    // compilation with "Paragraph ended before … was complete". The blank test
    // the style file carries is long for the same reason, and is asked here
    // over the same argument.
    let golden = blank();
    let body = "\\flmUDb{first paragraph\n\npar second paragraph}\n";
    let result = compile_document("userdefs-par-argument", "par", golden.style.as_slice(), "", body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    assert!(!result.log.contains("Paragraph ended before"), "{}", result.log);
}

// --- an environment that takes its body as an argument ----------------------------------------

/// The golden whose two environments take their body as an argument: `{wrap}`,
/// which sets the body inside another command's argument and names an argument
/// of its own after it, and `{twice}`, which sets the body twice.
fn body_argument() -> Golden {
    split(&fs::read_to_string(goldens().join("userdefs-body-argument.tex")).expect("the golden"))
}

#[test]
fn a_body_taken_as_an_argument_is_set_where_the_definition_puts_it_and_nests() {
    under_each_engine(a_body_taken_as_an_argument_is_set_where_the_definition_puts_it_and_nests_under);
}

fn a_body_taken_as_an_argument_is_set_where_the_definition_puts_it_and_nests_under() {
    // `+b` asks LaTeX to hand the body over as an argument instead of setting
    // it between the begin and the end code, which is what a definition needs
    // that puts the body inside another command's argument or in two places.
    // Three things are measured: the body reaches the place the definition puts
    // it (the call sets what the same text sets written out by hand), a body
    // holding it twice is set twice, and one such environment nested inside
    // another of the same name keeps the two apart — LaTeX reads the body up to
    // the matching `\end`, counting the nested pair. Inline verbatim content in
    // such a body is measured by the probe over the verbatim golden below.
    let golden = body_argument();
    let body = "\\sbox0{\\begin{flmUEwrap}{W}the body\\end{flmUEwrap}}\\typeout{W call = \\the\\wd0}
\\sbox1{(\\emph{the body} — W)}\\typeout{W hand = \\the\\wd1}
\\sbox2{\\begin{flmUEtwice}the body\\end{flmUEtwice}}\\typeout{W twice = \\the\\wd2}
\\sbox3{the body/the body}\\typeout{W twicehand = \\the\\wd3}
\\sbox4{\\begin{flmUEtwice}o \\begin{flmUEtwice}i\\end{flmUEtwice}\\end{flmUEtwice}}\\typeout{W nested = \\the\\wd4}
\\sbox5{o i/i/o i/i}\\typeout{W nestedhand = \\the\\wd5}
";
    let result =
        compile_document("userdefs-body-argument", "grabbed", golden.style.as_slice(), "", body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let w = typeout_values(&result.log, "W ");
    assert_eq!(width(&w, "call", &result.log), width(&w, "hand", &result.log), "{}", result.log);
    assert_eq!(
        width(&w, "twice", &result.log),
        width(&w, "twicehand", &result.log),
        "{}",
        result.log
    );
    // The nested pair: the outer body, which holds the inner environment, is
    // set twice, and the inner body twice inside each of those.
    assert_eq!(
        width(&w, "nested", &result.log),
        width(&w, "nestedhand", &result.log),
        "{}",
        result.log
    );
}

#[test]
fn a_body_taken_as_an_argument_may_hold_a_paragraph_break_where_a_short_one_may_not() {
    under_each_engine(a_body_taken_as_an_argument_may_hold_a_paragraph_break_where_a_short_one_may_not_under);
}

fn a_body_taken_as_an_argument_may_hold_a_paragraph_break_where_a_short_one_may_not_under() {
    // The `+` in `+b` makes the body **long**, so it may hold a paragraph
    // break — which a body ordinarily does, being document text. The plain `b`
    // does not, and the contrast is what pins the `+`: the same document over an
    // environment declared `{b}` stops with "Paragraph ended before … was
    // complete". Both halves are compiled here, the second with the error
    // tolerated, since it is the fact being pinned.
    let golden = body_argument();
    let body = "\\begin{flmUEtwice}first paragraph

par second paragraph\\end{flmUEtwice}
";
    let result =
        compile_document("userdefs-body-argument", "par", golden.style.as_slice(), "", body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    assert!(!result.log.contains("Paragraph ended before"), "{}", result.log);
    // The same body handed to an environment whose body argument is short.
    let short = "\\DeclareDocumentEnvironment{shortbody}{b}{[#1]}{}
";
    let short_body =
        "\\begin{shortbody}first paragraph

par second paragraph\\end{shortbody}
";
    let refused =
        compile_document("userdefs-body-argument", "short", &[], short, short_body);
    assert!(
        refused.log.contains("Paragraph ended before"),
        "a short body argument accepted a paragraph break:\n{}",
        refused.log
    );
}

/// A golden of `tests/goldens/latex`, split into its recomposed LaTeX and its style
/// file.
fn latex_golden(name: &str) -> Golden {
    split(&fs::read_to_string(goldens().join(name)).expect("the golden"))
}

/// The golden whose two definitions hold a verbatim block in their template,
/// each recomposed with the escaped block; its style file is the one that
/// defines the `flmVerbatimCode` environment, which the probes below build a
/// definition against.
fn verbatim_block_in_template() -> Golden {
    latex_golden("userdefs-verbatim-block-in-template.tex")
}

#[test]
fn a_definition_whose_template_holds_a_verbatim_block_does_not_compile_in_any_spelling() {
    under_each_engine(a_definition_whose_template_holds_a_verbatim_block_does_not_compile_in_any_spelling_under);
}

fn a_definition_whose_template_holds_a_verbatim_block_does_not_compile_in_any_spelling_under() {
    // Why a template holding a verbatim block is refused: the style file such a
    // template *would* have produced does not compile, in any spelling of it.
    // The definition is built by hand here, against the style file that defines
    // the `flmVerbatimCode` environment, and each of the three failures is
    // pinned with the message LaTeX gives, which is the same on both engines.
    //
    // The first case is the load-bearing one, because it holds no number sign
    // at all: the failure is structural. A definition's body is one stream of
    // tokens with the line ends already gone, so `fancyvrb` — which needs the
    // content to begin on the line after `\begin{Verbatim}` — meets the whole
    // block, its own `\end` included, on the opening line. The other two show
    // that doubling a number sign, the usual cure for a `#` in a definition,
    // only moves the error: doubled, the `#` survives the outer definition and
    // is a bare one again inside `fancyvrb`'s own re-scan.
    let golden = verbatim_block_in_template();
    let definition = |block: &str| {
        format!(
            "\\providecommand\\flmUDblk{{\\begin{{flmVerbatimCode}}[]\n{block}\n\\end{{flmVerbatimCode}}}}\n"
        )
    };
    let no_hash = compile_document(
        "userdefs-verbatim-block",
        "nohash",
        golden.style.as_slice(),
        &definition("f(x) = y"),
        "\\flmUDblk\n",
    );
    assert!(!no_hash.ok, "a verbatim block inside a definition compiled:\n{}", no_hash.log);
    assert!(
        no_hash.log.contains("FancyVerb Error")
            && no_hash.log.contains("Extraneous input")
            && no_hash.log.contains("and line end"),
        "{}",
        no_hash.log
    );
    let bare = compile_document(
        "userdefs-verbatim-block",
        "bare",
        golden.style.as_slice(),
        &definition("f(#1) = y"),
        "\\flmUDblk\n",
    );
    assert!(!bare.ok, "a verbatim block inside a definition compiled:\n{}", bare.log);
    assert!(
        bare.log.contains("Illegal parameter number in definition of \\flmUDblk"),
        "{}",
        bare.log
    );
    let doubled = compile_document(
        "userdefs-verbatim-block",
        "doubled",
        golden.style.as_slice(),
        &definition("f(##1) = y"),
        "\\flmUDblk\n",
    );
    assert!(!doubled.ok, "a verbatim block inside a definition compiled:\n{}", doubled.log);
    assert!(
        doubled.log.contains("Illegal parameter number in definition of \\@tempa"),
        "{}",
        doubled.log
    );
}

#[test]
fn inline_verbatim_content_in_a_grabbed_body_is_handed_over_unharmed() {
    under_each_engine(inline_verbatim_content_in_a_grabbed_body_is_handed_over_unharmed_under);
}

fn inline_verbatim_content_in_a_grabbed_body_is_handed_over_unharmed_under() {
    // `\EscVerb` is an ordinary command over a braced argument — which is the
    // reason the family writes inline verbatim content that way — so a body
    // holding one is grabbed like any other content and sets exactly what the
    // same text sets written out by hand. The golden
    // `userdefs-body-argument-verbatim.tex` is the recomposition's own document
    // for this, compiled by the suite above; the measurement here needs a body
    // of one paragraph, so it uses the wrapping environment and a stand-in for
    // the format hook the verbatim piece defines.
    let golden = body_argument();
    let verb = "\\EscVerb[formatcom=\\flmFmtVRB{verbtext}]{a\\_b}";
    let preamble = "\\usepackage{fvextra}\n\\newcommand\\flmFmtVRB[1]{}\n";
    let body = format!(
        "\\sbox0{{\\begin{{flmUEwrap}}{{W}}an {verb} body\\end{{flmUEwrap}}}}\
         \\typeout{{W call = \\the\\wd0}}\n         \\sbox1{{(\\emph{{an {verb} body}} — W)}}\\typeout{{W hand = \\the\\wd1}}\n         \\sbox2{{(\\emph{{an body}} — W)}}\\typeout{{W plain = \\the\\wd2}}\n"
    );
    let result =
        compile_document("userdefs-body-argument", "escverb", golden.style.as_slice(), preamble, &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let w = typeout_values(&result.log, "W ");
    assert_eq!(width(&w, "call", &result.log), width(&w, "hand", &result.log), "{}", result.log);
    // The verbatim content really is set, so the equality above says something.
    assert_ne!(width(&w, "call", &result.log), width(&w, "plain", &result.log));
}

#[test]
fn a_verbatim_block_a_use_site_wrote_in_a_grabbed_body_does_not_compile_either() {
    under_each_engine(a_verbatim_block_a_use_site_wrote_in_a_grabbed_body_does_not_compile_either_under);
}

fn a_verbatim_block_a_use_site_wrote_in_a_grabbed_body_does_not_compile_either_under() {
    // Why one use site is expanded rather than written as a call: a body LaTeX
    // hands over as an argument is tokens already read, so a verbatim block
    // written there fails exactly as one written into a definition's body does.
    // The call is built by hand over the style file of the environment that
    // takes its body as an argument, since the recomposition never writes it.
    let golden = body_argument();
    let style: Vec<(String, String)> = golden.style.iter().map(|(text, name)| {
        let mut text = text.clone();
        // The verbatim environment, which this golden's own document needs
        // nowhere, so that the probe can write a block at all.
        text = text.replace(
            "\\endinput",
            "\\ProvideDocumentEnvironment{flmVerbatimCode}{O{}}{%\n  \\VerbatimEnvironment\n  \\begin{Verbatim}}%\n  {\\end{Verbatim}}\n\\endinput",
        );
        (text, name.clone())
    }).collect();
    let body = "\\begin{flmUEtwice}\\begin{flmVerbatimCode}[]\nf(x) = y\n\\end{flmVerbatimCode}\\end{flmUEtwice}\n";
    let result = compile_document(
        "userdefs-body-argument",
        "verbatim",
        &style,
        "\\usepackage{fvextra}\n",
        body,
    );
    assert!(!result.ok, "a verbatim block in a grabbed body compiled:\n{}", result.log);
    assert!(result.log.contains("FancyVerb Error"), "{}", result.log);
}

// --- the escaped block: a verbatim block written as one call -----------------------------------

/// The escaped block a verbatim block is written as where it reaches LaTeX
/// inside a group LaTeX reads as tokens: the environment's own optional
/// argument, its name, and one brace group per line holding one brace group
/// per tab-separated segment, each segment in the encoding `\EscVerb` reads.
/// The spelling the recomposition writes is pinned by the goldens; this builds
/// one by hand so that a probe can put it where it belongs and read back what
/// LaTeX made of it.
fn escaped_block(lines: &[&[&str]]) -> String {
    let groups: String = lines
        .iter()
        .map(|line| {
            let segments: String =
                line.iter().map(|segment| format!("{{{segment}}}")).collect();
            format!("{{{segments}}}")
        })
        .collect();
    format!("\\flmVerbatimBlock[]{{flmVerbatimCode}}{{{groups}}}")
}

#[test]
fn an_escaped_block_compiles_in_every_place_a_call_carries_one() {
    under_each_engine(an_escaped_block_compiles_in_every_place_a_call_carries_one_under);
}

fn an_escaped_block_compiles_in_every_place_a_call_carries_one_under() {
    // The four places a verbatim block reaches LaTeX inside a group LaTeX
    // reads as tokens, each of which refuses a block written out in full (the
    // three probes above): an argument of a call, the body a body-grabbing
    // environment hands over, a definition's own body in the style file, and
    // that same body with a parameter standing inside the block's content.
    // Each is built by hand over the style file that defines the family's
    // verbatim machinery, and each must simply compile.
    let golden = verbatim_block_in_template();
    let block = escaped_block(&[&["f(x)\\ =\\ y"], &["\\ \\ indented\\ \\{b\\}\\ \\%\\ p"]]);
    let preamble = format!(
        "\\usepackage{{fvextra}}\n\
         \\ProvideDocumentCommand\\flmUDbox{{m}}{{\\fbox{{\\begin{{minipage}}{{0.8\\linewidth}}#1\\end{{minipage}}}}}}\n\
         \\ProvideDocumentEnvironment{{flmUEwrap}}{{+m +b}}{{(\\emph{{#2}} --- #1)}}{{}}\n\
         \\providecommand\\flmUDtpl{{Template says: {block} and done.}}\n\
         \\providecommand\\flmUDhole[1]{{Hole: {} done.}}\n",
        escaped_block(&[&["value\\ =\\ #1;"], &["end"]]),
    );
    let body = format!(
        "argument: \\flmUDbox{{{block}}}\n\n\
         grabbed body: \\begin{{flmUEwrap}}{{W}}{block}\\end{{flmUEwrap}}\n\n\
         definition body: \\flmUDtpl\n\n\
         parameter inside the content: \\flmUDhole{{\\{{a\\ \\%\\ b\\}}}}\n"
    );
    let result =
        compile_document("userdefs-verbatim-block", "escaped", golden.style.as_slice(), &preamble, &body);
    assert!(result.ok, "an escaped block did not compile: {}", first_error(&result.log));
}

#[test]
fn an_escaped_block_sets_what_a_block_written_out_in_full_sets() {
    under_each_engine(an_escaped_block_sets_what_a_block_written_out_in_full_sets_under);
}

fn an_escaped_block_sets_what_a_block_written_out_in_full_sets_under() {
    // The two forms must set the same block, character for character, or the
    // recomposition would print one thing where a document is written out in
    // full and another where it travels inside a call. The content is every
    // character that has ever been a problem: a tab, a number sign, a percent
    // sign, a backslash, an unbalanced closing brace, a line ending in a
    // backslash, a run of spaces, an empty line and non-ASCII text.
    //
    // Both forms are set into a box of their own — through `lrbox`, which
    // reads its content from lines of the file, so a block written out in full
    // can stand in it — and the boxes' three dimensions are compared. A
    // difference of any kind in what was set moves at least one of them.
    let golden = verbatim_block_in_template();
    let lines: &[&[&str]] = &[
        &["a", "b", "c"],
        &["f(x)\\ =\\ \\{y\\}\\ \\%\\ 100\\ \\#\\ h"],
        &[""],
        &["\\}\\ unbalanced\\ and\\ \\\\\\ backslash"],
        &["\\ \\ \\ \\ four\\ spaces,\\ then\\ a\\ line\\ ending\\ in\\ \\\\"],
        &["café\\ —\\ naïve"],
    ];
    let literal = "a\tb\tc\n\
                   f(x) = {y} % 100 # h\n\
                   \n\
                   } unbalanced and \\ backslash\n\
                   \u{20}\u{20}\u{20}\u{20}four spaces, then a line ending in \\\n\
                   café — naïve";
    let preamble = "\\usepackage[T1]{fontenc}\n\\usepackage{fvextra}\n\
                    \\newsavebox\\flmbxa\n\\newsavebox\\flmbxb\n";
    let body = format!(
        "\\begin{{lrbox}}{{\\flmbxa}}\\begin{{minipage}}{{12cm}}\n\
         \\begin{{flmVerbatimCode}}[]\n{literal}\n\\end{{flmVerbatimCode}}\n\
         \\end{{minipage}}\\end{{lrbox}}%\n\
         \\begin{{lrbox}}{{\\flmbxb}}\\begin{{minipage}}{{12cm}}\n{}\n\
         \\end{{minipage}}\\end{{lrbox}}%\n\
         \\typeout{{B full = \\the\\wd\\flmbxa/\\the\\ht\\flmbxa/\\the\\dp\\flmbxa}}\n\
         \\typeout{{B call = \\the\\wd\\flmbxb/\\the\\ht\\flmbxb/\\the\\dp\\flmbxb}}\n\
         \\usebox\\flmbxa\\usebox\\flmbxb\n",
        escaped_block(lines),
    );
    let result =
        compile_document("userdefs-verbatim-block", "same", golden.style.as_slice(), preamble, &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let boxes = typeout_values(&result.log, "B ");
    let full = boxes.get("full").unwrap_or_else(|| panic!("{}", result.log));
    let call = boxes.get("call").unwrap_or_else(|| panic!("{}", result.log));
    assert_eq!(full, call, "{}", result.log);
    // The comparison says something only if the block was set at all.
    assert_ne!(full, "0.0pt/0.0pt/0.0pt", "{}", result.log);
}

#[test]
fn an_escaped_block_in_a_heading_reaches_the_table_of_contents_and_is_set_again_from_it() {
    under_each_engine(an_escaped_block_in_a_heading_reaches_the_table_of_contents_and_is_set_again_from_it_under);
}

fn an_escaped_block_in_a_heading_reaches_the_table_of_contents_and_is_set_again_from_it_under() {
    // The standing requirement, in its sharpest form. A section title is a
    // *moving argument*: LaTeX writes it to the table-of-contents file and
    // reads it back on the next run. `\flmVerbatimBlock` is declared through
    // the document command interface, so it is engine-protected and travels
    // into that file as the call itself, brace groups and all; the line and
    // segment structure is carried by braces exactly because a line feed
    // written into such a file comes back from it as an ordinary space.
    //
    // The second run must therefore write the same table of contents as the
    // first and must set the same block — a title whose block had collapsed
    // would write a different line.
    let golden = verbatim_block_in_template();
    let block = escaped_block(&[&["f(x)\\ =\\ y"], &["g(x)"]]);
    let body = format!(
        "\\tableofcontents\n\n\\section{{Code {block} title}}\ntext\n\n\
         \\section{{A title ending in a block {block}}}\nmore text\n"
    );
    let result = compile_document(
        "userdefs-verbatim-block",
        "heading",
        golden.style.as_slice(),
        "\\usepackage{fvextra}\n\\usepackage{hyperref}\n",
        &body,
    );
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    // The call reaches the file unexpanded, its brace groups intact.
    assert!(
        result.toc.contains("\\flmVerbatimBlock []{flmVerbatimCode}{{{f(x)\\ =\\ y}}{{g(x)}}}"),
        "{}",
        result.toc
    );
    // The second run wrote the same lines as the first, which is what says
    // that nothing was lost on the way through the file.
    assert_eq!(result.toc_runs.len(), 2, "two runs");
    assert_eq!(result.toc_runs[0], result.toc_runs[1], "{}", result.toc);
    // The second title ends with the block, which is vertical material: without
    // the wrapper's trailing `\ifvmode\leavevmode\fi` the run that reads the
    // table of contents back stops with `\pdfendlink cannot be used in vertical
    // mode`. Reaching here at all is that check.
    assert!(!result.log.contains("pdfendlink"), "{}", result.log);
}

#[test]
fn a_bookmark_holds_the_text_of_inline_verbatim_content_and_of_a_block() {
    under_each_engine(a_bookmark_holds_the_text_of_inline_verbatim_content_and_of_a_block_under);
}

fn a_bookmark_holds_the_text_of_inline_verbatim_content_and_of_a_block_under() {
    // A PDF bookmark is plain text: it can carry neither the setting of inline
    // verbatim content nor a block at all, so the verbatim piece tells
    // `hyperref` what to put there instead. Inline content gives its text —
    // without that line `fvextra` disables `\EscVerb` without removing its
    // optional argument and the option list itself lands in the bookmark — and
    // a block gives its characters run together, with no line breaks and
    // nothing between the lines.
    //
    // Both definitions must be expandable, because `hyperref` removes an
    // unexpandable token from a PDF string and warns about it, so the log must
    // hold no such warning.
    let golden = verbatim_block_in_template();
    let block = escaped_block(&[&["f(x)\\ \\%\\ y"], &["g"]]);
    let body = format!(
        "\\section{{Inline \\EscVerb[formatcom=\\flmFmtVRB{{verbcode}}]{{a\\_b\\ \\%\\ c}} here}}\ntext\n\n\
         \\section{{Block {block} here}}\nmore text\n"
    );
    let result = compile_document(
        "userdefs-verbatim-block",
        "bookmark",
        golden.style.as_slice(),
        "\\usepackage{fvextra}\n\\usepackage{hyperref}\n",
        &body,
    );
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    // `hyperref` writes a bookmark's text as UTF-16 octal escapes, so the
    // characters are looked for one by one; `\000` before each is the high
    // byte of a Latin-1 character.
    let bookmark = |text: &str| {
        text.chars()
            .map(|ch| match ch {
                ch if ch.is_ascii_alphanumeric() => format!("\\000{ch}"),
                ch => format!("\\000\\{:03o}", u32::from(ch)),
            })
            .collect::<String>()
    };
    assert!(result.bookmarks.contains(&bookmark("Inline a_b % c here")), "{}", result.bookmarks);
    // The block's lines run together, with nothing between them.
    assert!(result.bookmarks.contains(&bookmark("Block f(x) % yg here")), "{}", result.bookmarks);
    assert!(!result.log.contains("Token not allowed in a PDF string"), "{}", result.log);
}

// --- an environment whose template drops its body -----------------------------------------------

#[test]
fn an_environment_that_drops_its_body_prints_what_the_expansion_prints() {
    under_each_engine(an_environment_that_drops_its_body_prints_what_the_expansion_prints_under);
}

fn an_environment_that_drops_its_body_prints_what_the_expansion_prints_under() {
    // An environment whose template never names its body is recomposed with
    // the body-grabbing argument `+b` and a begin code that never writes that
    // argument: LaTeX hands the body over and the definition drops it, which
    // is what FLM's own expansion does with it. The body here holds a
    // paragraph break, which `+b` reads without stopping the compilation.
    //
    // Both goldens are the emitter's own output over the same document, one
    // recomposed and one with the definition always expanded. Each is set into
    // a box of its own and the three dimensions are compared: a body that was
    // set rather than dropped would be two lines more.
    let call = latex_golden("userdefs-body-dropped.tex");
    let expanded = latex_golden("userdefs-body-dropped-expanded.tex");
    let preamble = "\\newsavebox\\flmbxa\n\\newsavebox\\flmbxb\n";
    let body = format!(
        "\\begin{{lrbox}}{{\\flmbxa}}\\begin{{minipage}}{{12cm}}\n{}\n\\end{{minipage}}\\end{{lrbox}}%\n\
         \\begin{{lrbox}}{{\\flmbxb}}\\begin{{minipage}}{{12cm}}\n{}\n\\end{{minipage}}\\end{{lrbox}}%\n\
         \\typeout{{D call = \\the\\wd\\flmbxa/\\the\\ht\\flmbxa/\\the\\dp\\flmbxa}}\n\
         \\typeout{{D full = \\the\\wd\\flmbxb/\\the\\ht\\flmbxb/\\the\\dp\\flmbxb}}\n\
         \\usebox\\flmbxa\\usebox\\flmbxb\n",
        call.output.trim_end(),
        expanded.output.trim_end(),
    );
    let result =
        compile_document("userdefs-body-dropped", "same", call.style.as_slice(), preamble, &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    // The paragraph break inside the dropped body was read rather than
    // stopping the compilation.
    assert!(!result.log.contains("Paragraph ended before"), "{}", result.log);
    let boxes = typeout_values(&result.log, "D ");
    let with_call = boxes.get("call").unwrap_or_else(|| panic!("{}", result.log));
    let full = boxes.get("full").unwrap_or_else(|| panic!("{}", result.log));
    assert_eq!(with_call, full, "{}", result.log);
    // The comparison says something only if something was set at all.
    assert_ne!(with_call, "0.0pt/0.0pt/0.0pt", "{}", result.log);
}

// --- a definition that uses itself ------------------------------------------------------------

#[test]
fn a_recursive_call_sets_what_the_expansion_sets_and_survives_a_heading() {
    under_each_engine(a_recursive_call_sets_what_the_expansion_sets_and_survives_a_heading_under);
}

fn a_recursive_call_sets_what_the_expansion_sets_and_survives_a_heading_under() {
    // A definition that uses itself is recomposed as a LaTeX macro whose body
    // calls that same macro: writing the definition is asking LaTeX to run
    // the expansion. `\peel` tests its argument and calls itself with an empty
    // one, so the recursion stops after one step, and what it prints must be
    // what FLM's own expansion of the same document prints — the two goldens
    // here are the emitter's output over one document, one recomposed and one
    // with the definition always expanded.
    //
    // The heading is the standing requirement. A section title is a *moving
    // argument*: LaTeX writes it to the table of contents and reads it back on
    // the next run. A recomposed definition is expandable, and so is the
    // style file's blank test, so the whole recursion runs on the way into the
    // file and what lands there is the text the definition stands for — the
    // same text the expanded document's own title holds.
    let call = latex_golden("userdefs-recursive.tex");
    let expanded = latex_golden("userdefs-recursive-expanded.tex");
    let body = format!(
        "\\tableofcontents\n{}\n\
         \\sbox0{{\\flmUDpeel{{y}}}}\\typeout{{R call = \\the\\wd0}}\n\
         \\sbox1{{y.}}\\typeout{{R text = \\the\\wd1}}\n\
         \\sbox2{{y}}\\typeout{{R wrong = \\the\\wd2}}\n",
        call.output.trim_end()
    );
    let result =
        compile_document("userdefs-recursive", "toc", call.style.as_slice(), "", &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    // The call prints what the definition stands for, and the measure tells
    // the two apart, so the equality says something.
    let w = typeout_values(&result.log, "R ");
    assert_eq!(width(&w, "call", &result.log), width(&w, "text", &result.log));
    assert_ne!(width(&w, "call", &result.log), width(&w, "wrong", &result.log));
    // The recursion ran on the way into the table of contents, and what stands
    // there is the title the expanded document writes for itself.
    assert!(result.toc.contains("A y. title"), "{}", result.toc);
    assert!(!result.toc.contains("flmUDpeel"), "{}", result.toc);
    assert!(expanded.output.contains("\\section{A y. title}"), "{}", expanded.output);
    // The second run wrote the same file as the first: nothing was lost on the
    // way through it.
    assert_eq!(result.toc_runs.len(), 2, "two runs");
    assert_eq!(result.toc_runs[0], result.toc_runs[1], "{}", result.toc);
}

// --- more than nine effective arguments: the packed group -------------------------------------

/// The golden whose definitions take more than nine effective arguments, so
/// that every call hands them over in one group: `\big` of twelve and `\huge`
/// of twenty, one of them used inside a heading title.
fn packed() -> Golden {
    split(&fs::read_to_string(goldens().join("userdefs-packed.tex")).expect("the golden"))
}

/// The golden whose definition takes six arguments and asks about five of
/// them — eleven effective arguments — so that the flags and the blank test
/// are read out of the packed group.
fn packed_flags() -> Golden {
    split(&fs::read_to_string(goldens().join("userdefs-packed-flags.tex")).expect("the golden"))
}

/// The golden whose environment takes ten arguments and uses one of them in
/// its end code.
fn packed_environment() -> Golden {
    split(
        &fs::read_to_string(goldens().join("userdefs-packed-environment.tex")).expect("the golden"),
    )
}

#[test]
fn a_packed_call_in_a_heading_reaches_the_table_of_contents_and_prints_what_flm_prints() {
    under_each_engine(a_packed_call_in_a_heading_reaches_the_table_of_contents_and_prints_what_flm_prints_under);
}

fn a_packed_call_in_a_heading_reaches_the_table_of_contents_and_prints_what_flm_prints_under() {
    // The form this golden adds — every argument in one brace group, the body
    // reading them back with `\flmArg` — inside a heading title, which is a
    // moving argument. The accessor is expandable like everything else in a
    // recomposed body, so the call expands on the way to the table of contents
    // and what lands there is what the definition stands for.
    //
    // The widths say that a packed call of twelve and one of twenty arguments
    // print exactly what the expanded document prints (`tests/latex_userdefs.rs`
    // holds both texts), which is what the packing has to preserve.
    let golden = packed();
    let body = format!(
        "\\tableofcontents\n{}\n\
         \\sbox0{{\\flmUDbig{{{{one}}{{b}}{{c}}{{d}}{{e}}{{f}}{{seven}}{{h}}{{i}}{{j}}{{k}}{{twelve}}}}}}\\typeout{{W big = \\the\\wd0}}\n\
         \\sbox1{{[one|seven|twelve]}}\\typeout{{W bigtext = \\the\\wd1}}\n\
         \\sbox2{{\\flmUDhuge{{{{1}}{{2}}{{3}}{{4}}{{5}}{{6}}{{7}}{{8}}{{9}}{{10}}{{11}}{{TWELVE}}{{13}}{{14}}{{15}}{{16}}{{17}}{{18}}{{19}}{{TWENTY}}}}}}\\typeout{{W huge = \\the\\wd2}}\n\
         \\sbox3{{(1/TWELVE/TWENTY)}}\\typeout{{W hugetext = \\the\\wd3}}\n\
         \\sbox4{{[one|seven|eleven]}}\\typeout{{W wrong = \\the\\wd4}}\n",
        golden.output
    );
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result =
            compile_document("userdefs-packed-toc", "toc", golden.style.as_slice(), preamble, &body);
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        // The twelfth argument of the call in the title is `L`, and the first
        // and the seventh are `A` and `G`.
        assert!(result.toc.contains("[A|G|L]"), "{}", result.toc);
        assert!(!result.toc.contains("flmUDbig"), "{}", result.toc);
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
        let w = typeout_values(&result.log, "W ");
        assert_eq!(width(&w, "big", &result.log), width(&w, "bigtext", &result.log));
        assert_eq!(width(&w, "huge", &result.log), width(&w, "hugetext", &result.log));
        // The measure really tells the texts apart, so the equalities say
        // something: the twelfth argument is not the eleventh.
        assert_ne!(width(&w, "big", &result.log), width(&w, "wrong", &result.log));
    }
}

#[test]
fn a_packed_argument_keeps_braces_a_paragraph_break_a_hash_and_a_nested_call() {
    under_each_engine(a_packed_argument_keeps_braces_a_paragraph_break_a_hash_and_a_nested_call_under);
}

fn a_packed_argument_keeps_braces_a_paragraph_break_a_hash_and_a_nested_call_under() {
    // What the packing must not damage. Each argument sits in a pair of braces
    // of its own inside the one group, which is what keeps arbitrary content
    // whole: a brace group of the document's own, a paragraph break (the
    // definition is long, and so is the accessor), an escaped `#`, and a call
    // of another recomposed definition. The accessor hands an argument's
    // content back untouched, so what each prints is what the same content
    // prints written out.
    let golden = packed();
    let rest = "{b}{c}{d}{e}{f}{g}{h}{i}{j}{k}";
    let body = format!(
        "\\sbox0{{\\flmUDbig{{{{{{x}}y}}{rest}{{z}}}}}}\\typeout{{W braces = \\the\\wd0}}\n\
         \\sbox1{{[{{x}}y|g|z]}}\\typeout{{W bracestext = \\the\\wd1}}\n\
         \\sbox2{{\\flmUDbig{{{{a\\#b}}{rest}{{z}}}}}}\\typeout{{W hash = \\the\\wd2}}\n\
         \\sbox3{{[a\\#b|g|z]}}\\typeout{{W hashtext = \\the\\wd3}}\n\
         \\sbox4{{\\flmUDbig{{{{\\flmUDhuge{{{{1}}{{2}}{{3}}{{4}}{{5}}{{6}}{{7}}{{8}}{{9}}{{10}}{{11}}{{T}}{{13}}{{14}}{{15}}{{16}}{{17}}{{18}}{{19}}{{Y}}}}}}{rest}{{z}}}}}}\\typeout{{W nested = \\the\\wd4}}\n\
         \\sbox5{{[(1/T/Y)|g|z]}}\\typeout{{W nestedtext = \\the\\wd5}}\n\
         \\flmUDbig{{{{first paragraph\n\npar second paragraph}}{rest}{{z}}}}\n"
    );
    let result =
        compile_document("userdefs-packed-content", "content", golden.style.as_slice(), "", &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    // The paragraph break inside a packed argument is read rather than
    // stopping the compilation.
    assert!(!result.log.contains("Paragraph ended before"), "{}", result.log);
    let w = typeout_values(&result.log, "W ");
    for (call, written) in [("braces", "bracestext"), ("hash", "hashtext"), ("nested", "nestedtext")]
    {
        assert_eq!(
            width(&w, call, &result.log),
            width(&w, written, &result.log),
            "the packed `{call}` does not print what the same content prints written out"
        );
    }
}

#[test]
fn the_flag_and_blank_tests_of_a_packed_definition_answer_what_flm_answers() {
    under_each_engine(the_flag_and_blank_tests_of_a_packed_definition_answer_what_flm_answers_under);
}

fn the_flag_and_blank_tests_of_a_packed_definition_answer_what_flm_answers_under() {
    // A packed definition reads a flag with `\flmIfArgTF` and a blank test
    // with `\flmIfArgBlankTF` rather than with LaTeX's `\IfBooleanTF` and the
    // style file's plain blank test, because neither of those can be handed
    // one part of a group: a test that reads the accessor directly stops the
    // compilation ("Invalid argument") or answers the wrong branch without a
    // word. The two helpers expand the accessor first, and the answers are
    // FLM's own: `\six[p][q][r][s][t][u]` prints `12345+u` and a `\six` with
    // nothing written prints `-`.
    let golden = packed_flags();
    let all = "{p}{q}{r}{s}{t}{u}{\\BooleanTrue}{\\BooleanTrue}{\\BooleanTrue}{\\BooleanTrue}{\\BooleanTrue}";
    let none = "{}{}{}{}{}{}{\\BooleanFalse}{\\BooleanFalse}{\\BooleanFalse}{\\BooleanFalse}{\\BooleanFalse}";
    let body = format!(
        "{}\n\
         \\sbox0{{\\flmUDsix{{{all}}}}}\\typeout{{W written = \\the\\wd0}}\n\
         \\sbox1{{12345+u}}\\typeout{{W writtentext = \\the\\wd1}}\n\
         \\sbox2{{\\flmUDsix{{{none}}}}}\\typeout{{W absent = \\the\\wd2}}\n\
         \\sbox3{{-}}\\typeout{{W absenttext = \\the\\wd3}}\n",
        golden.output
    );
    let result =
        compile_document("userdefs-packed-flags", "flags", golden.style.as_slice(), "", &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let w = typeout_values(&result.log, "W ");
    assert_eq!(width(&w, "written", &result.log), width(&w, "writtentext", &result.log));
    assert_eq!(width(&w, "absent", &result.log), width(&w, "absenttext", &result.log));
    assert_ne!(width(&w, "written", &result.log), width(&w, "absent", &result.log));
    assert!(!result.log.contains("There were undefined references"), "{}", result.log);
}

#[test]
fn a_test_that_reads_a_packed_argument_directly_errors_or_answers_the_wrong_branch() {
    under_each_engine(a_test_that_reads_a_packed_argument_directly_errors_or_answers_the_wrong_branch_under);
}

fn a_test_that_reads_a_packed_argument_directly_errors_or_answers_the_wrong_branch_under() {
    // The reason the two test helpers exist, pinned so that no later change
    // writes a direct read instead. Handed the accessor itself rather than
    // what it stands for, `\IfBooleanTF` stops the compilation with "Invalid
    // argument", and the plain blank test answers the wrong branch **without a
    // word**: it looks at the accessor's own tokens, which are never nothing,
    // so it calls an empty argument full.
    //
    // The document writes both answers into the log and is compiled with the
    // error tolerated, since the first of the two is an error on purpose.
    let golden = packed_flags();
    let body = "\\ExplSyntaxOn\n\
                \\providecommand\\flmIfBlankTF[3]{\\tl_if_empty:nTF {#1} {#2} {#3}}\n\
                \\ExplSyntaxOff\n\
                \\typeout{DIRECT-BLANK:\\flmIfBlankTF{\\flmArg{1}{{}{x}}}{BLANK}{NOTBLANK}}\n\
                \\typeout{HELPER-BLANK:\\flmIfArgBlankTF{1}{{}{x}}{BLANK}{NOTBLANK}}\n\
                \\typeout{HELPER-FLAG:\\flmIfArgTF{2}{{q}{\\BooleanTrue}}{YES}{NO}}\n\
                \\typeout{DIRECT-FLAG-BEFORE}\n\
                \\IfBooleanTF{\\flmArg{2}{{q}{\\BooleanTrue}}}{YES}{NO}\n\
                \\typeout{DIRECT-FLAG-AFTER}\n";
    let result =
        compile_document("userdefs-packed-traps", "traps", golden.style.as_slice(), "", body);
    // The direct flag test is an error, which is the fact being pinned.
    assert!(
        result.log.contains("Invalid argument"),
        "a direct `\\IfBooleanTF` over the accessor did not stop the compilation:\n{}",
        result.log
    );
    // The direct blank test says "not blank" about an argument that holds
    // nothing, and the helper says "blank" about the same one.
    assert!(result.log.contains("DIRECT-BLANK:NOTBLANK"), "{}", result.log);
    assert!(result.log.contains("HELPER-BLANK:BLANK"), "{}", result.log);
    assert!(result.log.contains("HELPER-FLAG:YES"), "{}", result.log);
}

#[test]
fn a_packed_environment_reads_an_argument_in_its_end_code() {
    under_each_engine(a_packed_environment_reads_an_argument_in_its_end_code_under);
}

fn a_packed_environment_reads_an_argument_in_its_end_code_under() {
    // An environment is never expandable, so a packed one declares one `+m`
    // holding every argument and reads out of it in its begin code and in its
    // end code alike — which is the thing an environment's arguments have to
    // survive: they are read before the body and used after it.
    let golden = packed_environment();
    let result = compile_document(
        "userdefs-packed-environment",
        "environment",
        golden.style.as_slice(),
        "",
        &format!("\\tableofcontents\n{}", golden.output),
    );
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    assert!(!result.log.contains("There were undefined references"), "{}", result.log);
    // The packed call inside the heading reached the table of contents.
    assert!(result.toc.contains("[A|G|L]"), "{}", result.toc);
}

#[test]
fn a_packed_call_inside_an_edef_leaves_the_definitions_text_behind() {
    under_each_engine(a_packed_call_inside_an_edef_leaves_the_definitions_text_behind_under);
}

fn a_packed_call_inside_an_edef_leaves_the_definitions_text_behind_under() {
    // The study's expandability check: `\edef` fixes a macro's replacement
    // text by expanding what it is given, so nothing that is not fully
    // expandable survives it. A packed call is a plain macro over one group
    // and the accessor is the kernel's own expandable `\tl_item:nn`, so the
    // whole call expands and what is stored is the definition's text.
    let golden = packed();
    let body = "\\edef\\stored{\\flmUDbig{{one}{b}{c}{d}{e}{f}{seven}{h}{i}{j}{k}{twelve}}}\n\
                \\typeout{EDEF:\\meaning\\stored}\n";
    let result =
        compile_document("userdefs-packed-edef", "edef", golden.style.as_slice(), "", body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    assert!(result.log.contains("EDEF:macro:->[one|seven|twelve]"), "{}", result.log);
}

// --- optional arguments anywhere, and what follows a call ---------------------------------------

/// The golden whose definitions take an optional argument at a position other
/// than the first: `\around[pre]{key}[post]` with `post` defaulting to `p.`, and
/// `\range{from}[step]{to}` with `step` defaulting to `one`, both recomposed as
/// three mandatory groups, one of them used inside a heading title.
fn optional_anywhere() -> Golden {
    split(&fs::read_to_string(goldens().join("userdefs-optional-anywhere.tex")).expect("the golden"))
}

#[test]
fn a_recomposed_call_with_optional_arguments_reaches_the_table_of_contents() {
    under_each_engine(a_recomposed_call_with_optional_arguments_reaches_the_table_of_contents_under);
}

fn a_recomposed_call_with_optional_arguments_reaches_the_table_of_contents_under() {
    // The form this golden adds — an optional argument that is not the first,
    // and a default on any of them — inside a heading title, which is a moving
    // argument. Every argument is written in a group of its own at the call,
    // the omitted ones holding their declared defaults, and the call expands
    // into the table-of-contents line as what the definition stands for.
    let golden = optional_anywhere();
    let body = format!("\\tableofcontents\n{}", golden.output);
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result = compile_document(
            "userdefs-optional-anywhere-toc",
            "toc",
            golden.style.as_slice(),
            preamble,
            &body,
        );
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        // `\around[see]{Knuth}` stands for `\textbf{see}\emph{Knuth}\textbf{p.}`,
        // and `\range{1}[2]{9}` for `1 to 9 by 2`.
        assert!(result.toc.contains("\\textbf {see}\\emph {Knuth}\\textbf {p.}"), "{}", result.toc);
        assert!(result.toc.contains("1 to 9 by 2"), "{}", result.toc);
        assert!(!result.toc.contains("flmUDaround"), "{}", result.toc);
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
    }
}

/// The `PROBE` lines a probe document writes to the log, as a map from the key
/// to what followed the `=`.
fn typeout_values(log: &str, prefix: &str) -> BTreeMap<String, String> {
    log.lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .filter_map(|rest| rest.split_once(" = "))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

#[test]
fn nothing_follows_a_canonical_call_that_latex_could_read_as_an_argument() {
    under_each_engine(nothing_follows_a_canonical_call_that_latex_could_read_as_an_argument_under);
}

fn nothing_follows_a_canonical_call_that_latex_could_read_as_an_argument_under() {
    // Why a canonical call needs no stop token after it, put to the compiler.
    // A command that declares an optional argument makes LaTeX look for a `[`
    // in whatever follows a call that left it out — skipping spaces and
    // comments as it looks — so an ordinary bracket the document wrote is
    // swallowed and printed as the argument instead. A recomposed definition
    // declares no optional argument at all: every effective argument is a
    // mandatory group the call writes, so there is nothing to look for and the
    // bracket is printed where the document put it, after a space, after a
    // comment, and across the comment line that joins two parsed fragments of
    // the recomposed document alike.
    //
    // Each case sets a box and writes its width to the log; the widths are
    // compared with the widths of what the two readings would print.
    let body = r"\DeclareDocumentCommand\flmLooks{m O{}}{[#1|#2]}
\providecommand\flmCanon[2]{[#1|#2]}
\sbox0{\flmLooks{a}[kept]}\typeout{W swallowed = \the\wd0}
\sbox1{[a|kept]}\typeout{W asargument = \the\wd1}
\sbox2{\flmCanon{a}{}[kept]}\typeout{W plain = \the\wd2}
\sbox3{[a|][kept]}\typeout{W astext = \the\wd3}
\sbox4{\flmCanon{a}{} [kept]}\typeout{W space = \the\wd4}
\sbox5{[a|] [kept]}\typeout{W spacetext = \the\wd5}
\sbox6{\flmCanon{a}{}%
[kept]}\typeout{W comment = \the\wd6}
\sbox7{\flmCanon{a}{}%%
[kept]}\typeout{W fragment = \the\wd7}
Nothing is typeset.
";
    let result = compile_document("canonical-call", "probe", &[], "", body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let w = typeout_values(&result.log, "W ");
    // The command with an optional argument swallowed the bracket and printed
    // its content as the argument: that is the hazard the call shape removes.
    assert_eq!(width(&w, "swallowed", &result.log), width(&w, "asargument", &result.log));
    // The canonical call printed the bracket as text, in every position. A
    // comment before it is discarded with its line end, which is why the
    // comment case has the width of the plain one rather than of the spaced
    // one, and the fragment boundary — the `%%` the recomposed document writes
    // between two parsed fragments — behaves the same way.
    assert_eq!(width(&w, "plain", &result.log), width(&w, "astext", &result.log));
    assert_eq!(width(&w, "space", &result.log), width(&w, "spacetext", &result.log));
    assert_eq!(width(&w, "comment", &result.log), width(&w, "astext", &result.log));
    assert_eq!(width(&w, "fragment", &result.log), width(&w, "astext", &result.log));
    // The two readings really do differ, so the equalities say something.
    assert_ne!(width(&w, "asargument", &result.log), width(&w, "astext", &result.log));
}

// --- a computed default --------------------------------------------------------------------------

/// The golden whose definitions declare a default that is not plain content:
/// `\flmUDtag`, whose omitted short form defaults to the other argument, and
/// `\flmUDmarked`, whose omitted mark defaults to a conditional over the other
/// argument. Both are recomposed as two mandatory groups, with one call inside
/// a heading title.
fn computed_default() -> Golden {
    split(
        &fs::read_to_string(goldens().join("userdefs-computed-default.tex")).expect("the golden"),
    )
}

#[test]
fn a_computed_default_prints_what_flm_prints_and_reaches_a_heading() {
    under_each_engine(a_computed_default_prints_what_flm_prints_and_reaches_a_heading_under);
}

fn a_computed_default_prints_what_flm_prints_and_reaches_a_heading_under() {
    // The form this golden adds — a default resolved at the use site and
    // written into the call — inside a heading title, which is a moving
    // argument. Nothing in the style file knows about defaults: the
    // definitions take two mandatory groups each, so what reaches the table of
    // contents is what the resolved call stands for.
    //
    // The widths say what the calls set, measured against what the expanded
    // document prints for the same use sites: the short form defaulting to the
    // long title, the mark defaulting to the bold text, and the mark
    // defaulting to `empty` where the text is blank.
    let golden = computed_default();
    let body = format!(
        "\\tableofcontents\n{}\n\
         \\sbox0{{\\flmUDtag{{A long title}}{{A long title}}}}\\typeout{{W default = \\the\\wd0}}\n\
         \\sbox1{{[A long title] A long title}}\\typeout{{W defaulttext = \\the\\wd1}}\n\
         \\sbox2{{\\flmUDtag{{TL;DR}}{{A long title}}}}\\typeout{{W written = \\the\\wd2}}\n\
         \\sbox3{{[TL;DR] A long title}}\\typeout{{W writtentext = \\the\\wd3}}\n\
         \\sbox4{{\\flmUDmarked{{\\textbf{{shout}}}}{{shout}}}}\\typeout{{W bold = \\the\\wd4}}\n\
         \\sbox5{{(\\textbf{{shout}}) shout}}\\typeout{{W boldtext = \\the\\wd5}}\n\
         \\sbox6{{\\flmUDmarked{{empty}}{{}}}}\\typeout{{W blank = \\the\\wd6}}\n\
         \\sbox7{{(empty) }}\\typeout{{W blanktext = \\the\\wd7}}\n",
        golden.output
    );
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result = compile_document(
            "userdefs-computed-default",
            if hyperref { "heading-hyperref" } else { "heading-plain" },
            golden.style.as_slice(),
            preamble,
            &body,
        );
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
        // The call in the heading expanded into the table-of-contents line as
        // what it stands for, the resolved default included.
        assert!(result.toc.contains("[A short heading] A short heading"), "{}", result.toc);
        assert!(!result.toc.contains("flmUDtag"), "{}", result.toc);
        let w = typeout_values(&result.log, "W ");
        assert_eq!(width(&w, "default", &result.log), width(&w, "defaulttext", &result.log));
        assert_eq!(width(&w, "written", &result.log), width(&w, "writtentext", &result.log));
        assert_eq!(width(&w, "bold", &result.log), width(&w, "boldtext", &result.log));
        assert_eq!(width(&w, "blank", &result.log), width(&w, "blanktext", &result.log));
        // The resolved default and the written argument really do differ, so
        // the equalities above say something.
        assert_ne!(width(&w, "default", &result.log), width(&w, "written", &result.log));
    }
}

// --- the star argument and the conditionals -----------------------------------------------------

/// The golden whose definitions take a `*` marker, recomposed as three
/// effective arguments each — two values and a flag: `\flmUDemphasize` and
/// `\flmUDmark`, one of them used inside a heading title.
fn star() -> Golden {
    split(&fs::read_to_string(goldens().join("userdefs-star.tex")).expect("the golden"))
}

/// The golden whose one definition asks whether its optional argument was
/// written: `\flmUDopt`, two values and a flag, used inside a heading title,
/// with the argument written and left out.
fn conditional_value() -> Golden {
    split(
        &fs::read_to_string(goldens().join("userdefs-conditional-value.tex")).expect("the golden"),
    )
}

/// The golden whose one definition asks whether its argument is blank:
/// `\flmUDb`, one argument, over the six spellings the equivalence rests on.
fn blank() -> Golden {
    split(&fs::read_to_string(goldens().join("userdefs-blank.tex")).expect("the golden"))
}

/// The width of one `\sbox` the probe wrote to the log under `prefix`.
fn width(values: &BTreeMap<String, String>, key: &str, log: &str) -> String {
    values
        .get(key)
        .unwrap_or_else(|| panic!("no width for `{key}` in the log:\n{log}"))
        .clone()
}

#[test]
fn a_recomposed_star_argument_reaches_a_heading_and_says_what_flm_says() {
    under_each_engine(a_recomposed_star_argument_reaches_a_heading_and_says_what_flm_says_under);
}

fn a_recomposed_star_argument_reaches_a_heading_and_says_what_flm_says_under() {
    // The form this golden adds — a `*` marker written in the argument's own
    // group and a flag for the question asked about it — inside a heading
    // title, which is a moving argument. The call expands into the
    // table-of-contents line as what it stands for, so the star's own text is
    // there and the call's name is not.
    //
    // The widths say what the calls set: `\flmUDemphasize{*}{bold}{\BooleanTrue}`
    // is the bold text and `\flmUDemphasize{}{plain}{\BooleanFalse}` the
    // emphasized text, and `\flmUDmark` writes `!?` after its argument only
    // where the star was written — which is what the expanded document prints.
    let golden = star();
    let body = format!(
        "\\tableofcontents\n{}\n\
         \\sbox0{{\\flmUDemphasize{{*}}{{bold}}{{\\BooleanTrue}}}}\\typeout{{W star = \\the\\wd0}}\n\
         \\sbox1{{\\textbf{{bold}}}}\\typeout{{W bold = \\the\\wd1}}\n\
         \\sbox2{{\\flmUDemphasize{{}}{{bold}}{{\\BooleanFalse}}}}\\typeout{{W nostar = \\the\\wd2}}\n\
         \\sbox3{{\\emph{{bold}}}}\\typeout{{W emph = \\the\\wd3}}\n\
         \\sbox4{{\\flmUDmark{{x}}{{*}}{{\\BooleanTrue}}}}\\typeout{{W marked = \\the\\wd4}}\n\
         \\sbox5{{x!?}}\\typeout{{W markedtext = \\the\\wd5}}\n\
         \\sbox6{{\\flmUDmark{{x}}{{}}{{\\BooleanFalse}}}}\\typeout{{W plain = \\the\\wd6}}\n\
         \\sbox7{{x}}\\typeout{{W plaintext = \\the\\wd7}}\n",
        golden.output
    );
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result =
            compile_document("userdefs-star-toc", "toc", golden.style.as_slice(), preamble, &body);
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        assert!(result.toc.contains("\\textbf {loud}"), "{}", result.toc);
        assert!(!result.toc.contains("flmUDemphasize"), "{}", result.toc);
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
        let w = typeout_values(&result.log, "W ");
        assert_eq!(width(&w, "star", &result.log), width(&w, "bold", &result.log));
        assert_eq!(width(&w, "nostar", &result.log), width(&w, "emph", &result.log));
        assert_eq!(width(&w, "marked", &result.log), width(&w, "markedtext", &result.log));
        assert_eq!(width(&w, "plain", &result.log), width(&w, "plaintext", &result.log));
        // The two really do differ, so the equalities above say something.
        assert_ne!(width(&w, "star", &result.log), width(&w, "nostar", &result.log));
    }
}

#[test]
fn an_omitted_argument_is_its_default_and_the_flag_answers_what_flm_answers() {
    under_each_engine(an_omitted_argument_is_its_default_and_the_flag_answers_what_flm_answers_under);
}

fn an_omitted_argument_is_its_default_and_the_flag_answers_what_flm_answers_under() {
    // A definition that asks whether its optional argument was written takes
    // one more effective argument, a flag; the argument's own group holds what
    // FLM substitutes there, which for an argument with no declared default is
    // nothing at all. So the call that leaves the argument out sets exactly
    // what the expanded document prints, and LaTeX's `-NoValue-` marker — the
    // value the earlier form had to guard every placeholder against — appears
    // nowhere.
    let golden = conditional_value();
    let body = format!(
        "{}\n\
         \\sbox0{{\\flmUDopt{{x}}{{y}}{{\\BooleanTrue}}}}\\typeout{{W written = \\the\\wd0}}\n\
         \\sbox1{{<x|V|F|y>}}\\typeout{{W writtentext = \\the\\wd1}}\n\
         \\sbox2{{\\flmUDopt{{}}{{z}}{{\\BooleanFalse}}}}\\typeout{{W absent = \\the\\wd2}}\n\
         \\sbox3{{<none|||z>}}\\typeout{{W absenttext = \\the\\wd3}}\n",
        golden.output
    );
    let result =
        compile_document("userdefs-conditional-value", "flag", golden.style.as_slice(), "", &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let w = typeout_values(&result.log, "W ");
    assert_eq!(width(&w, "written", &result.log), width(&w, "writtentext", &result.log));
    assert_eq!(width(&w, "absent", &result.log), width(&w, "absenttext", &result.log));
    assert_ne!(width(&w, "written", &result.log), width(&w, "absent", &result.log));
    assert!(!result.log.contains("-NoValue-"), "{}", result.log);
}

#[test]
fn the_blank_test_of_a_recomposed_definition_answers_what_flms_own_test_answers() {
    under_each_engine(the_blank_test_of_a_recomposed_definition_answers_what_flms_own_test_answers_under);
}

fn the_blank_test_of_a_recomposed_definition_answers_what_flms_own_test_answers_under() {
    // The equivalence the blank-test piece rests on, put to the compiler. FLM
    // calls content blank when it is empty or holds nothing but comments, and
    // counts whitespace as content; `\tl_if_empty:nTF` is true only for a
    // token list with no token at all. They agree because of what TeX does
    // while it reads an argument: a comment character discards the rest of the
    // line and the line end with it, and the spaces that open the next line
    // are skipped, so a comment never becomes a token.
    //
    // The eight calls measured are the recomposer's own bytes, cut out of the
    // golden's recomposed LaTeX rather than written again here, so that what the
    // compiler answers is what the output says and not a spelling of this
    // test's own — the document's comment prefix is `%%`, for one. They are
    // the eight arguments `tests/latex_userdefs.rs` puts to FLM's own test
    // through the `text` backend, which answers BLANK, FULL, BLANK, FULL,
    // BLANK, FULL, BLANK, FULL in this order. Each is set in a box whose width
    // goes to the log, and compared with the widths of the two words the
    // definition can print.
    let golden = blank();
    let keys = ["one", "two", "three", "four", "five", "six", "seven", "eight"];
    // The recomposed LaTeX writes the eight calls as `1:<call> 2:<call> …`; the cut
    // runs from each marker to the next, and the last to the blank line that
    // ends the paragraph.
    let mut calls: Vec<String> = Vec::new();
    for (at, key) in keys.iter().enumerate() {
        let marker = format!("{}:", at + 1);
        let from = golden
            .output
            .find(&marker)
            .unwrap_or_else(|| {
                panic!("`{marker}` is not in the recomposed LaTeX:\n{}", golden.output)
            })
            + marker.len();
        let rest = &golden.output[from..];
        let to = match keys.get(at + 1) {
            Some(_) => rest.find(&format!(" {}:", at + 2)).expect("the next marker"),
            None => rest.find("\n\n").expect("the end of the paragraph"),
        };
        let call = rest[..to].to_string();
        assert!(call.contains("\\flmUD"), "{key}: `{call}` is no call");
        calls.push(call);
    }
    let mut body = String::from(
        "\\sbox0{BLANK}\\typeout{W blank = \\the\\wd0}\n\
         \\sbox1{FULL}\\typeout{W full = \\the\\wd1}\n",
    );
    for (key, call) in keys.iter().zip(&calls) {
        body.push_str(&format!("\\sbox2{{{call}}}\\typeout{{W {key} = \\the\\wd2}}\n"));
    }
    let result = compile_document("userdefs-blank", "blank", golden.style.as_slice(), "", &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let w = typeout_values(&result.log, "W ");
    let blank_width = width(&w, "blank", &result.log);
    let full_width = width(&w, "full", &result.log);
    assert_ne!(blank_width, full_width);
    // The order is FLM's own: nothing, one space, a comment alone, a comment
    // then text, a comment then whitespace, whitespace then a comment, and the
    // two `\notblank` cases.
    for (key, expected) in keys.iter().zip([
        &blank_width,
        &full_width,
        &blank_width,
        &full_width,
        &blank_width,
        &full_width,
        &blank_width,
        &full_width,
    ]) {
        assert_eq!(&width(&w, key, &result.log), expected, "case {key}:\n{}", result.log);
    }
}

// --- one name declared for two parsing modes: the mode twins -------------------------------------

/// The golden whose one name is declared twice, once per parsing mode, and
/// used in a heading title in both modes: `\flmUTDvec` and `\flmUMDvec`.
fn mode_twins_heading() -> Golden {
    split(
        &fs::read_to_string(goldens().join("userdefs-mode-twins-heading.tex")).expect("the golden"),
    )
}

#[test]
fn a_mode_twin_chosen_in_a_heading_reaches_the_table_of_contents_as_its_own_text() {
    under_each_engine(a_mode_twin_chosen_in_a_heading_reaches_the_table_of_contents_as_its_own_text_under);
}

fn a_mode_twin_chosen_in_a_heading_reaches_the_table_of_contents_as_its_own_text_under() {
    // The case that decides there is no `\ifmmode` dispatcher in the output. A
    // heading title holding both a text-mode and a mathematics-mode use of one
    // name is written to the table of contents and read back on a later run,
    // and that reading is outside mathematics whatever the title said — so a
    // test asked there would answer the wrong question for the mathematics use.
    // The recomposition chose while it read the document, so each twin's own
    // name stands in the title, and what reaches the table of contents is each
    // twin's own text.
    let golden = mode_twins_heading();
    let body = format!(
        "\\tableofcontents\n{}\n\
         \\sbox0{{\\flmUTDvec{{v}}}}\\typeout{{W text = \\the\\wd0}}\n\
         \\sbox1{{\\textbf{{v}}}}\\typeout{{W bold = \\the\\wd1}}\n\
         \\sbox2{{\\(\\flmUMDvec{{}}{{w}}\\)}}\\typeout{{W math = \\the\\wd2}}\n\
         \\sbox3{{\\(\\mathbf{{w}}\\)}}\\typeout{{W mathbf = \\the\\wd3}}\n",
        golden.output
    );
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result = compile_document(
            "userdefs-mode-twins-heading",
            "toc",
            golden.style.as_slice(),
            preamble,
            &body,
        );
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        // Each twin expanded into the table-of-contents line as what it stands
        // for; neither name is there, and neither is the other twin's text.
        assert!(result.toc.contains("\\textbf {v}"), "{}", result.toc);
        assert!(result.toc.contains("\\mathbf {w}"), "{}", result.toc);
        assert!(!result.toc.contains("flmUTDvec"), "{}", result.toc);
        assert!(!result.toc.contains("flmUMDvec"), "{}", result.toc);
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
        let w = typeout_values(&result.log, "W ");
        assert_eq!(width(&w, "text", &result.log), width(&w, "bold", &result.log));
        assert_eq!(width(&w, "math", &result.log), width(&w, "mathbf", &result.log));
        // The two twins really do set different things, so the equalities say
        // something.
        assert_ne!(width(&w, "text", &result.log), width(&w, "math", &result.log));
    }
}

// --- verbatim arguments -------------------------------------------------------------------------

/// The golden whose one definition takes a verbatim argument: `\flmUDask`,
/// asking whether it is blank, over a verbatim argument holding a letter and
/// over an empty one.
fn verbatim_blank() -> Golden {
    split(&fs::read_to_string(goldens().join("userdefs-verbatim-blank.tex")).expect("the golden"))
}

/// The golden whose one definition takes a verbatim argument and is used in a
/// heading title: `\flmUDcode`, one argument.
fn verbatim_argument() -> Golden {
    split(&fs::read_to_string(goldens().join("userdefs-verbatim.tex")).expect("the golden"))
}

/// The golden whose one definition holds verbatim text with a number sign in
/// its own template: `\flmUDsharp`, one argument.
fn verbatim_in_template() -> Golden {
    split(
        &fs::read_to_string(goldens().join("userdefs-verbatim-in-template.tex"))
            .expect("the golden"),
    )
}

#[test]
fn the_blank_test_over_a_verbatim_argument_answers_what_flms_own_test_answers() {
    under_each_engine(the_blank_test_over_a_verbatim_argument_answers_what_flms_own_test_answers_under);
}

fn the_blank_test_over_a_verbatim_argument_answers_what_flms_own_test_answers_under() {
    // A verbatim argument stands for its characters, so FLM's blank test
    // answers "not blank" for one holding a letter and "blank" for an empty
    // one. The value group a call writes holds those characters — nothing at
    // all for the empty one — so LaTeX's test answers the same. The two calls
    // measured are the recomposer's own bytes, cut out of the golden's
    // recomposed LaTeX, and FLM's own answers for them are `1:FULL 2:BLANK`
    // (`tests/latex_userdefs.rs`).
    let golden = verbatim_blank();
    let mut calls: Vec<String> = Vec::new();
    for at in 1..=2usize {
        let marker = format!("{at}:");
        let from = golden
            .output
            .find(&marker)
            .unwrap_or_else(|| panic!("`{marker}` is not in:\n{}", golden.output))
            + marker.len();
        let rest = &golden.output[from..];
        let to = match at {
            1 => rest.find(" 2:").expect("the next marker"),
            _ => rest.find('\n').expect("the end of the line"),
        };
        let call = rest[..to].to_string();
        assert!(call.contains("\\flmUDask"), "`{call}` is no call");
        calls.push(call);
    }
    let mut body = String::from(
        "\\sbox0{BLANK}\\typeout{W blank = \\the\\wd0}\n\
         \\sbox1{FULL}\\typeout{W full = \\the\\wd1}\n",
    );
    for (key, call) in ["one", "two"].iter().zip(&calls) {
        body.push_str(&format!("\\sbox2{{{call}}}\\typeout{{W {key} = \\the\\wd2}}\n"));
    }
    let result =
        compile_document("userdefs-verbatim-blank", "blank", golden.style.as_slice(), "", &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let w = typeout_values(&result.log, "W ");
    let blank_width = width(&w, "blank", &result.log);
    let full_width = width(&w, "full", &result.log);
    assert_ne!(blank_width, full_width);
    // What FLM answers: the letter is not blank, the empty argument is.
    assert_eq!(width(&w, "one", &result.log), full_width, "{}", result.log);
    assert_eq!(width(&w, "two", &result.log), blank_width, "{}", result.log);
}

#[test]
fn a_verbatim_argument_in_a_heading_reaches_the_table_of_contents() {
    under_each_engine(a_verbatim_argument_in_a_heading_reaches_the_table_of_contents_under);
}

fn a_verbatim_argument_in_a_heading_reaches_the_table_of_contents_under() {
    // The standing requirement of piece 2, measured for a verbatim argument: a
    // section title is a moving argument, written to the table of contents and
    // read back on a later run. The argument stands for its characters, which
    // the value group a call writes holds as text, so they survive the trip:
    // the table-of-contents line holds the escaped text `t\_1`, and it holds
    // no call, since a recomposed macro is expandable.
    let golden = verbatim_argument();
    let body = format!(
        "\\tableofcontents\n{}\n\
         \\sbox0{{\\flmUDcode{{t\\_1}}}}\\typeout{{W call = \\the\\wd0}}\n\
         \\sbox1{{\\textbf{{t\\_1}}}}\\typeout{{W text = \\the\\wd1}}\n\
         \\sbox2{{\\textbf{{t1}}}}\\typeout{{W plain = \\the\\wd2}}\n",
        golden.output
    );
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result = compile_document(
            "userdefs-verbatim",
            "toc",
            golden.style.as_slice(),
            preamble,
            &body,
        );
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        // The escaped text, and no call.
        assert!(result.toc.contains("t\\_1"), "{}", result.toc);
        assert!(!result.toc.contains("flmUDcode"), "{}", result.toc);
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
        let w = typeout_values(&result.log, "W ");
        assert_eq!(width(&w, "call", &result.log), width(&w, "text", &result.log), "{}", result.log);
        // The underscore really is set, so the equality above says something.
        assert_ne!(width(&w, "call", &result.log), width(&w, "plain", &result.log));
    }
}

#[test]
fn verbatim_text_in_a_definitions_body_needs_no_hash_doubling() {
    under_each_engine(verbatim_text_in_a_definitions_body_needs_no_hash_doubling_under);
}

fn verbatim_text_in_a_definitions_body_needs_no_hash_doubling_under() {
    // A literal `#` inside a LaTeX definition's body is read as a parameter
    // number unless it is written twice. The recomposition never writes one:
    // ordinary text escapes `#` as `\#`, and verbatim content is written in
    // the encoding `\EscVerb` reads, which puts a backslash before every
    // printable character that is not a letter or a digit. What reaches the
    // body is therefore the control sequence `\#`, one token, which the
    // definition scanner never reads as a parameter. The golden compiles at
    // all — which is the first half of the proof — and the call sets exactly
    // what the same text set outside a definition, which is the second.
    let golden = verbatim_in_template();
    let body = format!(
        "\\tableofcontents\n{}\n\
         \\sbox0{{\\flmUDsharp{{t}}}}\\typeout{{W call = \\the\\wd0}}\n\
         \\sbox1{{\\EscVerb[formatcom=\\flmFmtVRB{{verbcode}}]{{a\\#b}} and \\# and t}}\
         \\typeout{{W text = \\the\\wd1}}\n\
         \\sbox2{{\\EscVerb[formatcom=\\flmFmtVRB{{verbcode}}]{{ab}} and \\# and t}}\
         \\typeout{{W nohash = \\the\\wd2}}\n",
        golden.output
    );
    let result =
        compile_document("userdefs-verbatim-in-template", "hash", golden.style.as_slice(), "", &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let w = typeout_values(&result.log, "W ");
    assert_eq!(width(&w, "call", &result.log), width(&w, "text", &result.log), "{}", result.log);
    // The number sign really is set, so the equality above says something.
    assert_ne!(width(&w, "call", &result.log), width(&w, "nohash", &result.log));
    // The definition is expandable, so the table-of-contents line holds what
    // it stands for — the verbatim text with its number sign — and not the
    // call's name; the `#` survived the trip through the auxiliary file.
    assert!(result.toc.contains("a\\#b"), "{}", result.toc);
    assert!(!result.toc.contains("flmUDsharp"), "{}", result.toc);
}

#[test]
fn a_reference_to_an_equation_line_resolves_to_that_line_in_latex() {
    under_each_engine(a_reference_to_an_equation_line_resolves_to_that_line_in_latex_under);
}

fn a_reference_to_an_equation_line_resolves_to_that_line_in_latex_under() {
    // A label on the third line of an `align` is written at that line's end,
    // and the reference to it names the same key: LaTeX pairs the two and
    // resolves the reference to the third line's number.
    let golden = split(
        &fs::read_to_string(goldens().join("math-line-reference.tex")).expect("the golden"),
    );
    for hyperref in [false, true] {
        let result = compile("math-line-reference", &golden, hyperref);
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        assert!(result.aux.contains("\\newlabel{ref1}"), "{}", result.aux);
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
        assert!(!result.log.contains("Multiple \\label"), "{}", result.log);
    }
}

#[test]
fn several_labels_on_one_equation_line_all_resolve_to_it() {
    under_each_engine(several_labels_on_one_equation_line_all_resolve_to_it_under);
}

fn several_labels_on_one_equation_line_all_resolve_to_it_under() {
    // `amsmath` keeps one label per line. The labels hook runs the first and
    // writes the rest to the auxiliary file as aliases of it, so that the
    // second run resolves every one of them to the line's own number.
    let golden = split(
        &fs::read_to_string(goldens().join("math-two-labels.tex")).expect("the golden"),
    );
    for hyperref in [false, true] {
        let result = compile("math-two-labels", &golden, hyperref);
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        // The first label of the line is a real label; the second is an alias
        // of it, written after every `\newlabel` of the document.
        assert!(result.aux.contains("\\newlabel{ref1}"), "{}", result.aux);
        assert!(result.aux.contains("\\flm@aliaslabel{ref2}{ref1}"), "{}", result.aux);
        assert!(!result.aux.contains("\\newlabel{ref2}"), "{}", result.aux);
        let alias_at = result.aux.find("\\flm@aliaslabel{ref2}{ref1}").expect("the alias line");
        let label_at = result.aux.find("\\newlabel{ref1}").expect("the label line");
        assert!(label_at < alias_at, "the alias must be read after the label:\n{}", result.aux);
        assert!(!result.log.contains("Multiple \\label"), "{}", result.log);
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
    }
}

#[test]
fn the_reference_hook_translates_flms_reference_options() {
    under_each_engine(the_reference_hook_translates_flms_reference_options_under);
}

fn the_reference_hook_translates_flms_reference_options_under() {
    // A *reference option* is what a document writes in the brackets of
    // `\ref[…]{…}`, and the vocabulary is FLM's own. The recomposed LaTeX
    // writes those options as the document wrote them, and the hook's default
    // translates them as it calls `\zcref`: `noprefix`, which `zref-clever`
    // has no key for, becomes its `noname`, and the capitalized variant `S`,
    // which both vocabularies spell alike, travels unchanged.
    //
    // What each reference printed is read from the log. `zref-clever` puts a
    // *name* before a reference's number — "section 1" — and takes it from the
    // reference type of the counter the label recorded, in two spellings: the
    // plain one and the capitalized one it uses when the reference asks to
    // capitalize. The preamble below gives the `section` type names of its own
    // that write a line to the log as they are typeset, so the three
    // references of the golden — a plain one, a `noprefix` one and an `S` one
    // — leave one plain line (the first), one capitalized line (the third) and
    // nothing for the second, whose name was suppressed.
    let golden = split(
        &fs::read_to_string(goldens().join("refs-options-translated.tex")).expect("the golden"),
    );
    //
    // What the names say is that the reference printed no name, a plain name
    // or a capitalized one. That a `noprefix` reference prints exactly what a
    // hand-written `\zcref[noname]` prints — the number, and the number alone
    // — is measured beside it: each option is set twice at the end of the
    // document, once through the hook and once in `zref-clever`'s own
    // vocabulary, into boxes whose widths the log then carries. Equal widths
    // are the translation; the third pair, a translated `noprefix` against an
    // optionless reference, must differ, or the option would have done
    // nothing.
    let probe = "\\newcommand\\flmProbeName[1]{\\typeout{FLM-PROBE-NAME:#1}}\n\
                 \\zcRefTypeSetup{section}{name-sg={\\flmProbeName{plain}section},\n\
                 Name-sg={\\flmProbeName{capitalized}Section}}\n\
                 \\newcommand\\flmProbeWidths[3]{%\n\
                 \\sbox0{#2}\\sbox1{#3}\\typeout{FLM-PROBE-WIDTH:#1:\\the\\wd0:\\the\\wd1}}\n\
                 \\AtEndDocument{\\renewcommand\\flmProbeName[1]{}%\n\
                 \\flmProbeWidths{noprefix}{\\flmRef[noprefix]{ref1}}{\\zcref[noname]{ref1}}%\n\
                 \\flmProbeWidths{S}{\\flmRef[S]{ref1}}{\\zcref[S]{ref1}}%\n\
                 \\flmProbeWidths{noprefix-vs-plain}{\\flmRef[noprefix]{ref1}}{\\zcref{ref1}}}\n";
    for hyperref in [false, true] {
        let preamble =
            if hyperref { format!("{probe}\\usepackage{{hyperref}}\n") } else { probe.to_string() };
        let result = compile_document(
            "refs-options-translated",
            if hyperref { "hyperref" } else { "plain" },
            golden.style.as_slice(),
            &preamble,
            &golden.output,
        );
        assert!(
            result.ok,
            "(hyperref: {hyperref}) did not compile: {}",
            first_error(&result.log)
        );
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
        let printed = |marker: &str| result.log.matches(marker).count();
        assert_eq!(
            printed("FLM-PROBE-NAME:plain"),
            1,
            "(hyperref: {hyperref}) the plain name was not printed exactly once — the \
             `noprefix` reference printed one too, or the plain one printed none:\n{}",
            result.log
        );
        assert_eq!(
            printed("FLM-PROBE-NAME:capitalized"),
            1,
            "(hyperref: {hyperref}) the capitalized name was not printed exactly once — the \
             `S` option did not reach `zref-clever`:\n{}",
            result.log
        );
        let widths = |row: &str| {
            let marker = format!("FLM-PROBE-WIDTH:{row}:");
            let line = result
                .log
                .lines()
                .find(|line| line.starts_with(&marker))
                .unwrap_or_else(|| panic!("(hyperref: {hyperref}) no {marker} line:\n{}", result.log));
            let rest = &line[marker.len()..];
            let (left, right) = rest.split_once(':').expect("two widths");
            (left.to_string(), right.to_string())
        };
        for row in ["noprefix", "S"] {
            let (through_the_hook, written_by_hand) = widths(row);
            assert_eq!(
                through_the_hook, written_by_hand,
                "(hyperref: {hyperref}) `{row}` through the hook did not set what \
                 `zref-clever`'s own key sets"
            );
        }
        let (translated, optionless) = widths("noprefix-vs-plain");
        assert_ne!(
            translated, optionless,
            "(hyperref: {hyperref}) `noprefix` set the same as a reference with no option \
             at all, so the option reached nothing"
        );
    }
}

/// The golden whose style file defines the hook of a reference showing the
/// author's own words, `\flmHyperref`: the two probes below compile documents
/// of their own over it.
fn hyperref_style() -> Golden {
    split(&fs::read_to_string(goldens().join("refs-hyperref.tex")).expect("the golden"))
}

#[test]
fn the_hyperref_hook_links_where_the_document_loaded_the_package_and_prints_the_text_where_it_did_not() {
    under_each_engine(the_hyperref_hook_links_where_the_document_loaded_the_package_and_prints_the_text_where_it_did_not_under);
}
fn the_hyperref_hook_links_where_the_document_loaded_the_package_and_prints_the_text_where_it_did_not_under() {
    // `\flmHyperref{<key>}{<text>}` shows `<text>` and links it to `<key>`.
    // Making a link in a PDF takes the `hyperref` package, and whether a
    // document is compiled with it is its author's decision, so the hook's
    // default asks — about the *document's* preamble, with the kernel's
    // `\IfPackageLoadedTF` — and links through the package where the document
    // loaded it, printing the text alone where it did not.
    //
    // What makes the link visible from outside the PDF is a reference to a key
    // nothing labels: `hyperref` reports a hyper reference it could not
    // resolve, and that report exists exactly where the hook linked through
    // the package. Where it did not, nothing is undefined at all, because no
    // link was asked for — and the text of both references is typeset either
    // way, which is what the compilation succeeding says.
    let golden = hyperref_style();
    let body = "\\section{A}\\label{ref1}\n\
                Text \\flmHyperref{ref1}{shown here} end.\n\n\
                And \\flmHyperref{nosuchkey}{shown too} end.\n";
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result = compile_document(
            "refs-hyperref-hook",
            if hyperref { "hyperref" } else { "plain" },
            golden.style.as_slice(),
            preamble,
            body,
        );
        assert!(
            result.ok,
            "(hyperref: {hyperref}) did not compile: {}",
            first_error(&result.log)
        );
        assert_eq!(
            result.log.contains("Hyper reference `nosuchkey'"),
            hyperref,
            "(hyperref: {hyperref}) the hook did not link as the package's presence decides:\n{}",
            result.log
        );
        assert_eq!(
            result.log.contains("There were undefined references"),
            hyperref,
            "(hyperref: {hyperref}) the unresolved key was reported where no link was made, \
             or not reported where one was:\n{}",
            result.log
        );
    }
}

#[test]
fn a_hyperref_in_a_heading_gives_the_pdf_bookmark_its_text_alone() {
    under_each_engine(a_hyperref_in_a_heading_gives_the_pdf_bookmark_its_text_alone_under);
}
fn a_hyperref_in_a_heading_gives_the_pdf_bookmark_its_text_alone_under() {
    // A PDF bookmark is plain text: it carries no link, and `hyperref` builds
    // it by expanding the title into a string. The style file therefore says
    // what such a reference contributes to one — the text alone — inside
    // `hyperref`'s own hook for that. Without that line the safe key lands in
    // the bookmark beside the text and `hyperref` warns about the tokens it
    // could not put there.
    let golden = hyperref_style();
    let body = "\\section{A}\\label{ref1}\n\
                \\section{B \\flmHyperref{ref1}{in a title}}\n";
    let result = compile_document(
        "refs-hyperref-bookmark",
        "hyperref",
        golden.style.as_slice(),
        "\\usepackage{hyperref}\n",
        body,
    );
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    assert!(
        !result.log.contains("Token not allowed in a PDF string"),
        "the bookmark was built from tokens `hyperref` refused:\n{}",
        result.log
    );
    // The bookmark file writes each character of the string as an octal
    // escape of its two bytes, `\000B` for `B` and `\000\040` for a space, so
    // the two escapes are undone before the text is read.
    let bookmarks = result.bookmarks.replace("\\000", "").replace("\\040", " ");
    assert!(bookmarks.contains("B in a title"), "{}", result.bookmarks);
    assert!(!bookmarks.contains("ref1"), "the safe key reached the bookmark: {}", result.bookmarks);
}

#[test]
fn a_reference_to_an_unnumbered_heading_prints_its_title() {
    under_each_engine(a_reference_to_an_unnumbered_heading_prints_its_title_under);
}

fn a_reference_to_an_unnumbered_heading_prints_its_title_under() {
    // The heading hook's whole purpose: `\section*` and `\paragraph` are
    // numbered by no class at its default depth, so the labels after them
    // take the heading's title as what a reference prints.
    let golden = split(
        &fs::read_to_string(goldens().join("headings-unnumbered.tex")).expect("the golden"),
    );
    for hyperref in [false, true] {
        let result = compile("headings-unnumbered", &golden, hyperref);
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        // The label records the title, not the number of what was numbered
        // last: LaTeX's own `\newlabel` carries it as the reference text, and
        // `zref`'s record carries it as the default `\zcref` prints.
        for title in ["An unnumbered section", "A paragraph heading"] {
            assert!(
                result.aux.contains(&format!("\\default{{{title}}}")),
                "the label of {title:?} does not carry it as what a reference prints:\n{}",
                result.aux
            );
            // `\newlabel{ref1}{{<title>}{<page>}…}`: the title is the first
            // field, so the text to find is `{{` before it and `}` after.
            let reference_text = String::from("{{") + title + "}";
            assert!(
                result.aux.contains(&reference_text),
                "the label of {title:?} is not the reference text of `\\newlabel`:\n{}",
                result.aux
            );
        }
        // Its counter is the private one, under the reference type that
        // prints the text alone.
        assert!(result.aux.contains("\\zc@counter{flm@internal@refcnt}"), "{}", result.aux);
        assert!(result.aux.contains("\\zc@type{flm--internal--ref}"), "{}", result.aux);
        // With `hyperref`, a link lands on the heading's own anchor, the one
        // the sectioning command placed at the title — not on an anchor of
        // the private counter, which the display-text hook does not place.
        if hyperref {
            assert!(
                result.aux.contains("{section*.1}") && result.aux.contains("{section*.2}"),
                "a label does not point at the heading's own anchor:\n{}",
                result.aux
            );
            assert!(
                !result.aux.contains("flm@internal@refcnt."),
                "the display-text hook placed an anchor of its own:\n{}",
                result.aux
            );
        }
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
    }
}

// --- the heading hook's key-value argument --------------------------------------------------

/// What the `headings` golden recorded in its auxiliary file *before* the
/// heading hook gained its optional argument, with `hyperref` loaded, as
/// [`heading_records`] reads it. Nothing in FLM writes the hook's keys, so
/// every heading FLM writes must still record exactly this; the test below
/// compares against it line for line. Measured with TeX Live 2023 and checked
/// again with TeX Live 2025.
///
/// It is a measurement, not a golden: `UPDATE_GOLDEN=1` does not maintain it.
/// It holds only what a heading decides — the table-of-contents entries, the
/// labels and the reference properties FLM sets — so that a new release of
/// LaTeX, `hyperref` or `zref-clever` that changes how they keep their own
/// books does not move it. A deliberate change to what a heading records
/// does, and the round that makes it re-measures this constant and says so.
const HEADINGS_AUX_BEFORE_THE_KNOB: &str = "\
\\@writefile{toc}{\\contentsline {section}{\\numberline {1}Introduction}{1}{section.1}}\n\
\\zref@newlabel{ref1}{\\default{1}\\page{1}\\zc@counter{section}\\thecounter{1}\\zc@type{section}\\anchor{section.1}}\n\
\\newlabel{ref1}{{1}{1}{Introduction}{section.1}{}}\n\
\\zref@newlabel{ref2}{\\default{1}\\page{1}\\zc@counter{section}\\thecounter{1}\\zc@type{section}\\anchor{section.1}}\n\
\\newlabel{ref2}{{1}{1}{Introduction}{section.1}{}}\n\
\\@writefile{toc}{\\contentsline {subsubsection}{\\numberline {1.0.1}Spaced}{1}{subsubsection.1.0.1}}\n\
\\zref@newlabel{ref3}{\\default{Para}\\page{1}\\zc@counter{flm@internal@refcnt}\\thecounter{2}\\zc@type{flm--internal--ref}\\anchor{section*.2}}\n\
\\newlabel{ref3}{{Para}{1}{Para}{section*.2}{}}\n\
\\@writefile{toc}{\\contentsline {paragraph}{Para}{1}{section*.2}}\n\
\\@writefile{toc}{\\contentsline {subparagraph}{Sub}{1}{section*.3}}\n\
";

/// The `zref` properties of a label that a heading and FLM decide: the text a
/// reference prints, the page, the counter and its value, the reference type
/// and the anchor. The others a `\zref@newlabel` line carries (`abspage`,
/// `zc@cntval`, `zc@pgval`, `zc@enclval`, `zc@pgfmt`, ...) are `zref` and
/// `zref-clever` keeping their own books, and their form changes between
/// releases.
const HEADING_LABEL_PROPERTIES: &[&str] =
    &["default", "page", "zc@counter", "thecounter", "zc@type", "anchor"];

/// Reads out of an auxiliary file what its headings recorded, one entry per
/// line: each table-of-contents entry (without the kernel's trailing
/// `\protected@file@percent`), each `\newlabel`, each `\zref@newlabel` reduced
/// to [`HEADING_LABEL_PROPERTIES`] in the order written, and each line of
/// FLM's own (`\flm@...`). Everything else — the `\providecommand` preamble
/// packages write, `\relax`, the last page — is left out.
fn heading_records(aux: &str) -> String {
    let mut records = String::new();
    for line in aux.lines() {
        let record = if line.starts_with("\\@writefile{toc}") {
            line.replace("\\protected@file@percent ", "")
        } else if let Some(rest) = line.strip_prefix("\\zref@newlabel{") {
            let (name, properties) = rest.split_once("}{").expect("a zref label and its properties");
            let properties = properties.strip_suffix('}').expect("a closed property list");
            let kept: String = zref_properties(properties)
                .into_iter()
                .filter(|(property, _)| HEADING_LABEL_PROPERTIES.contains(property))
                .map(|(property, value)| format!("\\{property}{{{value}}}"))
                .collect();
            format!("\\zref@newlabel{{{name}}}{{{kept}}}")
        } else if line.starts_with("\\newlabel{") || line.starts_with("\\flm@") {
            line.to_owned()
        } else {
            continue;
        };
        records.push_str(&record);
        records.push('\n');
    }
    records
}

/// Splits a `zref` property list, `\name{value}\name{value}...`, into its
/// names and values; a value may hold braces of its own.
fn zref_properties(list: &str) -> Vec<(&str, &str)> {
    let mut properties = Vec::new();
    let mut rest = list;
    while let Some(after) = rest.strip_prefix('\\') {
        let open = after.find('{').expect("a property value");
        let (name, group) = after.split_at(open);
        let mut depth = 0usize;
        let close = group
            .char_indices()
            .find(|&(_, c)| {
                match c {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                depth == 0
            })
            .map(|(at, _)| at)
            .expect("a balanced property value");
        properties.push((name, &group[1..close]));
        rest = &group[close + 1..];
    }
    assert!(rest.is_empty(), "an unreadable zref property list: {list}");
    properties
}

#[test]
fn a_heading_written_without_the_knob_writes_the_auxiliary_file_it_wrote_before_it() {
    under_each_engine(a_heading_written_without_the_knob_writes_the_auxiliary_file_it_wrote_before_it_under);
}

fn a_heading_written_without_the_knob_writes_the_auxiliary_file_it_wrote_before_it_under() {
    // A document written by hand writes `\flmHeading`, a newline, then the
    // sectioning command, with no optional argument: LaTeX's look for a `[`
    // after the hook skips the newline and stops at `\section`, which is not a
    // bracket, so no optional argument is read and the hook does what it did
    // before the argument existed. The recomposed LaTeX now writes the key
    // `toc` on every heading, so the golden is compiled with the key taken out
    // again, which is that hand-written document; the compilation is where a
    // swallowed token would show, as a heading, label or table-of-contents
    // entry that went missing or changed.
    let mut golden = latex_golden("headings.tex");
    assert!(golden.output.contains("\\flmHeading[toc=true]\n"), "{}", golden.output);
    golden.output = golden.output.replace("\\flmHeading[toc=true]\n", "\\flmHeading\n");
    let result = compile("headings-no-key", &golden, true);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    assert_eq!(heading_records(&result.aux), HEADINGS_AUX_BEFORE_THE_KNOB, "{}", result.aux);
}

#[test]
fn the_recomposed_headings_write_what_they_wrote_before_and_list_the_starred_one() {
    under_each_engine(the_recomposed_headings_write_what_they_wrote_before_and_list_the_starred_one_under);
}

fn the_recomposed_headings_write_what_they_wrote_before_and_list_the_starred_one_under() {
    // As the backend writes it, every heading of the golden carries
    // `toc=true`. The unstarred ones are listed by LaTeX as before, and the
    // starred `\subsection*{Starred}`, which LaTeX never lists, is added by
    // the hook: the auxiliary file is the one written before the key existed
    // with one more table-of-contents line, and nothing else changes.
    let golden = latex_golden("headings.tex");
    let result = compile("headings", &golden, true);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let expected = HEADINGS_AUX_BEFORE_THE_KNOB.replace(
        "\\@writefile{toc}{\\contentsline {subsubsection}",
        &format!("{STARRED_SUBSECTION_TOC_LINE}\n\\@writefile{{toc}}{{\\contentsline {{subsubsection}}"),
    );
    assert_eq!(heading_records(&result.aux), expected, "{}", result.aux);
}

/// The table-of-contents line the hook adds for the golden `headings`'
/// `\subsection*{Starred}`, written `toc=true`, under `hyperref`: the level is
/// the sectioning command's own name, and the anchor is the one the starred
/// command placed.
const STARRED_SUBSECTION_TOC_LINE: &str =
    "\\@writefile{toc}{\\contentsline {subsection}{Starred}{1}{section*.1}}";

#[test]
fn a_starred_heading_written_toc_true_is_listed_plain_and_under_hyperref() {
    under_each_engine(a_starred_heading_written_toc_true_is_listed_plain_and_under_hyperref_under);
}

fn a_starred_heading_written_toc_true_is_listed_plain_and_under_hyperref_under() {
    // The hook's default adds a starred heading written `toc=true` to the
    // table of contents, under the sectioning command's own name and with its
    // title, or with its alternative title where `alt` was written; a starred
    // heading written `toc=false`, or with no key, is not listed, as LaTeX
    // does. Under `hyperref` the line links to the anchor the starred command
    // placed at the title (`section*.1` is the anchor of the contents'
    // own starred heading, so the two listed ones are `.2` and `.3`).
    let golden = latex_golden("headings.tex");
    let body = "\\tableofcontents\n\
                \\flmHeading[toc=true]\n\
                \\section*{Listed}{\\flmLabels{}}\n\
                \n\
                \\flmHeading[toc=true,alt={Short}]\n\
                \\subsection*{A long title}{\\flmLabels{}}\n\
                \n\
                \\flmHeading[toc=false]\n\
                \\section*{Unlisted}{\\flmLabels{}}\n\
                \n\
                \\flmHeading\n\
                \\section*{Hand}{\\flmLabels{}}\n";
    for (variant, preamble) in [("plain", ""), ("hyperref", "\\usepackage{hyperref}\n")] {
        let result =
            compile_document("headings-starred-toc", variant, golden.style.as_slice(), preamble, body);
        assert!(result.ok, "{variant}: did not compile: {}", first_error(&result.log));
        let lines: Vec<&str> =
            result.toc.lines().filter(|line| line.starts_with("\\contentsline")).collect();
        let expected: Vec<String> = if variant == "plain" {
            vec![
                String::from("\\contentsline {section}{Listed}{1}{}%"),
                String::from("\\contentsline {subsection}{Short}{1}{}%"),
            ]
        } else {
            vec![
                String::from("\\contentsline {section}{Listed}{1}{section*.2}%"),
                String::from("\\contentsline {subsection}{Short}{1}{section*.3}%"),
            ]
        };
        assert_eq!(lines, expected, "{variant}: {}", result.toc);
    }
}

#[test]
fn toc_false_on_an_unstarred_heading_keeps_it_listed_and_warns() {
    under_each_engine(toc_false_on_an_unstarred_heading_keeps_it_listed_and_warns_under);
}

fn toc_false_on_an_unstarred_heading_keeps_it_listed_and_warns_under() {
    // The default hook does not take an unstarred heading out of the table of
    // contents: LaTeX lists it as it always does, and a package warning says
    // that `toc=false` was not honored.
    let golden = latex_golden("headings.tex");
    let body = "\\tableofcontents\n\\flmHeading[toc=false]\n\\section{Kept}{\\flmLabels{}}\n";
    let result = compile_document("headings-toc-false", "plain", golden.style.as_slice(), "", body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    assert!(result.toc.contains("\\numberline {1}Kept}"), "{}", result.toc);
    let log = result.log.replace('\n', "");
    assert!(log.contains("Package flm Warning: toc=false on the unstarred"), "{}", result.log);
}

#[test]
fn the_heading_knobs_alt_key_titles_the_table_of_contents_and_the_bookmark() {
    under_each_engine(the_heading_knobs_alt_key_titles_the_table_of_contents_and_the_bookmark_under);
}

fn the_heading_knobs_alt_key_titles_the_table_of_contents_and_the_bookmark_under() {
    // The knob written by hand over a recomposed style file, as a document
    // written by hand or a user who redefines the hook writes it. With `alt`
    // set and the heading unstarred, the hook hands the sectioning command
    // `[<alt>]{<title>}`, so the alternative title reaches the table of
    // contents and the PDF bookmark while the page keeps the full title.
    let golden = latex_golden("headings.tex");

    // Where the two titles went is measured with two commands that write a
    // line to the log every time they are typeset. Each is robust, so a
    // table-of-contents line is written with the command left unexpanded and
    // nothing is logged as it is written; a line appears when the title is set
    // on the page, and again when `\tableofcontents` reads back a line that
    // still holds the command.
    //
    // The first heading has an alternative title, the second has none. So
    // `PROBE-A` is logged **once** — the page, and not the table of contents,
    // whose line holds the alternative title instead — and `PROBE-B` is logged
    // **twice**, the page and the line read back, which is the control.
    let body = "\\tableofcontents\n\
                \\flmHeading[alt={Short title}]\n\
                \\section{Long \\probeA title}{\\flmLabels{\\label{ref1}}}\n\
                \n\
                \\flmHeading\n\
                \\section{Other \\probeB title}{\\flmLabels{\\label{ref2}}}\n";
    let preamble = "\\DeclareRobustCommand\\probeA{\\typeout{PROBE-A}}\n\
                    \\DeclareRobustCommand\\probeB{\\typeout{PROBE-B}}\n";
    let result =
        compile_document("headings-knob-alt", "alt", golden.style.as_slice(), preamble, body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    assert!(result.toc.contains("Short title"), "{}", result.toc);
    assert!(!result.toc.contains("Long"), "{}", result.toc);
    assert!(result.toc.contains("Other \\probeB title"), "{}", result.toc);
    assert_eq!(result.log.matches("PROBE-A").count(), 1, "{}", result.toc);
    assert_eq!(result.log.matches("PROBE-B").count(), 2, "{}", result.toc);

    // And the PDF bookmark, which `hyperref` builds by expanding the title
    // into a string rather than by typesetting it, holds the alternative title
    // too. The string is written as UTF-16, each ASCII character preceded by
    // `\000`.
    let body = "\\flmHeading[alt={Short title}]\n\
                \\section{A long and \\emph{fancy} title}{\\flmLabels{\\label{ref1}}}\n\
                \n\
                Back to \\flmRef{ref1}.\n";
    let result = compile_document(
        "headings-knob-alt",
        "bookmarks",
        golden.style.as_slice(),
        "\\usepackage{hyperref}\n",
        body,
    );
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let escaped = |word: &str| -> String { word.chars().map(|c| format!("\\000{c}")).collect() };
    assert!(result.bookmarks.contains("\\BOOKMARK"), "{}", result.bookmarks);
    for word in ["Short", "title"] {
        assert!(result.bookmarks.contains(&escaped(word)), "{}", result.bookmarks);
    }
    assert!(!result.bookmarks.contains(&escaped("long")), "{}", result.bookmarks);
    assert!(!result.log.contains("There were undefined references"), "{}", result.log);
}

#[test]
fn what_decides_the_relay_is_whether_the_key_was_written_not_what_it_holds() {
    under_each_engine(what_decides_the_relay_is_whether_the_key_was_written_not_what_it_holds_under);
}

fn what_decides_the_relay_is_whether_the_key_was_written_not_what_it_holds_under() {
    // `alt` gives the alternative title, full stop. An *empty* one is an empty
    // alternative title, not the absence of one: the hook hands the sectioning
    // command `[]`, which is how LaTeX itself asks for a deliberately empty
    // table-of-contents entry. Absence is the key not being written, and an
    // empty optional argument on the hook (`\flmHeading[]`) is a key list with
    // no key in it, so it is absence too. Four headings, one per case, in one
    // document, so that the four lines of the table of contents can be read
    // against one another.
    let golden = latex_golden("headings.tex");
    let body = "\\tableofcontents\n\
                \\flmHeading[alt={Short title}]\n\
                \\section{Full one}{\\flmLabels{}}\n\
                \n\
                \\flmHeading[alt={}]\n\
                \\section{Full two}{\\flmLabels{}}\n\
                \n\
                \\flmHeading\n\
                \\section{Full three}{\\flmLabels{}}\n\
                \n\
                \\flmHeading[]\n\
                \\section{Full four}{\\flmLabels{}}\n";
    let result =
        compile_document("headings-knob-written", "written", golden.style.as_slice(), "", body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let lines: Vec<&str> = result.toc.lines().collect();
    assert_eq!(lines.len(), 4, "{}", result.toc);
    assert!(lines[0].contains("{1}Short title}"), "{}", result.toc);
    // The empty alternative title: a numbered, otherwise empty line. Nothing
    // of the heading's own title is in it.
    assert!(lines[1].contains("{\\numberline {2}}"), "{}", result.toc);
    // And the two absences, which carry the heading's own title.
    assert!(lines[2].contains("{3}Full three}"), "{}", result.toc);
    assert!(lines[3].contains("{4}Full four}"), "{}", result.toc);
    for absent in ["Full one", "Full two"] {
        assert!(!result.toc.contains(absent), "{}", result.toc);
    }
}

#[test]
fn an_alternative_title_flm_writes_titles_the_table_of_contents_starred_or_not() {
    under_each_engine(an_alternative_title_flm_writes_titles_the_table_of_contents_starred_or_not_under);
}

fn an_alternative_title_flm_writes_titles_the_table_of_contents_starred_or_not_under() {
    // The golden `headings-alt` is what the recomposed LaTeX writes for a
    // heading with `alt=` holding markup, a starred one with `alt=`, and one
    // without: the hook hands the unstarred heading's alternative title to the
    // sectioning command, and lists the starred one under its alternative
    // title; the third keeps its own title. The optional argument braced whole
    // by the emitter is read as the key list it holds.
    let golden = latex_golden("headings-alt.tex");
    let body = format!("\\tableofcontents\n{}", golden.output);
    for (variant, preamble) in [("plain", ""), ("hyperref", "\\usepackage{hyperref}\n")] {
        let result = compile_document("headings-alt", variant, golden.style.as_slice(), preamble, &body);
        assert!(result.ok, "{variant}: did not compile: {}", first_error(&result.log));
        let lines: Vec<&str> =
            result.toc.lines().filter(|line| line.starts_with("\\contentsline")).collect();
        assert_eq!(lines.len(), 3, "{variant}: {}", result.toc);
        assert!(
            lines[0].starts_with("\\contentsline {section}{\\numberline {1}Short \\emph {title}}"),
            "{variant}: {}",
            result.toc
        );
        assert!(lines[1].starts_with("\\contentsline {subsection}{Starred short}"), "{variant}: {}", result.toc);
        assert!(
            lines[2].starts_with("\\contentsline {section}{\\numberline {2}No alternative}"),
            "{variant}: {}",
            result.toc
        );
        for full in ["fancy", "starred long"] {
            assert!(!result.toc.contains(full), "{variant}: {}", result.toc);
        }
        assert!(!result.log.contains("There were undefined references"), "{variant}: {}", result.log);
    }
}

#[test]
fn two_recomposed_style_files_load_in_one_latex_job() {
    under_each_engine(two_recomposed_style_files_load_in_one_latex_job_under);
}

fn two_recomposed_style_files_load_in_one_latex_job_under() {
    // A style file is named after a hash of its own content, so a document
    // that holds two recomposed documents loads two of these files under two
    // names, and every declaration the family makes is made twice. That is the
    // invariant the guard discipline of
    // `stdcore::backends::latex::preamble` buys and states, and nothing
    // else in this suite exercises it: every other test loads one file.
    //
    // The two files here are one golden's style file under two package names,
    // which is exactly the shape two recomposed documents produce. A
    // declaration that stops on a name already taken — expl3's `\tl_new:N` and
    // `\bool_new:N` among them — fails the job on the second file.
    let golden = latex_golden("headings.tex");
    let (text, name) = golden.style.as_ref().expect("the golden has a style file");
    let stem = name.strip_suffix(".sty").expect("a style file name");
    let styles: Vec<(String, String)> = ["flmfirst", "flmsecond"]
        .into_iter()
        .map(|renamed| {
            (text.replace(stem, renamed), format!("{renamed}.sty"))
        })
        .collect();
    let body = "\\tableofcontents\n\
                \\flmHeading[alt={Short title}]\n\
                \\section{A long title}{\\flmLabels{\\label{ref1}}}\n\
                \n\
                Back to \\flmRef{ref1}.\n";
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result =
            compile_document("headings-two-styles", "two", &styles, preamble, body);
        assert!(result.ok, "two style files did not compile: {}", first_error(&result.log));
        // And the hook still works: the second file's `\ProvideDocumentCommand`
        // left the first file's definition in place, which is the other half of
        // the guard discipline.
        assert!(result.toc.contains("Short title"), "{}", result.toc);
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
    }
}

#[test]
fn a_starred_heading_reads_the_knob_and_ignores_it() {
    under_each_engine(a_starred_heading_reads_the_knob_and_ignores_it_under);
}

fn a_starred_heading_reads_the_knob_and_ignores_it_under() {
    // A starred sectioning command takes no optional argument, and handing it
    // one is not even an error: the command takes one token as its heading, so
    // `\section*[Short]{Long}` sets the opening bracket alone as the heading
    // and runs "Short", the closing bracket and "Long" together into the
    // following paragraph (measured, with the page's text read out of the
    // uncompressed PDF). So the star branch of the
    // hook never writes one, and the key is read and dropped. What proves it
    // here: the heading's label takes the *full* title as what a reference
    // prints — the hook's numbering test still runs, sees that nothing was
    // numbered, and hands the full title to the display-text macro — and the
    // brackets are nowhere in the auxiliary file.
    let golden = latex_golden("headings.tex");
    let body = "\\flmHeading[alt={Short title}]\n\
                \\section*{Long title}{\\flmLabels{\\label{ref1}}}\n\
                \n\
                Back to \\flmRef{ref1}.\n";
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result = compile_document(
            "headings-knob-starred",
            "starred",
            golden.style.as_slice(),
            preamble,
            body,
        );
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        assert!(result.aux.contains("\\default{Long title}"), "{}", result.aux);
        assert!(!result.aux.contains("Short title"), "{}", result.aux);
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
    }
}

#[test]
fn an_unnumbered_headings_labels_print_the_full_title_not_the_alternative_one() {
    under_each_engine(an_unnumbered_headings_labels_print_the_full_title_not_the_alternative_one_under);
}

fn an_unnumbered_headings_labels_print_the_full_title_not_the_alternative_one_under() {
    // A heading below the class's numbering depth — `\paragraph` in `article`
    // — is unstarred, so the alternative title reaches it and goes into the
    // table of contents, and it is unnumbered, so the hook gives its labels a
    // display text of their own. That display text is the **full** title: the
    // key says what the sectioning command's optional argument does, and that
    // argument never reaches a label. So a reference to the heading reads what
    // the page reads, and the two titles are visible side by side in one
    // document.
    let golden = latex_golden("headings.tex");
    let body = "\\tableofcontents\n\
                \\flmHeading[alt={Short title}]\n\
                \\paragraph{A long title}{\\flmLabels{\\label{ref1}}}\n\
                \n\
                Back to \\flmRef{ref1}.\n";
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result = compile_document(
            "headings-knob-unnumbered",
            "unnumbered",
            golden.style.as_slice(),
            preamble,
            body,
        );
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        assert!(result.toc.contains("Short title"), "{}", result.toc);
        assert!(!result.toc.contains("A long title"), "{}", result.toc);
        assert!(result.aux.contains("\\default{A long title}"), "{}", result.aux);
        assert!(result.aux.contains("{{A long title}"), "{}", result.aux);
        assert!(!result.log.contains("There were undefined references"), "{}", result.log);
    }
}

// --- the default font encoding of a merged `fontenc` request -----------------------------

/// The encoding a merged `fontenc` request leaves Latin text in, measured
/// under each engine.
///
/// `fontenc` makes the **last** encoding of its option list the one a
/// document's text is set in, and the style file requests it once per engine
/// with every encoding its characters need under that engine, so which
/// encoding ends that list is an accident of the order the requests were
/// resolved in. The file therefore writes, right after its package block and
/// in a test of the engine of its own, `\renewcommand{\encodingdefault}{TU}`
/// for LuaLaTeX and XeLaTeX and `\renewcommand{\encodingdefault}{T1}` for
/// pdfLaTeX. This measures what that command does, over the two goldens whose
/// package block merges several `fontenc` requests:
///
/// - `\f@encoding` is the encoding LaTeX has selected where the body is set,
///   and `\fontname\font` is the font file it picked: `T1` and `ecrm1000`
///   under `pdflatex`; `TU` and Latin Modern's Unicode roman,
///   `[lmroman10-regular]:+tlig; at 10.0pt`, under `lualatex`. Under `X2` or
///   `T2A`, the Cyrillic encodings the same requests load, the font is
///   `rxrm1000` or `larm1000` — a different font, a different set of glyphs
///   for the same Latin letters.
/// - The Cyrillic text is unaffected either way, because each Cyrillic letter
///   carries its own `\fontencoding` switch.
///
/// Without the command the measurement reads `X2` and `rxrm1000` for
/// `text-chunks-declare` under both engines, and `T2A` and `larm1000` for
/// `text-fontenc-both` under `pdflatex`, which is what this pins against.
#[test]
fn the_default_encoding_command_sets_latin_text_in_each_engine_s_own_encoding_after_a_merged_fontenc() {
    under_each_engine(
        the_default_encoding_command_sets_latin_text_in_each_engine_s_own_encoding_after_a_merged_fontenc_under,
    );
}

fn the_default_encoding_command_sets_latin_text_in_each_engine_s_own_encoding_after_a_merged_fontenc_under() {
    let engine = ENGINE.get();
    // The encoding and the beginning of the font name; `lualatex`'s name goes
    // on with the font's features and size.
    let expected = match engine {
        "pdflatex" => "FLM-ENCODING [T1] [ecrm1000]",
        "lualatex" => "FLM-ENCODING [TU] [[lmroman10-regular]",
        other => panic!("no default encoding measured for `{other}`"),
    };
    for name in ["text-fontenc-both", "text-chunks-declare"] {
        let golden = latex_golden(&format!("{name}.tex"));
        let (style, _) = golden.style.as_ref().expect("the golden has a style file");
        for line in [
            "\\RequirePackage{iftex}",
            "\\renewcommand{\\encodingdefault}{T1}",
            "\\renewcommand{\\encodingdefault}{TU}",
        ] {
            assert!(style.contains(line), "{name} should carry {line}: {style}");
        }
        assert_eq!(style.matches("{fontenc}").count(), 2, "{name}: {style}");
        // The golden's own text, then the probe: the encoding in force and the
        // font it selected, written to the log.
        let body = format!(
            "{}\\makeatletter\n\
             \\typeout{{FLM-ENCODING [\\f@encoding] [\\fontname\\font]}}\n\
             \\makeatother\n",
            golden.output
        );
        for hyperref in [false, true] {
            let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
            let result = compile_document(
                name,
                "encoding",
                golden.style.as_slice(),
                preamble,
                &body,
            );
            assert!(result.ok, "{name} did not compile: {}", first_error(&result.log));
            assert!(
                result.log.contains(expected),
                "{name} (hyperref: {hyperref}) did not set its Latin text in the encoding of \
                 `{engine}`: expected `{expected}…`, and the log says {}. What this measures \
                 is the *encoding*; the font name goes with it. Without the \
                 default-encoding command the merged option list leaves the last encoding \
                 in force, which reads [X2] [rxrm1000] for text-chunks-declare under both \
                 engines, and [T2A] [larm1000] for text-fontenc-both on `pdflatex`.",
                result
                    .log
                    .lines()
                    .find(|line| line.starts_with("FLM-ENCODING"))
                    .unwrap_or("(no FLM-ENCODING line in the log)")
            );
        }
    }
}

// --- a reference to a labeled footnote -----------------------------------------------------

#[test]
fn a_label_in_an_xnote_refers_to_the_footnote_s_number() {
    under_each_engine(a_label_in_an_xnote_refers_to_the_footnote_s_number_under);
}

fn a_label_in_an_xnote_refers_to_the_footnote_s_number_under() {
    // The golden's third footnote carries the label `ref1`, which the style
    // file's `\flmXNfootnote` hands to `\footnote` at the head of its text:
    // there LaTeX's `\@currentlabel` is the footnote's number, and
    // `zref-clever` records the footnote counter as the reference's type,
    // so `\zcref{ref1}` prints "footnote 3".
    let golden = split(&fs::read_to_string(goldens().join("xnotes.tex")).expect("the golden"));
    for hyperref in [false, true] {
        let result = compile("xnotes-label", &golden, hyperref);
        assert!(result.ok, "did not compile: {}", first_error(&result.log));
        let record = result
            .aux
            .lines()
            .find(|line| line.starts_with("\\zref@newlabel{ref1}"))
            .unwrap_or_else(|| panic!("no record of ref1 (hyperref: {hyperref}): {}", result.aux));
        for part in ["\\default{3}", "\\zc@counter{footnote}", "\\zc@type{footnote}"] {
            assert!(record.contains(part), "{part} (hyperref: {hyperref}): {record}");
        }
    }
}

// --- a footnote in what a reference prints ------------------------------------------------------

#[test]
fn a_footnote_in_what_a_reference_prints_is_set_at_each_reference() {
    under_each_engine(a_footnote_in_what_a_reference_prints_is_set_at_each_reference_under);
}

fn a_footnote_in_what_a_reference_prints_is_set_at_each_reference_under() {
    // An unnumbered float's caption and a defined term each hold a footnote
    // and are referred to twice; the display text is the caption or the term
    // as written, so LaTeX sets the footnote again at each reference. A
    // second float's caption writes an alternative text without its note,
    // which a reference prints alone.
    let golden = latex_golden("xnotes-in-display-text.tex");
    for hyperref in [false, true] {
        let result = compile("xnotes-in-display-text", &golden, hyperref);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        let text = pdf_text(&result.pdf);
        for expected in ["C plot4 and C plot5", "Entropy6 and Entropy7", "A photo and A photo.8", "8 Last note."] {
            assert!(text.contains(expected), "{expected} (hyperref: {hyperref}) {text}");
        }
        // The caption's own note loses its text, as LaTeX sets a note in a
        // caption; the term's is set where it was written.
        assert_eq!(text.matches("Note C.").count(), 2, "(hyperref: {hyperref}) {text}");
        assert_eq!(text.matches("Note E.").count(), 3, "(hyperref: {hyperref}) {text}");
    }
}

// --- a nested footnote ------------------------------------------------------------------------

#[test]
fn a_nested_footnote_s_label_is_undefined_under_the_default_definition() {
    under_each_engine(a_nested_footnote_s_label_is_undefined_under_the_default_definition_under);
}

fn a_nested_footnote_s_label_is_undefined_under_the_default_definition_under() {
    // The inner footnote, labeled `ref1`, is handed to `\footnote` inside the
    // outer footnote's text; LaTeX leaves it unset, so its label is never
    // written and the reference to it is undefined. Everything else compiles.
    let golden = split(&fs::read_to_string(goldens().join("xnotes-nested.tex")).expect("the golden"));
    assert!(golden.output.contains("\\flmRef{ref1}"), "{}", golden.output);
    for hyperref in [false, true] {
        let result = compile("xnotes-nested", &golden, hyperref);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        assert!(!result.aux.contains("{ref1}"), "ref1 is recorded (hyperref: {hyperref}): {}", result.aux);
        assert!(
            result.log.contains("There were undefined references"),
            "the reference resolved (hyperref: {hyperref})"
        );
    }
}

// --- the hyperlink hooks ------------------------------------------------------------------------

#[test]
fn a_hyperlink_hook_in_a_heading_reaches_the_table_of_contents_and_the_bookmark() {
    under_each_engine(a_hyperlink_hook_in_a_heading_reaches_the_table_of_contents_and_the_bookmark_under);
}

fn a_hyperlink_hook_in_a_heading_reaches_the_table_of_contents_and_the_bookmark_under() {
    // `\section{See \flmUrl{https://example.com/docs/}}`: a title is a moving
    // argument, and the hook is defined with `\ProvideDocumentCommand`, so it
    // is written to the table of contents as its call and runs again there.
    // The bookmark `hyperref` builds from the title holds the display alone,
    // derived by expansion through the style file's
    // `\pdfstringdefDisableCommands` block, and nothing is reported.
    let golden = split(&fs::read_to_string(goldens().join("href.tex")).expect("the golden"));
    assert!(golden.output.contains("\\section{See \\flmUrl{"), "{}", golden.output);
    let body = format!("\\tableofcontents\n{}", golden.output);
    for hyperref in [false, true] {
        let preamble = if hyperref { "\\usepackage{hyperref}\n" } else { "" };
        let result = compile_document("href-toc", "toc", golden.style.as_slice(), preamble, &body);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        assert!(
            result.toc.contains("\\flmUrl {https://example.com/docs/}}"),
            "(hyperref: {hyperref}) {}",
            result.toc
        );
        assert!(!result.log.contains("Token not allowed in a PDF string"), "{}", result.log);
        // The bookmark string is UTF-16, `\000` before each ASCII character.
        let escaped = |word: &str| -> String { word.chars().map(|c| format!("\\000{c}")).collect() };
        assert!(result.bookmarks.contains(&escaped("example.com/docs")), "{}", result.bookmarks);
        // The display drops the address's trailing `/`.
        assert!(!result.bookmarks.contains(&escaped("docs/")), "{}", result.bookmarks);
        assert!(!result.bookmarks.contains(&escaped("https")), "{}", result.bookmarks);
    }
}

#[test]
fn a_document_that_skips_the_hyperref_request_loads_it_itself_with_options_and_links() {
    under_each_engine(a_document_that_skips_the_hyperref_request_loads_it_itself_with_options_and_links_under);
}

fn a_document_that_skips_the_hyperref_request_loads_it_itself_with_options_and_links_under() {
    // `hyperref` named in `skip_packages`, and loaded by the document after the
    // style file with an option of its own: the hooks link through it, and the
    // piece's bookmark block, asked when the document begins, still tells
    // `hyperref` what a hook contributes to a bookmark.
    let author = "\\usepackage[colorlinks]{hyperref}\n";
    let skipped = latex_golden("href-skip-hyperref.tex");
    assert!(!skipped.style.as_ref().expect("a style file").0.contains("\\RequirePackage{hyperref}"));
    let result =
        compile_document("href-skip", "colorlinks", skipped.style.as_slice(), author, &skipped.output);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    assert!(!result.log.contains("Token not allowed in a PDF string"), "{}", result.log);
    let escaped = |word: &str| -> String { word.chars().map(|c| format!("\\000{c}")).collect() };
    assert!(result.bookmarks.contains(&escaped("example.com/docs")), "{}", result.bookmarks);
    // Without the skip, the style file's request has loaded `hyperref` with
    // no options before the document's line, and LaTeX stops on the clash.
    let requested = latex_golden("href.tex");
    let result =
        compile_document("href-clash", "colorlinks", requested.style.as_slice(), author, &requested.output);
    assert!(!result.ok, "compiled although the options clash");
    assert!(result.log.contains("Option clash for package hyperref"), "{}", first_error(&result.log));
}

// --- the display of a `\url` and an `\email`, derived in LaTeX --------------------------------

/// The addresses whose display the hooks derive, each with whether it is an
/// `\email`'s — whose target, and so whose display, has `mailto:` before the
/// address: each prefix, the trailing run of `/`, `#` and `?`, the characters
/// `%`, `#`, `\`, `~`, `_` and braces, no scheme, an empty display, one prefix
/// only, and the case kept.
const DISPLAYED: &[(&str, bool)] = &[
    ("https://example.com/", false),
    ("https://example.com/?#", false),
    ("a@b.c", true),
    ("a@b.c/", true),
    ("http://x.org/100%25/a_b~c#top", false),
    ("example.org/a/", false),
    ("mailto:x@y.z?#/", false),
    ("https://x.org/back\\slash/{e}/", false),
    ("https://x.org/a#b?c", false),
    ("https://", false),
    ("#?/", false),
    ("https://http://x/", false),
    ("HTTPS://X.ORG/", false),
    ("x.org/?a=1#b", false),
];

#[test]
fn the_display_the_hooks_derive_in_latex_is_the_features_display() {
    under_each_engine(the_display_the_hooks_derive_in_latex_is_the_features_display_under);
}

fn the_display_the_hooks_derive_in_latex_is_the_features_display_under() {
    // For each address, the helper's result over what the hook hands it — the
    // address as the recomposed LaTeX writes it, escaped, with `mailto:`
    // before an `\email`'s — and then the hook itself, whose call of
    // `\nolinkurl` is watched: both written to the log, and both the display
    // the `href` feature's function derives from the link's target.
    let golden = latex_golden("href.tex");
    let mut body = String::from(
        "\\makeatletter\n\\let\\flmprobeNL\\nolinkurl\n\
         \\def\\nolinkurl#1{\\typeout{S\\flmprobeid\\space= [#1]}\\flmprobeNL{#1}}\n",
    );
    for (index, (address, email)) in DISPLAYED.iter().enumerate() {
        let escaped = escape_url(address);
        let (prefix, hook) = if *email { ("mailto:", "flmEmail") } else { ("", "flmUrl") };
        body.push_str(&format!(
            "\\typeout{{D{index} = [\\flm@href@display{{{prefix}{escaped}}}]}}\n\
             \\def\\flmprobeid{{{index}}}\\{hook}{{{escaped}}}\n\n"
        ));
    }
    body.push_str("\\makeatother\n");
    let result = compile_document("href-display", "display", golden.style.as_slice(), "", &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let derived = typeout_values(&result.log, "D");
    let shown = typeout_values(&result.log, "S");
    for (index, (address, email)) in DISPLAYED.iter().enumerate() {
        let target = format!("{}{address}", if *email { "mailto:" } else { "" });
        let expected = format!("[{}]", display(&target));
        assert_eq!(derived.get(&index.to_string()), Some(&expected), "the helper over {address:?}");
        assert_eq!(shown.get(&index.to_string()), Some(&expected), "the hook over {address:?}");
    }
}

#[test]
fn a_kept_url_expanded_at_its_use_sites_shows_the_display() {
    under_each_engine(a_kept_url_expanded_at_its_use_sites_shows_the_display_under);
}

fn a_kept_url_expanded_at_its_use_sites_shows_the_display_under() {
    // `\url{#1}` called with `https://x.org/` and `\url{https://example.com/#1}`
    // called with `a/`: each use site expanded, each hook showing the display
    // the text and HTML outputs show.
    let golden = latex_golden("href-kept-display.tex");
    let body = format!(
        "\\makeatletter\n\\let\\flmprobeNL\\nolinkurl\n\
         \\def\\nolinkurl#1{{\\typeout{{SHOWN = [#1]}}\\flmprobeNL{{#1}}}}\n\\makeatother\n{}",
        golden.output
    );
    let result = compile_document("href-kept-display", "shown", golden.style.as_slice(), "", &body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let shown: Vec<&str> = result
        .log
        .lines()
        .filter_map(|line| line.strip_prefix("SHOWN = "))
        .collect();
    assert_eq!(shown, ["[x.org]", "[example.com/a]"], "{}", result.log);
}

// --- the link targets in the PDF ---------------------------------------------------------------

/// The preamble line that makes the current engine ([`ENGINE`]) write the
/// PDF's objects uncompressed, so that a link's target stands in the file as
/// plain text, `/URI(<target>)`, which [`link_targets`] reads. Each engine
/// spells the setting its own way.
fn uncompressed_objects() -> &'static str {
    match ENGINE.get() {
        "lualatex" => "\\pdfvariable objcompresslevel=0\n",
        _ => "\\pdfobjcompresslevel=0\n",
    }
}

/// The targets of the links of a PDF whose objects are uncompressed
/// ([`uncompressed_objects`]), sorted and each once — a link broken across
/// two lines is two annotations of one target: the literal string after every
/// `/URI`, its escapes (`\\`, `\(`, `\)`) read back.
fn link_targets(pdf: &[u8]) -> Vec<String> {
    let marker = b"/URI(";
    let mut targets = Vec::new();
    let mut at = 0;
    while let Some(found) = pdf[at..].windows(marker.len()).position(|window| window == marker) {
        let mut index = at + found + marker.len();
        let mut depth = 0usize;
        let mut target = Vec::new();
        while index < pdf.len() {
            match pdf[index] {
                b'\\' if index + 1 < pdf.len() => {
                    target.push(pdf[index + 1]);
                    index += 2;
                    continue;
                }
                b'(' => depth += 1,
                b')' if depth == 0 => break,
                b')' => depth -= 1,
                _ => {}
            }
            target.push(pdf[index]);
            index += 1;
        }
        targets.push(String::from_utf8_lossy(&target).into_owned());
        at = index;
    }
    targets.sort();
    targets.dedup();
    targets
}

/// Compiles `golden` with the objects uncompressed and `author` in the
/// preamble, and gives back the targets of its links.
fn compiled_targets(name: &str, golden: &Golden, author: &str) -> Vec<String> {
    let preamble = format!("{author}{}", uncompressed_objects());
    let result = compile_document(name, "targets", golden.style.as_slice(), &preamble, &golden.output);
    assert!(result.ok, "{name} did not compile: {}", first_error(&result.log));
    link_targets(&result.pdf)
}

/// Sorted, as [`link_targets`] gives them.
fn sorted(targets: &[&str]) -> Vec<String> {
    let mut targets: Vec<String> = targets.iter().map(|target| target.to_string()).collect();
    targets.sort();
    targets
}

#[test]
fn the_hyperlink_hooks_link_to_exactly_the_addresses_written() {
    under_each_engine(the_hyperlink_hooks_link_to_exactly_the_addresses_written_under);
}

fn the_hyperlink_hooks_link_to_exactly_the_addresses_written_under() {
    // The escaping of an address, `\#`, `\%` and `\\`, is read back by `\href`
    // as the characters; `~`, `_`, `&`, `$` and balanced braces, written raw,
    // reach the target as themselves.
    let href = latex_golden("href.tex");
    let expected = sorted(&[
        "https://example.com/docs/",
        "https://example.com",
        "https://example.com/",
        "mailto:someone@example.com",
        "https://x.org/a%20b_c~d/{e}?q=1&r=$2#top",
        "https://x.org/~user/a%2Fb#sec_2",
        "https://x.org/back\\slash",
    ]);
    for author in ["", "\\usepackage{hyperref}\n"] {
        assert_eq!(compiled_targets("href", &href, author), expected, "(author: {author:?})");
    }
    // Hyperref skipped and loaded by the document with an option of its own.
    let skipped = latex_golden("href-skip-hyperref.tex");
    assert_eq!(compiled_targets("href-skip", &skipped, "\\usepackage[colorlinks]{hyperref}\n"), expected);
    // A kept definition's placeholder in an address: every use site is
    // expanded, and the target holds its characters, `100%` and `##` of the
    // template escaped around them.
    let kept = latex_golden("href-userdefs-kept.tex");
    assert_eq!(
        compiled_targets("href-kept", &kept, ""),
        sorted(&["https://doi.org/10.1000/xyz", "https://x.org/100%/#sec-intro"])
    );
    // A `\url` whose whole address, or its end, is a placeholder: the use
    // sites expanded, each target the whole address.
    let displayed = latex_golden("href-kept-display.tex");
    assert_eq!(
        compiled_targets("href-kept-display", &displayed, ""),
        sorted(&["https://example.com/a/", "https://x.org/"])
    );
    // A verbatim argument passed into an address, through `\href` and `\url`:
    // the use sites are expanded, and each target holds the use site's
    // characters — a backslash, `~` and `^` among them.
    let verbatim = latex_golden("href-userdefs-verbatim.tex");
    assert_eq!(
        compiled_targets("href-verbatim", &verbatim, ""),
        sorted(&[
            "https://doi.org/10.1000/a%20b#c",
            "https://doi.org/a^b",
            "https://doi.org/10.1000/x~y\\z^w",
            "https://x.org/a%20b#c",
        ])
    );
}

// --- a reference to a definition's label -----------------------------------------------------

#[test]
fn a_reference_to_a_definition_s_label_prints_the_term() {
    under_each_engine(a_reference_to_a_definition_s_label_prints_the_term_under);
}

fn a_reference_to_a_definition_s_label_prints_the_term_under() {
    // Every label of the definition stands inside `\flmLabelsWithDisplayText`,
    // which sets the text a label records to the term and its counter to the
    // one whose reference type has empty names: `\zcref` to any of them
    // prints "Entropy", the `topic:` label (`ref3`) and the `term:` label
    // (`ref4`) among them. `\phantomsection` before it gives them an anchor of
    // their own, not the section's.
    let golden =
        split(&fs::read_to_string(goldens().join("defterm-definition.tex")).expect("the golden"));
    assert!(golden.output.contains("See \\flmRef{ref3} and \\flmRef{ref4}."), "{}", golden.output);
    for hyperref in [false, true] {
        let result = compile("defterm-label", &golden, hyperref);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        for key in ["ref1", "ref2", "ref3", "ref4"] {
            let record = result
                .aux
                .lines()
                .find(|line| line.starts_with(&format!("\\zref@newlabel{{{key}}}")))
                .unwrap_or_else(|| panic!("no record of {key} (hyperref: {hyperref}): {}", result.aux));
            for part in
                ["\\default{Entropy}", "\\zc@type{flm--internal--ref}", "\\anchor{section*.1}"]
            {
                assert!(record.contains(part), "{key}, {part} (hyperref: {hyperref}): {record}");
            }
        }
    }
}

#[test]
fn a_term_link_links_to_its_definition_and_the_term_inside_it_links_nowhere() {
    under_each_engine(a_term_link_links_to_its_definition_and_the_term_inside_it_links_nowhere_under);
}

fn a_term_link_links_to_its_definition_and_the_term_inside_it_links_nowhere_under() {
    // Under `hyperref`, `\phantomsection` gives the definition of `Entropy` the
    // anchor `section*.2` (the table of contents' own heading is `section*.1`)
    // and that of `Temperature` `section*.3`. Six links name the first: the
    // term link in the section's title and its line in the table of contents,
    // the link by a bracket, the link by the alias, the one inside the
    // `\href`'s text, and the reference to the written label. One names the
    // second. `\flmTermInDefterm` inside the first definition makes none. The
    // hook in the title is robust, so it reaches the table of contents as its
    // call, and contributes its text alone to the bookmark.
    let golden = latex_golden("defterm-terms.tex");
    assert!(golden.output.contains("\\flmTermInDefterm{Entropy}"), "{}", golden.output);
    let body = format!("\\tableofcontents\n{}", golden.output);
    let links = |pdf: &[u8], anchor: &str| -> usize {
        let wanted = format!("/D ({anchor})");
        let text = String::from_utf8_lossy(pdf);
        text.matches(wanted.as_str()).count()
    };
    for author in ["", "\\usepackage{hyperref}\n"] {
        let preamble = format!("{}{author}", uncompressed_objects());
        let result =
            compile_document("defterm-terms", "links", golden.style.as_slice(), &preamble, &body);
        assert!(result.ok, "did not compile (author: {author:?}): {}", first_error(&result.log));
        assert!(!result.log.contains("undefined"), "(author: {author:?}) {}", result.log);
        assert_eq!(links(&result.pdf, "section*.2"), 6, "(author: {author:?})");
        assert_eq!(links(&result.pdf, "section*.3"), 1, "(author: {author:?})");
        assert!(result.toc.contains("About \\flmTerm {ref1}{Entropy}"), "{}", result.toc);
        assert!(!result.log.contains("Token not allowed in a PDF string"), "{}", result.log);
        let escaped = |word: &str| -> String { word.chars().map(|c| format!("\\000{c}")).collect() };
        // The space is written `\\000\\040` there.
        let title = format!("{}\\000\\040{}", escaped("About"), escaped("Entropy"));
        assert!(result.bookmarks.contains(&title), "{}", result.bookmarks);
        assert!(!result.bookmarks.contains(&escaped("ref1")), "{}", result.bookmarks);
    }
}

#[test]
fn an_unresolved_term_holding_mathematics_compiles_with_an_undefined_reference() {
    under_each_engine(an_unresolved_term_holding_mathematics_compiles_with_an_undefined_reference_under);
}

fn an_unresolved_term_holding_mathematics_compiles_with_an_undefined_reference_under() {
    // An unresolved term, one whose term holds mathematics, and an unresolved
    // `\eqref` are each written under an allocated key, so the compilation
    // goes through and LaTeX reports undefined references, nothing worse.
    let golden = latex_golden("defterm-unresolved.tex");
    assert!(golden.output.contains("\\flmTerm{ref2}{\\(\\alpha\\)-decay}"), "{}", golden.output);
    for hyperref in [false, true] {
        let result = compile("defterm-unresolved", &golden, hyperref);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        assert!(
            result.log.contains("There were undefined references"),
            "(hyperref: {hyperref}) {}",
            result.log
        );
    }
}

#[test]
fn a_use_site_expanded_at_lowering_writes_every_reference_under_a_name() {
    under_each_engine(a_use_site_expanded_at_lowering_writes_every_reference_under_a_name_under);
}

fn a_use_site_expanded_at_lowering_writes_every_reference_under_a_name_under() {
    // A kept definition whose template reads a reference target has its use
    // sites expanded at lowering; a target naming nothing — a verbatim
    // argument's `x%y` or `u\v` among them — is written under a name of the
    // use site's value (`ref-v1`), never as its characters, so the
    // compilation goes through and LaTeX reports undefined references,
    // nothing worse. A target that names a label or a term is written under
    // the document's key, which the auxiliary file records.
    for name in [
        "userdefs-refs-placeholder-unresolved",
        "userdefs-refs-placeholder-verbatim",
        "userdefs-refs-placeholder-terms",
    ] {
        let golden = latex_golden(&format!("{name}.tex"));
        assert!(golden.output.contains("{ref-v1}"), "{name}: {}", golden.output);
        for hyperref in [false, true] {
            let result = compile(name, &golden, hyperref);
            assert!(
                result.ok,
                "{name} did not compile (hyperref: {hyperref}): {}",
                first_error(&result.log)
            );
            assert!(
                result.log.contains("There were undefined references"),
                "{name} (hyperref: {hyperref}) {}",
                result.log
            );
            assert!(result.log.contains("ref-v1"), "{name} (hyperref: {hyperref}) {}", result.log);
            if name == "userdefs-refs-placeholder-terms" {
                assert!(result.aux.contains("\\newlabel{ref1}"), "{}", result.aux);
                assert!(!result.log.contains("`ref1'"), "(hyperref: {hyperref}) {}", result.log);
            }
        }
    }
}

// --- theorems ------------------------------------------------------------------------------------

/// The text of `pdf`, as the `pdftotext` program extracts it, its lines
/// joined by spaces.
fn pdf_text(pdf: &[u8]) -> String {
    let unique = NEXT_COMPILE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir()
        .join(format!("flm-latex-compile-{}-{unique}-text.pdf", std::process::id()));
    fs::write(&path, pdf).expect("the PDF");
    let out = Command::new("pdftotext")
        .arg(&path)
        .arg("-")
        .output()
        .unwrap_or_else(|error| panic!("`pdftotext` runs from the PATH: {error}"));
    let _ = fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A reference to the theorem of the theorem goldens, labeled `ref1`, through
/// the reference hook and through `\zcref` itself, each on a line of its own.
const THEOREM_REFERENCES: &str = "\nHook: \\flmRef{ref1}.\n\nPackage: \\zcref{ref1}.\n";

#[test]
fn the_theorem_goldens_compile_behind_the_authors_own_theorem() {
    under_each_engine(the_theorem_goldens_compile_behind_the_authors_own_theorem_under);
}

fn the_theorem_goldens_compile_behind_the_authors_own_theorem_under() {
    // Three preambles before the style file: none; the author's own
    // `theorem` over `amsthm`, which the style file's declaration of it,
    // guarded on its name, lets stand while the other kinds are declared
    // beside it; and, in the authored form, the author's own `phfthm` with
    // the option the style file requests it with, which LaTeX then loads
    // once.
    let own_theorem = "\\usepackage{amsthm}\n\\newtheorem{theorem}{Theorem}\n";
    for (name, preamble, before) in [
        ("theorems-authored", "none", ""),
        ("theorems-authored", "own-theorem", own_theorem),
        ("theorems-authored", "own-phfthm", "\\usepackage[thmset=empty]{phfthm}\n"),
        ("theorems-resolved", "none", ""),
        ("theorems-resolved", "own-theorem", own_theorem),
    ] {
        let golden = latex_golden(&format!("{name}.tex"));
        for hyperref in [false, true] {
            let result = compile_document_after(
                name,
                &format!("{preamble}{}", if hyperref { "-hyperref" } else { "" }),
                before,
                golden.style.as_slice(),
                if hyperref { "\\usepackage{hyperref}\n" } else { "" },
                &format!("{}{THEOREM_REFERENCES}", golden.output),
            );
            let case = format!("{name} behind {preamble} (hyperref: {hyperref})");
            assert!(result.ok, "{case} did not compile: {}", first_error(&result.log));
            for complaint in ["There were undefined references", "multiply defined"] {
                assert!(!result.log.contains(complaint), "{case}: {complaint}");
            }
            let text = pdf_text(&result.pdf);
            // The theorem is numbered 1, and both references print its
            // number.
            assert!(text.contains("Theorem 1 (My Theorem)."), "{case}: {text}");
            assert!(text.contains("Hook: theorem 1."), "{case}: {text}");
            assert!(text.contains("Package: theorem 1."), "{case}: {text}");
        }
    }
    // The author's own `phfthm` without the option the style file requests it
    // with: LaTeX's option clash, the standard behavior for any package
    // requested again with options its first request lacked.
    let golden = latex_golden("theorems-authored.tex");
    let result = compile_document_after(
        "theorems-authored",
        "own-phfthm-no-options",
        "\\usepackage{phfthm}\n",
        golden.style.as_slice(),
        "",
        &golden.output,
    );
    assert!(!result.ok, "an option-less phfthm before the style file compiled:\n{}", result.log);
    assert!(result.log.contains("Option clash for package phfthm"), "{}", result.log);
}

#[test]
fn a_proofs_heading_names_the_statement_it_proves() {
    under_each_engine(a_proofs_heading_names_the_statement_it_proves_under);
}

fn a_proofs_heading_names_the_statement_it_proves_under() {
    // The authored form: `phfthm` reads `[*ref1]` and names the statement
    // through `\autoref` where the document loads `hyperref`, through `\ref`
    // where it does not; `[**ref2]` is "Proof" alone.
    // The written title `[{\flmProofTitle{Sketch}}]` is an optional argument
    // that is no star reference, which the package reads as the thing the
    // proof proves and sets after its own "Proof of"; the hook's default in
    // this form is the title alone: "Proof of Sketch".
    let golden = latex_golden("theorems-authored.tex");
    for (hyperref, heading) in [(true, "Proof of Theorem 1."), (false, "Proof of 1.")] {
        let result = compile("theorems-authored", &golden, hyperref);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        let text = pdf_text(&result.pdf);
        assert!(text.contains(&format!("{heading} By contradiction.")), "(hyperref: {hyperref}) {text}");
        assert!(text.contains("Proof. Recorded only."), "(hyperref: {hyperref}) {text}");
        assert!(text.contains("Proof of Sketch. A sketch."), "(hyperref: {hyperref}) {text}");
        assert!(!text.contains("Proof (Sketch)"), "(hyperref: {hyperref}) {text}");
    }
    // The resolved form: the proof title hook around the written title and
    // around the reference hook, which prints what `\zcref` prints; two stars
    // write no optional argument, LaTeX's own "Proof".
    let golden = latex_golden("theorems-resolved.tex");
    for hyperref in [false, true] {
        let result = compile("theorems-resolved", &golden, hyperref);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        let text = pdf_text(&result.pdf);
        assert!(text.contains("Proof (theorem 1). By contradiction."), "(hyperref: {hyperref}) {text}");
        assert!(text.contains("Proof. Recorded only."), "(hyperref: {hyperref}) {text}");
        assert!(text.contains("Proof (Sketch). A sketch."), "(hyperref: {hyperref}) {text}");
    }
}

#[test]
fn an_author_redefines_the_proof_title_hook() {
    under_each_engine(an_author_redefines_the_proof_title_hook_under);
}

fn an_author_redefines_the_proof_title_hook_under() {
    // `\renewcommand\flmProofTitle[1]{#1}` after the style file, in the
    // resolved form, whose default is "Proof (<title>)": a proof's title is
    // set alone, the back-reference's too. (The authored form's default is
    // already the title alone.)
    let renew = "\\renewcommand\\flmProofTitle[1]{#1}\n";
    let name = "theorems-resolved";
    let golden = latex_golden(&format!("{name}.tex"));
    let result = compile_document(name, "renew", golden.style.as_slice(), renew, &golden.output);
    assert!(result.ok, "{name} did not compile: {}", first_error(&result.log));
    let text = pdf_text(&result.pdf);
    for expected in ["theorem 1. By contradiction.", "Sketch. A sketch."] {
        assert!(text.contains(expected), "{name}: {expected}: {text}");
    }
    assert!(!text.contains("Proof (Sketch)"), "{name}: {text}");
}

#[test]
fn a_reference_to_a_theorem_names_its_kind() {
    under_each_engine(a_reference_to_a_theorem_names_its_kind_under);
}

fn a_reference_to_a_theorem_names_its_kind_under() {
    // Each kind is its own reference type, named with the kind's words: a
    // lemma counting in the shared counter reads "lemma 2", and a host's
    // kind counting in a counter of its own reads its number, named with its
    // base words where it declares no lowercase ones.
    for (name, expected) in [
        ("theorems-authored", "See theorem 1, lemma 2 and definition 6."),
        ("theorems-resolved", "See theorem 1, lemma 2 and definition 6."),
        ("theorems-host-authored", "See lemma 1 and Exercise 2."),
        ("theorems-host-resolved", "See lemma 1 and Exercise 2."),
        // The theorems count on their own, the lemma and the proposition in
        // the shared counter.
        ("theorems-own-authored", "See theorem 1, lemma 1, theorem 2 and proposition 2."),
        ("theorems-own-resolved", "See theorem 1, lemma 1, theorem 2 and proposition 2."),
    ] {
        let golden = latex_golden(&format!("{name}.tex"));
        for hyperref in [false, true] {
            let result = compile(name, &golden, hyperref);
            assert!(result.ok, "{name} did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
            let text = pdf_text(&result.pdf);
            assert!(text.contains(expected), "{name} (hyperref: {hyperref}): {text}");
        }
    }
}

#[test]
fn a_reference_in_a_kinds_variant_wording_prints_that_wording() {
    under_each_engine(a_reference_in_a_kinds_variant_wording_prints_that_wording_under);
}

fn a_reference_in_a_kinds_variant_wording_prints_that_wording_under() {
    // The goldens reference a lemma in the wordings its kind declares —
    // `abbreviated`, `lowercase`, `capital`, and `S` — and a host's exercise
    // in the host's own variant `short`. The reference hook's default
    // translates the shipped names into `zref-clever`'s keys, the lemma's
    // reference type declaring its abbreviated words, and drops `short`,
    // which has no counterpart: that reference prints the default wording,
    // the exercise's base words, since it declares no lowercase ones.
    //
    // Appended: `abbreviated` on the exercise, whose kind declares no
    // abbreviated words, prints its full name, the package's own fallback;
    // and an option written with a value translates by its key alone. Then
    // `capital` capitalizes every type's name, not the first alone (a lemma
    // after an exercise, whose name is capitalized either way), and holds
    // under a document-wide `abbrev=true`, which the plain reference follows.
    let appended = "\n\nFallback: \\flmRef[abbreviated]{ref2}. Valued: \\flmRef[abbreviated=x]{ref1}.\n\n\
                    Both: \\flmRef[capital]{ref2,ref1}.\n\n\
                    {\\zcsetup{abbrev=true}Abbrev: \\flmRef[capital]{ref1}, \\flmRef{ref1}.}\n";
    for name in ["theorems-e2e-variants-authored", "theorems-e2e-variants-resolved"] {
        let golden = latex_golden(&format!("{name}.tex"));
        for hyperref in [false, true] {
            let result = compile_document(
                name,
                if hyperref { "hyperref" } else { "plain" },
                golden.style.as_slice(),
                if hyperref { "\\usepackage{hyperref}\n" } else { "" },
                &format!("{}{appended}", golden.output),
            );
            let case = format!("{name} (hyperref: {hyperref})");
            assert!(result.ok, "{case} did not compile: {}", first_error(&result.log));
            assert!(!result.log.contains("There were undefined references"), "{case}");
            let text = pdf_text(&result.pdf);
            for expected in [
                "Lem. 1, lemma 1, Lemma 1, Lemma 1 and Exercise 2.",
                "Fallback: Exercise 2. Valued: Lem. 1.",
                "Both: Exercise 2 and Lemma 1.",
                "Abbrev: Lemma 1, Lem. 1.",
            ] {
                assert!(text.contains(expected), "{case}: {expected}: {text}");
            }
        }
    }
}

// --- quotations -----------------------------------------------------------------------------------

#[test]
fn the_quotations_compile_and_print_every_section() {
    under_each_engine(the_quotations_compile_and_print_every_section_under);
}

fn the_quotations_compile_and_print_every_section_under() {
    let golden = latex_golden("quote-tests-1-3.tex");
    for needed in ["\\flmQuoteLines{", "\\flmIndent{1\\flmTabWidth}", "\\begin{blockquote}",
                   "\\begin{address}"] {
        assert!(golden.output.contains(needed), "{needed}: {}", golden.output);
    }
    for hyperref in [false, true] {
        let result = compile("quote-tests-1-3", &golden, hyperref);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        let text = pdf_text(&result.pdf);
        for expected in [
            "To be or not to be.",
            "Shakespeare",
            "First line Second line",
            "One paragraph. Another paragraph.",
            "\u{2014} Someone",
            "First paragraph. Second paragraph.",
            "one line",
            "123 Main St City, ST 12345 Country",
        ] {
            assert!(text.contains(expected), "(hyperref: {hyperref}) {expected}: {text}");
        }
    }
}

#[test]
fn a_line_a_bare_line_break_would_read_prints_as_written() {
    under_each_engine(a_line_a_bare_line_break_would_read_prints_as_written_under);
}

fn a_line_a_bare_line_break_would_read_prints_as_written_under() {
    let golden = latex_golden("quote-lines-edges.tex");
    for needed in ["\\\\\\relax\n[chorus]", "\\\\\\relax\n*starred", "\\flmQuoteLines{\\\\\\relax\n"] {
        assert!(golden.output.contains(needed), "{needed}: {}", golden.output);
    }
    for hyperref in [false, true] {
        let result = compile("quote-lines-edges", &golden, hyperref);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        let text = pdf_text(&result.pdf);
        for expected in [
            "First line",
            "[chorus] sung",
            "*starred line",
            "[after a comment]",
            "After an empty line",
            "Opening after an empty line",
        ] {
            assert!(text.contains(expected), "(hyperref: {hyperref}) {expected}: {text}");
        }
    }
}

#[test]
fn a_hosts_environment_compiles_behind_the_authors_definition_and_hook() {
    under_each_engine(a_hosts_environment_compiles_behind_the_authors_definition_and_hook_under);
}

fn a_hosts_environment_compiles_behind_the_authors_definition_and_hook_under() {
    // The author defines the host's environment, and an attribution hook of
    // their own before loading the style file, which its guarded definition
    // lets stand: the attribution is set between parentheses, with no dash.
    let golden = latex_golden("quote-myquote.tex");
    assert!(golden.output.contains("\\begin{myquote}"), "{}", golden.output);
    let before = "\\newenvironment{myquote}{\\begin{quote}\\itshape}{\\end{quote}}\n\
                  \\NewDocumentCommand\\flmQuoteAttributed{+m}{\\par(by #1)\\par}\n";
    for hyperref in [false, true] {
        let result = compile_document_after(
            "quote-myquote",
            if hyperref { "hyperref" } else { "plain" },
            before,
            golden.style.as_slice(),
            if hyperref { "\\usepackage{hyperref}\n" } else { "" },
            &golden.output,
        );
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        let text = pdf_text(&result.pdf);
        assert!(text.contains("Mine. (by Me)"), "(hyperref: {hyperref}) {text}");
        assert!(!text.contains('\u{2014}'), "(hyperref: {hyperref}) {text}");
    }
    // Without the author's environment the document does not compile: the
    // environment is left to the author, as the style file says.
    let result = compile("quote-myquote", &golden, false);
    assert!(!result.ok, "the host's environment was defined by nobody and yet compiled");
}

// --- annotations ---------------------------------------------------------------------------------

#[test]
fn the_annotations_golden_compiles_behind_the_authors_own_command() {
    under_each_engine(the_annotations_golden_compiles_behind_the_authors_own_command_under);
}

fn the_annotations_golden_compiles_behind_the_authors_own_command_under() {
    // Behind nothing, the configured initials are set beside the first
    // annotation's line; behind the author's own `\phf`, made before the
    // style file, the guard lets the author's command stand, and the
    // configured one is never made.
    let own = "\\usepackage{phfcc}\n\\phfMakeCommentingCommand[initials={Own}]{phf}\n";
    let golden = latex_golden("annotations.tex");
    for (preamble, before, initials, absent) in
        [("none", "", "PhF", "Own"), ("own-command", own, "Own", "PhF")]
    {
        for hyperref in [false, true] {
            let result = compile_document_after(
                "annotations",
                &format!("{preamble}{}", if hyperref { "-hyperref" } else { "" }),
                before,
                golden.style.as_slice(),
                if hyperref { "\\usepackage{hyperref}\n" } else { "" },
                &golden.output,
            );
            let case = format!("annotations behind {preamble} (hyperref: {hyperref})");
            assert!(result.ok, "{case} did not compile: {}", first_error(&result.log));
            let text = pdf_text(&result.pdf);
            assert!(text.contains(initials), "{case}: {text}");
            assert!(!text.contains(absent), "{case}: {text}");
            assert!(text.contains("With multiple paragraphs."), "{case}: {text}");
        }
    }
}

// --- copies of shown content ------------------------------------------------------------------

#[test]
fn the_copies_end_to_end_document_compiles_with_only_its_references_to_nothing_undefined() {
    under_each_engine(the_copies_end_to_end_document_compiles_with_only_its_references_to_nothing_undefined_under);
}

fn the_copies_end_to_end_document_compiles_with_only_its_references_to_nothing_undefined_under() {
    // The end-to-end document of `stdcore_copies_end_to_end.rs`. `sec:nowhere`
    // is written under the key `ref10`, in the heading's title and in the kept
    // definition's template; every other reference resolves, the one to the
    // footnote labeled inside a starred heading's title among them.
    let golden = latex_golden("copies-e2e.tex");
    assert!(golden.output.contains("\\section*{See \\flmRef{ref10}}"), "{}", golden.output);
    for hyperref in [false, true] {
        let result = compile("copies-e2e", &golden, hyperref);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        assert!(result.log.contains("There were undefined references"), "(hyperref: {hyperref})");
        let undefined: Vec<&str> = result
            .log
            .lines()
            .filter(|line| line.starts_with("LaTeX Warning: Reference `"))
            .collect();
        assert!(!undefined.is_empty(), "(hyperref: {hyperref})");
        for line in undefined {
            assert!(line.starts_with("LaTeX Warning: Reference `ref10'"), "(hyperref: {hyperref}) {line}");
        }
        assert!(!result.log.contains("multiply defined"), "(hyperref: {hyperref})");
        let text = pdf_text(&result.pdf);
        // The reference to the footnote prints `footnote` and a number, but not
        // which number: the heading's notes set again at each reference shift
        // the footnote numbers from one LaTeX run to the next.
        let note = text
            .split("its note is footnote ")
            .nth(1)
            .unwrap_or_else(|| panic!("(hyperref: {hyperref}) the reference to the note: {text}"));
        let digits = note.chars().take_while(char::is_ascii_digit).count();
        assert!(
            digits > 0 && note[digits..].starts_with('.'),
            "(hyperref: {hyperref}) the reference to the note: {text}"
        );
        let expected = "A definition refers nowhere: (see ??) and (see ??).";
        assert!(text.contains(expected), "(hyperref: {hyperref}) {expected}: {text}");
    }
}

// --- citations ----------------------------------------------------------------------------------

/// A `thebibliography` environment defining `keys`, in order, each entry its
/// key as text: LaTeX numbers the cited works in that order.
fn bibliography(keys: &[&str]) -> String {
    let mut out = String::from("\n\\begin{thebibliography}{9}\n");
    for key in keys {
        out.push_str(&format!("\\bibitem{{{key}}} Entry {}.\n", key.replace('_', "\\_")));
    }
    out.push_str("\\end{thebibliography}\n");
    out
}

/// Compiles the golden `name` (`tests/goldens/latex/<name>.tex`) with
/// `document_preamble`, plain or with `hyperref`, a `thebibliography`
/// environment defining `keys` appended to its body, and `extra` before it.
/// The compilation must succeed with no undefined or multiply defined
/// citation and no warning from `hyperref` about a PDF string; the PDF's text
/// and the PDF bookmarks (`doc.out`, empty without `hyperref`) are given back.
fn compile_with_bibliography(
    name: &str,
    hyperref: bool,
    document_preamble: &str,
    extra: &str,
    keys: &[&str],
) -> (String, String) {
    let golden = latex_golden(&format!("{name}.tex"));
    let preamble = format!("{document_preamble}{}", if hyperref { "\\usepackage{hyperref}\n" } else { "" });
    let body = format!("{}{extra}{}", golden.output, bibliography(keys));
    let variant = if hyperref { "hyperref" } else { "plain" };
    let result = compile_document(name, variant, golden.style.as_slice(), &preamble, &body);
    assert!(result.ok, "{name} ({variant}) did not compile: {}", first_error(&result.log));
    for complaint in ["There were undefined references", "multiply defined", "Token not allowed"] {
        assert!(!result.log.contains(complaint), "{name} ({variant}): {complaint}\n{}", result.log);
    }
    (pdf_text(&result.pdf), result.bookmarks)
}

#[test]
fn a_chunk_compiles_behind_a_bibliography_and_reads_the_same_with_and_without_a_space() {
    under_each_engine(a_chunk_compiles_behind_a_bibliography_and_reads_the_same_with_and_without_a_space_under);
}

fn a_chunk_compiles_behind_a_bibliography_and_reads_the_same_with_and_without_a_space_under() {
    let golden = latex_golden("citations-chunks.tex");
    assert!(golden.output.contains("\\flmCitePreSpace\\flmCite[Thm 4]{a,b}\\flmCite{c}"), "{}", golden.output);
    for hyperref in [false, true] {
        let (text, _) = compile_with_bibliography(
            "citations-chunks",
            hyperref,
            "",
            "",
            &["a", "b", "c", "arxiv:1234_5", "knuth_84"],
        );
        for expected in [
            "A chunk with notes [1, 2, Thm 4][3].",
            "Written with no space: text [1]. Written with one: text [1].",
            "Keys as typed [4, 5]; through a definition [5].",
        ] {
            assert!(text.contains(expected), "(hyperref: {hyperref}) {expected}: {text}");
        }
    }
}

#[test]
fn the_r_forms_compile_behind_a_bibliography_with_their_words() {
    under_each_engine(the_r_forms_compile_behind_a_bibliography_with_their_words_under);
}

fn the_r_forms_compile_behind_a_bibliography_with_their_words_under() {
    for hyperref in [false, true] {
        let (text, _) = compile_with_bibliography("citations-refcite", hyperref, "", "", &["a", "b", "c"]);
        let expected = "See refs. [1, 2, 3], Refs. [1][2], ref. [1], ([1, 2]) and Ref. [1, Thm 4].";
        assert!(text.contains(expected), "(hyperref: {hyperref}) {expected}: {text}");
    }
}

#[test]
fn a_citation_in_an_uppercased_heading_keeps_its_keys_and_its_note_is_uppercased() {
    under_each_engine(a_citation_in_an_uppercased_heading_keeps_its_keys_and_its_note_is_uppercased_under);
}

fn a_citation_in_an_uppercased_heading_keeps_its_keys_and_its_note_is_uppercased_under() {
    // The page style `headings` sets the section's title in the running head
    // through `\MakeUppercase`; the case-change exclusion list keeps the key,
    // the first brace group after `\flmCite`, and uppercases the note. The
    // same call uppercased directly, and a second chunk, cite the same key.
    // A second heading holds an `\R`. Nothing of a citation stands in a PDF
    // bookmark, the word of `\flmRefCite` included: each bookmark reads the
    // title's own words, written as UTF-16 escapes (`\000` before each ASCII
    // character, `\040` a space).
    let extra = "\n\\MakeUppercase{See\\flmCitePreSpace\\flmCite[Thm 5]{knuth_84}.}\n\
                 \\section{Refs \\flmRefCite{refs.~}{\\flmCite{knuth_84}}}\n";
    let escaped = |word: &str| -> String { word.chars().map(|c| format!("\\000{c}")).collect() };
    for hyperref in [false, true] {
        let (text, bookmarks) =
            compile_with_bibliography("citations-heading", hyperref, "\\pagestyle{headings}\n", extra, &["knuth_84"]);
        for expected in ["RESULTS [1, THM 4]", "Results [1, Thm 4] Text.", "SEE [1, THM 5].", "Refs refs. [1]"] {
            assert!(text.contains(expected), "(hyperref: {hyperref}) {expected}: {text}");
        }
        if hyperref {
            for title in [escaped("Results"), format!("{}\\000\\040", escaped("Refs"))] {
                assert!(bookmarks.contains(&format!("{{\\376\\377{title}}}")), "{title}: {bookmarks}");
            }
        }
    }
}

#[test]
fn the_end_to_end_document_of_citations_compiles_behind_a_bibliography() {
    under_each_engine(the_end_to_end_document_of_citations_compiles_behind_a_bibliography_under);
}

fn the_end_to_end_document_of_citations_compiles_behind_a_bibliography_under() {
    // The keys in the order of their first citation, so that LaTeX's numbers
    // are the document's own.
    let keys = ["shor95", "knuth84", "lamport94", "gottesman97", "unknown", "aharonov97"];
    for hyperref in [false, true] {
        let (text, _) = compile_with_bibliography("citations-e2e", hyperref, "", "", &keys);
        for expected in [
            "Results [1]",
            "A paragraph citing [2], then a chunk of several calls [3][2, Thm 4][1][4, Thm 4]",
            "As shown in ref. [3], in Refs. [2, 1] and in the works (see [3, 1]), the claim holds.",
            "An item citing [2].",
            "Theorem 1 (Main result [3]). A statement citing [1, Lemma 2].",
            "A proof citing [4].",
            "A quotation citing [2]. A line citing [3] a second line",
            "Figure 1: A caption citing [4]",
            "Knuth [2] Shor [1, Sec. 3]",
            "Table 1: A table citing [6]",
            "A note citing [6] and [2, p. 2].",
        ] {
            assert!(text.contains(expected), "(hyperref: {hyperref}) {expected}: {text}");
        }
    }
}

// --- floats --------------------------------------------------------------------------------------

#[test]
fn the_graphics_and_floats_end_to_end_document_compiles_behind_the_authors_float_environments() {
    under_each_engine(
        the_graphics_and_floats_end_to_end_document_compiles_behind_the_authors_float_environments_under,
    );
}

fn the_graphics_and_floats_end_to_end_document_compiles_behind_the_authors_float_environments_under() {
    // The end-to-end document of `stdcore_graphics_floats_end_to_end.rs`:
    // images and floats between paragraphs, in a list item, in a footnote,
    // in a quotation's block section and in a blockquote; the author declares
    // the two configured types' float environments; a small image is written
    // at every path.
    let golden = latex_golden("graphics-floats-e2e-shown.tex");
    let before = "\\usepackage{newfloat}\n\\DeclareFloatingEnvironment[name=Diagram]{diagram}\n\
                  \\DeclareFloatingEnvironment[name=Sketch]{sketch}\n";
    for hyperref in [false, true] {
        let result = compile_document_after(
            "graphics-floats-e2e-shown",
            if hyperref { "hyperref" } else { "plain" },
            before,
            golden.style.as_slice(),
            if hyperref { "\\usepackage{hyperref}\n" } else { "" },
            &golden.output,
        );
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        let text = pdf_text(&result.pdf);
        for expected in [
            "Figure 1: A photo with a note",
            "Table 1: A table shown as an image",
            "Diagram 1: Arrows",
            "An unlabeled diagram",
            "Figure 3: In an item",
            "Figure 4: In a quotation",
            "Figure 5: In a blockquote",
            "In a note",
            "See Fig. 1, Figure 1, Figs. 1 and 2, Photo C, Tab. 1, Diagram 1, A sketch, Fig. 3, In a note and Fig. 4.",
        ] {
            assert!(text.contains(expected), "(hyperref: {hyperref}) {expected}: {text}");
        }
    }
}

#[test]
fn the_four_cases_of_a_float_print_their_numbers_captions_and_references() {
    under_each_engine(the_four_cases_of_a_float_print_their_numbers_captions_and_references_under);
}

fn the_four_cases_of_a_float_print_their_numbers_captions_and_references_under() {
    // A numbered float shows its number and caption, one with no caption its
    // number alone, an unnumbered one its caption alone, with no number; a
    // reference to a numbered float prints the type's words for
    // `zref-clever` and its number — the short words by default, the capital
    // variant's under `S`, as on FLM's other outputs — one to the unnumbered
    // captioned float prints the caption's alternative text.
    let golden = latex_golden("floats-cases.tex");
    for hyperref in [false, true] {
        let result = compile("floats-cases", &golden, hyperref);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        let text = pdf_text(&result.pdf);
        for expected in ["Figure 1: A photo", "A photo of C", "See Fig. 1, Fig. 2 and Photo C. Figure 1 is a photo."] {
            assert!(text.contains(expected), "(hyperref: {hyperref}) {expected}: {text}");
        }
        assert!(text.contains("Figure 2"), "(hyperref: {hyperref}) {text}");
        assert!(!text.contains("Figure 2:"), "(hyperref: {hyperref}) {text}");
        assert!(!text.contains("Figure 3"), "(hyperref: {hyperref}) {text}");
    }
}

#[test]
fn a_configured_float_type_compiles_behind_the_authors_float_environment() {
    under_each_engine(a_configured_float_type_compiles_behind_the_authors_float_environment_under);
}

fn a_configured_float_type_compiles_behind_the_authors_float_environment_under() {
    // The author declares the configured type's float environment, whose
    // counter takes the type's name; the style file's words name its
    // references.
    let golden = latex_golden("floats-configured.tex");
    assert!(golden.output.contains("\\begin{flmFloat}{diagram}{NumCap}"), "{}", golden.output);
    let before = "\\usepackage{newfloat}\n\\DeclareFloatingEnvironment[name=Diagram]{diagram}\n";
    for hyperref in [false, true] {
        let result = compile_document_after(
            "floats-configured",
            if hyperref { "hyperref" } else { "plain" },
            before,
            golden.style.as_slice(),
            if hyperref { "\\usepackage{hyperref}\n" } else { "" },
            &golden.output,
        );
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        let text = pdf_text(&result.pdf);
        for expected in ["Table 1: A table", "Diagram 1: Arrows", "More boxes.", "See Tab. 1 and Diag. 1."] {
            assert!(text.contains(expected), "(hyperref: {hyperref}) {expected}: {text}");
        }
    }
    // Without the author's environment the document does not compile.
    let result = compile("floats-configured", &golden, false);
    assert!(!result.ok, "the configured type's environment was defined by nobody and yet compiled");
}

// --- tables of cells --------------------------------------------------------------------------

#[test]
fn a_table_of_cells_in_a_float_prints_its_cells_its_caption_and_the_references_to_it() {
    under_each_engine(a_table_of_cells_in_a_float_prints_its_cells_its_caption_and_the_references_to_it_under);
}

fn a_table_of_cells_in_a_float_prints_its_cells_its_caption_and_the_references_to_it_under() {
    let golden = latex_golden("cells-float.tex");
    for hyperref in [false, true] {
        let result = compile("cells-float", &golden, hyperref);
        assert!(result.ok, "did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
        let text = pdf_text(&result.pdf);
        for expected in ["A B 1 2", "Table 1: Numbers", "See Tab. 1 and Table 1."] {
            assert!(text.contains(expected), "(hyperref: {hyperref}) {expected}: {text}");
        }
    }
}

#[test]
fn a_cell_holding_two_paragraphs_and_a_nested_table_prints_them() {
    under_each_engine(a_cell_holding_two_paragraphs_and_a_nested_table_prints_them_under);
}

fn a_cell_holding_two_paragraphs_and_a_nested_table_prints_them_under() {
    // The two paragraphs of one cell are set one under the other, and a table
    // inside a cell, where no paragraph can begin, is set without centering.
    let golden = latex_golden("cells-content.tex");
    let result = compile("cells-content", &golden, false);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let text = pdf_text(&result.pdf);
    for expected in ["First paragraph. Second paragraph.", "Numbers"] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
}

#[test]
fn a_table_wider_than_the_line_is_set_again_with_the_fitted_columns() {
    under_each_engine(a_table_wider_than_the_line_is_set_again_with_the_fitted_columns_under);
}

fn a_table_wider_than_the_line_is_set_again_with_the_fitted_columns_under() {
    let golden = latex_golden("cells-wide.tex");
    let result = compile("cells-wide", &golden, false);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let text = pdf_text(&result.pdf);
    assert!(text.contains("A table of cells is a grid"), "{text}");
    // Set at its natural width, the cell's text would stand on one line far
    // beyond the page and be cut; set with the fitted columns it wraps, and
    // its last words reach the page.
    assert!(text.contains("laid out as blocks."), "{text}");
}

#[test]
fn a_cells_table_commands_compose_with_its_entry_and_with_one_another() {
    under_each_engine(a_cells_table_commands_compose_with_its_entry_and_with_one_another_under);
}

fn a_cells_table_commands_compose_with_its_entry_and_with_one_another_under() {
    // Over the style file of the host's styles (`orange` a background,
    // `bold` `cmd=\bfseries`): a cell right-aligned by its entry carrying two
    // table commands; the author's keys for `green`, defined before the style
    // file is loaded, which the registration lets stand; and a cell whose two
    // styles both set `cmd=`, the later winning.
    let golden = latex_golden("cells-host-styles.tex");
    let before = "\\newcommand\\flmCellsKeysgreen{cmd=\\MakeUppercase}\n";
    let body = "\\flmCellsTable{ccc}{X[-1]X[-1]X[-1]}{hspan=minimal,\n  cell{1}{1}={}{r}}{\\toprule\n\
                \\flmCellsSet{orange}\\flmCellsSet{bold}abc&\\flmCellsSet{green}ghi&\
                \\flmCellsSet{bold}\\flmCellsSet{green}jkl\\\\\n\\bottomrule\n}\n";
    let result = compile_document_after("cells-host-styles", "compose", before, golden.style.as_slice(), "", body);
    assert!(result.ok, "did not compile: {}", first_error(&result.log));
    let text = pdf_text(&result.pdf);
    for expected in ["abc", "GHI", "JKL"] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
}

#[test]
fn the_cells_end_to_end_documents_print_their_floats_and_the_references_to_them() {
    under_each_engine(the_cells_end_to_end_documents_print_their_floats_and_the_references_to_them_under);
}

fn the_cells_end_to_end_documents_print_their_floats_and_the_references_to_them_under() {
    // The end-to-end documents of `stdcore_cells_end_to_end.rs`: under
    // `global`, every place, every token and every mistake of a table, the
    // mistakes' marks comment lines; under the default, tables in `table`
    // floats alone, one of them holding a picture (a one-pixel image written
    // at its path), one a table nested in a cell, and a third with no label
    // and no caption, which the default numbering policy (a float is numbered
    // when it carries a label) leaves with no number.
    for (name, expected, absent) in [
        ("cells-e2e-global", &["Table 1: A simple table.", "See Tab. 1 and Table 1."][..], &[][..]),
        (
            "cells-e2e-floats",
            &["Table 1: A table holding a picture", "Table 2: A table nested in a cell", "See Tab. 1 and Table 2."][..],
            &["Table 3"][..],
        ),
    ] {
        let golden = latex_golden(&format!("{name}.tex"));
        for hyperref in [false, true] {
            let result = compile(name, &golden, hyperref);
            assert!(result.ok, "{name} did not compile (hyperref: {hyperref}): {}", first_error(&result.log));
            let text = pdf_text(&result.pdf);
            for expected in expected {
                assert!(text.contains(expected), "{name} (hyperref: {hyperref}) {expected}: {text}");
            }
            for absent in absent {
                assert!(!text.contains(absent), "{name} (hyperref: {hyperref}) not {absent}: {text}");
            }
        }
    }
}

#[test]
fn an_author_redefines_the_table_hook() {
    under_each_engine(an_author_redefines_the_table_hook_under);
}

fn an_author_redefines_the_table_hook_under() {
    // The author's `\flmCellsTable`, defined before the style file is loaded,
    // sets every table at its natural width, never centered: the wide table's
    // cell stands on one line, its last words beyond the page. Every golden of
    // the tables of cells compiles under it.
    let before = "\\NewDocumentCommand\\flmCellsTable{+m +m +m +m}{\\noindent\\begin{tblr}{colspec={#1},#3}#4\\end{tblr}}\n";
    for name in [
        "cells-simple",
        "cells-wide",
        "cells-tokens",
        "cells-content",
        "cells-float",
        "cells-marks",
        "cells-host-styles",
    ] {
        let golden = latex_golden(&format!("{name}.tex"));
        let result = compile_document_after(name, "redefined", before, golden.style.as_slice(), "", &golden.output);
        assert!(result.ok, "{name} did not compile: {}", first_error(&result.log));
        let text = pdf_text(&result.pdf);
        if name == "cells-simple" {
            assert!(text.contains("One Two A B C D"), "{text}");
        } else if name == "cells-wide" {
            assert!(text.contains("A table of cells is a grid"), "{text}");
            assert!(!text.contains("laid out as blocks."), "{text}");
        }
    }
}

// --- a style file named through the naming service -----------------------------------------

#[test]
fn a_style_file_named_through_the_naming_service_is_found_under_that_name() {
    under_each_engine(a_style_file_named_through_the_naming_service_is_found_under_that_name_under);
}

fn a_style_file_named_through_the_naming_service_is_found_under_that_name_under() {
    // A registry whose naming service names a side artifact `job-NAME.EXT`: the
    // recomposed LaTeX loads its style file under that name, which is written beside
    // the document under that name.
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.contribute_to(NAMER_SLOT, Arc::new(JobNamer) as Arc<dyn ArtifactNamer>);
    let registry = builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()));
    let run = run_workflow(
        &registry,
        latex_recomposed::WORKFLOW,
        &["\\section{A}\\label{sec:a} See \\ref{sec:a} and \\verbtext{v}.\n"],
        &Overrides::none(),
    );
    let (style, path) = latex::style(&run);
    assert!(path.starts_with("job-flmrecomposed.") && path.ends_with(".sty"), "{path}");
    let result = compile_document("named", "plain", &[(style, path)], "", &latex::body(&run));
    assert!(result.ok, "the document did not compile: {}", first_error(&result.log));
    for complaint in ["There were undefined references", "but the package provides"] {
        assert!(!result.log.contains(complaint), "{complaint}");
    }
}
