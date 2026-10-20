//! The live preview's loop (`watch::Watch`): driven over files held in memory with no
//! watcher, and over files of the disk under `target/` with its watcher. Every wait has an
//! upper bound and ends as soon as what is expected arrives.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use flm_main::files::{DiskFiles, FileError, Files, MemoryFiles};
use flm_main::output::Naming;
use flm_main::packages::ExtensionFolder;
use flm_main::session::{Input, Request};
use flm_core::stdcore::features::compose::region_id;
use flm_core::techy::error::Severity;
use flm_main::watch::update::PageChange;
use flm_main::watch::{Built, Event, Handle, Watch};

/// The upper bound of every wait.
const BOUND: Duration = Duration::from_secs(30);

/// Files held in memory that a test changes while the loop runs.
#[derive(Clone, Default)]
struct Shared(Arc<Mutex<MemoryFiles>>);

impl Shared {
    fn with(files: MemoryFiles) -> Shared {
        Shared(Arc::new(Mutex::new(files)))
    }

    fn write(&self, path: &str, text: &str) {
        self.0.lock().unwrap().insert(path, text.as_bytes());
    }
}

impl Files for Shared {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError> {
        self.0.lock().unwrap().read(path)
    }

    fn entries(&self, path: &Path) -> Result<Vec<String>, FileError> {
        self.0.lock().unwrap().entries(path)
    }
}

/// A loop running on a thread of its own, its events received here.
struct Running {
    events: Receiver<Event>,
    handle: Handle,
    ended: Receiver<()>,
}

impl Running {
    /// Runs `watch`, calling `hook` on each event on the loop's thread before handing it
    /// here.
    fn start(watch: Watch, mut hook: impl FnMut(&Event, &Handle) + Send + 'static) -> Running {
        let handle = watch.handle();
        let inner = handle.clone();
        let (send, events) = channel();
        let (done, ended) = channel();
        thread::spawn(move || {
            let result = watch.run(|event| {
                hook(&event, &inner);
                let _ = send.send(event);
            });
            result.expect("the loop ran");
            let _ = done.send(());
        });
        Running { events, handle, ended }
    }

    /// The next build done.
    fn built(&self) -> Built {
        loop {
            match self.events.recv_timeout(BOUND).expect("a build within the bound") {
                Event::Built(built) => return *built,
                Event::Building { .. } => {}
                Event::Watcher(error) => panic!("the watcher: {error}"),
            }
        }
    }

    /// Asks for a build now and gives it, passing over any build caused by a change before
    /// it: a build asked for names no file.
    fn asked(&self) -> Built {
        self.handle.build_again();
        loop {
            let built = self.built();
            if built.changed.is_empty() {
                return built;
            }
        }
    }

    /// Stops the loop and waits until it has ended.
    fn stop(self) {
        self.handle.stop();
        self.ended.recv_timeout(BOUND).expect("the loop ends within the bound");
    }
}

/// The loop over `doc/paper.flm` of `files`, held in memory, with no watcher.
fn in_memory(files: &Shared) -> Watch {
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.naming = Naming::File { name: String::from("paper.html") };
    Watch::new(request, Arc::new(files.clone()), Vec::new())
}

const DOC: &str = "---\ntitle: Notes\n---\nHello\\footnote{A first note.}.\n\nA second paragraph.\n";

/// The identifiers of the regions an update names.
fn named(built: &Built) -> Vec<String> {
    match &built.update.page {
        PageChange::Regions(regions) => regions.iter().map(|region| region.id.clone()).collect(),
        other => panic!("not regions: {other:?}"),
    }
}

// --- acceptance 7: the loop over files held in memory ---------------------------------------

#[test]
fn the_loop_over_files_in_memory_gives_the_updates_of_each_kind_of_change() {
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", DOC));
    let running = Running::start(in_memory(&files), |_, _| {});
    let first = running.built();
    assert_eq!(first.update.build, 1);
    assert!(first.prepared.is_some());
    assert_eq!(first.update.page, PageChange::Reload);
    let output = first.output.clone().expect("an output");
    assert!(output.entry().is_some_and(|entry| entry.path == "paper.html"));
    assert!(first.outcome.as_ref().is_ok_and(|diagnostics| diagnostics.is_empty()));
    // The places in the source are written into the page.
    assert!(output.page.as_ref().unwrap().get(&region_id("content")).unwrap().text.contains("data-flm-source-span"));

    // Nothing changed.
    let same = running.asked();
    assert_eq!(same.update.build, 2);
    assert!(same.update.changes_nothing());

    // A change inside the content; then inside a footnote.
    files.write("doc/paper.flm", &DOC.replace("A second paragraph.", "A longer second paragraph."));
    assert_eq!(named(&running.asked()), vec![region_id("content")]);
    files.write("doc/paper.flm", &DOC.replace("A first note.", "A mended note.").replace("A second", "A longer second"));
    assert!(named(&running.asked()).contains(&region_id("flm.xnotes.list.footnote")));

    // The first formula: the page is loaded anew.
    files.write("doc/paper.flm", &DOC.replace("A second paragraph.", "A formula \\(x^2\\)."));
    assert_eq!(running.asked().update.page, PageChange::Reload);

    // An error: the regions and the problems.
    files.write("doc/paper.flm", &DOC.replace("A second paragraph.", "A formula \\(x^2\\) and \\nosuchmacro."));
    let broken = running.asked();
    assert!(broken.update.problems.iter().any(|problem| problem.text.contains("nosuchmacro")));
    assert!(matches!(broken.update.page, PageChange::Reload | PageChange::Regions(_)));

    // No page at all: the update says so, and the output before stays current.
    let shown = broken.output.clone().unwrap();
    files.write("doc/paper.flm", "---\ntitle: [unclosed\n---\nHello.\n");
    let absent = running.asked();
    assert_eq!(absent.update.page, PageChange::Absent);
    assert!(absent.outcome.is_err());
    assert_eq!(absent.update.problems.len(), 1);
    assert!(Arc::ptr_eq(absent.output.as_ref().unwrap(), &shown));

    // Mended: compared with the last output there was.
    files.write("doc/paper.flm", &DOC.replace("A second paragraph.", "A formula \\(x^2\\) and \\nosuchmacro."));
    let mended = running.asked();
    assert!(mended.prepared.is_some());
    assert!(mended.update.changes_nothing(), "{:?}", mended.update.page);
    running.stop();
}

#[test]
fn a_warning_found_before_the_work_stopped_comes_before_the_problems() {
    // `false` for a feature that no package has, beside a problem with the settings: the
    // warning, then the problem.
    let document = "---\nflm:\n  features:\n    nosuch: false\n    text: {wdth: 3}\n---\nHello.\n";
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", document));
    let running = Running::start(in_memory(&files), |_, _| {});
    let first = running.built();
    let problems: Vec<(Severity, &str)> =
        first.update.problems.iter().map(|problem| (problem.severity, problem.text.as_str())).collect();
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert_eq!(
        problems[0],
        (
            Severity::Warning,
            "warning: `nosuch` is switched off, but no package in use has a feature `nosuch`\n  at doc/paper.flm:4:13"
        )
    );
    assert_eq!(problems[1].0, Severity::Error);
    assert!(first.outcome.as_ref().is_err_and(|error| error.diagnostics.len() == 1));
    running.stop();
}

// --- acceptance 11a: preparing again only when needed ---------------------------------------
//
// `Built::prepared` is the observable of the registry's identity: it is `None` exactly when
// the loop built over the `Prepared` it held, whose registry is then the same object.

#[test]
fn only_a_change_to_what_prepare_read_prepares_again() {
    let package = "flm:\n  features:\n    text:\n      width: 60\n";
    let doc = "---\nflm:\n  use: [notes-pkg]\n---\nHello \\input{part.flm}\n";
    // The package is looked for first in `local`, where it is not.
    let files = Shared::with(
        MemoryFiles::new().with("defs/notes-pkg.yaml", package).with("doc/paper.flm", doc).with("doc/part.flm", "one"),
    );
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.extension_folders = vec![
        ExtensionFolder { path: PathBuf::from("local"), required: false },
        ExtensionFolder { path: PathBuf::from("defs"), required: false },
    ];
    request.naming = Naming::File { name: String::from("paper.html") };
    let watch = Watch::new(request, Arc::new(files.clone()), Vec::new());
    let running = Running::start(watch, |_, _| {});
    assert!(running.built().prepared.is_some());
    assert!(running.asked().prepared.is_none());
    files.write("doc/paper.flm", &doc.replace("Hello", "Hello again"));
    assert!(running.asked().prepared.is_none(), "the content alone changed");
    files.write("doc/part.flm", "two");
    assert!(running.asked().prepared.is_none(), "an included file changed");
    files.write("doc/paper.flm", &doc.replace("---\nflm:", "---\ntitle: T\nflm:"));
    assert!(running.asked().prepared.is_some(), "the front matter changed");
    files.write("defs/notes-pkg.yaml", &package.replace("60", "70"));
    assert!(running.asked().prepared.is_some(), "a manifest changed");
    files.write("local/notes-pkg.yaml", package);
    assert!(running.asked().prepared.is_some(), "a manifest appeared where it was looked for first");
    assert!(running.asked().prepared.is_none());
    running.stop();
}

#[cfg(feature = "graphics")]
#[test]
fn a_change_to_an_image_does_not_prepare_again() {
    let doc = "---\nflm:\n  features:\n    graphics:\n      global: true\n---\nA \\includegraphics{plot.png}.\n";
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", doc));
    files.0.lock().unwrap().insert("doc/plot.png", png(2));
    let running = Running::start(in_memory(&files), |_, _| {});
    let first = running.built();
    assert!(first.prepared.is_some());
    assert!(first.output.is_some());
    files.0.lock().unwrap().insert("doc/plot.png", png(3));
    let changed = running.asked();
    assert!(changed.prepared.is_none(), "the image alone changed");
    assert_eq!(named(&changed), vec![region_id("content")]);
    running.stop();
}

// --- acceptance 11 and 13: a build asked for, and changes during a build --------------------

#[test]
fn a_build_asked_for_builds_once_and_gives_the_update_a_change_would() {
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", DOC));
    let running = Running::start(in_memory(&files), |_, _| {});
    running.built();
    files.write("doc/paper.flm", &DOC.replace("A second paragraph.", "A longer second paragraph."));
    let asked = running.asked();
    assert_eq!(asked.update.build, 2);
    assert!(asked.changed.is_empty());
    assert_eq!(named(&asked), vec![region_id("content")]);
    running.stop();
}

#[test]
fn requests_and_changes_while_a_build_runs_cause_exactly_one_more_build() {
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", DOC));
    let during = files.clone();
    // As the second build starts: a change, and two requests, which arrive while it runs.
    let running = Running::start(in_memory(&files), move |event, handle| {
        if let Event::Building { .. } = event {
            static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                handle.build_again();
                handle.build_again();
                during.write("doc/paper.flm", &DOC.replace("A second paragraph.", "Changed."));
            }
        }
    });
    running.built();
    running.handle.build_again();
    let second = running.built();
    assert_eq!(second.update.build, 2);
    assert_eq!(named(&second), vec![region_id("content")]);
    let third = running.built();
    assert_eq!(third.update.build, 3);
    assert!(third.update.changes_nothing());
    running.handle.stop();
    // Nothing but the end follows the third build.
    loop {
        match running.events.recv_timeout(BOUND) {
            Ok(Event::Built(built)) => panic!("a fourth build: {}", built.update.build),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    running.ended.recv_timeout(BOUND).expect("the loop ends");
}

// --- the watcher, over files of the disk under `target/` -----------------------------------

/// An empty folder of its own for the test `name`, under `target/`.
fn folder(name: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("watch").join(name);
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).unwrap();
    folder
}

/// The loop over `paper.flm` in `folder`, on the disk, watching files; it has built once.
fn on_disk(folder: &Path) -> Running {
    let mut request = Request::new(Input::File(folder.join("paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.naming = Naming::File { name: String::from("paper.html") };
    let running = Running::start(Watch::new(request, Arc::new(DiskFiles), Vec::new()).watching_files(), |_, _| {});
    running.built();
    // A change the system reports late from the files' creation is passed over here.
    running.asked();
    running
}

/// The next build caused by a change, and the names of the files that changed.
fn changed(running: &Running) -> (Built, Vec<String>) {
    let built = running.built();
    assert!(!built.changed.is_empty(), "a build caused by a change");
    let names = built.changed.iter().map(|path| path.file_name().unwrap().to_string_lossy().into_owned()).collect();
    (built, names)
}

// acceptance 8 and 12
#[test]
fn a_write_to_a_watched_file_rebuilds_once_and_other_files_of_its_folder_are_ignored() {
    let folder = folder("write");
    fs::write(folder.join("paper.flm"), DOC).unwrap();
    let running = on_disk(&folder);

    fs::write(folder.join("paper.flm"), DOC.replace("A second", "A longer second")).unwrap();
    let (built, names) = changed(&running);
    assert_eq!(names, ["paper.flm"]);
    assert_eq!(named(&built), vec![region_id("content")]);

    // Several writes within the quiet period: one build.
    for text in ["one", "two", "three"] {
        fs::write(folder.join("paper.flm"), DOC.replace("A second paragraph.", text)).unwrap();
    }
    let (built, names) = changed(&running);
    assert_eq!(names, ["paper.flm"]);
    assert!(built.output.unwrap().page.as_ref().unwrap().get(&region_id("content")).unwrap().text.contains("three"));
    // No second build followed the writes: the next build is the one a later save causes,
    // numbered one more.
    let number = built.update.build;
    fs::write(folder.join("paper.flm"), DOC.replace("A second paragraph.", "four")).unwrap();
    let (built, names) = changed(&running);
    assert_eq!((built.update.build, names), (number + 1, vec![String::from("paper.flm")]));

    // Files of the folder that the build did not read, the citations cache among them, then
    // the document: the next build names the document alone and is numbered one more. A
    // build caused by the other files would come first and name them, or raise the number,
    // or join the same batch and be named in it.
    fs::write(folder.join("notes.txt"), "x").unwrap();
    fs::write(folder.join(".flm-citations.jsonl"), "{}\n").unwrap();
    fs::write(folder.join("._.flm-citations-scratch"), "{}\n").unwrap();
    fs::write(folder.join("paper.flm"), DOC.replace("A second paragraph.", "five")).unwrap();
    let (built, names) = changed(&running);
    assert_eq!((built.update.build, names), (number + 2, vec![String::from("paper.flm")]));

    // The stop ends the loop. The watcher is released with it: `Watch::run` owns the
    // watcher, which is dropped when `run` returns.
    running.stop();
}

// acceptance 11, the first build included
#[test]
fn a_save_of_the_document_while_the_first_build_runs_causes_one_more_build() {
    let folder = folder("first-build");
    fs::write(folder.join("paper.flm"), DOC).unwrap();
    let mut request = Request::new(Input::File(folder.join("paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.naming = Naming::File { name: String::from("paper.html") };
    let watch = Watch::new(request, Arc::new(DiskFiles), Vec::new()).watching_files();
    let path = folder.join("paper.flm");
    // Saved as the first build starts: the hook runs on the loop's thread, before the build.
    let running = Running::start(watch, move |event, _| {
        if let Event::Building { changed } = event {
            static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            if COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                assert!(changed.is_empty());
                fs::write(&path, DOC.replace("A second", "A saved second")).unwrap();
            }
        }
    });
    assert_eq!(running.built().update.build, 1);
    let (second, names) = changed(&running);
    assert_eq!(second.update.build, 2);
    assert_eq!(names, ["paper.flm"]);
    // Exactly one more: the next build is the one a later save causes.
    fs::write(folder.join("paper.flm"), DOC.replace("A second", "A later second")).unwrap();
    let (third, names) = changed(&running);
    assert_eq!(third.update.build, 3);
    assert_eq!(names, ["paper.flm"]);
    running.stop();
}

// what a build that stopped early read is watched
#[test]
fn an_extension_written_after_a_build_that_could_not_find_it_causes_a_build() {
    let folder = folder("extension");
    fs::create_dir_all(folder.join("defs")).unwrap();
    fs::write(folder.join("paper.flm"), "---\nflm:\n  use: [notes-pkg]\n---\nHello.\n").unwrap();
    let mut request = Request::new(Input::File(folder.join("paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.naming = Naming::File { name: String::from("paper.html") };
    request.extension_folders = vec![ExtensionFolder { path: folder.join("defs"), required: false }];
    let running = Running::start(Watch::new(request, Arc::new(DiskFiles), Vec::new()).watching_files(), |_, _| {});
    let first = running.built();
    assert!(first.output.is_none());
    let error = first.outcome.expect_err("the extension is not found");
    assert!(error.files_read.iter().any(|read| read.path == folder.join("defs/notes-pkg.yaml") && !read.found), "{:?}", error.files_read);
    fs::write(folder.join("defs/notes-pkg.yaml"), "flm:\n  features:\n    text:\n      width: 60\n").unwrap();
    let (built, names) = changed(&running);
    // The system may report the document's first writing late, with this change.
    assert!(names.iter().all(|name| name == "notes-pkg.yaml" || name == "paper.flm") && names.contains(&String::from("notes-pkg.yaml")), "{names:?}");
    assert!(built.outcome.is_ok());
    assert_eq!(built.update.page, PageChange::Reload);
    assert!(built.output.is_some());
    running.stop();
}

// acceptance 9
#[test]
fn a_file_replaced_by_renaming_another_over_it_is_seen() {
    let folder = folder("rename");
    fs::write(folder.join("paper.flm"), DOC).unwrap();
    let running = on_disk(&folder);
    fs::write(folder.join(".paper.flm.swp"), DOC.replace("A second", "A renamed second")).unwrap();
    fs::rename(folder.join(".paper.flm.swp"), folder.join("paper.flm")).unwrap();
    let (built, names) = changed(&running);
    assert!(names.contains(&String::from("paper.flm")), "{names:?}");
    assert!(built.output.unwrap().page.as_ref().unwrap().get(&region_id("content")).unwrap().text.contains("renamed"));
    running.stop();
}

// acceptance 10
#[test]
fn a_file_brought_in_by_input_is_watched_once_a_build_read_it() {
    let folder = folder("input");
    fs::create_dir_all(folder.join("parts")).unwrap();
    fs::write(folder.join("paper.flm"), "Main.\n").unwrap();
    fs::write(folder.join("parts/one.flm"), "One.\n").unwrap();
    let running = on_disk(&folder);
    fs::write(folder.join("paper.flm"), "Main \\input{parts/one.flm}\n").unwrap();
    assert_eq!(changed(&running).1, ["paper.flm"]);
    fs::write(folder.join("parts/one.flm"), "One, changed.\n").unwrap();
    let (built, names) = changed(&running);
    assert_eq!(names, ["one.flm"]);
    assert!(built.changed[0].ends_with("parts/one.flm"));
    running.stop();
}

// acceptance 10: an image, found or looked for and not found
#[cfg(feature = "graphics")]
#[test]
fn an_image_file_is_watched_and_so_is_a_folder_it_was_looked_for_in() {
    let folder = folder("image");
    fs::create_dir_all(folder.join("figures")).unwrap();
    let front = "---\nflm:\n  features:\n    graphics:\n      global: true\n    graphics-files:\n      search_path: [figures, .]\n---\n";
    fs::write(folder.join("paper.flm"), format!("{front}A \\includegraphics{{plot.png}}.\n")).unwrap();
    fs::write(folder.join("plot.png"), b"not yet an image").unwrap();
    let running = on_disk(&folder);
    fs::write(folder.join("plot.png"), b"still not an image").unwrap();
    assert_eq!(changed(&running).1, ["plot.png"]);
    // Looked for first in `figures`, where it was not found.
    fs::write(folder.join("figures/plot.png"), b"found here now").unwrap();
    let (built, names) = changed(&running);
    assert_eq!(names, ["plot.png"]);
    assert!(built.changed[0].ends_with("figures/plot.png"), "{:?}", built.changed);
    running.stop();
}

/// A bibliography file in CSL-JSON holding one book titled `title`.
#[cfg(feature = "citations")]
fn record(title: &str) -> String {
    format!("[{{\"id\": \"knuth84\", \"type\": \"book\", \"title\": \"{title}\", \"author\": [{{\"family\": \"Knuth\"}}]}}]")
}

// acceptance 10: a bibliography file
#[cfg(feature = "citations")]
#[test]
fn a_bibliography_file_the_build_read_is_watched() {
    let folder = folder("bibliography");
    let doc = "---\nflm:\n  features:\n    citations-standard:\n      bibliography: [refs.json]\n    text: {}\n---\nSee \\cite{bib:knuth84}.\n";
    fs::write(folder.join("paper.flm"), doc).unwrap();
    fs::write(folder.join("refs.json"), record("The TeXbook")).unwrap();
    let running = on_disk(&folder);
    fs::write(folder.join("refs.json"), record("The METAFONTbook")).unwrap();
    let (built, names) = changed(&running);
    assert_eq!(names, ["refs.json"]);
    assert_eq!(named(&built), vec![region_id("flm.citations.bibliography")]);
    running.stop();
}

// --- acceptance 4: an image copied beside the page -----------------------------------------

/// A PNG image of `width` × 1 gray pixels.
#[cfg(feature = "graphics")]
fn png(width: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, 1);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&vec![128; width as usize]).unwrap();
    writer.finish().unwrap();
    bytes
}

#[cfg(feature = "graphics")]
#[test]
fn an_image_copied_beside_the_page_is_named_when_it_changes_and_the_content_names_the_new_copy() {
    let doc = "---\nflm:\n  features:\n    graphics:\n      global: true\n---\nA \\includegraphics{plot.png}.\n";
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", doc));
    files.0.lock().unwrap().insert("doc/plot.png", png(2));
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.naming = Naming::File { name: String::from("paper.html") };
    request.options.push(flm_main::settings::CommandLineSetting::new("graphics.strategy.html=copy", "test"));
    let running = Running::start(Watch::new(request, Arc::new(files.clone()), Vec::new()), |_, _| {});
    let first = running.built();
    let output = first.output.clone().unwrap();
    let copies: Vec<&String> = output.files.keys().filter(|path| path.ends_with(".png")).collect();
    assert_eq!(copies.len(), 1, "{:?}", output.files.keys());
    let old = copies[0].clone();
    // The same bytes: nothing named.
    assert!(running.asked().update.changes_nothing());
    // Other bytes: a copy under a new name, the old one gone, and the content names it.
    files.0.lock().unwrap().insert("doc/plot.png", png(3));
    let changed = running.asked();
    assert_eq!(changed.update.files.len(), 2, "{:?}", changed.update.files);
    assert!(changed.update.files.contains(&old));
    let new = changed.update.files.iter().find(|path| **path != old).unwrap();
    assert!(changed.output.as_ref().unwrap().files.contains_key(new));
    assert_eq!(named(&changed), vec![region_id("content")]);
    let PageChange::Regions(regions) = &changed.update.page else { unreachable!() };
    assert!(regions[0].text.contains(new.as_str()), "{}", regions[0].text);
    running.stop();
}

// --- an extension written in JavaScript: an edit of a script prepares again ----------------------

/// The text of the content region of a build's page.
#[cfg(feature = "js")]
fn content(built: &Built) -> String {
    built.output.as_ref().expect("an output").page.as_ref().unwrap().get(&region_id("content")).unwrap().text.clone()
}

/// The extension `wordcount` of `flm-ext`'s tests, in memory under
/// `doc/_flmextensions/wordcount/`.
#[cfg(feature = "js")]
const WORDCOUNT: [(&str, &str); 3] = [
    ("flmextension.yaml", include_str!("../../flm-ext/tests/js/wordcount/flmextension.yaml")),
    ("main.js", include_str!("../../flm-ext/tests/js/wordcount/main.js")),
    ("helpers.js", include_str!("../../flm-ext/tests/js/wordcount/helpers.js")),
];

#[cfg(feature = "js")]
#[test]
fn an_edit_of_a_script_of_an_extension_prepares_again_and_the_output_follows() {
    let doc = "---\nflm:\n  use: [wordcount]\n  features:\n    wordcount:\n      shout_suffix: \"!\"\n---\nSay \\shout{hey}.\n";
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", doc));
    for (name, text) in WORDCOUNT {
        files.write(&format!("doc/_flmextensions/wordcount/{name}"), text);
    }
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("doc/_flmextensions"), required: false }];
    request.naming = Naming::File { name: String::from("paper.html") };
    let running = Running::start(Watch::new(request, Arc::new(files.clone()), Vec::new()), |_, _| {});
    let first = running.built();
    assert!(first.outcome.as_ref().is_ok_and(|diagnostics| diagnostics.is_empty()), "{:?}", first.outcome);
    assert!(content(&first).contains("HEY!"), "{}", content(&first));
    assert!(running.asked().prepared.is_none());

    // The main module, with another suffix: prepared again, and the output follows.
    files.write("doc/_flmextensions/wordcount/main.js", &WORDCOUNT[1].1.replace("text.toUpperCase() + suffix", "text.toUpperCase() + suffix + suffix"));
    let built = running.asked();
    assert!(built.prepared.is_some());
    assert!(content(&built).contains("HEY!!"), "{}", content(&built));

    // A module the main module imports.
    files.write("doc/_flmextensions/wordcount/helpers.js", &format!("{}\n// edited\n", WORDCOUNT[2].1));
    assert!(running.asked().prepared.is_some());

    // A file of the extension's folder that no module imports was never read.
    files.write("doc/_flmextensions/wordcount/notes.js", "export const unused = 1;\n");
    assert!(running.asked().prepared.is_none());
    running.stop();
}

// --- the host service `files`: a file a handler reads is watched ------------------------------

/// The extension `bibfile` of `flm-ext`'s tests, in memory under `doc/_flmextensions/bibfile/`.
#[cfg(feature = "js")]
const BIBFILE: [(&str, &str); 2] = [
    ("flmextension.yaml", include_str!("../../flm-ext/tests/js/bibfile/flmextension.yaml")),
    ("main.js", include_str!("../../flm-ext/tests/js/bibfile/main.js")),
];

/// The extension `tinytpl` of `flm-ext`'s tests, in memory under `doc/_flmextensions/tinytpl/`.
#[cfg(feature = "js")]
const TINYTPL: [(&str, &str); 4] = [
    ("flmextension.yaml", include_str!("../../flm-ext/tests/js/tinytpl/flmextension.yaml")),
    ("main.js", include_str!("../../flm-ext/tests/js/tinytpl/main.js")),
    ("templates/html/tiny.js", include_str!("../../flm-ext/tests/js/tinytpl/templates/html/tiny.js")),
    ("templates/html/head.html", include_str!("../../flm-ext/tests/js/tinytpl/templates/html/head.html")),
];

/// The entries `bib.json` of the document of `bibfile`, the title of `knuth84` given.
#[cfg(feature = "js")]
fn entries(title: &str) -> String {
    format!("{{\"knuth84\": {{\"author\": \"D. E. Knuth\", \"title\": \"{title}\", \"year\": 1984}}}}")
}

/// The request of `doc/paper.flm`, its extensions under `doc/_flmextensions/`.
#[cfg(feature = "js")]
fn with_extensions_beside() -> Request {
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("doc/_flmextensions"), required: false }];
    request.naming = Naming::File { name: String::from("paper.html") };
    request
}

/// The file a scripted citation provider reads through the host service `files` is among
/// the files the build read; a change to it builds again over the same preparation, and the
/// bibliography follows.
#[cfg(feature = "js")]
#[test]
fn a_file_a_scripted_provider_read_is_watched() {
    let doc = "---\nflm:\n  use: [bibfile]\n---\nSee \\cite{knuth84}.\n";
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", doc));
    for (name, text) in BIBFILE {
        files.write(&format!("doc/_flmextensions/bibfile/{name}"), text);
    }
    files.write("doc/bib.json", &entries("The TeXbook"));
    let running = Running::start(Watch::new(with_extensions_beside(), Arc::new(files.clone()), Vec::new()), |_, _| {});
    let first = running.built();
    assert!(first.outcome.as_ref().is_ok_and(|diagnostics| diagnostics.is_empty()), "{:?}", first.outcome);
    assert!(first.files_read.iter().any(|read| read.path == Path::new("doc/bib.json") && read.found), "{:?}", first.files_read);
    assert!(page_of(&first).contains("The TeXbook"));
    files.write("doc/bib.json", &entries("The METAFONTbook"));
    let built = running.asked();
    assert!(built.prepared.is_none());
    assert_eq!(named(&built), vec![region_id("flm.citations.bibliography")]);
    assert!(page_of(&built).contains("The METAFONTbook"), "{}", page_of(&built));
    running.stop();
}

/// The partial a template module reads from its extension's folder at render time is among
/// the files the build read; a change to it builds again over the same preparation, and the
/// page follows.
#[cfg(feature = "js")]
#[test]
fn a_partial_a_template_module_reads_is_watched() {
    let doc = "---\nflm:\n  use: [tinytpl]\n  features:\n    compose:\n      template_config:\n        html:\n          tiny:\n            banner: logo.png\n---\nHello.\n";
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", doc));
    for (name, text) in TINYTPL {
        files.write(&format!("doc/_flmextensions/tinytpl/{name}"), text);
    }
    files.0.lock().unwrap().insert("doc/logo.png", include_bytes!("../../flm-cli/tests/documents/extensions/logo.png").to_vec());
    let mut request = with_extensions_beside();
    request.options.push(flm_main::settings::CommandLineSetting::new("compose.template={html: tiny}", "test"));
    let running = Running::start(Watch::new(request, Arc::new(files.clone()), Vec::new()), |_, _| {});
    let first = running.built();
    assert!(first.outcome.as_ref().is_ok_and(|diagnostics| diagnostics.is_empty()), "{:?}", first.outcome);
    assert!(page_of(&first).starts_with("<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n"), "{}", page_of(&first));
    for path in ["doc/_flmextensions/tinytpl/templates/html/head.html", "doc/logo.png"] {
        assert!(first.files_read.iter().any(|read| read.path == Path::new(path) && read.found), "{path}: {:?}", first.files_read);
    }
    files.write("doc/_flmextensions/tinytpl/templates/html/head.html", "<meta name=\"edited\">\n");
    let built = running.asked();
    assert!(built.prepared.is_none());
    assert!(page_of(&built).contains("<head>\n<meta name=\"edited\">\n<title>"), "{}", page_of(&built));
    running.stop();
}

// --- every output of the workflow: the leading one shown, a shipped file watched --------------

#[test]
fn a_build_of_two_outputs_shows_the_leading_one_and_an_edit_of_a_shipped_file_prepares_again() {
    let doc = "---\nflm:\n  use: [pair]\n---\nHello.\n";
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", doc));
    files.write("defs/pair/flmextension.yaml", "flm:\n  workflows:\n    pair:\n      extends: html\n      outputs: [html, text]\n");
    files.write(
        "defs/pair/templates/html/pair/flmtemplate.yaml",
        "output: html\nengine: minijinja\nmain: pair.html\naccepts: all\nartifacts:\n  style: {file: look.css}\n",
    );
    files.write("defs/pair/templates/html/pair/pair.html", "<link href=\"{{ artifacts.style }}\">{{ regions.content }}\n");
    files.write("defs/pair/templates/html/pair/look.css", "p { color: teal; }\n");
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.profile = Some(String::from("pair"));
    request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("defs"), required: false }];
    request.options.push(flm_main::settings::CommandLineSetting::new("compose.template={html: pair}", "test"));
    request.naming = Naming::File { name: String::from("paper.html") };
    let running = Running::start(Watch::new(request, Arc::new(files.clone()), Vec::new()), |_, _| {});
    let first = running.built();
    assert!(first.outcome.as_ref().is_ok_and(|diagnostics| diagnostics.is_empty()), "{:?}", first.outcome);
    assert_eq!(first.prepared.as_ref().unwrap().outputs, ["html", "text"]);
    let output = first.output.clone().unwrap();
    assert_eq!(output.leading, "html");
    assert_eq!(output.entry().unwrap().path, "paper.html");
    // The other output's files are held, and served, by path.
    assert_eq!(output.files.keys().collect::<Vec<_>>(), ["paper-style.css", "paper.html", "paper.txt"]);
    assert_eq!(output.files["paper.txt"].output, "text");
    assert!(running.asked().prepared.is_none());
    // The template's shipped file changes: prepared again, and the copy follows.
    files.write("defs/pair/templates/html/pair/look.css", "p { color: red; }\n");
    let built = running.asked();
    assert!(built.prepared.is_some());
    assert_eq!(built.update.files, ["paper-style.css"]);
    assert_eq!(built.output.unwrap().files["paper-style.css"].bytes, b"p { color: red; }\n");
    running.stop();
}

// --- the files a compose template includes: read by the build, watched ----------------------

/// The extension `inc` under `defs/` of `files`: a compose template whose page includes
/// `partials/head.html` and writes `dist/x.js` as it is.
fn write_including_extension(write: &dyn Fn(&str, &str)) {
    write("defs/inc/flmextension.yaml", "flm: {}\n");
    write("defs/inc/templates/html/inc/flmtemplate.yaml", "output: html\nengine: minijinja\nmain: inc.html\naccepts: all\n");
    write(
        "defs/inc/templates/html/inc/inc.html",
        "{% include \"partials/head.html\" %}<script>{{ include_raw(\"../../../dist/x.js\") }}</script>{{ regions.content }}\n",
    );
    write("defs/inc/templates/html/inc/partials/head.html", "<header>head one</header>");
    write("defs/inc/dist/x.js", "let x = 1;");
}

/// The request of `doc/paper.flm` (under `root`) with the compose template `inc` of the
/// extension folder `defs` (under `root`).
fn including_request(root: &Path) -> Request {
    let mut request = Request::new(Input::File(root.join("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.extension_folders = vec![ExtensionFolder { path: root.join("defs"), required: false }];
    request.options.push(flm_main::settings::CommandLineSetting::new("compose.template={html: inc}", "test"));
    request.naming = Naming::File { name: String::from("paper.html") };
    request
}

/// The entry page of a build's output.
fn page_of(built: &Built) -> String {
    let output = built.output.clone().expect("an output");
    String::from_utf8(output.entry().expect("the page").bytes.clone()).unwrap()
}

#[test]
fn an_included_file_is_among_the_files_read_and_an_edit_of_it_builds_again_with_the_same_preparation() {
    let doc = "---\nflm:\n  use: [inc]\n---\nHello.\n";
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", doc));
    write_including_extension(&|path, text| files.write(path, text));
    let running = Running::start(Watch::new(including_request(Path::new("")), Arc::new(files.clone()), Vec::new()), |_, _| {});
    let first = running.built();
    assert!(first.outcome.as_ref().is_ok_and(|diagnostics| diagnostics.is_empty()), "{:?}", first.outcome);
    assert!(page_of(&first).starts_with("<header>head one</header><script>let x = 1;</script>"), "{}", page_of(&first));
    // Both included files are among the files the build read, found.
    for path in ["defs/inc/templates/html/inc/partials/head.html", "defs/inc/dist/x.js"] {
        assert!(first.files_read.iter().any(|read| read.path == Path::new(path) && read.found), "{path}: {:?}", first.files_read);
    }
    assert!(running.asked().prepared.is_none());
    // The included template changes: built again with the same preparation, and the page
    // follows.
    files.write("defs/inc/templates/html/inc/partials/head.html", "<header>head two</header>");
    let built = running.asked();
    assert!(built.prepared.is_none());
    assert!(page_of(&built).starts_with("<header>head two</header>"), "{}", page_of(&built));
    // So does the file included as it is.
    files.write("defs/inc/dist/x.js", "let x = 2;");
    let built = running.asked();
    assert!(built.prepared.is_none());
    assert!(page_of(&built).contains("<script>let x = 2;</script>"), "{}", page_of(&built));
    running.stop();
}

#[test]
fn the_files_a_compose_template_includes_are_read_by_each_build_not_by_the_preparation() {
    let doc = "---\nflm:\n  use: [inc]\n---\nHello.\n";
    let files = Shared::with(MemoryFiles::new().with("doc/paper.flm", doc));
    write_including_extension(&|path, text| files.write(path, text));
    let prepared = flm_main::session::prepare(including_request(Path::new("")), Arc::new(files.clone()), &[]).expect("prepared");
    let prepared_reads = prepared.files_read();
    let included = ["defs/inc/templates/html/inc/partials/head.html", "defs/inc/dist/x.js"];
    // The number of times each build lists each included file: the same for every build (the
    // page of an HTML output is rendered twice, once with its regions left out, so each read
    // appears twice), never growing with the builds made before.
    let mut counts = Vec::new();
    for _ in 0..3 {
        let built = flm_main::session::build(&prepared).expect("built");
        let count: Vec<usize> = included
            .iter()
            .map(|path| built.files_read.iter().filter(|read| read.path == Path::new(path) && read.found).count())
            .collect();
        assert!(count.iter().all(|&count| count > 0), "{:?}", built.files_read);
        counts.push(count);
        // The preparation's own record is unchanged by the build.
        assert_eq!(prepared.files_read(), prepared_reads);
    }
    assert!(counts.windows(2).all(|pair| pair[0] == pair[1]), "{counts:?}");
    for path in included {
        assert!(!prepared_reads.iter().any(|read| read.path == Path::new(path)), "{path}: {prepared_reads:?}");
    }
}

#[test]
fn a_write_to_an_included_file_on_the_disk_causes_a_build() {
    let root = folder("include");
    let write = |path: &str, text: &str| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    write("doc/paper.flm", "---\nflm:\n  use: [inc]\n---\nHello.\n");
    write_including_extension(&write);
    let running = Running::start(Watch::new(including_request(&root), Arc::new(DiskFiles), Vec::new()).watching_files(), |_, _| {});
    let first = running.built();
    assert!(first.outcome.as_ref().is_ok_and(|diagnostics| diagnostics.is_empty()), "{:?}", first.outcome);
    running.asked();
    write("defs/inc/dist/x.js", "let x = 3;");
    let (built, names) = changed(&running);
    assert_eq!(names, ["x.js"]);
    assert!(page_of(&built).contains("<script>let x = 3;</script>"), "{}", page_of(&built));
    write("defs/inc/templates/html/inc/partials/head.html", "<header>head three</header>");
    let (built, names) = changed(&running);
    assert_eq!(names, ["head.html"]);
    assert!(page_of(&built).starts_with("<header>head three</header>"), "{}", page_of(&built));
    running.stop();
}
