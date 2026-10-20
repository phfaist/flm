//! The engine of the command `flm dump`: it processes one document's text with the library
//! [`flm_core`] alone and gives, as JSON, one view of what the run holds: a document tree
//! at a chosen point of the processing, a computed table, or the artifacts of an output.
//!
//! It serves a reader who wants to see what the library makes of a document: the tree the
//! parser produced, the tree after the rewrite stages, the presentation tree a backend
//! built, or the data a stage computed. It reads no front matter and no settings: the
//! document is the text given, and the registry is built from the library's standard
//! package under its default configuration. The command `flm dump` of the `flm` program is
//! the program's view of this engine: the program parses its command line into a
//! [`Request`], reads the file, calls [`dump`] and prints what the engine gives it.
//!
//! # What the engine does
//!
//! In order, [`dump`]:
//!
//! 1. builds a [registry](flm_core::core::registry), the frozen record of everything the
//!    language and the pipeline consult for a set of features, from the standard package
//!    ([`flm_core::stdcore::package`]) under its default configuration and the package
//!    `math-passthrough` ([`flm_core::math_passthrough::package`]), which processes the
//!    mathematics;
//! 2. parses the text into a [`Fragment`], its origin naming the file, and places that one
//!    fragment in a [`Doc`];
//! 3. unless only the parsed tree is asked for, builds a [`Plan`] for the request's
//!    workflow and executes one [`Run`] of it;
//! 4. gives one view of what that run holds ([`Subject`], [`View`]).
//!
//! Everything it gives goes through the caller's function, in order, as a [`Printed`]
//! value: the diagnostics of the registry build, of the parse and of the run, each time as
//! one text ([`Printed::Diagnostics`]), and the view once, as one text of JSON
//! ([`Printed::View`]). Diagnostics do not decide the outcome: a document that reports
//! errors still gives its view.
//!
//! # Outcome
//!
//! [`dump`] returns `Ok` once the view is given. It returns an [`Error`] with the status 1
//! when a registry, plan or run cannot be built or fails, when a name matches nothing (an
//! unknown table, output or backend), or when the chosen workflow does not write the table
//! or output asked for; and with the status 2 when the tree asked for does not exist (the
//! parse produced none, for instance for content nested too deeply; a rewrite stage
//! produced none; the chosen workflow did not lower the tree for the backend asked for).
//! The `flm` program uses the status as its exit status. The messages of the errors refer
//! to the options of `flm dump` by which a person chooses the subject and the workflow.

use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::project::{ProjectOptions, Projector, project_table, project_tree};
use flm_core::dump::session::{write_fragment, write_tree, writer};
use flm_core::dump::{segment_line, to_json_pretty};
use flm_core::lang::{FlmOrigin, ParseOptions, guard, parse_fragment};
use flm_core::model::addr::TreeId;
use flm_core::model::doc::{Doc, DocContent, Fragment, FragmentLayout};
use flm_core::model::names::BackendName;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::lowering::lowered_table;
use flm_core::stdcore::backends::text::{self, TextRenderOptions};
use flm_core::stdcore::{StdcoreConfig, package};
use flm_core::techy::core::node::NodeTree;
use flm_core::techy::source::Source;

/// The point of the processing at which the engine gives the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum At {
    /// The tree exactly as the parser produced it. No workflow runs for it.
    Parsed,
    /// The tree after the plan's rewrite stages: the frozen tree, which every later
    /// stage of the run reads.
    Frozen,
    /// The tree that the named backend built from the frozen tree for its output: the
    /// same content rewritten into elements that the backend writes natively, such as the
    /// presentation elements of [`flm_core::stdcore::backends::pres`]. The string is the
    /// backend's name.
    Lowered(String),
}

/// How a tree is given. Tables and outputs are always given as projections, so this
/// applies to trees only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// The projection, a one-way, readable view of the tree in the library's own
    /// vocabulary, as indented JSON. See [`flm_core::dump::project`].
    Compact,
    /// techy's serialization of the tree, as one JSON line: the lossless form
    /// that reads back into live objects. See [`flm_core::dump::session`].
    Wire,
}

/// What the engine gives: a tree, a table or an output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// A tree, at the given point of the processing.
    Tree(At),
    /// The computed table of this name, given under its name.
    Table(String),
    /// The artifact set of the output of this name, given under the name of the table
    /// that holds it. An artifact's bytes are given in the JSON form that techy gives
    /// binary data: the object `{"$bytes": "<base64>"}`.
    Output(String),
}

/// What one call of [`dump`] gives, and the run it makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// What to give.
    pub subject: Subject,
    /// How to give a tree.
    pub view: View,
    /// The name of the workflow to plan and run; unused when the subject is the parsed
    /// tree, which runs no workflow.
    pub workflow: String,
    /// The text backend's line width in columns ([`TextRenderOptions`]), when given; the
    /// backend's default (80) otherwise.
    pub width: Option<u32>,
}

impl Request {
    /// The request for `subject`, given in the compact view, under the text backend's
    /// workflow ([`text::WORKFLOW`]) at the default width.
    pub fn new(subject: Subject) -> Request {
        Request { subject, view: View::Compact, workflow: String::from(text::WORKFLOW), width: None }
    }
}

/// A failure that ends a call of [`dump`]: its message, and the status a program exits
/// with (see the [module documentation](self#outcome)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// The message, which states the condition that is not met.
    pub message: String,
    /// 2 for a tree that does not exist, 1 for every other failure.
    pub status: u8,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// Returns an [`Error`] with `message` and the status 1, the status of every failure
/// except a tree that does not exist.
fn failure(message: impl Into<String>) -> Error {
    Error { message: message.into(), status: 1 }
}

/// One text that [`dump`] gives its caller to print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Printed<'a> {
    /// The rendered diagnostics of one step (the registry build, the parse, the run), for
    /// the standard error stream.
    Diagnostics(&'a str),
    /// The view asked for, as JSON, for the standard output stream.
    View(&'a str),
}

/// Processes `text`, the content of the file named `origin`, as `request` asks, and gives
/// each diagnostics text and the view to `print`, in order (see the [module
/// documentation](self)).
///
/// The work runs on a worker thread, a thread with the large stack that [`flm_core`]
/// requires for its guarded descents over trees ([`guard::on_worker_thread`]). Every view
/// is a descent over a tree, performed once the parse and the run have finished. The wire
/// view's serialization is a guarded descent under a stack budget, and that budget limits
/// how much stack is used rather than measuring how much exists: on a thread with a smaller
/// stack, it overflows instead of refusing. The compact view's projection has no limit of
/// its own at all, so the stack it runs on is its only limit. Creating the worker thread
/// once, here, covers both, and the parse needs no thread of its own: on a thread already
/// marked as a worker, the parse runs in place. `print` is called on that thread.
///
/// # Errors
///
/// The [`Error`] of the [module documentation](self#outcome); also one with status 1 when
/// no worker thread can be created.
pub fn dump(request: &Request, text: String, origin: &str, print: &mut (dyn FnMut(Printed<'_>) + Send)) -> Result<(), Error> {
    match guard::on_worker_thread(|| dump_on_worker_thread(request, text, origin, print)) {
        Ok(result) => result,
        Err(error) => Err(failure(format!(
            "`flm dump` needs a thread with a large stack, and the operating system refused to \
             create one: {error}"
        ))),
    }
}

/// Builds the registry that the engine works from: the standard package ([`package`])
/// under its default configuration ([`StdcoreConfig::new`]) and `math-passthrough`, and
/// nothing else, since the engine reads no settings. The build's diagnostics go to `print`;
/// a registry built from the whole standard package reports none.
///
/// # Errors
///
/// An [`Error`] with status 1 when the registry cannot be built, carrying the build
/// error's text.
fn build_registry(print: &mut (dyn FnMut(Printed<'_>) + Send)) -> Result<Arc<FlmRegistry>, Error> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    let registry = builder
        .build(&RegistryInputs::new())
        .map_err(|error| failure(format!("the registry cannot be built: {}", error.render())))?;
    if !registry.build_diagnostics().is_empty() {
        print(Printed::Diagnostics(&registry.build_diagnostics().render_all()));
    }
    Ok(registry)
}

/// Parses `text` into a [`Fragment`], recording `origin` (the file's path) as the
/// fragment's origin so that diagnostics give it. The parse's own diagnostics go to
/// `print`; a document that reports errors still yields a fragment.
///
/// # Errors
///
/// An [`Error`] with status 2 when the parse yields no tree at all, for instance for
/// content nested more deeply than the recursion guard allows.
fn parse_file(
    registry: &FlmRegistry,
    text: String,
    origin: &str,
    print: &mut (dyn FnMut(Printed<'_>) + Send),
) -> Result<Arc<Fragment>, Error> {
    let source = Arc::new(Source::new(text).with_origin(FlmOrigin::new(origin)));
    let fragment =
        parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .map_err(|error| Error { message: error.diagnostic().render(), status: 2 })?;
    if !fragment.diags().is_empty() {
        print(Printed::Diagnostics(&fragment.diags().render_all()));
    }
    Ok(Arc::new(fragment))
}

/// Builds the plan for `workflow` and executes one [`Run`] of it over a document whose
/// only content is `fragment`. A `width`, when given, is set as the text backend's render
/// option ([`TextRenderOptions`]) through the plan's [`Overrides`]. The run's diagnostics
/// go to `print`.
///
/// # Errors
///
/// An [`Error`] with status 1 when no plan can be built for the workflow (an unknown
/// workflow name among the causes), when the run cannot be constructed, or when the run
/// itself fails.
fn run_workflow(
    registry: &Arc<FlmRegistry>,
    fragment: Arc<Fragment>,
    workflow: &str,
    width: Option<u32>,
    print: &mut (dyn FnMut(Printed<'_>) + Send),
) -> Result<Run, Error> {
    let mut content = DocContent::new();
    content.place(fragment);
    let doc = Arc::new(Doc::new(content));
    let mut overrides = Overrides::none();
    if let Some(width) = width {
        overrides.render_options =
            OptionValues::new().with(TextRenderOptions { width: Some(width), ..TextRenderOptions::default() });
    }
    let plan = Plan::for_workflow(registry, workflow, &overrides)
        .map_err(|error| failure(format!("the workflow `{workflow}` cannot be run: {error}")))?;
    let mut run = Run::new(doc, Arc::new(plan), Externals::none())
        .map_err(|error| failure(format!("the run of the workflow `{workflow}` could not be made: {error}")))?;
    run.execute(&RunOptions::new()).map_err(|error| failure(format!("the run of the workflow `{workflow}` failed: {error}")))?;
    if !run.pipeline_diagnostics().is_empty() {
        print(Printed::Diagnostics(&run.pipeline_diagnostics().render_all()));
    }
    Ok(run)
}

/// Gives `tree` to `print` in the requested view.
///
/// # Errors
///
/// An [`Error`] with status 1 when the tree cannot be written into a serialization
/// session, or when the resulting value cannot be rendered as JSON.
fn print_tree(
    registry: &FlmRegistry,
    tree: &Arc<NodeTree<flm_core::lang::Flm>>,
    view: View,
    print: &mut (dyn FnMut(Printed<'_>) + Send),
) -> Result<(), Error> {
    match view {
        View::Compact => {
            let value = project_tree(tree, &ProjectOptions::new());
            print(Printed::View(&to_json_pretty(&value).map_err(|error| failure(error.to_string()))?));
        }
        View::Wire => {
            let mut session = writer(registry.lang());
            write_tree(&mut session, tree).map_err(|error| failure(error.to_string()))?;
            let segment = session.take_segment();
            print(Printed::View(&segment_line(&segment).map_err(|error| failure(error.to_string()))?));
        }
    }
    Ok(())
}

/// Gives the fragment as parsed. The compact view is the projection of the fragment's
/// tree. The wire view writes the fragment itself and records it as the segment's main
/// entry, so that reading the segment back yields the fragment and not a bare tree.
///
/// # Errors
///
/// An [`Error`] with status 1 when the fragment cannot be written into a serialization
/// session, or when the resulting value cannot be rendered as JSON.
fn print_parsed(
    registry: &FlmRegistry,
    fragment: &Arc<Fragment>,
    view: View,
    print: &mut (dyn FnMut(Printed<'_>) + Send),
) -> Result<(), Error> {
    match view {
        View::Compact => print_tree(registry, fragment.tree(), view, print),
        View::Wire => {
            let mut session = writer(registry.lang());
            let position = write_fragment(&mut session, fragment).map_err(|error| failure(error.to_string()))?;
            let segment = session.take_segment_with_main(position).map_err(|error| failure(error.to_string()))?;
            print(Printed::View(&segment_line(&segment).map_err(|error| failure(error.to_string()))?));
            Ok(())
        }
    }
}

/// Gives the run's table `name`, under that name, as a projection. Each table type
/// projects itself, so tables are given in shapes of their own: an artifact set as its
/// artifacts and its violations, a table keyed by node as node references, and so on.
///
/// # Errors
///
/// An [`Error`] with status 1 when the registry declares no table of that name (the
/// message then lists the tables it declares), when the run of `workflow` wrote no value
/// for that table (a table is written only by a plan that schedules its stage), or when
/// the projection cannot be written as JSON.
fn print_table(run: &Run, workflow: &str, name: &str, print: &mut (dyn FnMut(Printed<'_>) + Send)) -> Result<(), Error> {
    let registry = run.plan().registry();
    let id = registry.table_id(name).ok_or_else(|| {
        let known: Vec<&str> = registry.tables().iter().map(|t| t.def.name.as_str()).collect();
        failure(format!("the table `{name}` must be one of the tables: `{}`", known.join("`, `")))
    })?;
    let value = run.computed().get_erased(id).ok_or_else(|| {
        failure(format!(
            "the workflow `{workflow}` must write the table `{name}`, and it does not: choose, \
             with `--workflow`, a workflow that writes it"
        ))
    })?;
    let projector = Projector::new(ProjectOptions::new());
    let table_type = &registry.table(id).def.ty;
    let projected = table_type.project(value.as_ref(), &projector.table_projection(run));
    let value = project_table(name, projected);
    print(Printed::View(&to_json_pretty(&value).map_err(|error| failure(error.to_string()))?));
    Ok(())
}

/// The engine itself, on a worker thread: builds the registry, parses the text, runs a
/// workflow if the subject needs one, and gives the requested view.
///
/// # Errors
///
/// An [`Error`] with status 1 for the failures of the steps it calls and for a backend
/// that the registry does not declare; with status 2 when the tree asked for does not
/// exist, because a rewrite stage produced none or the tree was not lowered for the
/// backend asked for.
fn dump_on_worker_thread(
    request: &Request,
    text: String,
    origin: &str,
    print: &mut (dyn FnMut(Printed<'_>) + Send),
) -> Result<(), Error> {
    let registry = build_registry(print)?;
    let fragment = parse_file(&registry, text, origin, print)?;
    let first = TreeId::tree(0);
    match &request.subject {
        Subject::Tree(At::Parsed) => print_parsed(&registry, &fragment, request.view, print),
        Subject::Tree(At::Frozen) => {
            let run = run_workflow(&registry, fragment, &request.workflow, request.width, print)?;
            let tree = run.frozen_tree(first).ok_or_else(|| Error {
                message: format!(
                    "`--at frozen` needs the document's frozen tree, and no stage of the workflow `{}` \
                     produced one",
                    request.workflow
                ),
                status: 2,
            })?;
            print_tree(&registry, tree, request.view, print)
        }
        Subject::Tree(At::Lowered(backend)) => {
            let run = run_workflow(&registry, fragment, &request.workflow, request.width, print)?;
            let table = lowered_table(&BackendName::from(backend.as_str()));
            // An undeclared lowered table means the registry holds no such
            // backend. What the message names back is the set of backends this
            // plan actually lowers for, which is what the reader is looking
            // for, rather than everything the registry declares.
            // See design rationale at [§flm-dd-dr:pipe-active-backends].
            let Ok(trees) = run.table(table) else {
                let active: Vec<String> = run.plan().active_backends().iter().map(|b| b.to_string()).collect();
                return Err(failure(format!(
                    "the backend `{backend}` of `--at lowered:{backend}` must be one of the backends that the workflow `{}` uses: `{}`",
                    request.workflow,
                    active.join("`, `")
                )));
            };
            let lowered = trees.and_then(|trees| trees.get(first)).ok_or_else(|| Error {
                message: format!(
                    "the workflow `{}` must lower the document's tree for the backend `{backend}`, \
                     and it does not: choose, with `--workflow`, a workflow that lowers it for \
                     the backend `{backend}`",
                    request.workflow
                ),
                status: 2,
            })?;
            print_tree(&registry, &lowered.tree, request.view, print)
        }
        Subject::Table(name) => {
            let run = run_workflow(&registry, fragment, &request.workflow, request.width, print)?;
            print_table(&run, &request.workflow, name, print)
        }
        Subject::Output(name) => {
            let entry = registry.output(name).ok_or_else(|| {
                let known: Vec<String> = registry.outputs().map(|o| o.def.name.to_string()).collect();
                failure(format!("the output `{name}` must be one of the outputs: `{}`", known.join("`, `")))
            })?;
            let table = registry.table(entry.table).def.name.to_string();
            let run = run_workflow(&registry, fragment, &request.workflow, request.width, print)?;
            print_table(&run, &request.workflow, &table, print)
        }
    }
}
