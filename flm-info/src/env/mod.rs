//! The queries over an environment: [`Environment`] and the views its methods answer.
//!
//! An [`Environment`] is made from a [registry](flm_core::core::registry::FlmRegistry), an
//! optional [plan](flm_core::core::plan::Plan) (the workflow in view, when the caller built
//! one) and the [package factories](flm_core::core::registry::PackageFactory) the registry
//! was built from. Each method answers a *view*: a plain value that holds what a person or
//! a program asks about one part of the environment, and that serializes to JSON (see
//! [`crate::describe`] for the rules of the JSON form). The methods, by subject:
//!
//! - **Packages and features.** [`packages`](Environment::packages) lists the packages,
//!   [`features`](Environment::features) and [`feature`](Environment::feature) the
//!   features, each with the inventory of what it declares.
//! - **Commands.** [`commands`](Environment::commands) lists the commands one feature
//!   declares, grouped by kind; [`command`](Environment::command) gives every declaration
//!   of one command across features; [`scoped_packages`](Environment::scoped_packages) and
//!   [`scoped_package`](Environment::scoped_package) describe the commands defined only
//!   inside certain environments or arguments.
//! - **Settings.** [`settings`](Environment::settings) describes a feature's settings keys,
//!   [`parsing_settings`](Environment::parsing_settings) the parsing configuration.
//! - **Workflows.** [`workflows`](Environment::workflows) and
//!   [`workflow`](Environment::workflow) describe the workflows, with the plan in view.
//! - **Names.** [`namespaces`](Environment::namespaces) and
//!   [`namespace`](Environment::namespace) list the names of each family of names the
//!   registry holds; [`entity`](Environment::entity) describes one entity of a namespace
//!   with every declaration of its name; [`lookup`](Environment::lookup) finds every entity
//!   and command of a given name.
//!
//! A *summary* is the first paragraph of a documentation text ([`summary_of`]); every view
//! that holds a documentation text holds its summary too.

// The query API is the primary API of flm-info; the description and the pages derive from
// its views. See decision rationale at [§flm-dd-dr:info-single-source].

mod commands;
mod entities;
mod namespaces;
mod settings;
mod workflows;

use std::sync::Arc;

use flm_core::core::plan::Plan;
use flm_core::core::registry::{ElementKind, FlmRegistry, PackageFactory};
use flm_core::lang::mint::{CallableKind, ElementDef, ScopedPackageDef, SyntaxDef, TackOnField};
use flm_core::model::element::ElementSpec;
use serde::Serialize;

pub use commands::{
    ArgumentView, BodyView, CommandView, CommandsView, ElementView, FieldView, ScopedPackageView,
    UseSiteView,
};
pub use entities::{EntityDeclarationView, EntityView, StageStatusView};
pub use namespaces::{EntryView, Hit, NamespaceView};
pub use settings::{ParsingKeyView, ParsingView, SettingKeyView, SettingsView};
pub use workflows::{
    LoweringView, PlanView, PlannedStageView, ReadView, RuleChoiceView, WorkflowView,
};

/// An FLM environment, ready to be asked about: a registry, the plan in view when there is
/// one, and the package factories the registry was built from.
///
/// Make one with [`Environment::new`] from the parts a program holds, or, under the cargo
/// feature `flm-main`, from a document the host prepared (`Environment::of_prepared`). Its
/// methods answer the views of the [module documentation](self). An environment is
/// immutable; each method computes its view anew from the registry.
///
/// See the [crate documentation](crate) for an example.
#[derive(Clone)]
pub struct Environment {
    registry: Arc<FlmRegistry>,
    plan: Option<Arc<Plan>>,
    factories: Vec<PackageFactory>,
    #[cfg(feature = "flm-main")]
    pub(crate) host: Option<crate::host::HostInfo>,
}

impl Environment {
    /// Makes the environment of `registry`, built from the packages of `factories`, with
    /// `plan`, the plan of the workflow in view, when the caller built one.
    ///
    /// The factories give which package holds which feature, and each feature's settings
    /// declaration ([`PackageFactory::features`]); a feature that no factory lists is
    /// described without a package and without settings. The plan, when given, should be a
    /// plan of `registry`: [`workflow`](Environment::workflow) shows it for the workflow it
    /// was built from.
    pub fn new(registry: Arc<FlmRegistry>, plan: Option<Arc<Plan>>, factories: Vec<PackageFactory>) -> Environment {
        Environment {
            registry,
            plan,
            factories,
            #[cfg(feature = "flm-main")]
            host: None,
        }
    }

    /// The registry the environment describes.
    pub fn registry(&self) -> &Arc<FlmRegistry> {
        &self.registry
    }

    /// The plan of the workflow in view, when there is one.
    pub fn plan(&self) -> Option<&Arc<Plan>> {
        self.plan.as_ref()
    }

    /// The package factories the registry was built from, in the order given.
    pub fn factories(&self) -> &[PackageFactory] {
        &self.factories
    }

    /// Every package of the environment, in the order of its factories.
    pub fn packages(&self) -> Vec<PackageView> {
        self.factories
            .iter()
            .map(|factory| PackageView {
                name: factory.name().to_string(),
                uses: factory.used().iter().map(ToString::to_string).collect(),
                features: factory.features().iter().map(|feature| feature.name.to_string()).collect(),
                not_in_registry: factory
                    .features()
                    .iter()
                    .filter(|feature| self.registry.feature(&feature.name).is_none())
                    .map(|feature| feature.name.to_string())
                    .collect(),
                #[cfg(feature = "flm-main")]
                origin: self.host.as_ref().map(|host| host.origin_of(factory.name())),
            })
            .collect()
    }

    /// Every feature of the registry, in the registry's final feature order.
    pub fn features(&self) -> Vec<FeatureView> {
        self.registry.features().iter().map(|info| self.feature_view(info)).collect()
    }

    /// The feature called `name`, or `None` when the registry has no such feature.
    pub fn feature(&self, name: &str) -> Option<FeatureView> {
        self.registry.feature(name).map(|info| self.feature_view(info))
    }

    /// The name of the package whose factory lists the feature `feature`.
    pub(crate) fn package_of(&self, feature: &str) -> Option<String> {
        self.factories
            .iter()
            .find(|factory| factory.features().iter().any(|listed| listed.name == *feature))
            .map(|factory| factory.name().to_string())
    }

    fn feature_view(&self, info: &flm_core::core::registry::FeatureInfo) -> FeatureView {
        let name = info.name.as_str();
        FeatureView {
            name: name.to_string(),
            title: info.title.clone(),
            version: info.version.clone(),
            docs: info.docs.clone(),
            summary: summary_of(&info.docs).to_string(),
            package: self.package_of(name),
            elements: self.element_types(name),
            declares: self.declared_by(name),
        }
    }

    /// The element types the feature `feature` registers, each with its values in
    /// numbering order and, for each value, the commands written for it.
    fn element_types(&self, feature: &str) -> Vec<ElementTypeView> {
        let mut types: Vec<ElementTypeView> = Vec::new();
        for entry in self.registry.elements() {
            if entry.owner != *feature || entry.kind != ElementKind::Source {
                continue;
            }
            let value = ElementValueView {
                number: entry.idx.0,
                docs: entry.element.docs.clone(),
                summary: summary_of(&entry.element.docs).to_string(),
                commands: self.commands_for(&entry.element),
            };
            let name = entry.element.name.to_string();
            match types.iter_mut().find(|known| known.name == name) {
                Some(known) => known.values.push(value),
                None => types.push(ElementTypeView { name, values: vec![value] }),
            }
        }
        types
    }

    /// The commands written for the element value `element`: every declaration whose
    /// nodes carry it, and every tack-on field whose nodes carry it.
    pub(super) fn commands_for(&self, element: &Arc<ElementSpec>) -> Vec<CommandRef> {
        let mut out = Vec::new();
        for (def, within) in self.all_declarations() {
            if Arc::ptr_eq(&def.element, element) {
                out.extend(command_refs(def, within, None));
            }
            for field in fields_of(def.syntax.args()) {
                if Arc::ptr_eq(field.element(), element) {
                    out.push(CommandRef {
                        kind: CallableKind::Macro,
                        name: field.name.clone(),
                        feature: def.owner.to_string(),
                        within: within.map(|package| package.name.to_string()),
                        field_of: def.syntax.kind().map(|kind| CommandName { kind, name: written_name(def) }),
                    });
                }
            }
        }
        out
    }

    /// Every element declaration with the scoped package that holds it: the declarations
    /// that features made directly first, then those of each scoped package.
    pub(crate) fn all_declarations(&self) -> impl Iterator<Item = (&ElementDef, Option<&ScopedPackageDef>)> {
        let declarations = self.registry.declarations();
        declarations.elements().iter().map(|def| (def, None)).chain(
            declarations
                .scoped_packages()
                .iter()
                .flat_map(|package| package.elements.iter().map(move |def| (def, Some(package)))),
        )
    }
}

/// The references to the commands a declaration defines, one per name it is written under
/// (a special character sequence's trigger for specials; none for a declaration without
/// syntax).
fn command_refs(def: &ElementDef, within: Option<&ScopedPackageDef>, field_of: Option<CommandName>) -> Vec<CommandRef> {
    let Some(kind) = def.syntax.kind() else { return Vec::new() };
    let names: Vec<String> = match &def.syntax {
        SyntaxDef::Specials { trigger, .. } => vec![trigger.clone()],
        _ => def.names.iter().map(ToString::to_string).collect(),
    };
    names
        .into_iter()
        .map(|name| CommandRef {
            kind,
            name,
            feature: def.owner.to_string(),
            within: within.map(|package| package.name.to_string()),
            field_of: field_of.clone(),
        })
        .collect()
}

/// The name a declaration is written under, for a message or a reference: its first name,
/// or its trigger for a special character sequence; empty for a declaration without
/// syntax.
pub(crate) fn written_name(def: &ElementDef) -> String {
    match &def.syntax {
        SyntaxDef::Specials { trigger, .. } => trigger.clone(),
        _ => def.names.first().map(ToString::to_string).unwrap_or_default(),
    }
}

/// Every tack-on field of the arguments `args`, and of those fields' own arguments, in the
/// order written.
fn fields_of(args: &[flm_core::lang::mint::ArgDef]) -> Vec<&TackOnField> {
    let mut out = Vec::new();
    for arg in args {
        for field in arg.kind.tack_on_fields().unwrap_or_default() {
            out.push(field);
            out.extend(fields_of(&field.args));
        }
    }
    out
}

/// Returns the *summary* of a documentation text: its first paragraph, up to the first
/// blank line (a line holding only whitespace), trimmed. Blank lines before the first
/// paragraph are skipped; a text with no blank line is its own summary; an empty text has
/// an empty summary.
///
/// Every view that holds a documentation text holds its summary, computed by this
/// function, so that every listing shows the same short text.
///
/// ```
/// use flm_info::summary_of;
///
/// assert_eq!(summary_of("A cell.\n\nIts placement is the next column."), "A cell.");
/// assert_eq!(summary_of("\n  \nOne line,\nthen another.\n"), "One line,\nthen another.");
/// assert_eq!(summary_of(""), "");
/// ```
pub fn summary_of(docs: &str) -> &str {
    split_docs(docs).0
}

/// The documentation text `docs` split into its summary ([`summary_of`]) and the rest, the
/// paragraphs after the blank line that ends the summary, each trimmed; the rest is empty
/// for a text of one paragraph.
pub(crate) fn split_docs(docs: &str) -> (&str, &str) {
    let mut offset = 0;
    for line in docs.split_inclusive('\n') {
        if line.trim().is_empty() && !docs[..offset].trim().is_empty() {
            return (docs[..offset].trim(), docs[offset..].trim());
        }
        offset += line.len();
    }
    (docs.trim(), "")
}

/// Whether a string has nothing to show; for `skip_serializing_if`.
pub(crate) fn is_empty(text: &str) -> bool {
    text.is_empty()
}

/// Whether a flag is `false`; for `skip_serializing_if`.
pub(crate) fn is_false(flag: &bool) -> bool {
    !*flag
}

/// One package of an environment, as [`Environment::packages`] lists it.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct PackageView {
    /// The package's name, as its factory gives it. For a package of an extension that the
    /// host found, the name is `<identity>#<package>`.
    pub name: String,
    /// The names of the packages it uses, in the order its factory lists them.
    pub uses: Vec<String>,
    /// The names of its features, in order.
    pub features: Vec<String>,
    /// The names of those of its features that the registry does not hold, in order: the
    /// features that a settings source switched off (by writing `false` for the feature),
    /// or that the registry was built without.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_in_registry: Vec<String>,
    /// Where the package comes from; present when the environment was made from a prepared
    /// document (cargo feature `flm-main`).
    #[cfg(feature = "flm-main")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<crate::host::PackageOrigin>,
}

/// One feature of an environment, with the inventory of what it declares, as
/// [`Environment::feature`] and [`Environment::features`] answer it.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct FeatureView {
    /// The feature's registered name.
    pub name: String,
    /// Its title, a few words for a person.
    pub title: String,
    /// Its version, when it states one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Its documentation text.
    #[serde(skip_serializing_if = "is_empty")]
    pub docs: String,
    /// The first paragraph of [`docs`](FeatureView::docs) ([`summary_of`]).
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
    /// The package whose factory lists the feature; absent when no factory of the
    /// environment lists it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// The element types the feature registers, in numbering order, each with its values
    /// and the commands written for each value.
    pub elements: Vec<ElementTypeView>,
    /// Everything else the feature declares, by namespace (scoped packages, stages, tables,
    /// capabilities, rules, backends, outputs, workflows, option types, service slots,
    /// argument roles, styles, scripts, settings keys and the others; see
    /// [`Environment::namespaces`]), in the order of the namespaces; a namespace in which
    /// the feature declares nothing is left out.
    pub declares: Vec<DeclaredView>,
}

/// The names a feature declares in one namespace, as [`FeatureView::declares`] lists them.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct DeclaredView {
    /// The namespace's name (`stage`, `table`, …), as [`NamespaceView::name`] gives it.
    pub namespace: String,
    /// The entries the feature declares, in the namespace's order.
    pub entries: Vec<EntryView>,
}

/// One element type that a feature registers: its name and its values.
///
/// An element type is the name of what a node means; a feature may register several
/// *values* under one name (the headings feature registers one value per kind of heading),
/// each a separate element of the registry with its own number.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct ElementTypeView {
    /// The element type's name.
    pub name: String,
    /// Its values that the feature registers, in numbering order.
    pub values: Vec<ElementValueView>,
}

/// One element value: its number in the registry, its documentation, and the commands
/// written for it.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct ElementValueView {
    /// The value's number in the registry.
    pub number: u32,
    /// The value's documentation text.
    #[serde(skip_serializing_if = "is_empty")]
    pub docs: String,
    /// The first paragraph of [`docs`](ElementValueView::docs).
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
    /// The commands whose nodes carry the value: the declarations' names (and triggers),
    /// and the tack-on fields; empty for a value that no command is written for, for
    /// example a value of the language's own structure (which the parser makes from what
    /// the author writes), a value that a stage creates, or a value that only declares its
    /// element type and that no node carries.
    pub commands: Vec<CommandRef>,
}

/// A reference to one command: its kind, the name it is written under, the feature that
/// declares it, and where it is defined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CommandRef {
    /// The command's kind: `macro`, `environment` or `specials`.
    pub kind: CallableKind,
    /// The name it is written under, without the backslash; for specials, the trigger.
    pub name: String,
    /// The feature that declares it.
    pub feature: String,
    /// The scoped package that defines it, for a command defined only inside certain
    /// environments or arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
    /// For a tack-on field (a macro written after a command, such as `\label` after
    /// `\section`): the kind and the name of the command that accepts it, which the same
    /// feature declares, in the same scoped package as [`within`](CommandRef::within) says.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_of: Option<CommandName>,
}

/// The kind of a command and the name it is written under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CommandName {
    /// The command's kind: `macro`, `environment` or `specials`.
    pub kind: CallableKind,
    /// The name it is written under, without the backslash; for specials, the trigger.
    pub name: String,
}
