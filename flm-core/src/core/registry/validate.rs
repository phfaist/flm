//! The checks of the registry build's validate phase, which run over the
//! collected declarations ([`Roster`]) before anything is numbered, and the
//! resolution of the peer declarations.
//!
//! Every check here works on names alone, over the declarations the feature
//! builders recorded. Every problem found is collected, not raised, so that
//! one build reports every problem of the phase at once. The declarations are
//! numbered into the frozen registry only after these checks pass.
//!
//! # The checks of [`validate`]
//!
//! - Duplicate declarations in every namespace: features, presentation
//!   elements (including a presentation element that reuses the name of an
//!   element a document can contain), capabilities, tables, collections,
//!   argument roles, stages, lowering rules, backends, outputs, option types,
//!   workflows, and the peer declarations of one feature. For a peer declaration, only a repeat with
//!   another signature is a duplicate; [`merge_repeated_peers`] has already
//!   merged an identical repeat into the first declaration.
//! - The namespace that capabilities, tables and collections share.
//! - References to entities that no feature declares, from every place where
//!   one can be written: the capability a capability refines; a stage's
//!   provided capabilities and written tables (with the value type each write
//!   expects); a collection member; a rule's element, and what it produces,
//!   requires and reads; the backend in a rule's default mark; a backend's
//!   native elements; an ordering edge's stage and capabilities; an output's
//!   capability and table; a workflow's `extends`, outputs, demands,
//!   preferred stages, the backends of its lowering selections and their
//!   preferred lowering rules, and the backends and elements of its list of
//!   elements accepted as unrenderable; the element of an aspect
//!   attachment; and the role of an argument of an element declaration
//!   (including the arguments of its tack-on fields and of the declarations
//!   of scoped packages), which must be declared by some feature or declared
//!   as a peer by the feature whose declaration holds the argument.
//! - Chains of capability refinements and chains of workflows that lead back
//!   to themselves.
//! - At most one default refinement per capability, and no default
//!   refinement without a parent.
//! - A check stage that declares a write, and any stage that writes an
//!   external table.
//! - A collection where a capability is expected: a collection can be
//!   demanded, never provided.
//! - At most one default lowering rule per element and backend.
//! - An output whose capability is neither its table's capability nor a
//!   refinement of it, and an output whose table does not hold an artifact
//!   set.
//! - A native-set implication whose required element no feature declares.
//! - A part of a style or script definition taken from a table
//!   ([`DefContent::Table`]) that names a table no feature declares, or a
//!   table that does not hold [`DefinitionText`].
//! - A workflow's render option value whose option type no feature declares,
//!   or whose option type cannot be set on a render request.
//!
//! [`validate`] does not report three kinds of reference as undeclared: a
//! backend in a rule's default mark, an element in the rule's `produces`, and
//! a table in the rule's `reads` or `reads_opt`, when the rule's own feature
//! made a [peer declaration](crate::core::registry::PeerName) of that name.
//! [`resolve_peers`] handles those; it runs *before* [`validate`] over the
//! same declarations, resolves every peer declaration against what the
//! features declared, reports each signature mismatch, and marks as set
//! aside (excluded from rule selection) the rules that depend on an
//! unresolved name.
//!
//! Every table a rule reads is checked, whether or not the rule is set aside:
//! the name must be a table that some feature of the registry declares, or a
//! peer declaration of the rule's own feature, and any other name is an
//! undeclared reference. A feature can therefore ship a lowering rule for
//! another feature's backend that reads a third feature's table (for
//! example, a mathematics environment written back to LaTeX with the label
//! keys the references feature assigns) and still be registered alone, while
//! a misspelled table name is still caught wherever it is written.
//!
//! One check runs apart from the others, after the aspects that features
//! attach to other features' elements have been merged into the element
//! values: [`native_implications`], which checks every backend's native set
//! against every declared native-set implication.
//!
//! By then, the build has already dropped, without any diagnostic, every
//! member added to a collection that no feature declares; the checks above
//! check the remaining members.
//!
//! # Checks that run when a plan is built
//!
//! What a stage requires and reads is a need of one plan, not of the
//! registry: one registry can serve a plan in which a requirement can be
//! satisfied and a plan in which it cannot. These checks therefore run when a
//! plan is built: whether every requirement can be satisfied, whether a
//! stage requires a capability that shares an ancestor with one it provides,
//! whether a rewrite stage declares a read of computed data, and whether a
//! stage's requirements list a collection.

use std::any::TypeId;
use std::collections::{BTreeMap, BTreeSet};

use crate::core::artifact::ArtifactSet;
use crate::model::element::ElementSpec;
use crate::model::names::{
    BackendName, CapName, ElementName, FeatureName, Name, RuleName, StageName, WorkflowName,
};
use crate::model::options::{BindSite, OptionKey};
use crate::model::options::TypeKey;
use crate::core::registry::builder::{ArgumentPlace, BuildError, Roster, Site};
use crate::core::registry::declarations::arguments_of;
use crate::core::registry::entities::{DefContent, DefinitionText, NativeImplicationKind};
use crate::core::registry::entities::PeerName;
use crate::core::stage::Effect;
use crate::core::data::table::{TableRef, TableType};
use crate::lang::mint::{ElementDef, SyntaxDef};
use crate::lang::structural::StructuralElements;

/// The kind of declaration that took a name in the namespace that
/// capabilities, tables and collections share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapKind {
    /// A capability declared on its own.
    Explicit,
    /// A table, whose name is also a capability.
    Table,
    /// A collection, which may be demanded but never provided.
    Collection,
}

/// What the checks need to know about one name in the namespace that
/// capabilities, tables and collections share.
struct CapInfo {
    /// The feature that declared it.
    owner: FeatureName,
    /// Which kind of declaration it was.
    kind: CapKind,
    /// The capability it refines, if the declaration gives one.
    refines: Option<CapName>,
    /// Whether it is marked as its parent's default refinement.
    default: bool,
}

/// The command that the element declaration `def` defines, as an author
/// writes it, for a message: `\name` for a macro, `\begin{name}` for an
/// environment (the first of its names), the trigger of a special character
/// sequence, and the element's name for a declaration with no syntax.
fn written_command(def: &ElementDef) -> String {
    let first = def.names.first().map(Name::as_str).unwrap_or_default();
    match &def.syntax {
        SyntaxDef::Macro { .. } | SyntaxDef::Inclusion { .. } => format!("\\{first}"),
        SyntaxDef::Environment { .. } => format!("\\begin{{{first}}}"),
        SyntaxDef::Specials { trigger, .. } => trigger.clone(),
        SyntaxDef::Synthesized => def.element.name.to_string(),
    }
}

/// Adds to `out` the element type name of the declaration `def`'s own
/// element and of each field of each of its tack-on arguments.
fn element_names_of(def: &ElementDef, out: &mut BTreeSet<ElementName>) {
    out.insert(def.element.name.clone());
    let args = match &def.syntax {
        SyntaxDef::Macro { args } | SyntaxDef::Inclusion { args } | SyntaxDef::Specials { args, .. } => args.as_slice(),
        SyntaxDef::Environment { args, .. } => args.as_slice(),
        SyntaxDef::Synthesized => &[],
    };
    for arg in args {
        for field in arg.kind.tack_on_fields().unwrap_or_default() {
            out.insert(field.element().name.clone());
        }
    }
}

/// Every element type name that the declarations `roster` create: the
/// structural element names passed in `structural`, the names of the declared
/// elements and of their fields, of the fallbacks, of the elements of scoped
/// packages, and of the presentation elements.
///
/// A reference to an element type outside this set is a reference to an
/// element that no feature declares.
pub(crate) fn declared_elements(roster: &Roster, structural: &[&str]) -> BTreeSet<ElementName> {
    let mut names: BTreeSet<ElementName> =
        structural.iter().map(|n| ElementName::from(*n)).collect();
    for def in &roster.elements {
        element_names_of(def, &mut names);
    }
    for fallback in &roster.fallbacks {
        names.insert(fallback.element.name.clone());
    }
    for package in &roster.scoped_packages {
        for def in &package.elements {
            element_names_of(def, &mut names);
        }
    }
    for pres in &roster.presentation {
        names.insert(pres.element.name.clone());
    }
    names
}

/// The cycle found while following a chain of parent links, given the chain
/// followed so far (`chain`) and the name that appeared a second time
/// (`repeated`).
///
/// The result starts at the first name of the cycle in name order, so one
/// cycle is always reported the same way, wherever the walk entered it, and
/// two builds produce the same messages.
fn canonical_cycle<N: Clone + Ord>(chain: &[N], repeated: &N) -> Vec<N> {
    let start = chain.iter().position(|name| name == repeated).unwrap_or(0);
    let mut cycle: Vec<N> = chain[start..].to_vec();
    let least = cycle
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.cmp(b))
        .map(|(index, _)| index)
        .unwrap_or(0);
    cycle.rotate_left(least);
    cycle
}

/// Merges into the earlier declaration every peer declaration that repeats
/// it identically (the same feature, the same name and, for an element or a
/// table, the same signature type); the earlier declaration's documentation
/// text is kept.
///
/// A peer declaration registers nothing; it states an expectation, and
/// stating the same expectation twice changes nothing. A function that
/// declares on a feature's behalf may therefore declare every name it needs
/// without checking what the feature declared already. A repeat with another
/// signature stays, and [`validate`] refuses it as
/// [`BuildError::Duplicate`].
///
/// Runs before [`resolve_peers`] and [`validate`], so that each of them sees
/// one entry per declared expectation.
// See the design rationale at [§flm-dd-dr:feat-external-declarations].
pub(crate) fn merge_repeated_peers(roster: &mut Roster) {
    let mut backends: BTreeSet<(FeatureName, BackendName)> = BTreeSet::new();
    roster
        .peer_backends
        .retain(|record| backends.insert((record.owner.clone(), record.name.clone())));
    let mut elements: BTreeSet<(FeatureName, ElementName, TypeId)> = BTreeSet::new();
    roster.peer_elements.retain(|record| {
        elements.insert((record.owner.clone(), record.name.clone(), record.payload.type_id()))
    });
    let mut tables: BTreeSet<(FeatureName, CapName, TypeId)> = BTreeSet::new();
    roster.peer_tables.retain(|record| {
        tables.insert((record.owner.clone(), record.name.clone(), record.value.type_id()))
    });
    let mut packages: BTreeSet<(FeatureName, Name)> = BTreeSet::new();
    roster
        .peer_scoped_packages
        .retain(|record| packages.insert((record.owner.clone(), record.name.clone())));
    let mut roles: BTreeSet<(FeatureName, Name)> = BTreeSet::new();
    roster
        .peer_argument_roles
        .retain(|record| roles.insert((record.owner.clone(), record.name.clone())));
}

/// Resolves every peer declaration of an element value
/// ([`FeatureBuilder::peer_element_value`](crate::core::registry::FeatureBuilder::peer_element_value))
/// against the element types that the other declarations of `roster` create
/// ([`declared_elements`], with `structural` giving the names of the
/// language's structural elements): the plain element declarations, the
/// elements of scoped packages, the fallbacks, the presentation elements and
/// the fields, without the peer declarations themselves. A declaration is
/// resolved when its element type's name is among them.
///
/// Each record is marked as resolved or not, and the unresolved declarations
/// are removed from [`Roster::elements`], so that no later step sees them;
/// the resolved ones stay where they were declared.
///
/// Returns each pair of feature and element type left unresolved, once, in
/// declaration order.
// See decision rationale at [§flm-dd-dr:feat-peer-element-value].
pub(crate) fn resolve_peer_element_values(
    roster: &mut Roster,
    structural: &[&str],
) -> Vec<(FeatureName, ElementName)> {
    let peers: BTreeSet<usize> = roster.peer_element_values.iter().map(|record| record.position).collect();
    // The peer declarations are set apart while the declared types are read,
    // then put back in their places.
    let mut all: Vec<Option<ElementDef>> = std::mem::take(&mut roster.elements).into_iter().map(Some).collect();
    let mut set_apart: BTreeMap<usize, ElementDef> = BTreeMap::new();
    for position in &peers {
        if let Some(def) = all.get_mut(*position).and_then(Option::take) {
            set_apart.insert(*position, def);
        }
    }
    roster.elements = all.iter_mut().filter_map(Option::take).collect();
    let declared = declared_elements(roster, structural);
    let mut unresolved: Vec<(FeatureName, ElementName)> = Vec::new();
    for record in &mut roster.peer_element_values {
        record.resolved = declared.contains(&record.element);
        if record.resolved {
            continue;
        }
        set_apart.remove(&record.position);
        let key = (record.owner.clone(), record.element.clone());
        if !unresolved.contains(&key) {
            unresolved.push(key);
        }
    }
    let plain = std::mem::take(&mut roster.elements);
    let mut plain = plain.into_iter();
    for position in 0..all.len() {
        if peers.contains(&position) {
            roster.elements.extend(set_apart.remove(&position));
        } else {
            roster.elements.extend(plain.next());
        }
    }
    unresolved
}

/// Runs every check listed in the [module documentation](self) over the
/// collected declarations `roster`, with `structural` giving the names of the
/// language's structural elements.
///
/// Returns every problem found, in the order found; an empty result means
/// that the declarations can be numbered into the frozen registry.
pub(crate) fn validate(roster: &Roster, structural: &[&str]) -> Vec<BuildError> {
    let mut errors: Vec<BuildError> = Vec::new();

    // --- features -------------------------------------------------------------------
    let mut seen_features: BTreeSet<&FeatureName> = BTreeSet::new();
    for info in &roster.features {
        if !seen_features.insert(&info.name) {
            errors.push(BuildError::DuplicateFeature { feature: info.name.clone() });
        }
    }

    // --- the capability name space ------------------------------------------------
    let mut caps: BTreeMap<CapName, CapInfo> = BTreeMap::new();
    for (owner, def) in &roster.capabilities {
        if let Some(existing) = caps.get(&def.name) {
            errors.push(BuildError::Duplicate {
                kind: "capability",
                name: def.name.name().clone(),
                first: existing.owner.clone(),
                second: owner.clone(),
            });
            continue;
        }
        caps.insert(
            def.name.clone(),
            CapInfo {
                owner: owner.clone(),
                kind: CapKind::Explicit,
                refines: def.refines.clone(),
                default: def.default,
            },
        );
    }
    let mut tables: BTreeMap<CapName, (FeatureName, TableRef, bool)> = BTreeMap::new();
    for record in &roster.tables {
        let def = &record.def;
        if let Some((first, _, _)) = tables.get(&def.name) {
            errors.push(BuildError::Duplicate {
                kind: "table",
                name: def.name.name().clone(),
                first: first.clone(),
                second: def.owner.clone(),
            });
            continue;
        }
        if let Some(existing) = caps.get(&def.name) {
            if existing.kind == CapKind::Explicit {
                errors.push(BuildError::TableCapabilityClash {
                    name: def.name.clone(),
                    table_owner: def.owner.clone(),
                    capability_owner: existing.owner.clone(),
                });
                continue;
            }
        }
        tables.insert(
            def.name.clone(),
            (
                def.owner.clone(),
                TableRef {
                    name: def.name.clone(),
                    type_id: def.ty.type_id(),
                    type_name: def.ty.type_name(),
                },
                record.external,
            ),
        );
        caps.insert(
            def.name.clone(),
            CapInfo {
                owner: def.owner.clone(),
                kind: CapKind::Table,
                refines: record.refines.clone(),
                default: record.default,
            },
        );
    }
    for (owner, def) in &roster.collections {
        if let Some(existing) = caps.get(&def.name) {
            errors.push(BuildError::Duplicate {
                kind: if existing.kind == CapKind::Collection {
                    "collection"
                } else {
                    "capability"
                },
                name: def.name.name().clone(),
                first: existing.owner.clone(),
                second: owner.clone(),
            });
            continue;
        }
        caps.insert(
            def.name.clone(),
            CapInfo {
                owner: owner.clone(),
                kind: CapKind::Collection,
                refines: None,
                default: false,
            },
        );
    }
    let cap_roster: Vec<Name> = caps
        .iter()
        .filter(|(_, info)| info.kind != CapKind::Collection)
        .map(|(name, _)| name.name().clone())
        .collect();
    let collection_roster: Vec<Name> = caps
        .iter()
        .filter(|(_, info)| info.kind == CapKind::Collection)
        .map(|(name, _)| name.name().clone())
        .collect();
    let table_roster: Vec<Name> = tables.keys().map(|name| name.name().clone()).collect();
    let is_cap = |name: &CapName| {
        caps.get(name).map(|info| info.kind != CapKind::Collection).unwrap_or(false)
    };
    let is_collection = |name: &CapName| {
        caps.get(name).map(|info| info.kind == CapKind::Collection).unwrap_or(false)
    };

    let undeclared_cap =
        |feature: &FeatureName, site: Site, name: &CapName, errors: &mut Vec<BuildError>| {
            if !is_cap(name) {
                errors.push(BuildError::Undeclared {
                    feature: feature.clone(),
                    kind: "capability",
                    site,
                    name: name.name().clone(),
                    roster: cap_roster.clone(),
                });
            }
        };
    let check_table = |feature: &FeatureName,
                       site: Site,
                       table: &TableRef,
                       errors: &mut Vec<BuildError>|
     -> bool {
        match tables.get(&table.name) {
            None => {
                errors.push(BuildError::Undeclared {
                    feature: feature.clone(),
                    kind: "table",
                    site,
                    name: table.name.name().clone(),
                    roster: table_roster.clone(),
                });
                false
            }
            Some((_, declared, _)) if declared.type_id != table.type_id => {
                errors.push(BuildError::TableTypeMismatch {
                    feature: feature.clone(),
                    site,
                    table: table.name.clone(),
                    declared: declared.short_type_name(),
                    referenced: table.short_type_name(),
                });
                false
            }
            Some(_) => true,
        }
    };

    // Refinement chains: parents declared and not collections, no cycles, one
    // default per parent, default only with a parent.
    let mut defaults: BTreeMap<CapName, CapName> = BTreeMap::new();
    for (name, info) in &caps {
        match &info.refines {
            Some(parent) => {
                if !is_cap(parent) {
                    errors.push(BuildError::Undeclared {
                        feature: info.owner.clone(),
                        kind: "capability",
                        site: Site::CapabilityRefines { capability: name.clone() },
                        name: parent.name().clone(),
                        roster: cap_roster.clone(),
                    });
                } else if info.default {
                    if let Some(first) = defaults.get(parent) {
                        errors.push(BuildError::TwoDefaults {
                            parent: parent.clone(),
                            first: first.clone(),
                            second: name.clone(),
                        });
                    } else {
                        defaults.insert(parent.clone(), name.clone());
                    }
                }
            }
            None if info.default => {
                errors.push(BuildError::DefaultWithoutParent { name: name.clone() })
            }
            None => {}
        }
    }
    let mut cycles: BTreeSet<Vec<CapName>> = BTreeSet::new();
    for name in caps.keys() {
        let mut chain = vec![name.clone()];
        let mut seen: BTreeSet<CapName> = BTreeSet::new();
        seen.insert(name.clone());
        let mut current = name.clone();
        while let Some(Some(parent)) = caps.get(&current).map(|info| info.refines.clone()) {
            if !is_cap(&parent) {
                break;
            }
            if !seen.insert(parent.clone()) {
                cycles.insert(canonical_cycle(&chain, &parent));
                break;
            }
            chain.push(parent.clone());
            current = parent;
        }
    }
    for cycle in cycles {
        errors.push(BuildError::RefinesCycle { cycle });
    }

    // --- elements -------------------------------------------------------------------
    let elements = declared_elements(roster, structural);
    let element_roster: Vec<Name> = elements.iter().map(|name| name.name().clone()).collect();
    let undeclared_element =
        |feature: &FeatureName, site: Site, name: &ElementName, errors: &mut Vec<BuildError>| {
            if !elements.contains(name) {
                errors.push(BuildError::Undeclared {
                    feature: feature.clone(),
                    kind: "element",
                    site,
                    name: name.name().clone(),
                    roster: element_roster.clone(),
                });
            }
        };
    // Every source-side element name with the feature that first declared it,
    // so a presentation element reusing such a name names both owners.
    let mut source_owners: BTreeMap<ElementName, FeatureName> = BTreeMap::new();
    {
        let mut note = |name: ElementName, owner: &FeatureName| {
            source_owners.entry(name).or_insert_with(|| owner.clone());
        };
        let language_owner = FeatureName::from(crate::lang::mint::LANGUAGE_OWNER);
        for name in structural {
            note(ElementName::from(*name), &language_owner);
        }
        let of_def = |def: &ElementDef, note: &mut dyn FnMut(ElementName, &FeatureName)| {
            let mut names: BTreeSet<ElementName> = BTreeSet::new();
            element_names_of(def, &mut names);
            for name in names {
                note(name, &def.owner);
            }
        };
        for def in &roster.elements {
            of_def(def, &mut note);
        }
        for fallback in &roster.fallbacks {
            note(fallback.element.name.clone(), &fallback.owner);
        }
        for package in &roster.scoped_packages {
            for def in &package.elements {
                of_def(def, &mut note);
            }
        }
    }
    let mut seen_pres: BTreeMap<ElementName, FeatureName> = BTreeMap::new();
    for pres in &roster.presentation {
        let name = &pres.element.name;
        if let Some(first) = seen_pres.get(name) {
            errors.push(BuildError::Duplicate {
                kind: "presentation element",
                name: name.name().clone(),
                first: first.clone(),
                second: pres.owner.clone(),
            });
            continue;
        }
        if let Some(first) = source_owners.get(name) {
            errors.push(BuildError::Duplicate {
                kind: "element",
                name: name.name().clone(),
                first: first.clone(),
                second: pres.owner.clone(),
            });
        }
        seen_pres.insert(name.clone(), pres.owner.clone());
    }
    for attachment in &roster.attachments {
        undeclared_element(
            &attachment.owner,
            Site::Attachment { aspect: attachment.aspect_name },
            &attachment.element,
            &mut errors,
        );
    }

    // --- argument roles ----------------------------------------------------------------
    // A role name is declared once in its namespace. A role set on an argument
    // must be declared by some feature, or declared as a peer by the feature
    // whose declaration holds the argument: the rule every other namespace
    // follows for a reference to another feature's entity.
    // See decision rationale at [§flm-dd-dr:arg-role].
    let mut role_owners: BTreeMap<&Name, &FeatureName> = BTreeMap::new();
    for (owner, def) in &roster.argument_roles {
        if let Some(first) = role_owners.get(&def.name) {
            errors.push(BuildError::Duplicate {
                kind: "argument role",
                name: def.name.clone(),
                first: (*first).clone(),
                second: owner.clone(),
            });
            continue;
        }
        role_owners.insert(&def.name, owner);
    }
    let peer_roles: BTreeSet<(&FeatureName, &Name)> =
        roster.peer_argument_roles.iter().map(|record| (&record.owner, &record.name)).collect();
    let role_roster: Vec<Name> = role_owners.keys().map(|name| (*name).clone()).collect();
    let mut arguments = Vec::new();
    for def in &roster.elements {
        arguments_of(def, None, &mut arguments);
    }
    for package in &roster.scoped_packages {
        for def in &package.elements {
            arguments_of(def, Some(package), &mut arguments);
        }
    }
    for argument in &arguments {
        let Some(role) = &argument.arg.role else { continue };
        let owner = &argument.declaration.owner;
        if role_owners.contains_key(role) || peer_roles.contains(&(owner, role)) {
            continue;
        }
        errors.push(BuildError::UndeclaredArgumentRole {
            feature: owner.clone(),
            place: Box::new(ArgumentPlace {
                command: written_command(argument.declaration),
                field: argument.field.map(|field| field.name.clone()),
                argument: argument.arg.name.clone(),
                index: argument.index,
            }),
            role: role.clone(),
            roster: role_roster.clone(),
        });
    }

    // --- collections' members -------------------------------------------------------
    // A member of a collection no feature declares was already dropped by the
    // build module's `drop_members_of_undeclared_collections`; what remains is
    // checked for its capability. See the design rationale at
    // [§flm-dd-dr:feat-undeclared-collection].
    for (owner, collection, member) in &roster.members {
        undeclared_cap(
            owner,
            Site::CollectionMember { collection: collection.clone(), member: member.clone() },
            member,
            &mut errors,
        );
    }

    // --- stages ---------------------------------------------------------------------
    let mut stage_owners: BTreeMap<Name, FeatureName> = BTreeMap::new();
    for record in &roster.stages {
        let decl = &record.decl;
        if let Some(first) = stage_owners.get(decl.name.name()) {
            errors.push(BuildError::Duplicate {
                kind: "stage",
                name: decl.name.name().clone(),
                first: first.clone(),
                second: record.owner.clone(),
            });
            continue;
        }
        stage_owners.insert(decl.name.name().clone(), record.owner.clone());
        for capability in &decl.provides {
            if is_collection(capability) {
                errors.push(BuildError::CollectionAsCapability {
                    stage: decl.name.clone(),
                    site: Site::StageProvides { stage: decl.name.clone() },
                    collection: capability.clone(),
                });
                continue;
            }
            undeclared_cap(
                &record.owner,
                Site::StageProvides { stage: decl.name.clone() },
                capability,
                &mut errors,
            );
        }
        for table in &decl.writes {
            if decl.effect == Effect::Check {
                errors.push(BuildError::CheckWrites {
                    stage: decl.name.clone(),
                    table: table.name.clone(),
                });
            }
            if check_table(
                &record.owner,
                Site::StageWrites { stage: decl.name.clone() },
                table,
                &mut errors,
            ) {
                if let Some((_, _, true)) = tables.get(&table.name) {
                    errors.push(BuildError::ExternalTableWritten {
                        stage: decl.name.clone(),
                        table: table.name.clone(),
                    });
                }
            }
        }
    }
    let stage_roster: Vec<Name> = stage_owners.keys().cloned().collect();
    for (owner, ordering) in &roster.orderings {
        if !stage_owners.contains_key(ordering.stage.name()) {
            errors.push(BuildError::Undeclared {
                feature: owner.clone(),
                kind: "stage",
                site: Site::OrderingEdges,
                name: ordering.stage.name().clone(),
                roster: stage_roster.clone(),
            });
        }
        for capability in ordering.after.iter().chain(&ordering.before) {
            undeclared_cap(
                owner,
                Site::OrderingEdge { stage: ordering.stage.clone() },
                capability,
                &mut errors,
            );
        }
    }

    // --- backends ---------------------------------------------------------------------
    let mut backend_owners: BTreeMap<BackendName, FeatureName> = BTreeMap::new();
    for backend in &roster.backends {
        if let Some(first) = backend_owners.get(&backend.name) {
            errors.push(BuildError::Duplicate {
                kind: "backend",
                name: backend.name.name().clone(),
                first: first.clone(),
                second: backend.owner.clone(),
            });
            continue;
        }
        backend_owners.insert(backend.name.clone(), backend.owner.clone());
        for element in &backend.native {
            undeclared_element(
                &backend.owner,
                Site::BackendNative { backend: backend.name.clone() },
                element,
                &mut errors,
            );
        }
    }
    let backend_roster: Vec<Name> = backend_owners.keys().map(|name| name.name().clone()).collect();

    // --- native-set implications ------------------------------------------------------
    for (owner, implication) in &roster.native_implications {
        undeclared_element(owner, Site::NativeImplication, &implication.must_claim, &mut errors);
    }

    // --- peer declarations ------------------------------------------------------------
    // A declaration counts for its own feature alone, so the namespace is keyed by
    // (feature, name): one feature declaring a name twice under two signatures is a
    // duplicate, as in every other namespace (an identical repeat was merged by
    // `merge_repeated_peers`); two features each declaring the same name are not. A
    // rule's reference to a name its own feature declared is left to `resolve_peers`.
    // See the design rationale at [§flm-dd-dr:feat-external-declarations].
    let mut peer_backends: BTreeSet<(&FeatureName, &BackendName)> = BTreeSet::new();
    for record in &roster.peer_backends {
        if !peer_backends.insert((&record.owner, &record.name)) {
            errors.push(BuildError::Duplicate {
                kind: "peer backend",
                name: record.name.name().clone(),
                first: record.owner.clone(),
                second: record.owner.clone(),
            });
        }
    }
    let mut peer_elements: BTreeSet<(&FeatureName, &ElementName)> = BTreeSet::new();
    for record in &roster.peer_elements {
        if !peer_elements.insert((&record.owner, &record.name)) {
            errors.push(BuildError::Duplicate {
                kind: "peer element",
                name: record.name.name().clone(),
                first: record.owner.clone(),
                second: record.owner.clone(),
            });
        }
    }
    let mut peer_tables: BTreeSet<(&FeatureName, &CapName)> = BTreeSet::new();
    for record in &roster.peer_tables {
        if !peer_tables.insert((&record.owner, &record.name)) {
            errors.push(BuildError::Duplicate {
                kind: "peer table",
                name: record.name.name().clone(),
                first: record.owner.clone(),
                second: record.owner.clone(),
            });
        }
    }
    let mut peer_packages: BTreeSet<(&FeatureName, &Name)> = BTreeSet::new();
    for record in &roster.peer_scoped_packages {
        if !peer_packages.insert((&record.owner, &record.name)) {
            errors.push(BuildError::Duplicate {
                kind: "peer scoped package",
                name: record.name.clone(),
                first: record.owner.clone(),
                second: record.owner.clone(),
            });
        }
    }

    // --- rules ----------------------------------------------------------------------
    let mut rule_owners: BTreeMap<RuleName, FeatureName> = BTreeMap::new();
    let mut general_defaults: BTreeMap<ElementName, RuleName> = BTreeMap::new();
    let mut specific_defaults: BTreeMap<(ElementName, BackendName), RuleName> = BTreeMap::new();
    for (owner, rule) in &roster.rules {
        if let Some(first) = rule_owners.get(&rule.name) {
            errors.push(BuildError::Duplicate {
                kind: "lowering rule",
                name: rule.name.name().clone(),
                first: first.clone(),
                second: owner.clone(),
            });
            continue;
        }
        rule_owners.insert(rule.name.clone(), owner.clone());
        undeclared_element(
            owner,
            Site::RuleLowers { rule: rule.name.clone() },
            &rule.element,
            &mut errors,
        );
        for produced in &rule.decl.produces {
            if peer_elements.contains(&(owner, produced)) {
                continue;
            }
            undeclared_element(
                owner,
                Site::RuleProduces { rule: rule.name.clone() },
                produced,
                &mut errors,
            );
        }
        for capability in &rule.decl.requires {
            undeclared_cap(
                owner,
                Site::RuleRequires { rule: rule.name.clone() },
                capability,
                &mut errors,
            );
        }
        // Every table a rule reads is checked, set aside or not: a table this
        // registry declares must hold what the rule expects, and a table it
        // does not declare must be a peer declaration of the rule's own
        // feature — which is how a rule reads another feature's table in a
        // registry that does not hold that feature. Anything else is a
        // misspelled name, reported here.
        // See the design rationale at [§flm-dd-dr:feat-external-declarations].
        for table in rule.decl.reads.iter().chain(&rule.decl.reads_opt) {
            if peer_tables.contains(&(owner, &table.name)) && !tables.contains_key(&table.name) {
                continue;
            }
            check_table(owner, Site::RuleReads { rule: rule.name.clone() }, table, &mut errors);
        }
        match &rule.default {
            crate::core::lowering::DefaultMark::Everywhere => {
                if let Some(first) = general_defaults.get(&rule.element) {
                    errors.push(BuildError::TwoDefaultRules {
                        element: rule.element.clone(),
                        backend: None,
                        first: first.clone(),
                        second: rule.name.clone(),
                    });
                } else {
                    general_defaults.insert(rule.element.clone(), rule.name.clone());
                }
            }
            crate::core::lowering::DefaultMark::OnBackends(backends) => {
                for backend in backends {
                    if !backend_owners.contains_key(backend)
                        && !peer_backends.contains(&(owner, backend))
                    {
                        errors.push(BuildError::Undeclared {
                            feature: owner.clone(),
                            kind: "backend",
                            site: Site::RuleDefaultMark { rule: rule.name.clone() },
                            name: backend.name().clone(),
                            roster: backend_roster.clone(),
                        });
                        continue;
                    }
                    let key = (rule.element.clone(), backend.clone());
                    if let Some(first) = specific_defaults.get(&key) {
                        errors.push(BuildError::TwoDefaultRules {
                            element: rule.element.clone(),
                            backend: Some(backend.clone()),
                            first: first.clone(),
                            second: rule.name.clone(),
                        });
                    } else {
                        specific_defaults.insert(key, rule.name.clone());
                    }
                }
            }
            crate::core::lowering::DefaultMark::Never => {}
        }
    }
    let rule_roster: Vec<Name> = rule_owners.keys().map(|name| name.name().clone()).collect();

    // --- outputs ----------------------------------------------------------------------
    let refines_or_is = |capability: &CapName, table: &CapName| -> bool {
        let mut current = capability.clone();
        let mut seen: BTreeSet<CapName> = BTreeSet::new();
        loop {
            if &current == table {
                return true;
            }
            if !seen.insert(current.clone()) {
                return false;
            }
            match caps.get(&current).and_then(|info| info.refines.clone()) {
                Some(parent) => current = parent,
                None => return false,
            }
        }
    };
    let mut output_owners: BTreeMap<Name, FeatureName> = BTreeMap::new();
    for (owner, output) in &roster.outputs {
        if let Some(first) = output_owners.get(output.name.name()) {
            errors.push(BuildError::Duplicate {
                kind: "output",
                name: output.name.name().clone(),
                first: first.clone(),
                second: owner.clone(),
            });
            continue;
        }
        output_owners.insert(output.name.name().clone(), owner.clone());
        undeclared_cap(
            owner,
            Site::OutputCapability { output: output.name.clone() },
            &output.capability,
            &mut errors,
        );
        match tables.get(&output.table) {
            None => errors.push(BuildError::Undeclared {
                feature: owner.clone(),
                kind: "table",
                site: Site::OutputTable { output: output.name.clone() },
                name: output.table.name().clone(),
                roster: table_roster.clone(),
            }),
            Some((_, declared, _)) => {
                if declared.type_id != TypeId::of::<ArtifactSet>() {
                    errors.push(BuildError::OutputTableNotArtifactSet {
                        output: output.name.clone(),
                        table: output.table.clone(),
                        ty: declared.short_type_name(),
                    });
                }
                if is_cap(&output.capability) && !refines_or_is(&output.capability, &output.table) {
                    errors.push(BuildError::OutputCapabilityMismatch {
                        output: output.name.clone(),
                        capability: output.capability.clone(),
                        table: output.table.clone(),
                    });
                }
            }
        }
    }
    let output_roster: Vec<Name> = output_owners.keys().cloned().collect();
    for (owner, output) in &roster.outputs {
        for source in &output.made_from {
            if !output_owners.contains_key(source.name()) {
                errors.push(BuildError::Undeclared {
                    feature: owner.clone(),
                    kind: "output",
                    site: Site::OutputMadeFrom { output: output.name.clone() },
                    name: source.name().clone(),
                    roster: output_roster.clone(),
                });
            }
        }
    }

    // --- the parts of style and script definitions taken from a table -----------------
    let styles = roster.styles.iter().map(|(_, def)| ("style", &def.id, &def.content));
    let scripts = roster.scripts.iter().map(|(_, def)| ("script", &def.id, &def.content));
    for (kind, definition, content) in styles.chain(scripts) {
        for part in content {
            let DefContent::Table(table) = part else { continue };
            let found = match tables.get(table) {
                None => None,
                Some((_, declared, _)) if declared.type_id == TypeId::of::<DefinitionText>() => continue,
                Some((_, declared, _)) => Some(declared.short_type_name()),
            };
            errors.push(BuildError::DefinitionPartTable {
                kind,
                definition: definition.clone(),
                table: table.clone(),
                found,
            });
        }
    }

    // --- options ----------------------------------------------------------------------
    let mut option_decls: BTreeMap<OptionKey, &crate::model::options::OptionDecl> = BTreeMap::new();
    for decl in &roster.options {
        if let Some(first) = option_decls.get(&decl.key) {
            errors.push(BuildError::Duplicate {
                kind: "option type",
                name: Name::from(decl.key.short_name()),
                first: first.owner.clone(),
                second: decl.owner.clone(),
            });
            continue;
        }
        option_decls.insert(decl.key.clone(), decl);
    }
    let option_roster: Vec<OptionKey> = option_decls.keys().cloned().collect();

    // --- workflows --------------------------------------------------------------------
    let mut workflows: BTreeMap<WorkflowName, (FeatureName, Option<WorkflowName>)> =
        BTreeMap::new();
    for (owner, workflow) in &roster.workflows {
        if let Some((first, _)) = workflows.get(&workflow.name) {
            errors.push(BuildError::Duplicate {
                kind: "workflow",
                name: workflow.name.name().clone(),
                first: first.clone(),
                second: owner.clone(),
            });
            continue;
        }
        workflows.insert(workflow.name.clone(), (owner.clone(), workflow.extends.clone()));
    }
    let workflow_roster: Vec<Name> = workflows.keys().map(|name| name.name().clone()).collect();
    for (owner, workflow) in &roster.workflows {
        if let Some(parent) = &workflow.extends {
            if !workflows.contains_key(parent) {
                errors.push(BuildError::Undeclared {
                    feature: owner.clone(),
                    kind: "workflow",
                    site: Site::WorkflowExtends { workflow: workflow.name.clone() },
                    name: parent.name().clone(),
                    roster: workflow_roster.clone(),
                });
            }
        }
        for output in &workflow.outputs {
            if !output_owners.contains_key(output.name()) {
                errors.push(BuildError::Undeclared {
                    feature: owner.clone(),
                    kind: "output",
                    site: Site::WorkflowOutputs { workflow: workflow.name.clone() },
                    name: output.name().clone(),
                    roster: output_roster.clone(),
                });
            }
        }
        for demand in &workflow.demands {
            if !caps.contains_key(demand) {
                errors.push(BuildError::Undeclared {
                    feature: owner.clone(),
                    kind: "capability",
                    site: Site::WorkflowDemands { workflow: workflow.name.clone() },
                    name: demand.name().clone(),
                    roster: cap_roster.iter().chain(&collection_roster).cloned().collect(),
                });
            }
        }
        for stage in &workflow.prefer {
            if !stage_owners.contains_key(stage.name()) {
                errors.push(BuildError::Undeclared {
                    feature: owner.clone(),
                    kind: "stage",
                    site: Site::WorkflowPrefers { workflow: workflow.name.clone() },
                    name: stage.name().clone(),
                    roster: stage_roster.clone(),
                });
            }
        }
        for (backend, selection) in &workflow.lowering {
            if !backend_owners.contains_key(backend) {
                errors.push(BuildError::Undeclared {
                    feature: owner.clone(),
                    kind: "backend",
                    site: Site::WorkflowLoweringSelection { workflow: workflow.name.clone() },
                    name: backend.name().clone(),
                    roster: backend_roster.clone(),
                });
            }
            for rule in &selection.prefer {
                if !rule_owners.contains_key(rule) {
                    errors.push(BuildError::Undeclared {
                        feature: owner.clone(),
                        kind: "lowering rule",
                        site: Site::WorkflowLoweringPreference { workflow: workflow.name.clone() },
                        name: rule.name().clone(),
                        roster: rule_roster.clone(),
                    });
                }
            }
        }
        for (backend, holes) in &workflow.unsupported {
            if !backend_owners.contains_key(backend) {
                errors.push(BuildError::Undeclared {
                    feature: owner.clone(),
                    kind: "backend",
                    site: Site::WorkflowUnsupported { workflow: workflow.name.clone() },
                    name: backend.name().clone(),
                    roster: backend_roster.clone(),
                });
            }
            for element in holes {
                undeclared_element(
                    owner,
                    Site::WorkflowUnsupported { workflow: workflow.name.clone() },
                    element,
                    &mut errors,
                );
            }
        }
        for error in crate::model::options::validate_values(
            &workflow.render_options,
            BindSite::Render,
            |key| option_decls.get(key).copied(),
            &option_roster,
            Some(&workflow.name),
        ) {
            errors.push(BuildError::Feature { feature: owner.clone(), detail: error.to_string() });
        }
    }
    let mut extends_cycles: BTreeSet<Vec<WorkflowName>> = BTreeSet::new();
    for name in workflows.keys() {
        let mut chain = vec![name.clone()];
        let mut seen: BTreeSet<WorkflowName> = BTreeSet::new();
        seen.insert(name.clone());
        let mut current = name.clone();
        while let Some(Some(parent)) = workflows.get(&current).map(|(_, parent)| parent.clone()) {
            if !workflows.contains_key(&parent) {
                break;
            }
            if !seen.insert(parent.clone()) {
                extends_cycles.insert(canonical_cycle(&chain, &parent));
                break;
            }
            chain.push(parent.clone());
            current = parent;
        }
    }
    for cycle in extends_cycles {
        errors.push(BuildError::ExtendsCycle { cycle });
    }

    errors
}

/// Adds to `out`, under its element type name, the element value of the
/// declaration `def`'s own element and of each field of each of its tack-on
/// arguments.
fn element_specs_of<'a>(
    def: &'a ElementDef,
    out: &mut BTreeMap<ElementName, Vec<&'a ElementSpec>>,
) {
    out.entry(def.element.name.clone()).or_default().push(&def.element);
    let args = match &def.syntax {
        SyntaxDef::Macro { args } | SyntaxDef::Inclusion { args } | SyntaxDef::Specials { args, .. } => args.as_slice(),
        SyntaxDef::Environment { args, .. } => args.as_slice(),
        SyntaxDef::Synthesized => &[],
    };
    for arg in args {
        for field in arg.kind.tack_on_fields().unwrap_or_default() {
            out.entry(field.element().name.clone()).or_default().push(field.element());
        }
    }
}

/// The check that every backend's native set satisfies every declared
/// [native-set implication](crate::core::registry::NativeImplication).
///
/// It runs after the aspects that features attach to other features'
/// elements have been merged into the element values, so that an aspect one
/// feature attaches to another feature's element counts; `structural` holds
/// the structural elements with their merged aspects.
///
/// For each backend and each implication, both in declaration order: nothing
/// is reported when the backend's native set holds the required element;
/// otherwise one [`BuildError::NativeSetIncomplete`] is reported if the
/// implication applies to every backend, or if any element of the backend's
/// native set carries the implication's aspect, and the first such element in
/// the native set's declared order is given as the reason. A native element
/// name that several element values share makes the implication apply if any
/// of those values carries the aspect. An implication whose required element
/// no feature declares is skipped here: [`validate`] has already reported it
/// once as [`BuildError::Undeclared`], and no backend could hold the element
/// anyway.
pub(crate) fn native_implications(
    roster: &Roster,
    structural: &StructuralElements,
) -> Vec<BuildError> {
    if roster.native_implications.is_empty() {
        return Vec::new();
    }
    // The element values, under their names, from the same five sources in the
    // same order as `declared_elements` above, which gathers the names: a source
    // of element values added to one must be added to the other, and the key set
    // of this map is exactly the set `declared_elements` returns.
    let mut specs: BTreeMap<ElementName, Vec<&ElementSpec>> = BTreeMap::new();
    for element in structural.all() {
        specs.entry(element.name.clone()).or_default().push(element);
    }
    for def in &roster.elements {
        element_specs_of(def, &mut specs);
    }
    for fallback in &roster.fallbacks {
        specs.entry(fallback.element.name.clone()).or_default().push(&fallback.element);
    }
    for package in &roster.scoped_packages {
        for def in &package.elements {
            element_specs_of(def, &mut specs);
        }
    }
    for pres in &roster.presentation {
        specs.entry(pres.element.name.clone()).or_default().push(&pres.element);
    }

    let mut errors: Vec<BuildError> = Vec::new();
    for backend in &roster.backends {
        let claimed: BTreeSet<&ElementName> = backend.native.iter().collect();
        for (_, implication) in &roster.native_implications {
            if claimed.contains(&implication.must_claim) {
                continue;
            }
            if !specs.contains_key(&implication.must_claim) {
                continue;
            }
            let because = match implication.kind {
                NativeImplicationKind::Always => None,
                NativeImplicationKind::Aspect { .. } => {
                    let carries = |spec: &&ElementSpec| implication.applies_to(spec);
                    let carrier = backend.native.iter().find(|name| {
                        specs.get(*name).is_some_and(|values| values.iter().any(carries))
                    });
                    match carrier {
                        Some(name) => Some(name.clone()),
                        None => continue,
                    }
                }
            };
            errors.push(BuildError::NativeSetIncomplete {
                backend: backend.name.clone(),
                because,
                must_claim: implication.must_claim.clone(),
            });
        }
    }
    errors
}

/// The result of [`resolve_peers`]: what resolving every peer declaration
/// against the collected declarations found.
pub(crate) struct PeerResolution {
    /// The signature mismatches: one [`BuildError::PeerElementMismatch`] per
    /// peer element declaration whose name is declared with another payload
    /// type or as a source element, and one [`BuildError::PeerTableMismatch`]
    /// per peer table declaration whose name is declared holding another value
    /// type.
    pub errors: Vec<BuildError>,
    /// Every peer declaration that no declaration resolves, in declaration
    /// order: the declaring feature, the name, and the rules of that feature
    /// set aside because they depend on it, in registration order. There is
    /// one entry per feature and name, because identical repeats were merged
    /// and [`validate`] refuses other repeats.
    pub unresolved: Vec<(FeatureName, PeerName, Vec<RuleName>)>,
    /// Every rule set aside, keyed by its name, with the unresolved peer
    /// declarations of its own feature that it depends on, in the order of its
    /// default mark, then its `produces`, then its `reads`.
    pub set_aside: BTreeMap<RuleName, Vec<PeerName>>,
    /// Every stage set aside, keyed by its name: a stage that writes a table
    /// that its own feature declared as a peer and that stays unresolved. Each
    /// comes with the feature that registered it and the unresolved peer
    /// tables it writes, in the order of its declaration. A stage set aside is
    /// not registered.
    pub set_aside_stages: BTreeMap<StageName, (FeatureName, Vec<PeerName>)>,
}

/// Resolves every [peer declaration](crate::core::registry::PeerName) of the
/// collected declarations `roster` against what the features declared, with
/// `structural` giving the names of the language's structural elements.
///
/// A peer backend resolves when some feature registers a backend of that name.
/// A peer element resolves when some feature registers a presentation element
/// of that name whose payload type is the one the declaration gives; a
/// presentation element of another payload type, or a source element of that
/// name, is a mismatch reported as an error. A peer table resolves when some
/// feature declares a table of that name holding the value type the
/// declaration gives; one holding another type is a mismatch reported as an
/// error. A peer scoped package resolves when some feature declares a scoped
/// package of that name, and a peer argument role when some feature declares
/// an argument role of that name.
///
/// A name that resolves to nothing is unresolved, and every rule of the
/// declaring feature that *depends* on it is set aside: a rule whose default
/// mark lists the backend, whose `produces` lists the element, or whose
/// `reads` lists the table. A rule that lists the table only in `reads_opt`
/// is not set aside (it runs and finds the table absent), but the unresolved
/// declaration is still reported.
///
/// A stage of the declaring feature that *writes* an unresolved peer table is
/// set aside too: without the table it has nowhere to write, and the feature
/// that would read the table is absent.
///
/// A declaration counts for its own feature only: [`validate`] reports a rule
/// of another feature that refers to the same undeclared entity like any
/// other undeclared reference. [`merge_repeated_peers`] has merged identical
/// repeats before this function runs, and [`validate`] refuses a repeat with
/// another signature, so this function records one entry per declaration.
pub(crate) fn resolve_peers(roster: &Roster, structural: &[&str]) -> PeerResolution {
    let backends: BTreeSet<&BackendName> = roster.backends.iter().map(|b| &b.name).collect();
    let presentation: BTreeMap<&ElementName, TypeKey> =
        roster.presentation.iter().map(|record| (&record.element.name, record.payload)).collect();
    let table_types: BTreeMap<&CapName, &TableType> =
        roster.tables.iter().map(|record| (&record.def.name, &record.def.ty)).collect();
    let elements = declared_elements(roster, structural);

    let mut errors: Vec<BuildError> = Vec::new();
    let mut unresolved: Vec<(FeatureName, PeerName, Vec<RuleName>)> = Vec::new();
    for record in &roster.peer_backends {
        if !backends.contains(&record.name) {
            unresolved.push((
                record.owner.clone(),
                PeerName::Backend(record.name.clone()),
                Vec::new(),
            ));
        }
    }
    for record in &roster.peer_elements {
        match presentation.get(&record.name) {
            Some(payload) if payload.type_id() == record.payload.type_id() => {}
            Some(payload) => errors.push(BuildError::PeerElementMismatch {
                feature: record.owner.clone(),
                element: record.name.clone(),
                declared: Some(payload.short_name()),
                expected: record.payload.short_name(),
            }),
            None if elements.contains(&record.name) => {
                errors.push(BuildError::PeerElementMismatch {
                    feature: record.owner.clone(),
                    element: record.name.clone(),
                    declared: None,
                    expected: record.payload.short_name(),
                })
            }
            None => unresolved.push((
                record.owner.clone(),
                PeerName::Element(record.name.clone()),
                Vec::new(),
            )),
        }
    }
    for record in &roster.peer_tables {
        match table_types.get(&record.name) {
            Some(declared) if declared.type_id() == record.value.type_id() => {}
            Some(declared) => errors.push(BuildError::PeerTableMismatch {
                feature: record.owner.clone(),
                table: record.name.clone(),
                declared: declared.short_name(),
                expected: record.value.short_name(),
            }),
            None => unresolved.push((
                record.owner.clone(),
                PeerName::Table(record.name.clone()),
                Vec::new(),
            )),
        }
    }
    let packages: BTreeSet<&Name> = roster.scoped_packages.iter().map(|p| &p.name).collect();
    for record in &roster.peer_scoped_packages {
        if !packages.contains(&record.name) {
            unresolved.push((
                record.owner.clone(),
                PeerName::ScopedPackage(record.name.clone()),
                Vec::new(),
            ));
        }
    }
    let roles: BTreeSet<&Name> = roster.argument_roles.iter().map(|(_, def)| &def.name).collect();
    for record in &roster.peer_argument_roles {
        if !roles.contains(&record.name) {
            unresolved.push((
                record.owner.clone(),
                PeerName::ArgumentRole(record.name.clone()),
                Vec::new(),
            ));
        }
    }

    let mut set_aside: BTreeMap<RuleName, Vec<PeerName>> = BTreeMap::new();
    for (owner, rule) in &roster.rules {
        // What the rule depends on: the backend it is the default on, the
        // elements it produces, and the tables it reads as a hard dependency.
        // A table read optionally is deliberately absent — such a rule runs
        // and sees the table as absent.
        let mut refers: Vec<PeerName> = Vec::new();
        if let crate::core::lowering::DefaultMark::OnBackends(backends) = &rule.default {
            refers.extend(backends.iter().cloned().map(PeerName::Backend));
        }
        refers.extend(rule.decl.produces.iter().cloned().map(PeerName::Element));
        refers.extend(rule.decl.reads.iter().map(|table| PeerName::Table(table.name.clone())));
        let mut names: Vec<PeerName> = Vec::new();
        for name in refers {
            let unresolved_here = unresolved
                .iter_mut()
                .find(|(feature, known, _)| feature == owner && *known == name);
            if let Some((_, _, rules)) = unresolved_here {
                if !rules.contains(&rule.name) {
                    rules.push(rule.name.clone());
                }
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        if !names.is_empty() {
            set_aside.insert(rule.name.clone(), names);
        }
    }
    let mut set_aside_stages: BTreeMap<StageName, (FeatureName, Vec<PeerName>)> = BTreeMap::new();
    for record in &roster.stages {
        let names: Vec<PeerName> = record
            .decl
            .writes
            .iter()
            .map(|table| PeerName::Table(table.name.clone()))
            .filter(|name| {
                unresolved.iter().any(|(feature, known, _)| *feature == record.owner && known == name)
            })
            .collect();
        if !names.is_empty() {
            set_aside_stages.insert(record.decl.name.clone(), (record.owner.clone(), names));
        }
    }
    PeerResolution { errors, unresolved, set_aside, set_aside_stages }
}
