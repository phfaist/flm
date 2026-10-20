//! The converter `magick`, which runs ImageMagick's program (cargo feature `graphics`). No
//! test starts the real program: each names, through the request's
//! `magick_program` or the converter's constructor, a stand-in of its own — a shell script
//! written under the build folder, which reads its standard input, notes its arguments
//! and writes a known PNG image. The document and its images are held in memory.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use flm_core::stdcore::features::graphics::{ConversionOptions, ConversionRequest, GraphicsConverter, ImageFormat};
use flm_main::files::MemoryFiles;
use flm_main::graphics::MagickConverter;
use flm_main::output::{MemorySink, Naming, Sink};
use flm_main::session::{Built, Input, PrepareErrorKind, Prepared, Request, build, prepare};
use flm_main::settings::CommandLineSetting;

// --- the stand-in program -------------------------------------------------------------------

/// The PNG image every stand-in writes: one gray pixel.
fn stand_in_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.write_header().unwrap().write_image_data(&[128]).unwrap();
    bytes
}

/// A stand-in for `magick` in a fresh folder `name` under the build folder: a script that
/// reads its standard input, appends its arguments as one line to `runs.log` beside it,
/// runs `finish` (a line of shell), and writes [`stand_in_png`].
struct StandIn {
    folder: PathBuf,
}

impl StandIn {
    fn new(name: &str, finish: &str) -> StandIn {
        let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-main-magick-{name}"));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("result.png"), stand_in_png()).unwrap();
        let script = format!(
            "#!/bin/sh\ncat > /dev/null\necho \"$@\" >> '{log}'\n{finish}\ncat '{result}'\n",
            log = folder.join("runs.log").display(),
            result = folder.join("result.png").display(),
        );
        let program = folder.join("magick");
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        StandIn { folder }
    }

    /// A stand-in that succeeds.
    fn succeeding(name: &str) -> StandIn {
        StandIn::new(name, "")
    }

    fn program(&self) -> PathBuf {
        self.folder.join("magick")
    }

    /// The arguments of each run, one line per run.
    fn runs(&self) -> Vec<String> {
        std::fs::read_to_string(self.folder.join("runs.log"))
            .map(|log| log.lines().map(String::from).collect())
            .unwrap_or_default()
    }
}

// --- documents and images -------------------------------------------------------------------

/// A PDF file of one page of 200 × 100 points.
fn pdf() -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] >>",
    ];
    let mut text = String::from("%PDF-1.6\n");
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(text.len());
        text.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
    }
    let xref = text.len();
    text.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
    for offset in offsets {
        text.push_str(&format!("{offset:010} 00000 n \n"));
    }
    text.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1));
    text.into_bytes()
}

/// A GIF image of one pixel.
fn gif() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageLuma8(image::GrayImage::new(1, 1))
        .to_rgba8()
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Gif)
        .unwrap();
    bytes
}

/// `doc/paper.flm` holding `content`, whose front matter adds the lines `graphics` among
/// the keys of the feature `graphics`; and the images `doc/figure.pdf` and `doc/icon.gif`.
fn files(graphics: &str, content: &str) -> MemoryFiles {
    let text = format!("---\nflm:\n  features:\n    graphics:\n      global: true\n{graphics}---\n{content}\n");
    let mut files = MemoryFiles::new().with("doc/paper.flm", &text);
    files.insert("doc/figure.pdf", pdf());
    files.insert("doc/icon.gif", gif());
    files
}

/// The request for the page of `doc/paper.flm`, its images copied beside it, into `sink`,
/// the converter `magick` running `program`.
fn page_request(sink: &Arc<MemorySink>, program: &Path) -> Request {
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.format = Some(String::from("html"));
    request.options.push(CommandLineSetting::new("graphics.strategy.html=copy", "-O graphics.strategy.html=copy"));
    request.sink = Some(Arc::clone(sink) as Arc<dyn Sink>);
    request.naming = Naming::File { name: String::from("paper.html") };
    request.magick_program = program.into();
    request
}

fn prepared(files: MemoryFiles, request: Request) -> Prepared {
    prepare(request, Arc::new(files), &[]).unwrap_or_else(|error| panic!("{error:?}"))
}

fn built(prepared: &Prepared) -> Built {
    build(prepared).unwrap_or_else(|error| panic!("{error:?}"))
}

/// The files `sink` received whose names end with `extension`, with their content.
fn received(sink: &MemorySink, extension: &str) -> Vec<(String, Vec<u8>)> {
    sink.files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(extension))
        .map(|(name, file)| (name, file.bytes))
        .collect()
}

/// The Errors of `built`, as the program prints them.
fn errors(built: &Built) -> Vec<String> {
    built
        .diagnostics
        .iter()
        .filter(|d| d.severity() == flm_core::techy::error::Severity::Error)
        .map(flm_main::report::diagnostic)
        .collect()
}

// --- acceptance 1: the rule "PDF to PNG at 192 dots per inch" -------------------------------

#[test]
fn a_pdf_figure_is_copied_as_the_png_image_the_program_wrote() {
    let stand_in = StandIn::succeeding("pdf-to-png");
    let sink = Arc::new(MemorySink::new());
    let prepared = prepared(files("", "\\includegraphics{figure.pdf}"), page_request(&sink, &stand_in.program()));
    let built = built(&prepared);
    assert!(errors(&built).is_empty(), "{:?}", errors(&built));
    let copies = received(&sink, ".png");
    assert_eq!(copies.len(), 1, "{:?}", sink.files().keys().collect::<Vec<_>>());
    assert!(copies[0].0.starts_with("paper-figure-"), "{}", copies[0].0);
    assert_eq!(copies[0].1, stand_in_png());
    assert_eq!(stand_in.runs(), ["-density 192 pdf:-[0] png:-"]);
}

#[test]
fn the_option_transparent_bg_is_given_to_the_program() {
    let stand_in = StandIn::succeeding("transparent");
    let magick = MagickConverter::new(stand_in.program());
    let content = pdf();
    for transparent in [true, false] {
        let options = ConversionOptions::new().transparent_bg(transparent);
        let request = ConversionRequest::new(&content, ImageFormat::Svg, ImageFormat::Pdf, &options);
        assert_eq!(magick.convert(&request), Ok(stand_in_png()));
    }
    assert_eq!(
        stand_in.runs(),
        ["-background none svg:-[0] pdf:-", "svg:-[0] -background white -alpha remove -alpha off pdf:-"]
    );
}

// --- acceptance 2: the results kept -----------------------------------------------------------

#[test]
fn one_figure_shown_twice_and_built_twice_runs_the_program_once() {
    let stand_in = StandIn::succeeding("kept");
    let sink = Arc::new(MemorySink::new());
    let content = "\\includegraphics{figure.pdf} and \\includegraphics{figure.pdf}";
    let prepared = prepared(files("", content), page_request(&sink, &stand_in.program()));
    for _ in 0..2 {
        let built = built(&prepared);
        assert!(errors(&built).is_empty(), "{:?}", errors(&built));
    }
    assert_eq!(stand_in.runs().len(), 1, "{:?}", stand_in.runs());
}

#[test]
fn a_conversion_that_failed_is_asked_of_the_program_again() {
    let stand_in = StandIn::new("failed-again", "exit 1");
    let magick = MagickConverter::new(stand_in.program());
    let (content, options) = (pdf(), ConversionOptions::new());
    let request = ConversionRequest::new(&content, ImageFormat::Pdf, ImageFormat::Png, &options);
    assert!(magick.convert(&request).is_err());
    assert!(magick.convert(&request).is_err());
    assert_eq!(stand_in.runs().len(), 2);
}

#[test]
fn the_same_conversion_asked_twice_at_once_runs_the_program_once() {
    // The stand-in waits before writing, so that the second request arrives while the
    // program runs for the first.
    let stand_in = StandIn::new("at-once", "sleep 0.3");
    let magick = MagickConverter::new(stand_in.program());
    let (content, options) = (pdf(), ConversionOptions::new());
    let request = ConversionRequest::new(&content, ImageFormat::Pdf, ImageFormat::Png, &options);
    let start = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    magick.convert(&request)
                })
            })
            .collect();
        threads.into_iter().map(|thread| thread.join().unwrap()).collect::<Vec<_>>()
    });
    assert_eq!(results, [Ok(stand_in_png()), Ok(stand_in_png())]);
    assert_eq!(stand_in.runs().len(), 1, "{:?}", stand_in.runs());
}

// --- acceptance 3: a program that cannot be started, or that fails -----------------------------

#[test]
fn a_program_that_cannot_be_started_is_reported_at_the_conversion() {
    let absent = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-main-magick-absent/magick");
    let sink = Arc::new(MemorySink::new());
    let mut request = page_request(&sink, &absent);
    request.continue_with_errors = true;
    let prepared = prepared(files("", "\\includegraphics{figure.pdf}"), request);
    let built = built(&prepared);
    let errors = errors(&built);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("figure.pdf") && errors[0].contains("could not be started"), "{}", errors[0]);
    // Written with its errors, the page then holds a copy of the figure as the file is.
    let copies = received(&sink, ".pdf");
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].1, pdf());
}

#[test]
fn a_program_ending_with_a_failure_is_reported_with_what_it_wrote() {
    let stand_in = StandIn::new("exit-status", "echo 'no decode delegate' >&2\nexit 3");
    let sink = Arc::new(MemorySink::new());
    let mut request = page_request(&sink, &stand_in.program());
    request.continue_with_errors = true;
    let prepared = prepared(files("", "\\includegraphics{figure.pdf}"), request);
    let built = built(&prepared);
    let errors = errors(&built);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("failed with the exit status 3: no decode delegate") && errors[0].contains("no decode delegate"), "{}", errors[0]);
    assert_eq!(received(&sink, ".pdf").len(), 1);
}

// --- acceptance 4: which converter performs a rule --------------------------------------------

/// The lines of a front matter setting the page's rules to "GIF to PNG" and "PDF to PNG",
/// each allowing the converters `via` where it is not empty.
fn rules(via: &str) -> String {
    format!(
        "      convert:\n        html:\n          - {{from: gif, to: png{via}}}\n          \
         - {{from: pdf, to: png{via}}}\n"
    )
}

#[test]
fn a_raster_image_is_converted_in_process_and_a_pdf_file_by_the_program() {
    let stand_in = StandIn::succeeding("fallback");
    let sink = Arc::new(MemorySink::new());
    let content = "\\includegraphics{icon.gif} \\includegraphics{figure.pdf}";
    let built = built(&prepared(files(&rules(""), content), page_request(&sink, &stand_in.program())));
    assert!(errors(&built).is_empty(), "{:?}", errors(&built));
    assert_eq!(stand_in.runs(), ["pdf:-[0] png:-"]);
    let copies = received(&sink, ".png");
    assert_eq!(copies.len(), 2);
    let icon = copies.iter().find(|(name, _)| name.starts_with("paper-icon-")).expect("the icon's copy");
    assert_ne!(icon.1, stand_in_png());
    assert_eq!(image::load_from_memory(&icon.1).unwrap().width(), 1);
}

#[test]
fn a_rule_whose_via_names_magick_alone_is_performed_by_the_program() {
    let stand_in = StandIn::succeeding("via");
    let sink = Arc::new(MemorySink::new());
    let content = "\\includegraphics{icon.gif} \\includegraphics{figure.pdf}";
    let built = built(&prepared(files(&rules(", via: [magick]"), content), page_request(&sink, &stand_in.program())));
    assert!(errors(&built).is_empty(), "{:?}", errors(&built));
    let mut runs = stand_in.runs();
    runs.sort();
    assert_eq!(runs, ["gif:-[0] png:-", "pdf:-[0] png:-"]);
    assert!(received(&sink, ".png").iter().all(|(_, bytes)| *bytes == stand_in_png()));
}

// --- acceptance 6: the program is started only for a conversion that needs it ------------------

#[test]
fn a_document_needing_no_conversion_by_the_program_never_starts_it() {
    let absent = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-main-magick-never/magick");
    let sink = Arc::new(MemorySink::new());
    let built = built(&prepared(files("", "\\includegraphics{icon.gif}"), page_request(&sink, &absent)));
    assert!(errors(&built).is_empty(), "{:?}", errors(&built));
}

// --- the program is the embedding program's to give -------------------------------------------

#[test]
fn a_front_matter_naming_the_program_is_refused_and_the_program_never_started() {
    let stand_in = StandIn::succeeding("front-matter");
    let absent = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-main-magick-front-matter-absent/magick");
    let named = format!("    graphics-files:\n      converters:\n        magick:\n          program: '{}'\n", stand_in.program().display());
    let sink = Arc::new(MemorySink::new());
    let mut request = page_request(&sink, &absent);
    request.continue_with_errors = true;
    let error = match prepare(request, Arc::new(files(&named, "\\includegraphics{figure.pdf}")), &[]) {
        Ok(_) => panic!("a front matter naming the program is refused"),
        Err(error) => error,
    };
    assert_eq!(error.kind, PrepareErrorKind::Settings);
    assert_eq!(error.problems.len(), 1, "{error:?}");
    assert_eq!(error.problems[0].detail, "the feature `graphics-files` has no setting `converters`");
    assert!(stand_in.runs().is_empty(), "{:?}", stand_in.runs());
}
