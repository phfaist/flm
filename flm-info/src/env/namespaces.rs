//! The families of names a registry holds ([`Environment::namespaces`],
//! [`Environment::namespace`]), the lookup of a name across them
//! ([`Environment::lookup`]), and the inventory of what one feature declares.

use flm_core::core::registry::ElementKind;
use flm_core::lang::mint::{CallableKind, SyntaxDef};
use flm_core::stdcore::backends::pieces::declared_pieces;
use serde::Serialize;

use super::{DeclaredView, Environment, is_empty, summary_of};

/// One family of names a registry holds. The order of [`FAMILIES`] is the order of
/// [`Environment::namespaces`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    Feature,
    Package,
    Element,
    Capability,
    Table,
    Collection,
    Stage,
    LoweringRule,
    Backend,
    Output,
    Workflow,
    OptionType,
    ServiceSlot,
    ScopedPackage,
    PresentationElement,
    Piece,
    Style,
    Script,
    SettingsKey,
    ArgumentRole,
}

pub(super) const FAMILIES: [Family; 20] = [
    Family::Feature,
    Family::Package,
    Family::Element,
    Family::Capability,
    Family::Table,
    Family::Collection,
    Family::Stage,
    Family::LoweringRule,
    Family::Backend,
    Family::Output,
    Family::Workflow,
    Family::OptionType,
    Family::ServiceSlot,
    Family::ScopedPackage,
    Family::PresentationElement,
    Family::Piece,
    Family::Style,
    Family::Script,
    Family::SettingsKey,
    Family::ArgumentRole,
];

impl Family {
    /// The family whose namespace is called `name`.
    pub(super) fn named(name: &str) -> Option<Family> {
        FAMILIES.into_iter().find(|family| family.name() == name)
    }

    /// The namespace's name, as the views spell it.
    pub(super) fn name(self) -> &'static str {
        match self {
            Family::Feature => "feature",
            Family::Package => "package",
            Family::Element => "element",
            Family::Capability => "capability",
            Family::Table => "table",
            Family::Collection => "collection",
            Family::Stage => "stage",
            Family::LoweringRule => "lowering-rule",
            Family::Backend => "backend",
            Family::Output => "output",
            Family::Workflow => "workflow",
            Family::OptionType => "option-type",
            Family::ServiceSlot => "service-slot",
            Family::ScopedPackage => "scoped-package",
            Family::PresentationElement => "presentation-element",
            Family::Piece => "piece",
            Family::Style => "style",
            Family::Script => "script",
            Family::SettingsKey => "settings-key",
            Family::ArgumentRole => "argument-role",
        }
    }

    /// One sentence saying what the names of the namespace are, written as a documentation
    /// text is (FLM, with the backtick for code).
    fn what(self) -> &'static str {
        match self {
            Family::Feature => {
                "A feature is a unit of the registry: it declares commands, stages, tables, settings and the other \
                 entities listed here, under names it owns."
            }
            Family::Package => {
                "A package is a named bundle of features, together with the packages it builds on, that a document \
                 or the program chooses to use."
            }
            Family::Element => {
                "An element type says what a node of a document's tree means; the nodes of every command carry an \
                 element of one type."
            }
            Family::Capability => {
                "A capability is a named condition that a stage brings about when it runs, such as a table having \
                 been written; a plan schedules the stages that provide the capabilities it needs."
            }
            Family::Table => "A table holds data that one stage computes over a document and later stages read.",
            Family::Collection => {
                "A collection is a name for a set of capabilities: demanding the collection demands every member."
            }
            Family::Stage => {
                "A stage is one step of the processing of a document: it rewrites the trees, computes a table, \
                 checks something, or writes output."
            }
            Family::LoweringRule => {
                "A lowering rule turns the nodes of one element type into elements that a backend writes \
                 natively."
            }
            Family::Backend => {
                "A backend turns a document into one output format, through its lowering, emit and final stages."
            }
            Family::Output => "An output is a set of files that a plan can produce, such as an HTML page.",
            Family::Workflow => {
                "A workflow says what a build produces: its outputs, the capabilities it demands, and its \
                 preferences among stages and lowering rules."
            }
            Family::OptionType => {
                "An option type is a typed group of values, set on the registry, on a document or on a render \
                 request, that stages and lowering rules read."
            }
            Family::ServiceSlot => {
                "A service slot is a named place where features and the embedding program contribute objects that \
                 stages use."
            }
            Family::ScopedPackage => {
                "A scoped package is a set of commands defined only inside certain environment bodies or \
                 arguments, such as the item command inside a list."
            }
            Family::PresentationElement => {
                "A presentation element is an element of the trees a backend builds from the document's trees; it \
                 never appears in a parsed document."
            }
            Family::Piece => {
                "A piece is a value computed over the whole document, such as the list of footnotes, that a backend \
                 places before or after the document's content."
            }
            Family::Style => {
                "A style definition is a part of a stylesheet that a backend writes into its output when a tree \
                 requires it."
            }
            Family::Script => {
                "A script definition is a part of a script that a backend writes into its output when a tree \
                 requires it."
            }
            Family::SettingsKey => {
                "A settings key is one value of a feature's settings, written under the feature's name; this list \
                 gives each key as the feature's name, a dot and the key."
            }
            Family::ArgumentRole => {
                "An argument role is a registered name for what an argument of a command holds, such as a label or \
                 a file path."
            }
        }
    }
}

impl Environment {
    /// Every family of names the registry holds, in a fixed order: `feature`, `package`,
    /// `element`, `capability`, `table`, `collection`, `stage`, `lowering-rule`, `backend`,
    /// `output`, `workflow`, `option-type`, `service-slot`, `scoped-package`,
    /// `presentation-element`, `piece`, `style`, `script`, `settings-key`,
    /// `argument-role`. Each holds a sentence saying what its names are and its entries.
    ///
    /// The entries of a namespace are in the registry's order for that namespace (the
    /// order of registration, of numbering, or of names, as the registry keeps them). An
    /// element type that two features register appears once per feature; the capability of
    /// a table appears under `capability` as well as the table under `table`; a piece's
    /// owner is the feature that declares the table holding its content.
    pub fn namespaces(&self) -> Vec<NamespaceView> {
        FAMILIES.iter().map(|family| self.namespace_view(*family)).collect()
    }

    /// The namespace called `name` (one of the names [`namespaces`](Environment::namespaces)
    /// lists), or `None` for any other name.
    pub fn namespace(&self, name: &str) -> Option<NamespaceView> {
        FAMILIES.iter().find(|family| family.name() == name).map(|family| self.namespace_view(*family))
    }

    /// Every entity of any namespace whose name is `name`, then every command written as
    /// `name`.
    ///
    /// A command is looked up by kind: `\name` (with the backslash) finds the macros and the
    /// environments called `name`; a name without the backslash finds the macros and the
    /// environments of that name and the special character sequences whose trigger it is.
    /// Commands are searched among the declarations that features made directly, then
    /// among those of each scoped package. Each hit gives what identifies it: the
    /// namespace and the name of an entity, or the kind, the name, the declaring feature
    /// and the scoped package of a command. Empty when nothing has that name.
    pub fn lookup(&self, name: &str) -> Vec<Hit> {
        let mut hits: Vec<Hit> = Vec::new();
        for family in FAMILIES {
            for entry in self.entries(family) {
                if entry.name == name {
                    hits.push(Hit::Entity {
                        namespace: family.name().to_string(),
                        name: entry.name,
                        owner: entry.owner,
                        summary: entry.summary,
                    });
                }
            }
        }
        let (bare, kinds): (&str, &[CallableKind]) = match name.strip_prefix('\\') {
            Some(bare) => (bare, &[CallableKind::Macro, CallableKind::Environment]),
            None => (name, &[CallableKind::Macro, CallableKind::Environment, CallableKind::Specials]),
        };
        for (def, within) in self.all_declarations() {
            let Some(kind) = def.syntax.kind() else { continue };
            if !kinds.contains(&kind) {
                continue;
            }
            let written = match &def.syntax {
                SyntaxDef::Specials { trigger, .. } => trigger == bare,
                _ => def.names.iter().any(|written| written == bare),
            };
            if written {
                hits.push(Hit::Command {
                    kind,
                    name: bare.to_string(),
                    feature: def.owner.to_string(),
                    within: within.map(|package| package.name.to_string()),
                    summary: summary_of(&def.docs).to_string(),
                });
            }
        }
        hits
    }

    /// The names the feature `feature` declares, by namespace, in the order of the
    /// namespaces; the features, packages and element types are left out (a feature view
    /// lists its element types apart), and so is a namespace in which the feature declares
    /// nothing.
    pub(crate) fn declared_by(&self, feature: &str) -> Vec<DeclaredView> {
        FAMILIES
            .iter()
            .filter(|family| !matches!(family, Family::Feature | Family::Package | Family::Element))
            .filter_map(|family| {
                let entries: Vec<EntryView> = self
                    .entries(*family)
                    .into_iter()
                    .filter(|entry| entry.owner.as_deref() == Some(feature))
                    .collect();
                (!entries.is_empty()).then(|| DeclaredView { namespace: family.name().to_string(), entries })
            })
            .collect()
    }

    fn namespace_view(&self, family: Family) -> NamespaceView {
        NamespaceView { name: family.name().to_string(), what: family.what().to_string(), entries: self.entries(family) }
    }

    /// The entries of the namespace `family`, in the registry's order.
    fn entries(&self, family: Family) -> Vec<EntryView> {
        let registry = &self.registry;
        match family {
            Family::Feature => registry.features().iter().map(|info| entry(&info.name, None, &info.docs)).collect(),
            Family::Package => self.factories.iter().map(|factory| entry(factory.name(), None, "")).collect(),
            Family::Element => {
                let mut out: Vec<EntryView> = Vec::new();
                for element in registry.elements().iter().filter(|element| element.kind != ElementKind::Presentation) {
                    let owner = element.owner.to_string();
                    let known = out.iter().any(|seen| seen.name == element.element.name.as_str() && seen.owner.as_deref() == Some(owner.as_str()));
                    if !known {
                        out.push(entry(&element.element.name, Some(&owner), &element.element.docs));
                    }
                }
                out
            }
            Family::Capability => registry
                .capabilities()
                .iter()
                .map(|capability| entry(&capability.name, Some(&capability.owner), &capability.docs))
                .collect(),
            Family::Table => {
                registry.tables().iter().map(|table| entry(&table.def.name, Some(&table.def.owner), &table.def.docs)).collect()
            }
            Family::Collection => registry
                .collections()
                .map(|collection| entry(&collection.name, Some(&collection.owner), &collection.docs))
                .collect(),
            Family::Stage => {
                registry.stages().iter().map(|stage| entry(&stage.name, Some(&stage.owner), &stage.decl.docs)).collect()
            }
            Family::LoweringRule => {
                registry.rules().iter().map(|rule| entry(&rule.rule.name, Some(&rule.owner), &rule.rule.docs)).collect()
            }
            Family::Backend => {
                registry.backends().map(|backend| entry(&backend.name, Some(&backend.owner), &backend.docs)).collect()
            }
            Family::Output => {
                registry.outputs().map(|output| entry(&output.def.name, Some(&output.owner), &output.def.docs)).collect()
            }
            Family::Workflow => registry
                .workflows()
                .map(|workflow| entry(&workflow.def.name, Some(&workflow.owner), &workflow.def.docs))
                .collect(),
            Family::OptionType => registry
                .options()
                .values()
                .map(|option| entry(option.key.short_name(), Some(&option.owner), &option.docs))
                .collect(),
            Family::ServiceSlot => {
                registry.services().slots().map(|slot| entry(&slot.name, Some(&slot.owner), &slot.docs)).collect()
            }
            Family::ScopedPackage => registry
                .declarations()
                .scoped_packages()
                .iter()
                .map(|package| entry(&package.name, Some(&package.owner), &package.docs))
                .collect(),
            Family::PresentationElement => registry
                .presentation_elements()
                .map(|presentation| entry(&presentation.element.name, Some(&presentation.owner), &presentation.docs))
                .collect(),
            Family::Piece => declared_pieces(registry)
                .map(|piece| {
                    let owner = registry.table_named(piece.table.name()).map(|table| table.def.owner.to_string());
                    entry(&piece.name, owner.as_deref(), &piece.docs)
                })
                .collect(),
            Family::Style => registry.styles().iter().map(|(owner, style)| entry(&style.id, Some(owner), &style.docs)).collect(),
            Family::Script => {
                registry.scripts().iter().map(|(owner, script)| entry(&script.id, Some(owner), &script.docs)).collect()
            }
            Family::SettingsKey => {
                let mut out = Vec::new();
                for factory in &self.factories {
                    for feature in factory.features() {
                        let Some(decl) = &feature.settings else { continue };
                        let schema = decl.json_schema();
                        for key in decl.keys() {
                            let description = schema
                                .pointer(&format!("/properties/{}/description", key.name))
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default();
                            out.push(entry(&format!("{}.{}", feature.name, key.name), Some(&feature.name), description));
                        }
                    }
                }
                out
            }
            Family::ArgumentRole => {
                registry.argument_roles().iter().map(|role| entry(&role.name, Some(&role.owner), &role.docs)).collect()
            }
        }
    }
}

/// The entry `name`, declared by `owner` if any, with the summary of `docs`.
fn entry(name: &str, owner: Option<&str>, docs: &str) -> EntryView {
    EntryView { name: name.to_string(), owner: owner.map(str::to_string), summary: summary_of(docs).to_string() }
}

/// One family of names a registry holds, with its entries, as
/// [`Environment::namespaces`] and [`Environment::namespace`] answer it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct NamespaceView {
    /// The namespace's name: `feature`, `stage`, `settings-key`, …
    pub name: String,
    /// One sentence saying what the names of the namespace are, written as a documentation
    /// text is (FLM, with the backtick for code).
    pub what: String,
    /// Its entries, in the registry's order.
    pub entries: Vec<EntryView>,
}

/// One name of a namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct EntryView {
    /// The name.
    pub name: String,
    /// The feature that declares it. An element type of the language's own structure that
    /// no feature declares (`flm.text`, `flm.comment`, `flm.paragraph`, …) has the owner
    /// `flm`, the language's own name, which is not a feature. Absent for a feature and a
    /// package, and for a piece whose table no feature declares.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The first paragraph of its documentation text.
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
}

/// One thing that [`Environment::lookup`] found: an entity of a namespace, or a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "hit", rename_all = "lowercase")]
pub enum Hit {
    /// An entity of a namespace.
    Entity {
        /// The namespace's name, as [`NamespaceView::name`] gives it.
        namespace: String,
        /// The entity's name.
        name: String,
        /// The feature that declares it, as [`EntryView::owner`] gives it.
        #[serde(skip_serializing_if = "Option::is_none")]
        owner: Option<String>,
        /// The first paragraph of its documentation text.
        #[serde(skip_serializing_if = "is_empty")]
        summary: String,
    },
    /// A declaration of a command.
    Command {
        /// The command's kind: `macro`, `environment` or `specials`, as
        /// [`CommandRef::kind`](super::CommandRef::kind) spells it.
        kind: CallableKind,
        /// The name it is written under, without the backslash; for specials, the
        /// trigger.
        name: String,
        /// The feature that declares it.
        feature: String,
        /// The scoped package that defines it, for a command defined only inside certain
        /// environments or arguments.
        #[serde(skip_serializing_if = "Option::is_none")]
        within: Option<String>,
        /// The first paragraph of its documentation text.
        #[serde(skip_serializing_if = "is_empty")]
        summary: String,
    },
}
