//! The paths of the pages: the path of each thing a page shows, and the reading of a path.

use flm_core::lang::mint::CallableKind;

/// The path of the root page.
pub(crate) const ROOT: &str = "/";
pub(crate) const PACKAGES: &str = "packages/";
pub(crate) const FEATURES: &str = "features/";
pub(crate) const SETTINGS: &str = "settings/";
pub(crate) const SETTINGS_PARSING: &str = "settings/parsing";
pub(crate) const SETTINGS_FRONT_MATTER: &str = "settings/front-matter";
pub(crate) const WORKFLOWS: &str = "workflows/";
pub(crate) const NAMESPACES: &str = "namespaces/";
pub(crate) const PROGRAM: &str = "program/";
pub(crate) const PROGRAM_PATHS: &str = "program/paths";
pub(crate) const PROGRAM_EXTENSIONS: &str = "program/extensions";

/// The segment of a scoped package's pages under its feature.
const SCOPED_PACKAGES: &str = "scopedpackages";

/// The owner that the registry gives the element types of the language's own structure,
/// which no feature declares: the language's own name, which is not a feature and has no
/// page.
pub(crate) const LANGUAGE_OWNER: &str = "flm";

/// The namespaces whose entities have their page elsewhere than under `namespaces/`.
const ELSEWHERE: [&str; 5] = ["feature", "package", "workflow", "scoped-package", "settings-key"];

/// The segment of the commands of `kind` under a feature or a scoped package.
pub(crate) fn kind_segment(kind: CallableKind) -> &'static str {
    match kind {
        CallableKind::Macro => "macros",
        CallableKind::Environment => "environments",
        CallableKind::Specials => "specials",
    }
}

pub(crate) fn package(name: &str) -> String {
    format!("{PACKAGES}{name}")
}

pub(crate) fn feature(name: &str) -> String {
    format!("{FEATURES}{name}")
}

/// The path under which the commands of the feature `feature`, or of its scoped package
/// `within`, are listed.
fn commands_base(feature: &str, within: Option<&str>) -> String {
    match within {
        None => format!("{FEATURES}{feature}"),
        Some(package) => format!("{FEATURES}{feature}/{SCOPED_PACKAGES}/{package}"),
    }
}

pub(crate) fn kind_index(feature: &str, within: Option<&str>, kind: CallableKind) -> String {
    format!("{}/{}", commands_base(feature, within), kind_segment(kind))
}

/// The page of the command written `name` (without the backslash; a trigger for specials)
/// of the kind `kind`, declared by `feature`, in its scoped package `within` if any.
pub(crate) fn command(feature: &str, within: Option<&str>, kind: CallableKind, name: &str) -> String {
    format!("{}/{name}", kind_index(feature, within, kind))
}

pub(crate) fn scoped_packages(feature: &str) -> String {
    format!("{FEATURES}{feature}/{SCOPED_PACKAGES}")
}

pub(crate) fn scoped_package(owner: &str, name: &str) -> String {
    format!("{FEATURES}{owner}/{SCOPED_PACKAGES}/{name}")
}

pub(crate) fn settings(feature: &str) -> String {
    format!("{SETTINGS}{feature}")
}

pub(crate) fn workflow(name: &str) -> String {
    format!("{WORKFLOWS}{name}")
}

pub(crate) fn namespace(name: &str) -> String {
    format!("{NAMESPACES}{name}")
}

/// Whether the entities of the namespace `namespace` have pages `namespaces/<namespace>/<name>`.
pub(crate) fn has_entity_pages(namespace: &str) -> bool {
    !ELSEWHERE.contains(&namespace)
}

/// The page of the entity `name` of the namespace `namespace`, declared by `owner`: its
/// page under `namespaces/`, or its page elsewhere for the five namespaces whose entities
/// have one (a feature, a package, a workflow, a scoped package, a settings key).
pub(crate) fn entity(namespace: &str, name: &str, owner: Option<&str>) -> String {
    match (namespace, owner) {
        ("feature", _) => feature(name),
        ("package", _) => package(name),
        ("workflow", _) => workflow(name),
        ("scoped-package", Some(owner)) => scoped_package(owner, name),
        ("settings-key", Some(owner)) => settings(owner),
        _ => format!("{NAMESPACES}{namespace}/{name}"),
    }
}

/// What a path designates, as [`parse`] reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Location {
    Root,
    PackageIndex,
    Package(String),
    FeatureIndex,
    Feature(String),
    CommandIndex { feature: String, within: Option<String>, kind: CallableKind },
    Command { feature: String, within: Option<String>, kind: CallableKind, name: String },
    ScopedPackageIndex(String),
    ScopedPackage { feature: String, name: String },
    SettingsIndex,
    Settings(String),
    Parsing,
    FrontMatter,
    WorkflowIndex,
    Workflow(String),
    NamespaceIndex,
    Namespace(String),
    Entity { namespace: String, name: String },
    Program,
    ProgramPaths,
    ProgramExtensions,
}

/// Reads `path` as the paths of the [module documentation](super) write it; `None` for a
/// text that no path has the form of. The index page of a section may be written without
/// its final slash.
pub(crate) fn parse(path: &str) -> Option<Location> {
    let sections: [(&str, Location); 6] = [
        (PACKAGES, Location::PackageIndex),
        (FEATURES, Location::FeatureIndex),
        (SETTINGS, Location::SettingsIndex),
        (WORKFLOWS, Location::WorkflowIndex),
        (NAMESPACES, Location::NamespaceIndex),
        (PROGRAM, Location::Program),
    ];
    if path == ROOT {
        return Some(Location::Root);
    }
    for (section, location) in sections {
        if path == section || Some(path) == section.strip_suffix('/') {
            return Some(location);
        }
    }
    match path {
        SETTINGS_PARSING => return Some(Location::Parsing),
        SETTINGS_FRONT_MATTER => return Some(Location::FrontMatter),
        PROGRAM_PATHS => return Some(Location::ProgramPaths),
        PROGRAM_EXTENSIONS => return Some(Location::ProgramExtensions),
        _ => {}
    }
    if let Some(name) = nonempty(path.strip_prefix(PACKAGES)) {
        return Some(Location::Package(name.to_string()));
    }
    if let Some(name) = nonempty(path.strip_prefix(SETTINGS)) {
        return Some(Location::Settings(name.to_string()));
    }
    if let Some(name) = nonempty(path.strip_prefix(WORKFLOWS)) {
        return Some(Location::Workflow(name.to_string()));
    }
    if let Some(rest) = nonempty(path.strip_prefix(NAMESPACES)) {
        return Some(match rest.split_once('/') {
            None => Location::Namespace(rest.to_string()),
            Some((namespace, name)) if !namespace.is_empty() && !name.is_empty() => {
                Location::Entity { namespace: namespace.to_string(), name: name.to_string() }
            }
            Some(_) => return None,
        });
    }
    let rest = nonempty(path.strip_prefix(FEATURES))?;
    let Some((feature, rest)) = rest.split_once('/') else {
        return Some(Location::Feature(rest.to_string()));
    };
    if feature.is_empty() {
        return None;
    }
    let feature = feature.to_string();
    if let Some(rest) = rest.strip_prefix(SCOPED_PACKAGES) {
        let Some(rest) = rest.strip_prefix('/') else {
            return rest.is_empty().then_some(Location::ScopedPackageIndex(feature));
        };
        let (package, rest) = match rest.split_once('/') {
            None => (rest, None),
            Some((package, rest)) => (package, Some(rest)),
        };
        if package.is_empty() {
            return None;
        }
        return match rest {
            None => Some(Location::ScopedPackage { feature, name: package.to_string() }),
            Some(rest) => commands_location(feature, Some(package.to_string()), rest),
        };
    }
    commands_location(feature, None, rest)
}

/// Reads `rest`, the part of a path after a feature's or a scoped package's path and a
/// slash, as the page of a kind of commands or of one command.
fn commands_location(feature: String, within: Option<String>, rest: &str) -> Option<Location> {
    let (segment, name) = match rest.split_once('/') {
        None => (rest, None),
        Some((segment, name)) => (segment, Some(name)),
    };
    let kind = [CallableKind::Macro, CallableKind::Environment, CallableKind::Specials]
        .into_iter()
        .find(|kind| kind_segment(*kind) == segment)?;
    match name {
        None => Some(Location::CommandIndex { feature, within, kind }),
        Some("") => None,
        Some(name) => Some(Location::Command { feature, within, kind, name: name.to_string() }),
    }
}

fn nonempty(text: Option<&str>) -> Option<&str> {
    text.filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_form_reads_back() {
        let cases = [
            ("/", Location::Root),
            ("features", Location::FeatureIndex),
            ("features/", Location::FeatureIndex),
            ("features/headings", Location::Feature(String::from("headings"))),
            (
                "features/headings/macros",
                Location::CommandIndex { feature: String::from("headings"), within: None, kind: CallableKind::Macro },
            ),
            (
                "features/linebreak/macros//",
                Location::Command {
                    feature: String::from("linebreak"),
                    within: None,
                    kind: CallableKind::Macro,
                    name: String::from("/"),
                },
            ),
            (
                "features/lists/scopedpackages/flm.lists.items/macros/item",
                Location::Command {
                    feature: String::from("lists"),
                    within: Some(String::from("flm.lists.items")),
                    kind: CallableKind::Macro,
                    name: String::from("item"),
                },
            ),
            ("features/lists/scopedpackages", Location::ScopedPackageIndex(String::from("lists"))),
            (
                "features/lists/scopedpackages/flm.lists.items",
                Location::ScopedPackage { feature: String::from("lists"), name: String::from("flm.lists.items") },
            ),
            ("packages/doc/_flmextensions/narrow.yaml#_", Location::Package(String::from("doc/_flmextensions/narrow.yaml#_"))),
            ("settings/parsing", Location::Parsing),
            ("settings/html", Location::Settings(String::from("html"))),
            ("namespaces/stage", Location::Namespace(String::from("stage"))),
            (
                "namespaces/package/bundled:rust/flmdoc#_",
                Location::Entity { namespace: String::from("package"), name: String::from("bundled:rust/flmdoc#_") },
            ),
            ("program", Location::Program),
        ];
        for (path, location) in cases {
            assert_eq!(parse(path), Some(location), "{path}");
        }
        for path in ["", "nothing/here", "features/headings/", "features/headings/tables", "namespaces/stage/", "features//macros"] {
            assert_eq!(parse(path), None, "{path}");
        }
    }

    #[test]
    fn a_command_path_reads_back_as_its_command() {
        let path = command("lists", Some("flm.lists.items"), CallableKind::Environment, "x");
        assert_eq!(
            parse(&path),
            Some(Location::Command {
                feature: String::from("lists"),
                within: Some(String::from("flm.lists.items")),
                kind: CallableKind::Environment,
                name: String::from("x"),
            })
        );
        assert_eq!(entity("settings-key", "html.theme", Some("html")), "settings/html");
        assert_eq!(entity("stage", "flm.refs.resolve", Some("refs")), "namespaces/stage/flm.refs.resolve");
    }
}
