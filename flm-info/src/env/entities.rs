//! One entity of a namespace, with every declaration of its name ([`Environment::entity`]).

use flm_core::core::registry::ElementKind;
use flm_core::stdcore::backends::pieces::declared_pieces;
use serde::Serialize;
use serde_json::Value;

use super::namespaces::Family;
use super::workflows::planned_stage;
use super::{ElementValueView, Environment, PlannedStageView, is_empty, summary_of};

impl Environment {
    /// The entity called `name` in the namespace called `namespace` (one of the names
    /// [`namespaces`](Environment::namespaces) lists), with every declaration of that name in
    /// the namespace, in the namespace's order; `None` when the namespace does not exist or
    /// holds no entity of that name.
    ///
    /// Most names are declared once. An element type that two features register has one
    /// declaration per feature, each with its own values. Each declaration holds its owner,
    /// its documentation text, the declaration as `flm-core` serializes it where it has a
    /// serialized form, and what the namespace adds: an element type's values with the
    /// commands written for each, a collection's members, the element type a lowering rule
    /// lowers, the capability a capability refines, the stages that write a table, and, for
    /// a stage, whether the plan in view schedules it.
    pub fn entity(&self, namespace: &str, name: &str) -> Option<EntityView> {
        let family = Family::named(namespace)?;
        let declarations = self.entity_declarations(family, name);
        if declarations.is_empty() {
            return None;
        }
        Some(EntityView { namespace: namespace.to_string(), name: name.to_string(), declarations })
    }

    fn entity_declarations(&self, family: Family, name: &str) -> Vec<EntityDeclarationView> {
        let registry = &self.registry;
        match family {
            Family::Feature => registry
                .feature(name)
                .map(|info| declaration(None, &info.docs).with_data(serde_json::to_value(info).ok()))
                .into_iter()
                .collect(),
            Family::Package => self
                .factories
                .iter()
                .filter(|factory| factory.name() == name)
                .map(|_| declaration(None, ""))
                .collect(),
            Family::Element => {
                let mut out: Vec<(String, EntityDeclarationView)> = Vec::new();
                for entry in registry.elements() {
                    if entry.kind == ElementKind::Presentation || entry.element.name != *name {
                        continue;
                    }
                    let owner = entry.owner.to_string();
                    let value = ElementValueView {
                        number: entry.idx.0,
                        docs: entry.element.docs.clone(),
                        summary: summary_of(&entry.element.docs).to_string(),
                        commands: self.commands_for(&entry.element),
                    };
                    match out.iter_mut().find(|(known, _)| *known == owner) {
                        Some((_, view)) => view.values.push(value),
                        None => {
                            let mut view = declaration(Some(&owner), &entry.element.docs);
                            view.values.push(value);
                            out.push((owner, view));
                        }
                    }
                }
                out.into_iter().map(|(_, view)| view).collect()
            }
            Family::Capability => registry
                .capability_named(name)
                .map(|entry| {
                    let mut view = declaration(Some(&entry.owner), &entry.docs);
                    view.refines = entry.refines.map(|id| registry.capability(id).name.to_string());
                    view
                })
                .into_iter()
                .collect(),
            Family::Table => registry
                .table_named(name)
                .map(|entry| {
                    let mut view = declaration(Some(&entry.def.owner), &entry.def.docs)
                        .with_data(serde_json::to_value(&entry.def).ok());
                    view.written_by = registry
                        .stages()
                        .iter()
                        .filter(|stage| stage.writes.contains(&entry.id))
                        .map(|stage| stage.name.to_string())
                        .collect();
                    view
                })
                .into_iter()
                .collect(),
            Family::Collection => registry
                .collections()
                .filter(|entry| entry.name == *name)
                .map(|entry| {
                    let mut view = declaration(Some(&entry.owner), &entry.docs);
                    view.members = entry.members.iter().map(|id| registry.capability(*id).name.to_string()).collect();
                    view
                })
                .collect(),
            Family::Stage => registry
                .stage_named(name)
                .map(|entry| {
                    let mut view = declaration(Some(&entry.owner), &entry.decl.docs)
                        .with_data(serde_json::to_value(&entry.decl).ok());
                    view.plan = self.stage_status(entry.idx);
                    view
                })
                .into_iter()
                .collect(),
            Family::LoweringRule => registry
                .rules()
                .iter()
                .filter(|entry| entry.rule.name == *name)
                .map(|entry| {
                    let mut view = declaration(Some(&entry.owner), &entry.rule.docs);
                    view.lowers = Some(entry.rule.element.to_string());
                    view
                })
                .collect(),
            Family::Backend => registry
                .backends()
                .filter(|entry| entry.name == *name)
                .map(|entry| declaration(Some(&entry.owner), &entry.docs))
                .collect(),
            Family::Output => registry
                .outputs()
                .filter(|entry| entry.def.name == *name)
                .map(|entry| {
                    declaration(Some(&entry.owner), &entry.def.docs).with_data(serde_json::to_value(&entry.def).ok())
                })
                .collect(),
            Family::Workflow => registry
                .workflow(name)
                .map(|entry| {
                    declaration(Some(&entry.owner), &entry.def.docs).with_data(serde_json::to_value(&entry.def).ok())
                })
                .into_iter()
                .collect(),
            Family::OptionType => registry
                .options()
                .values()
                .filter(|option| option.key.short_name() == name)
                .map(|option| declaration(Some(&option.owner), &option.docs).with_data(serde_json::to_value(option).ok()))
                .collect(),
            Family::ServiceSlot => registry
                .services()
                .slots()
                .filter(|slot| slot.name == *name)
                .map(|slot| declaration(Some(&slot.owner), &slot.docs))
                .collect(),
            Family::ScopedPackage => registry
                .declarations()
                .scoped_package(name)
                .map(|package| declaration(Some(&package.owner), &package.docs))
                .into_iter()
                .collect(),
            Family::PresentationElement => registry
                .presentation_elements()
                .filter(|entry| entry.element.name == *name)
                .map(|entry| declaration(Some(&entry.owner), &entry.docs))
                .collect(),
            Family::Piece => declared_pieces(registry)
                .filter(|piece| piece.name == name)
                .map(|piece| {
                    let owner = registry.table_named(piece.table.name()).map(|table| table.def.owner.to_string());
                    declaration(owner.as_deref(), &piece.docs)
                })
                .collect(),
            Family::Style => registry
                .styles()
                .iter()
                .filter(|(_, style)| style.id == name)
                .map(|(owner, style)| declaration(Some(owner), &style.docs))
                .collect(),
            Family::Script => registry
                .scripts()
                .iter()
                .filter(|(_, script)| script.id == name)
                .map(|(owner, script)| declaration(Some(owner), &script.docs))
                .collect(),
            Family::SettingsKey => {
                let mut out = Vec::new();
                for factory in &self.factories {
                    for feature in factory.features() {
                        let Some(decl) = &feature.settings else { continue };
                        let Some(key) = name.strip_prefix(feature.name.as_str()).and_then(|rest| rest.strip_prefix('.')) else {
                            continue;
                        };
                        if !decl.keys().any(|known| known.name == key) {
                            continue;
                        }
                        let description = decl
                            .json_schema()
                            .pointer(&format!("/properties/{key}/description"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .unwrap_or_default();
                        out.push(declaration(Some(&feature.name), &description));
                    }
                }
                out
            }
            Family::ArgumentRole => registry
                .argument_roles()
                .iter()
                .filter(|role| role.name == *name)
                .map(|role| declaration(Some(&role.owner), &role.docs))
                .collect(),
        }
    }

    /// Whether the plan in view schedules the stage `idx`, rules it out, or neither; `None`
    /// without a plan in view.
    fn stage_status(&self, idx: flm_core::core::registry::StageIdx) -> Option<StageStatusView> {
        let plan = self.plan.as_ref()?;
        let workflow = plan.request().workflow.as_ref().map(ToString::to_string);
        if plan.rewrites().contains(&idx) {
            return Some(StageStatusView::Scheduled { workflow, stage: planned_stage(plan, idx, true, None) });
        }
        if let Some(position) = plan.order().iter().position(|scheduled| *scheduled == idx) {
            let batch = plan.batches().iter().position(|range| range.contains(&position));
            return Some(StageStatusView::Scheduled { workflow, stage: planned_stage(plan, idx, false, batch) });
        }
        let name = &self.registry.stage(idx).name;
        Some(match plan.excluded().iter().find(|excluded| excluded.stage == *name) {
            Some(excluded) => StageStatusView::RuledOut {
                workflow,
                by: excluded.excluded_by.to_string(),
                over: excluded.over.to_string(),
            },
            None => StageStatusView::NotScheduled { workflow },
        })
    }
}

/// A declaration by `owner` with the documentation text `docs`, holding nothing else yet.
fn declaration(owner: Option<&str>, docs: &str) -> EntityDeclarationView {
    EntityDeclarationView {
        owner: owner.map(str::to_string),
        docs: docs.to_string(),
        summary: summary_of(docs).to_string(),
        declaration: None,
        values: Vec::new(),
        members: Vec::new(),
        lowers: None,
        refines: None,
        written_by: Vec::new(),
        plan: None,
    }
}

impl EntityDeclarationView {
    fn with_data(mut self, data: Option<Value>) -> EntityDeclarationView {
        self.declaration = data;
        self
    }
}

/// One entity of a namespace, with every declaration of its name, as
/// [`Environment::entity`] answers it.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct EntityView {
    /// The namespace's name, as [`NamespaceView::name`](super::NamespaceView::name) gives it.
    pub namespace: String,
    /// The entity's name.
    pub name: String,
    /// Every declaration of the name in the namespace, in the namespace's order: one, except
    /// for an element type that several features register.
    pub declarations: Vec<EntityDeclarationView>,
}

/// One declaration of an entity.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct EntityDeclarationView {
    /// The feature that declares it, as [`EntryView::owner`](super::EntryView::owner) gives
    /// it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Its documentation text; for a settings key, its description from the settings'
    /// JSON Schema.
    #[serde(skip_serializing_if = "is_empty")]
    pub docs: String,
    /// The first paragraph of [`docs`](EntityDeclarationView::docs).
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
    /// The declaration in the serialized form `flm-core` gives it, for a feature, a table,
    /// a stage, an output, a workflow and an option type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declaration: Option<Value>,
    /// For an element type, the values this declaration registers under the name, in
    /// numbering order, each with the commands written for it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<ElementValueView>,
    /// For a collection, its members, in identifier order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<String>,
    /// For a lowering rule, the element type it lowers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lowers: Option<String>,
    /// For a capability, the capability it refines: providing this one also provides that
    /// one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refines: Option<String>,
    /// For a table, the stages that write it, in registration order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub written_by: Vec<String>,
    /// For a stage, whether the plan in view schedules it; absent without a plan in view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<StageStatusView>,
}

/// Whether the plan in view schedules a stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum StageStatusView {
    /// The plan schedules the stage.
    Scheduled {
        /// The workflow the plan was built from; absent for a plan built from a request made
        /// by hand.
        #[serde(skip_serializing_if = "Option::is_none")]
        workflow: Option<String>,
        /// The stage as the plan schedules it.
        stage: PlannedStageView,
    },
    /// The plan rules the stage out: another stage that provides the same capability was
    /// chosen.
    RuledOut {
        /// The workflow the plan was built from, when there is one.
        #[serde(skip_serializing_if = "Option::is_none")]
        workflow: Option<String>,
        /// The scheduled stage that ruled it out.
        by: String,
        /// A capability both stages provide.
        over: String,
    },
    /// The plan neither schedules nor rules out the stage: nothing the plan needs requires
    /// it.
    NotScheduled {
        /// The workflow the plan was built from, when there is one.
        #[serde(skip_serializing_if = "Option::is_none")]
        workflow: Option<String>,
    },
}
