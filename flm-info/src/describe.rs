//! The description of an environment: the views of [`crate::env`] as JSON, and the JSON
//! Schema of the whole description ([`schema`]).
//!
//! The views that an [`Environment`](crate::Environment) answers are the description: each
//! derives `serde::Serialize`, so that `serde_json::to_value(&view)` gives its JSON form,
//! and `schemars::JsonSchema`, so that [`schema`] describes every form. A program that
//! writes the description as JSON puts a header first, of the type [`Meta`] (the kind of
//! document, the program's version and the format number of that kind). A page of
//! documentation ([`Page::to_json`]) holds one view as its body and is written by the same
//! rules; [`Output`] holds several pages behind their header.
//!
//! # The rules of the JSON form
//!
//! - **An object per view.** Each view is written as a JSON object whose keys are its
//!   field names. The declaration types of `flm-core` that a view holds (a workflow's
//!   definition, a plan's records of ruled-out stages and of warnings, an argument's
//!   structured syntax) are written in the serialized forms `flm-core` states for them.
//! - **Nothing to show, no key.** A key whose value has nothing to show is absent: an empty
//!   documentation text or summary, a value that does not apply (an argument's name for an
//!   argument read by position, the structured syntax of an argument whose parser is a
//!   feature's own, the trigger of a macro), and the lists and maps that only
//!   some instances have (an argument's tack-on fields, the scoped packages available in an
//!   argument, the places that make a scoped package's command available, the places where
//!   the entries of a setting merged entry by entry were written).
//! - **Flags are written when `true`.** A flag, a key whose value is `true` or `false`
//!   (`opaque_wrapper`, `repeatable`, `synthesized`, …), is absent when it is `false`: a reader
//!   takes an absent flag as `false`.
//! - **Lists that always exist are always written.** A list that every instance of a view
//!   has is present even when it is empty: a command's names, modes and arguments, an
//!   element's aspects, a feature's element types.
//! - **Names are their dotted strings** (`flm.refs.resolve`); a command's name is written
//!   without the backslash.
//! - **Enumerated values are lowercase words**, as the projection of `flm dump` spells them:
//!   a command's kind (`macro`, `environment`, `specials`), a layout (`inline`, `blocks`,
//!   `inline-or-blocks`, `from-role`), a parsing mode (`text`, `math`), a binding
//!   (`registry`, `document`, `render`, `either`).
//! - **Order.** A list keeps the order of the registry or of the declarations it comes
//!   from. The maps of the settings in effect (by feature, by key and by entry; cargo
//!   feature `flm-main`) are written in the order of their keys' names. What a view takes
//!   from a JSON Schema (a key's schema, the definitions that schemas refer to) and a value
//!   given as data (a default, a value in effect) are written as they are held, the keys
//!   of each object in the order in which they were inserted.

pub use flm_core::dump::Meta;
use serde_json::{Map, Value};

use crate::env::{
    CommandView, CommandsView, EntityView, FeatureView, Hit, NamespaceView, PackageView, ParsingView,
    ScopedPackageView, SettingsView, WorkflowView,
};
use crate::pages::{Output, Page};

/// The version of JSON Schema in which [`schema`] is written, as the value of its
/// `$schema` key.
pub const JSON_SCHEMA_VERSION: &str = "https://json-schema.org/draft/2020-12/schema";

/// Returns the JSON Schema of the description of an environment, in the JSON form that
/// the views are written in.
///
/// The schema's root accepts one view: a package, a feature, the commands of a feature, a
/// command, a scoped package, a feature's settings, the parsing configuration, a workflow,
/// a namespace, a hit of a lookup, or an entity of a namespace; or a page of documentation
/// ([`Page`], as [`Page::to_json`] writes it) or the output that holds several pages
/// ([`Output`]). A query of [`Environment`](crate::Environment) that
/// answers a list of views, such as [`features`](crate::Environment::features) or
/// [`lookup`](crate::Environment::lookup), gives a JSON array, each item of which the root
/// accepts. Each view type is defined under `$defs` by its type name (`FeatureView`,
/// `CommandView`, …), together with the types the views refer to. A key that the JSON form
/// may leave out (see the [module documentation](self)) is not required by the schema.
///
/// Under the cargo feature `flm-main` the schema includes the views' host fields, and its
/// root also accepts the host's own views: what the host adds to an environment
/// (`host::HostInfo`) and the keys of a front matter (`host::FrontMatterView`).
///
/// ```
/// let schema = flm_info::describe::schema();
/// assert_eq!(schema["$schema"], flm_info::describe::JSON_SCHEMA_VERSION);
/// assert!(schema["$defs"]["CommandView"].is_object());
/// ```
pub fn schema() -> Value {
    // The schema describes the JSON that the views serialize to, in which a key skipped
    // when it has nothing to show is optional.
    let mut generator = schemars::generate::SchemaSettings::draft2020_12().for_serialize().into_generator();
    let mut roots = vec![
        generator.subschema_for::<PackageView>(),
        generator.subschema_for::<FeatureView>(),
        generator.subschema_for::<CommandsView>(),
        generator.subschema_for::<CommandView>(),
        generator.subschema_for::<ScopedPackageView>(),
        generator.subschema_for::<SettingsView>(),
        generator.subschema_for::<ParsingView>(),
        generator.subschema_for::<WorkflowView>(),
        generator.subschema_for::<NamespaceView>(),
        generator.subschema_for::<Hit>(),
        generator.subschema_for::<EntityView>(),
        generator.subschema_for::<Page>(),
        generator.subschema_for::<Output>(),
    ];
    #[cfg(feature = "flm-main")]
    {
        roots.push(generator.subschema_for::<crate::host::HostInfo>());
        roots.push(generator.subschema_for::<crate::host::FrontMatterView>());
    }
    let mut root = Map::new();
    root.insert("$schema".into(), Value::from(JSON_SCHEMA_VERSION));
    root.insert("title".into(), Value::from("The description of an FLM environment"));
    let views = if cfg!(feature = "flm-main") {
        "a package, a feature, the commands of a feature, a command, a scoped package, a feature's settings, \
         the parsing configuration, a workflow, a namespace, a hit of a lookup, an entity of a namespace, a \
         page of documentation, the output that holds several pages, what the host adds to an environment, or \
         the keys of a front matter"
    } else {
        "a package, a feature, the commands of a feature, a command, a scoped package, a feature's settings, \
         the parsing configuration, a workflow, a namespace, a hit of a lookup, an entity of a namespace, a \
         page of documentation, or the output that holds several pages"
    };
    root.insert(
        "description".into(),
        Value::from(format!(
            "One view of an FLM environment, as the queries of flm-info answer it: {views}. A query that \
             answers a list of views gives an array of them."
        )),
    );
    root.insert("anyOf".into(), Value::Array(roots.drain(..).map(|schema| schema.to_value()).collect()));
    root.insert("$defs".into(), Value::Object(generator.take_definitions(true)));
    Value::Object(root)
}
