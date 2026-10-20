//! The making of each page: its body, its children, its title and its summary.

use flm_core::lang::mint::CallableKind;

use super::paths::{self, Location};
use super::source::code_span;
use super::texts::{self, TextSource};
use super::{
    CommandPageView, Entry, FeatureEntryView, Node, Page, PageKind, ProgramView, RootView, Tree,
    WorkflowEntryView,
};
use crate::env::{CommandView, CommandsView, Hit};
use crate::summary_of;

/// The title and summary of each section's index page, by the section's path.
const SECTIONS: [(&str, &str, &str); 6] = [
    (
        paths::PACKAGES,
        "Packages",
        "The packages of the environment: named bundles of features that a document or the program chooses to use.",
    ),
    (paths::FEATURES, "Features", "The features of the environment, in the order the registry holds them."),
    (
        paths::SETTINGS,
        "Settings",
        "The settings keys of each feature, the parsing settings, and the keys of a front matter.",
    ),
    (
        paths::WORKFLOWS,
        "Workflows",
        "The workflows of the environment: what a build produces. Each workflow is also the profile of the same name.",
    ),
    (paths::NAMESPACES, "Namespaces", "The families of names the registry holds, each with every name it holds."),
    (paths::PROGRAM, "The program", "The program's commands with their options, and the environment variables it reads."),
];

const ROOT_TITLE: &str = "The FLM environment";
const ROOT_SUMMARY: &str =
    "The packages, features, settings and workflows of the environment, and the names its registry holds.";
const PARSING_TITLE: &str = "Parsing settings";
const PARSING_SUMMARY: &str = "The settings that decide how a document's source is read, as the registry was built with them.";
#[cfg(feature = "flm-main")]
const FRONT_MATTER_TITLE: &str = "Front matter";
#[cfg(feature = "flm-main")]
const FRONT_MATTER_SUMMARY: &str = "The keys that a document's front matter takes under `flm`.";
const PATHS_TITLE: &str = "The program's folders";
const PATHS_SUMMARY: &str =
    "The folders the program reads: the search path of extensions, the folder of the citation cache, the image folders.";
const EXTENSIONS_TITLE: &str = "Packages and extensions available";
const EXTENSIONS_SUMMARY: &str = "The packages and extensions a document may use: those compiled into the program, those found \
                                  on the search path, and those bundled into the program.";

/// The kinds of commands, in the order of their pages.
const KINDS: [CallableKind; 3] = [CallableKind::Macro, CallableKind::Environment, CallableKind::Specials];

/// The page at `location`, or `None` when it does not exist.
pub(super) fn page(tree: &Tree, location: &Location) -> Option<Page> {
    let env = tree.environment();
    let registry = env.registry();
    let page = match location {
        Location::Root => {
            let mut children: Vec<Entry> =
                SECTIONS[..5].iter().map(|(path, title, summary)| entry(path, title, summary)).collect();
            if tree.program().is_some() {
                let (path, title, summary) = SECTIONS[5];
                children.push(entry(path, title, summary));
            }
            let body = RootView {
                workflow: env.plan().and_then(|plan| plan.request().workflow.as_ref().map(ToString::to_string)),
                #[cfg(feature = "flm-main")]
                profile: env.host().map(|host| host.profile.clone()),
                #[cfg(feature = "flm-main")]
                math: env.host().and_then(|host| host.math.clone()),
                packages: env.packages(),
                features: registry
                    .features()
                    .iter()
                    .map(|info| FeatureEntryView {
                        name: info.name.to_string(),
                        title: info.title.clone(),
                        summary: summary_of(&info.docs).to_string(),
                    })
                    .collect(),
                workflows: registry
                    .workflows()
                    .map(|workflow| WorkflowEntryView {
                        name: workflow.def.name.to_string(),
                        outputs: registry
                            .combined_workflow(&workflow.def.name)
                            .map(|combined| combined.outputs.iter().map(ToString::to_string).collect())
                            .unwrap_or_default(),
                        summary: summary_of(&workflow.def.docs).to_string(),
                    })
                    .collect(),
            };
            make(tree, paths::ROOT, PageKind::Root, ROOT_TITLE, ROOT_SUMMARY, Node::Root(body), children)
        }
        Location::PackageIndex => {
            let children = env
                .packages()
                .iter()
                .map(|package| entry(&paths::package(&package.name), &package_title(&package.name), &package_summary(&package.features)))
                .collect();
            section(tree, PageKind::PackageIndex, 0, children)
        }
        Location::Package(name) => {
            let package = env.packages().into_iter().find(|package| package.name == *name)?;
            let summary = package_summary(&package.features);
            make(tree, &paths::package(name), PageKind::Package, &package_title(name), &summary, Node::Package(package), Vec::new())
        }
        Location::FeatureIndex => {
            let children = registry
                .features()
                .iter()
                .map(|info| {
                    let source = texts::declared(env, Some(&info.name), &info.docs);
                    entry_from(&paths::feature(&info.name), &feature_title(&info.name), summary_of(&info.docs), source)
                })
                .collect();
            section(tree, PageKind::FeatureIndex, 1, children)
        }
        Location::Feature(name) => {
            let feature = env.feature(name)?;
            let mut children = kind_entries(name, None, &env.commands(name));
            let packages = env.scoped_packages(name);
            if !packages.is_empty() {
                children.push(entry(&paths::scoped_packages(name), &scoped_packages_title(name), &scoped_packages_summary(name)));
            }
            let summary = feature.summary.clone();
            make(tree, &paths::feature(name), PageKind::Feature, &feature_title(name), &summary, Node::Feature(feature), children)
        }
        Location::CommandIndex { feature, within, kind } => {
            let commands = commands_of(tree, feature, within.as_deref())?;
            let children: Vec<Entry> = names_of(group(&commands, *kind))
                .into_iter()
                .map(|(name, view)| {
                    let source = texts::declared(env, Some(&view.feature), &view.docs);
                    let path = paths::command(feature, within.as_deref(), *kind, &name);
                    entry_from(&path, &command_title(*kind, &name), &view.summary, source)
                })
                .collect();
            if children.is_empty() {
                return None;
            }
            let (title, summary) = kind_index_text(feature, within.as_deref(), *kind);
            make(tree, &paths::kind_index(feature, within.as_deref(), *kind), PageKind::CommandIndex, &title, &summary, Node::Index, children)
        }
        Location::Command { feature, within, kind, name } => {
            let commands = commands_of(tree, feature, within.as_deref())?;
            let declarations: Vec<CommandView> =
                group(&commands, *kind).iter().filter(|view| writes(view, name)).cloned().collect();
            let summary = declarations.last()?.summary.clone();
            let body = CommandPageView {
                kind: *kind,
                name: name.clone(),
                feature: feature.clone(),
                within: within.clone(),
                declarations,
            };
            make(
                tree,
                &paths::command(feature, within.as_deref(), *kind, name),
                PageKind::Command,
                &command_title(*kind, name),
                &summary,
                Node::Command(body),
                Vec::new(),
            )
        }
        Location::ScopedPackageIndex(feature) => {
            let children: Vec<Entry> = env
                .scoped_packages(feature)
                .iter()
                .map(|package| {
                    let source = texts::declared(env, Some(&package.owner), &package.docs);
                    let path = paths::scoped_package(&package.owner, &package.name);
                    entry_from(&path, &scoped_package_title(&package.name), &package.summary, source)
                })
                .collect();
            if children.is_empty() {
                return None;
            }
            make(
                tree,
                &paths::scoped_packages(feature),
                PageKind::ScopedPackageIndex,
                &scoped_packages_title(feature),
                &scoped_packages_summary(feature),
                Node::Index,
                children,
            )
        }
        Location::ScopedPackage { feature, name } => {
            let package = env.scoped_package(name).filter(|package| package.owner == *feature)?;
            let children = kind_entries(feature, Some(name), &package.commands);
            let summary = package.summary.clone();
            make(
                tree,
                &paths::scoped_package(feature, name),
                PageKind::ScopedPackage,
                &scoped_package_title(name),
                &summary,
                Node::ScopedPackage(package),
                children,
            )
        }
        Location::SettingsIndex => {
            #[allow(unused_mut)]
            let mut children = vec![entry(paths::SETTINGS_PARSING, PARSING_TITLE, PARSING_SUMMARY)];
            #[cfg(feature = "flm-main")]
            children.push(entry(paths::SETTINGS_FRONT_MATTER, FRONT_MATTER_TITLE, FRONT_MATTER_SUMMARY));
            // Every feature of the packages that takes settings, those the registry does not
            // hold included: the page of a feature switched off says where it was. A feature
            // named `parsing` or `front-matter` has no settings page of its own (its path is a
            // fixed page's), in both builds.
            for factory in env.factories() {
                for feature in factory.features().iter().filter(|feature| feature.settings.is_some()) {
                    let path = paths::settings(&feature.name);
                    if path == paths::SETTINGS_PARSING || path == paths::SETTINGS_FRONT_MATTER {
                        continue;
                    }
                    if !children.iter().any(|known| known.path == path) {
                        children.push(entry(&path, &settings_title(&feature.name), &settings_summary(&feature.name)));
                    }
                }
            }
            section(tree, PageKind::SettingsIndex, 2, children)
        }
        Location::Settings(feature) => {
            let settings = env.settings(feature)?;
            make(
                tree,
                &paths::settings(feature),
                PageKind::Settings,
                &settings_title(feature),
                &settings_summary(feature),
                Node::Settings(settings),
                Vec::new(),
            )
        }
        Location::Parsing => make(
            tree,
            paths::SETTINGS_PARSING,
            PageKind::ParsingSettings,
            PARSING_TITLE,
            PARSING_SUMMARY,
            Node::Parsing(env.parsing_settings()),
            Vec::new(),
        ),
        #[cfg(feature = "flm-main")]
        Location::FrontMatter => make(
            tree,
            paths::SETTINGS_FRONT_MATTER,
            PageKind::FrontMatter,
            FRONT_MATTER_TITLE,
            FRONT_MATTER_SUMMARY,
            Node::FrontMatter(env.front_matter()),
            Vec::new(),
        ),
        #[cfg(not(feature = "flm-main"))]
        Location::FrontMatter => return None,
        Location::WorkflowIndex => {
            let children = registry
                .workflows()
                .map(|workflow| {
                    let name = &workflow.def.name;
                    let source = texts::workflow(env, name, &workflow.def.docs);
                    entry_from(&paths::workflow(name), &workflow_title(name), summary_of(&workflow.def.docs), source)
                })
                .collect();
            section(tree, PageKind::WorkflowIndex, 3, children)
        }
        Location::Workflow(name) => {
            let workflow = env.workflow(name)?;
            let summary = workflow.summary.clone();
            make(tree, &paths::workflow(name), PageKind::Workflow, &workflow_title(name), &summary, Node::Workflow(workflow), Vec::new())
        }
        Location::NamespaceIndex => {
            let children = env
                .namespaces()
                .iter()
                .map(|namespace| entry(&paths::namespace(&namespace.name), &namespace_title(&namespace.name), &namespace.what))
                .collect();
            section(tree, PageKind::NamespaceIndex, 4, children)
        }
        Location::Namespace(name) => {
            let namespace = env.namespace(name)?;
            let mut children: Vec<Entry> = Vec::new();
            if paths::has_entity_pages(name) {
                for entry_view in &namespace.entries {
                    let path = paths::entity(name, &entry_view.name, entry_view.owner.as_deref());
                    let source = texts::declared(env, entry_view.owner.as_deref(), &entry_view.summary);
                    match children.iter_mut().find(|known| known.path == path) {
                        Some(known) => {
                            if known.summary.is_empty() {
                                known.summary = entry_view.summary.clone();
                                known.source = source;
                            }
                        }
                        None => children.push(entry_from(&path, &entity_title(name, &entry_view.name), &entry_view.summary, source)),
                    }
                }
            }
            let summary = namespace.what.clone();
            make(tree, &paths::namespace(name), PageKind::Namespace, &namespace_title(name), &summary, Node::Namespace(namespace), children)
        }
        Location::Entity { namespace, name } => {
            if !paths::has_entity_pages(namespace) {
                return None;
            }
            let entity = env.entity(namespace, name)?;
            let summary = entity
                .declarations
                .iter()
                .map(|declaration| declaration.summary.as_str())
                .find(|summary| !summary.is_empty())
                .unwrap_or_default()
                .to_string();
            make(
                tree,
                &paths::entity(namespace, name, None),
                PageKind::Entity,
                &entity_title(namespace, name),
                &summary,
                Node::Entity(entity),
                Vec::new(),
            )
        }
        Location::Program => {
            let program = tree.program()?;
            let children = vec![
                entry(paths::PROGRAM_PATHS, PATHS_TITLE, PATHS_SUMMARY),
                entry(paths::PROGRAM_EXTENSIONS, EXTENSIONS_TITLE, EXTENSIONS_SUMMARY),
            ];
            let body = ProgramView {
                version: program.version.clone(),
                commands: program.commands.clone(),
                environment_variables: program.environment_variables.clone(),
            };
            section(tree, PageKind::Program, 5, children).with_body(Node::Program(body))
        }
        Location::ProgramPaths => {
            let program = tree.program()?;
            make(
                tree,
                paths::PROGRAM_PATHS,
                PageKind::ProgramPaths,
                PATHS_TITLE,
                PATHS_SUMMARY,
                Node::ProgramPaths(program.paths.clone()),
                Vec::new(),
            )
        }
        Location::ProgramExtensions => {
            let program = tree.program()?;
            make(
                tree,
                paths::PROGRAM_EXTENSIONS,
                PageKind::ProgramExtensions,
                EXTENSIONS_TITLE,
                EXTENSIONS_SUMMARY,
                Node::ProgramExtensions(program.extensions.clone()),
                Vec::new(),
            )
        }
    };
    Some(page)
}

/// The pages of every entity and command called `name` ([`Tree::lookup`]).
pub(super) fn lookup(tree: &Tree, name: &str) -> Vec<Page> {
    let env = tree.environment();
    let features = env.registry().features();
    let feature_rank = |feature: &str| features.iter().position(|info| info.name == *feature).unwrap_or(usize::MAX);
    let mut found: Vec<((usize, usize, usize), String)> = Vec::new();
    for hit in env.lookup(name) {
        let (key, path) = match hit {
            Hit::Entity { namespace, name, owner, .. } => {
                let key = match namespace.as_str() {
                    "package" => (0, 0, 0),
                    "feature" => (1, feature_rank(&name), 0),
                    "scoped-package" => (1, owner.as_deref().map_or(usize::MAX, feature_rank), 4),
                    "settings-key" => (2, 0, 0),
                    "workflow" => (3, 0, 0),
                    _ => (4, 0, 0),
                };
                (key, paths::entity(&namespace, &name, owner.as_deref()))
            }
            Hit::Command { kind, name, feature, within, .. } => {
                let rank = match within {
                    Some(_) => 4,
                    None => 1 + KINDS.iter().position(|known| *known == kind).unwrap_or(0),
                };
                ((1, feature_rank(&feature), rank), paths::command(&feature, within.as_deref(), kind, &name))
            }
        };
        if !found.iter().any(|(_, known)| *known == path) {
            found.push((key, path));
        }
    }
    found.sort_by_key(|(key, _)| *key);
    found.into_iter().filter_map(|(_, path)| tree.page(&path)).collect()
}

impl Page {
    fn with_body(mut self, body: Node) -> Page {
        self.body = body;
        self
    }
}

fn make(tree: &Tree, path: &str, kind: PageKind, title: &str, summary: &str, body: Node, children: Vec<Entry>) -> Page {
    Page {
        path: path.to_string(),
        kind,
        title: title.to_string(),
        summary: summary.to_string(),
        body,
        children,
        tree: tree.clone(),
    }
}

/// The index page of the section number `index` of [`SECTIONS`].
fn section(tree: &Tree, kind: PageKind, index: usize, children: Vec<Entry>) -> Page {
    let (path, title, summary) = SECTIONS[index];
    make(tree, path, kind, title, summary, Node::Index, children)
}

/// The entry of the page `path`, whose summary is the pages' own text.
fn entry(path: &str, title: &str, summary: &str) -> Entry {
    entry_from(path, title, summary, TextSource::Flm)
}

/// The entry of the page `path`, whose summary comes from `source`.
fn entry_from(path: &str, title: &str, summary: &str, source: TextSource) -> Entry {
    Entry { path: path.to_string(), title: title.to_string(), summary: summary.to_string(), source }
}

/// The commands of the feature `feature`, or of its scoped package `within`; `None` when
/// the feature does not exist, or the scoped package does not exist or belongs to another
/// feature.
fn commands_of(tree: &Tree, feature: &str, within: Option<&str>) -> Option<CommandsView> {
    let env = tree.environment();
    match within {
        None => env.registry().feature(feature).map(|_| env.commands(feature)),
        Some(name) => env.scoped_package(name).filter(|package| package.owner == *feature).map(|package| package.commands),
    }
}

fn group(commands: &CommandsView, kind: CallableKind) -> &[CommandView] {
    match kind {
        CallableKind::Macro => &commands.macros,
        CallableKind::Environment => &commands.environments,
        CallableKind::Specials => &commands.specials,
    }
}

/// Whether the declaration `view` is written as `name`: one of its names, or its trigger.
fn writes(view: &CommandView, name: &str) -> bool {
    match &view.trigger {
        Some(trigger) => trigger == name,
        None => view.names.iter().any(|written| written == name),
    }
}

/// Every name the declarations `views` are written under, each once, in order, with the
/// last declaration written under it (whose summary the name's entry shows).
fn names_of(views: &[CommandView]) -> Vec<(String, &CommandView)> {
    let mut out: Vec<(String, &CommandView)> = Vec::new();
    for view in views {
        let names: Vec<&String> = match &view.trigger {
            Some(trigger) => vec![trigger],
            None => view.names.iter().collect(),
        };
        for name in names {
            match out.iter_mut().find(|(known, _)| known == name) {
                Some((_, last)) => *last = view,
                None => out.push((name.clone(), view)),
            }
        }
    }
    out
}

/// The entries of the index pages of the kinds of commands that `commands` holds, for the
/// feature `feature` or its scoped package `within`.
fn kind_entries(feature: &str, within: Option<&str>, commands: &CommandsView) -> Vec<Entry> {
    KINDS
        .iter()
        .filter(|kind| !group(commands, **kind).is_empty())
        .map(|kind| {
            let (title, summary) = kind_index_text(feature, within, *kind);
            entry(&paths::kind_index(feature, within, *kind), &title, &summary)
        })
        .collect()
}

/// The title and summary of the index page of the commands of `kind` of the feature
/// `feature`, or of its scoped package `within`.
fn kind_index_text(feature: &str, within: Option<&str>, kind: CallableKind) -> (String, String) {
    let (word, words) = match kind {
        CallableKind::Macro => ("Macros", "macros"),
        CallableKind::Environment => ("Environments", "environments"),
        CallableKind::Specials => ("Special character sequences", "special character sequences"),
    };
    match within {
        None => (
            format!("{word} of feature {feature}"),
            format!("The {words} that the feature {} declares.", code_span(feature)),
        ),
        Some(package) => (
            format!("{word} of scoped package {package}"),
            format!("The {words} defined inside the scoped package {}.", code_span(package)),
        ),
    }
}

fn command_title(kind: CallableKind, name: &str) -> String {
    match kind {
        CallableKind::Macro => format!("Macro \\{name}"),
        CallableKind::Environment => format!("Environment {name}"),
        CallableKind::Specials => format!("Special character sequence {name}"),
    }
}

fn package_title(name: &str) -> String {
    format!("Package {name}")
}

/// The summary of a package's page: how many features it holds.
fn package_summary(features: &[String]) -> String {
    match features {
        [] => String::from("A package holding no feature."),
        [feature] => format!("A package holding the feature {}.", code_span(feature)),
        _ => format!("A package holding {} features.", features.len()),
    }
}

fn feature_title(name: &str) -> String {
    format!("Feature {name}")
}

fn scoped_packages_title(feature: &str) -> String {
    format!("Scoped packages of feature {feature}")
}

fn scoped_packages_summary(feature: &str) -> String {
    format!(
        "The scoped packages that the feature {} declares: sets of commands defined only inside certain environment \
         bodies or arguments.",
        code_span(feature)
    )
}

fn scoped_package_title(name: &str) -> String {
    format!("Scoped package {name}")
}

fn settings_title(feature: &str) -> String {
    format!("Settings of feature {feature}")
}

fn settings_summary(feature: &str) -> String {
    format!("The settings keys of the feature {}.", code_span(feature))
}

fn workflow_title(name: &str) -> String {
    format!("Workflow {name}")
}

fn namespace_title(name: &str) -> String {
    format!("Namespace {name}")
}

fn entity_title(namespace: &str, name: &str) -> String {
    format!("{} {name}", namespace_label(namespace).0)
}

/// The words that name an entity of the namespace `namespace`, singular and plural, as the
/// pages write them.
pub(super) fn namespace_label(namespace: &str) -> (&'static str, &'static str) {
    match namespace {
        "feature" => ("Feature", "Features"),
        "package" => ("Package", "Packages"),
        "element" => ("Element type", "Element types"),
        "capability" => ("Capability", "Capabilities"),
        "table" => ("Table", "Tables"),
        "collection" => ("Collection", "Collections"),
        "stage" => ("Stage", "Stages"),
        "lowering-rule" => ("Lowering rule", "Lowering rules"),
        "backend" => ("Backend", "Backends"),
        "output" => ("Output", "Outputs"),
        "workflow" => ("Workflow", "Workflows"),
        "option-type" => ("Option type", "Option types"),
        "service-slot" => ("Service slot", "Service slots"),
        "scoped-package" => ("Scoped package", "Scoped packages"),
        "presentation-element" => ("Presentation element", "Presentation elements"),
        "piece" => ("Piece", "Pieces"),
        "style" => ("Style", "Styles"),
        "script" => ("Script", "Scripts"),
        "settings-key" => ("Settings key", "Settings keys"),
        "argument-role" => ("Argument role", "Argument roles"),
        _ => ("Entity", "Entities"),
    }
}
