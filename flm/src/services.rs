//! The program's services to the live preview ([`flm_host::watch::services`]): what only
//! the program knows, which `flm watch` gives the preview's server when it starts it.
//!
//! **The help pages** ([`HelpPages`]) are the pages of `flm info -d DOCUMENT` for the
//! environment of the document's last preparation, made by [`flm_info::pages`] with the
//! program's information ([`program_info`]): the index of every page, with the names it is
//! looked up by ([`Tree::index`]) and its summary as plain text; one page, its content
//! rendered as `flm info` renders a page ([`info::render`]) but with the profile `html`, no
//! compose template (`compose.template=none`) and each link to another page written as the
//! page's path followed by `.html` (`flmdoc.link_style=path`); the nearest paths, as `flm
//! info` gives them ([`nearest`]), for a path without a page; and the page of a construct
//! written in the document, the page of the command that its feature declares.
//!
//! What the pages need is kept for the environment of the last preparation asked about,
//! by the digest of its registry, and computed when it is first asked for, on the thread of
//! the request that asks: the tree of the pages, then the index, then each page rendered.
//! A preparation of another environment replaces it.
//!
//! **The command line of an export** ([`RenderLine`]) is the `flm render` command that
//! writes what the preview's export writes: `flm render -p TYPE [--continue-with-errors]
//! [OPTIONS] -o FOLDER/STEM.EXT DOCUMENT --view`, where `OPTIONS` are the options of `flm
//! watch` that change the build, as typed, `DOCUMENT` the document's file as typed, and
//! `FOLDER` its folder as typed, so that the command writes beside the document from the
//! folder `flm watch` was started in. Each word that a POSIX shell would read otherwise is
//! written between single quotation marks.
//!
//! **Opening a file** is [`opener::open_file`](crate::opener::open_file), as `flm render
//! -V` opens the file it wrote.
//!
//! **The home folder**, in which the preview keeps the viewer settings, is the one in
//! which the user's extensions are looked for: the environment variable `HOME`, or
//! `USERPROFILE` where `HOME` is not set or empty
//! ([`home_folder`](flm_host::files::system::home_folder)).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use flm_core::lang::text::plain_text;
use flm_core::lang::{BuiltLang, FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::model::doc::FragmentLayout;
use flm_info::Environment;
use flm_info::pages::{Node, Page, PageKind, SourceOptions, Tree};
use flm_host::files::system::{home_folder, to_system};
use flm_host::files::{DiskFiles, Files};
use flm_host::session::{Input, Prepared};
use flm_host::settings::CommandLineSetting;
use flm_host::watch::services::{Help, HelpEntry, HelpError, HelpPage, HelpSegment, OpenFileError, Services};
use flm_host::watch::snapshot::Construct;

use crate::Carried;
use crate::args::BuildArgs;
use crate::info::{self, nearest, prepare_page, program_info};

/// The profile with which a help page is rendered for the preview.
const PROFILE: &str = "html";

/// The settings with which a help page is rendered for the preview: no compose template, so
/// that the output is the page's content alone, and each link to another page written as
/// the page's path followed by `.html`.
const SETTINGS: [&str; 2] = ["compose.template=none", "flmdoc.link_style=path"];

/// The services the `flm` program gives the preview: the help pages, the command line of
/// an export, the opening of a file, and the home folder.
pub(crate) struct ProgramServices {
    help: HelpPages,
    render: RenderLine,
    home: Option<PathBuf>,
}

impl ProgramServices {
    /// The services for the preview of the document `input`, read with what the executable
    /// carries (`carried`), and built as the arguments `build` of `flm watch` ask.
    pub(crate) fn new(carried: &Carried, build: &BuildArgs, input: &Input) -> ProgramServices {
        ProgramServices {
            help: HelpPages {
                carried: carried.clone(),
                extensions_dirs: build.extensions_dirs.clone(),
                input: input.clone(),
                current: Mutex::new(None),
            },
            render: RenderLine::of(build),
            home: home_folder().ok().flatten().and_then(|home| to_system(&home).ok()),
        }
    }
}

impl Services for ProgramServices {
    fn help(&self) -> Option<&dyn Help> {
        Some(&self.help)
    }

    fn export_command(&self, profile: &str, name: &str, continue_with_errors: bool) -> Option<String> {
        self.render.command(profile, name, continue_with_errors)
    }

    fn open_file(&self, path: &Path) -> Result<(), OpenFileError> {
        crate::opener::open_file(path).map_err(|error| {
            OpenFileError::Failed(format!("Failed to open ‘{}’ with the system's default application: {error}", path.display()))
        })
    }

    fn home_folder(&self) -> Option<PathBuf> {
        self.home.clone()
    }
}

/// What the command line of an export takes from `flm watch`'s arguments: see the [module
/// documentation](self).
struct RenderLine {
    /// The document's file, as typed; `None` for a document that is not a file.
    document: Option<PathBuf>,
    /// The options of `flm watch` that change the build, each word as typed.
    options: Vec<String>,
}

impl RenderLine {
    /// The command line's parts from the arguments `build` of `flm watch`.
    fn of(build: &BuildArgs) -> RenderLine {
        RenderLine { document: build.input.clone(), options: build.build_options() }
    }

    /// The command line of `flm render` that writes the export of the build type `profile`
    /// named `name` beside the document; `None` for a document that is not a file.
    fn command(&self, profile: &str, name: &str, continue_with_errors: bool) -> Option<String> {
        let document = self.document.as_deref()?;
        let output = document.with_file_name(name);
        let mut words: Vec<String> = vec![String::from("flm"), String::from("render"), String::from("-p"), profile.to_owned()];
        if continue_with_errors {
            words.push(String::from("--continue-with-errors"));
        }
        words.extend(self.options.iter().cloned());
        words.extend([String::from("-o"), output.display().to_string(), document.display().to_string(), String::from("--view")]);
        Some(words.iter().map(|word| shell_word(word)).collect::<Vec<_>>().join(" "))
    }
}

/// `word` as a POSIX shell reads it back: as it is when it holds only letters, digits and
/// `_ - . / , : = + @ %`, else between single quotation marks, each one inside written
/// `'\''`.
fn shell_word(word: &str) -> String {
    let plain = |character: char| character.is_ascii_alphanumeric() || "_-./,:=+@%".contains(character);
    if !word.is_empty() && word.chars().all(plain) {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// The help pages of the document's environment: see the [module documentation](self).
pub(crate) struct HelpPages {
    carried: Carried,
    extensions_dirs: Vec<PathBuf>,
    input: Input,
    /// What the pages of the environment last asked about need.
    current: Mutex<Option<Arc<Pages>>>,
}

/// What the pages of one environment need, each part computed when first asked for.
struct Pages {
    /// The digest of the environment's registry.
    digest: u64,
    /// The tree of the pages.
    tree: Tree,
    /// The index, or why it cannot be made.
    index: OnceLock<Result<Arc<[HelpEntry]>, HelpError>>,
    /// The pages rendered so far, by path.
    rendered: Mutex<BTreeMap<String, HelpPage>>,
}

impl HelpPages {
    /// What the pages of `prepared`'s environment need: those kept when they are of that
    /// environment, else made now and kept.
    fn pages(&self, prepared: &Prepared) -> Result<Arc<Pages>, HelpError> {
        let digest = prepared.registry().digest();
        // The lock is held while the tree is made, so that requests that arrive meanwhile
        // wait for it rather than make it again.
        let mut current = lock(&self.current);
        if let Some(pages) = current.as_ref().filter(|pages| pages.digest == digest) {
            return Ok(Arc::clone(pages));
        }
        let env = Environment::of_prepared(prepared);
        let files: Arc<dyn Files> = Arc::new(DiskFiles);
        let program = program_info(&self.extensions_dirs, &env, &self.carried, &self.input, &files)
            .map_err(|detail| HelpError::Failed(format!("Failed to describe the program for the help pages: {detail}")))?;
        let pages = Arc::new(Pages {
            digest,
            tree: Tree::new(&env, Some(program)),
            index: OnceLock::new(),
            rendered: Mutex::new(BTreeMap::new()),
        });
        *current = Some(Arc::clone(&pages));
        Ok(pages)
    }

    /// The index of `pages`, made once.
    fn index_of(&self, pages: &Pages) -> Result<Arc<[HelpEntry]>, HelpError> {
        pages.index.get_or_init(|| make_index(&pages.tree, &self.carried)).clone()
    }
}

impl Help for HelpPages {
    fn index(&self, prepared: &Prepared) -> Result<Arc<[HelpEntry]>, HelpError> {
        let pages = self.pages(prepared)?;
        self.index_of(&pages)
    }

    fn page(&self, prepared: &Prepared, path: &str) -> Result<HelpPage, HelpError> {
        let pages = self.pages(prepared)?;
        if let Some(page) = lock(&pages.rendered).get(path) {
            return Ok(page.clone());
        }
        let Some(page) = pages.tree.page(path) else {
            let index = self.index_of(&pages)?;
            let every: Vec<String> = index.iter().map(|entry| entry.path.clone()).collect();
            let nearest = nearest(&every, path)
                .into_iter()
                .map(|near| {
                    let title = index.iter().find(|entry| entry.path == near).map(|entry| entry.title.clone());
                    (near, title.unwrap_or_default())
                })
                .collect();
            return Err(HelpError::NoPage { path: path.to_string(), nearest });
        };
        let rendered = render(&page, &self.carried)?;
        lock(&pages.rendered).insert(path.to_string(), rendered.clone());
        Ok(rendered)
    }

    fn construct_page(&self, prepared: &Prepared, construct: &Construct) -> Option<String> {
        let feature = construct.feature.as_deref()?;
        let pages = self.pages(prepared).ok()?;
        // The lookup by the name without the backslash finds the commands of every kind; a
        // command that the feature declares outside a scoped package comes first.
        pages.tree.lookup(&construct.name).into_iter().find_map(|page| match &page.body {
            Node::Command(view) if view.kind == construct.kind && view.feature == feature => Some(page.path),
            _ => None,
        })
    }
}

/// Locks `mutex`, whose data stays whole when a thread that held it panicked.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The index of `tree`: every page with its path, its kind, its title, its summary as plain
/// text ([`summary_text`]) and the names it is looked up by.
fn make_index(tree: &Tree, carried: &Carried) -> Result<Arc<[HelpEntry]>, HelpError> {
    let index = tree.index();
    // The summaries are read with the language that renders the pages.
    let root = tree.page("/").ok_or_else(|| HelpError::Failed(String::from("Missing help page ‘/’")))?;
    let prepared = prepare_page(&root, &SourceOptions::default(), "text", &[], None, carried)
        .map_err(|_| HelpError::Failed(String::from("Failed to make the index of the help pages (the problems are printed in the preview's terminal)")))?;
    let lang = prepared.registry().lang();
    Ok(index
        .into_iter()
        .map(|entry| HelpEntry {
            kind: kind_name(entry.kind),
            summary: summary_text(lang, &entry.summary),
            path: entry.path,
            title: entry.title,
            names: entry.names,
        })
        .collect())
}

/// `summary`, a page's summary in FLM, as plain text, its spaces and line ends each made
/// one space; as written when it does not parse.
fn summary_text(lang: &BuiltLang, summary: &str) -> String {
    let source = Arc::new(FlmSource::new(summary.to_string()).with_origin(FlmOrigin::new("summary")));
    let text = match parse_fragment(lang, source, FragmentLayout::Blocks, &ParseOptions::default()) {
        Ok(fragment) if fragment.diags().as_slice().is_empty() => plain_text(fragment.tree()),
        _ => summary.to_string(),
    };
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The name of the kind of page `kind`, as the JSON form of a page writes it (`command`,
/// `settings`, `feature-index`).
fn kind_name(kind: PageKind) -> String {
    serde_json::to_value(kind).ok().and_then(|value| value.as_str().map(str::to_string)).unwrap_or_default()
}

/// `page` rendered for the preview ([`info::render`] with [`PROFILE`] and [`SETTINGS`]).
///
/// A feature's page and the page of a feature's settings name that feature
/// ([`HelpPage::settings_feature`]); every other page names none.
fn render(page: &Page, carried: &Carried) -> Result<HelpPage, HelpError> {
    let settings: Vec<CommandLineSetting> =
        SETTINGS.iter().map(|setting| CommandLineSetting::new(*setting, "the preview's help pages")).collect();
    let content = info::render(page, PROFILE, &settings, carried).map_err(|_| {
        HelpError::Failed(format!(
            "Failed to render the help page ‘{}’ (the problems are printed in the preview's terminal)",
            page.path
        ))
    })?;
    let settings_feature = match &page.body {
        Node::Feature(view) => Some(view.name.clone()),
        Node::Settings(view) => Some(view.feature.clone()),
        _ => None,
    };
    let path_line =
        page.path_line().into_iter().map(|segment| HelpSegment { text: segment.text, path: segment.path }).collect();
    Ok(HelpPage {
        path: page.path.clone(),
        kind: kind_name(page.kind),
        title: page.title.clone(),
        content,
        settings_feature,
        path_line,
    })
}
