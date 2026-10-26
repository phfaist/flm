//! The configuration of the [`userdefs`](super) feature, [`UserdefsConfig`]:
//! the user definitions and the expansion depth limit, and the forms in which
//! settings written as data give them.
//!
//! [`Userdefs::new`](super::Userdefs::new) takes a [`UserdefsConfig`]: the user
//! definitions ([`Definition`], one per macro, environment or special
//! character sequence) and the limit on how many expansions one chain of use
//! sites may nest before a use site is replaced by its literal source text and
//! reported with `flm.userdefs.recursion-limit`. The configuration is fixed
//! when the registry is built ([settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//!
//! Settings written as data ([`crate::core::settings`]) give the configuration
//! as a [`UserdefsData`]: one map per kind of definition (`macros`,
//! `environments`, `specials`) from a name to a [`DefinitionData`], and the
//! depth limit. The library converts it to a [`UserdefsConfig`].
//!
//! # The canonical bytes of the configuration
//!
//! [`UserdefsConfig::digest_bytes`] writes the configuration as bytes, field
//! by field. The feature gives these bytes as an input of the [registry
//! digest](crate::model::digest), because a template text changes the trees a
//! run freezes, so two registries that differ in one must not share cached
//! trees. The same bytes are what the type's `Hash` implementation hashes: the
//! feature's settings declaration declares the configuration as an option type
//! bound to the registry ([`Bind::Registry`](crate::model::options::Bind)), and
//! every option type implements `Hash`. The bytes are written field by field
//! rather than taken from the type's `Debug` output, which could hold a memory
//! address and would change with a field rename.

// The depth limit is configuration of the feature rather than a per-document
// or per-render option, because the trees a run freezes depend on it and the
// registry digest must see it. See design rationale at
// [§flm-dd-dr:std-userdefs-config] and [§flm-dd-dr:feat-config-hash].

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use serde::Deserialize;
use serde_json::Value;

use crate::core::settings::named::{self, NamedEntry};
use crate::model::aspects::{BlockRole, Placement};
use crate::model::names::Name;
use crate::lang::mint::{deserialize_arg_list, ArgDef, ArgLayout, ArgListData};
use crate::lang::rewrites::userdefs::{
    DEFAULT_RECURSION_LIMIT, Definition, DefinitionKind, ExpandPolicy,
};

/// The configuration of the [`userdefs`](super) feature: the user definitions
/// and the expansion depth limit. It is fixed when the registry is built; see
/// the module documentation.
///
/// Build a value with [`UserdefsConfig::new`] and the builder methods
/// [`definition`](UserdefsConfig::definition),
/// [`definitions`](UserdefsConfig::definitions) and
/// [`recursion_limit`](UserdefsConfig::recursion_limit), or read it from data
/// as a [`UserdefsData`] and convert it with `From`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::userdefs::{Definition, UserdefsConfig};
///
/// let config = UserdefsConfig::new()
///     .definition(Definition::macro_("hi", Vec::new()).content("Hello"))
///     .recursion_limit(4);
///
/// assert_eq!(config.definitions.len(), 1);
/// assert_eq!(config.recursion_limit, 4);
/// ```
///
#[derive(Debug, Clone)]
pub struct UserdefsConfig {
    /// The user definitions, in the order they were declared. In data, they are
    /// written in one map per kind ([`UserdefsData`]).
    pub definitions: Vec<Definition>,
    /// The expansion depth limit: how many expansions one chain of use sites
    /// may nest before a use site is reported with
    /// `flm.userdefs.recursion-limit`. The default is
    /// [`DEFAULT_RECURSION_LIMIT`] (32).
    pub recursion_limit: usize,
}

impl UserdefsConfig {
    /// Returns the default configuration: no definitions, and the default
    /// expansion depth limit ([`DEFAULT_RECURSION_LIMIT`]).
    pub fn new() -> UserdefsConfig {
        UserdefsConfig::default()
    }

    /// Adds one definition after the ones already declared, and returns the
    /// configuration.
    pub fn definition(mut self, definition: Definition) -> UserdefsConfig {
        self.definitions.push(definition);
        self
    }

    /// Adds several definitions, in the order given, after the ones already
    /// declared, and returns the configuration.
    pub fn definitions(
        mut self,
        definitions: impl IntoIterator<Item = Definition>,
    ) -> UserdefsConfig {
        self.definitions.extend(definitions);
        self
    }

    /// Sets the expansion depth limit, and returns the configuration.
    pub fn recursion_limit(mut self, limit: usize) -> UserdefsConfig {
        self.recursion_limit = limit;
        self
    }

    /// Returns the canonical bytes of the configuration: the input the
    /// `userdefs` feature gives to the registry digest, and what `Hash` hashes.
    ///
    /// Two configurations give the same bytes exactly when they declare the
    /// same definitions in the same order under the same expansion depth
    /// limit. A definition is written as its name, its kind, its arguments
    /// (each one's name, the identity of its parser, its parsing mode, its
    /// layout, whether it is declared with an opaque wrapper and its scoped
    /// packages), its template text, the parsing modes it applies in, its
    /// default values, its argument number offset, its block role, its
    /// expansion policy, whether it overrides another feature's command, and
    /// its extra placeholders (each one's name and whether it accepts a
    /// substitution argument; [`Definition::extra_placeholder`]).
    /// Documentation strings change nothing in a parse and are left out.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_count(&mut out, self.recursion_limit);
        put_count(&mut out, self.definitions.len());
        for definition in &self.definitions {
            put_definition(&mut out, definition);
        }
        out
    }
}

impl Default for UserdefsConfig {
    fn default() -> UserdefsConfig {
        UserdefsConfig { definitions: Vec::new(), recursion_limit: DEFAULT_RECURSION_LIMIT }
    }
}

impl Hash for UserdefsConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

/// Writes `text` to `out`, its length first.
fn put(out: &mut Vec<u8>, text: &str) {
    put_count(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// Writes an absent text as `-`, and a present one as `+` followed by the
/// text.
fn put_opt(out: &mut Vec<u8>, text: Option<&str>) {
    match text {
        None => put(out, "-"),
        Some(text) => {
            put(out, "+");
            put(out, text);
        }
    }
}

/// Writes a count or a flag as eight bytes, least significant first.
fn put_count(out: &mut Vec<u8>, count: usize) {
    out.extend_from_slice(&(count as u64).to_le_bytes());
}

/// Writes one definition, field by field, in the order
/// [`UserdefsConfig::digest_bytes`] lists.
fn put_definition(out: &mut Vec<u8>, definition: &Definition) {
    put(out, definition.name.as_str());
    put(
        out,
        match definition.kind {
            DefinitionKind::Macro => "macro",
            DefinitionKind::Environment => "environment",
            DefinitionKind::Specials => "specials",
        },
    );
    put_count(out, definition.args.len());
    for arg in &definition.args {
        put_arg(out, arg);
    }
    put_opt(out, definition.content.as_deref());
    put(out, definition.modes.token());
    put_count(out, definition.defaults.len());
    for (key, text) in &definition.defaults {
        put(out, key);
        put(out, text);
    }
    put_count(out, definition.argument_number_offset);
    put_role(out, definition.block_role);
    put(
        out,
        match definition.expand {
            ExpandPolicy::Always => "always",
            ExpandPolicy::PerPlan => "per-plan",
        },
    );
    put_count(out, usize::from(definition.overrides));
    put_count(out, definition.extra_placeholders.len());
    for extra in &definition.extra_placeholders {
        put(out, &extra.name);
        put_count(out, usize::from(extra.substitution));
    }
}

/// Writes a block role: its kind, then the two whitespace-absorption flags of
/// an inline role, or the placement of a block decorator.
fn put_role(out: &mut Vec<u8>, role: BlockRole) {
    match role {
        BlockRole::Block => put(out, "block"),
        BlockRole::Inline(spacing) => {
            put(out, "inline");
            put_count(out, usize::from(spacing.absorb_before));
            put_count(out, usize::from(spacing.absorb_after));
        }
        BlockRole::BlockDecorator(placement) => {
            put(out, "decorator");
            put(
                out,
                match placement {
                    Placement::Leads => "leads",
                    Placement::Trails => "trails",
                    Placement::Free => "free",
                },
            );
        }
    }
}

/// Writes one argument: its name; the identity of its parser
/// ([`FlmArgumentParser::identity`](crate::lang::mint::FlmArgumentParser::identity)),
/// its length first; its parsing mode; its layout; whether it is declared with
/// an opaque wrapper; and its scoped packages, their count first.
///
/// The identity of a standard parser gives its kind with every parameter it
/// holds, whether the argument was read from a code or from a structured
/// syntax, so two arguments that differ in one parameter are written apart;
/// for a tack-on argument, it gives each field's name, arguments and whether
/// it repeats, and a field's element is built from its name, so the name
/// covers it. The parsing mode is written as
/// the `Debug` output of techy's parse mode (`Some(Math)`), which holds no
/// memory address; the registry digest writes it the same way.
fn put_arg(out: &mut Vec<u8>, arg: &ArgDef) {
    put_opt(out, arg.name.as_deref());
    let identity = arg.kind.identity();
    put_count(out, identity.len());
    out.extend_from_slice(&identity);
    put(out, &format!("{:?}", arg.mode));
    put(
        out,
        match arg.layout {
            ArgLayout::Inline => "inline",
            ArgLayout::Blocks => "blocks",
            ArgLayout::InlineOrBlocks => "inline-or-blocks",
        },
    );
    put_count(out, usize::from(arg.opaque_wrapper));
    put_count(out, arg.scoped.len());
    for package in &arg.scoped {
        put(out, package.as_str());
    }
}

/// The settings of the [`userdefs`](super) feature as data gives them: the user
/// definitions in one map per kind, and the expansion depth limit.
///
/// The library reads the settings into this type and converts it to a
/// [`UserdefsConfig`] with `From`: the macros in the order written, then the
/// environments, then the special character sequences, each entry giving the
/// definitions that [`DefinitionData::definitions`] returns for it (one, or one
/// per parsing mode). The field documentation below is the description of each
/// settings key in the settings' JSON Schema.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::userdefs::{Definition, UserdefsConfig, UserdefsData};
///
/// let data = serde_json::json!({
///     "macros": { "hi": { "content": "Hello" } },
///     "environments": { "hi": { "content": "#{body}" } },
/// });
/// let config = UserdefsConfig::from(serde_json::from_value::<UserdefsData>(data)?);
/// assert_eq!(config.definitions.len(), 2);
/// assert_eq!(config.definitions[1].display_name(), "\\begin{hi}");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct UserdefsData {
    /// The macros: a map from a macro's name, without the backslash, to its
    /// definition. The order written is kept.
    #[serde(deserialize_with = "named::deserialize")]
    #[schemars(
        transform = named::map_schema,
        with = "BTreeMap<String, DefinitionData>"
    )]
    pub macros: Vec<NamedDefinition>,
    /// The environments: a map from an environment's name to its definition.
    /// The order written is kept.
    #[serde(deserialize_with = "named::deserialize")]
    #[schemars(
        transform = named::map_schema,
        with = "BTreeMap<String, DefinitionData>"
    )]
    pub environments: Vec<NamedDefinition>,
    /// The special character sequences: a map from the characters of a
    /// sequence to its definition. The order written is kept.
    #[serde(deserialize_with = "named::deserialize")]
    #[schemars(
        transform = named::map_schema,
        with = "BTreeMap<String, DefinitionData>"
    )]
    pub specials: Vec<NamedDefinition>,
    /// The expansion depth limit: how many expansions one chain of use sites
    /// may nest before a use site is reported with
    /// `flm.userdefs.recursion-limit`. The default is 32
    /// ([`UserdefsConfig::recursion_limit`]).
    pub recursion_limit: usize,
}

impl Default for UserdefsData {
    fn default() -> UserdefsData {
        UserdefsData {
            macros: Vec::new(),
            environments: Vec::new(),
            specials: Vec::new(),
            recursion_limit: UserdefsConfig::default().recursion_limit,
        }
    }
}

impl From<UserdefsData> for UserdefsConfig {
    fn from(data: UserdefsData) -> UserdefsConfig {
        let kinds = [
            (DefinitionKind::Macro, data.macros),
            (DefinitionKind::Environment, data.environments),
            (DefinitionKind::Specials, data.specials),
        ];
        let definitions = kinds.into_iter().flat_map(|(kind, entries)| {
            entries.into_iter().flat_map(move |entry| entry.definition.definitions(kind, entry.name))
        });
        UserdefsConfig::new().definitions(definitions).recursion_limit(data.recursion_limit)
    }
}

/// One entry of a map of [`UserdefsData`]: the name the entry is written under,
/// and the definition written there.
#[derive(Debug, Clone)]
pub struct NamedDefinition {
    /// The name the entry is written under: a macro's or an environment's
    /// name, or the characters of a special character sequence.
    pub name: Name,
    /// The definition written under that name.
    pub definition: DefinitionData,
}

impl NamedEntry for NamedDefinition {
    fn read_entry(name: &str, data: Value) -> Result<NamedDefinition, String> {
        let definition = named::read_unnamed(name, data)?;
        Ok(NamedDefinition { name: Name::from(name), definition })
    }
}

/// A user definition as data gives it, under its name in the map of its kind
/// ([`UserdefsData`]): the fields of a [`Definition`] except its name, its
/// kind and its parsing modes, which the entry's name, the map and the content
/// give, and except its extra placeholders
/// ([`Definition::extra_placeholder`]), which only a definition declared in
/// Rust can have. A field not written keeps the value that
/// [`Definition::new`] gives it.
///
/// [`definitions`](DefinitionData::definitions) returns the definitions the
/// entry declares, given its kind and its name. The field documentation below
/// is the description of each settings key in the settings' JSON Schema.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DefinitionData {
    /// The arguments, in the order they are written at a use site. Each
    /// argument gives its syntax as a structured syntax: a map whose key
    /// `syntax` holds the name of a standard kind of argument and whose other
    /// keys hold the kind's parameters, beside the argument's own keys
    /// (`name`, `mode`, `layout`, `opaque_wrapper`, `scoped`, `docs`, `role`),
    /// as in `{syntax: delimited-group, delimiters: ["(", ")"], name: size}`
    /// for a group in parentheses named `size`. An argument code is a
    /// shorthand for a structured syntax: in a map, the key `code` holds it in
    /// place of `syntax`, so `{code: "r()", name: size}` declares the same
    /// argument; a code alone (`"{"`) declares an argument with no other key.
    /// The list is written in one of three shapes: one text in which each
    /// character is one code (`"[{"`); a list of codes (`["[", "r()"]`); or a
    /// list whose entries are each a code or a map. The kinds and their
    /// parameters are listed in [the documentation of the standard
    /// kinds](crate::lang::mint::ArgStdKind#the-kind-as-data), and the codes
    /// in [the table of argument codes](crate::lang::mint::codes#the-codes).
    #[serde(deserialize_with = "deserialize_arg_list")]
    #[schemars(with = "ArgListData")]
    pub args: Vec<ArgDef>,
    /// The template: FLM text with placeholders, which replaces every use site.
    /// One text is the template in text mode and in mathematics mode, and the
    /// definition applies in each mode where the text parses. A map with the
    /// keys `text` and `math`, one or both written, gives one template per
    /// mode, and the definition applies in the modes written.
    pub content: Option<DefinitionContent>,
    /// The default values of the arguments: template text, keyed by the
    /// argument's declared name or by the number a placeholder refers to it
    /// by. A default value replaces the placeholders of an argument that a use
    /// site leaves out.
    pub defaults: BTreeMap<String, String>,
    /// The argument number offset: how many of the first declared arguments
    /// the numbered placeholders skip, so that `#1` refers to the argument at
    /// this index (counting from zero).
    pub argument_number_offset: usize,
    /// The block role of the definition's element: a block of its own, inline
    /// content, or a block decorator.
    pub block_role: BlockRole,
    /// When the use sites are expanded: `always`, in every plan, or `per-plan`
    /// (the default), as each plan decides.
    pub expand: ExpandPolicy,
    /// Whether this definition replaces another feature's definition of the
    /// same command. Without it, two features that define one command make the
    /// registry build fail, unless the feature order places the feature
    /// registered last explicitly after the other.
    pub overrides: bool,
    /// The definition's documentation, shown wherever the registry describes
    /// its element.
    pub docs: String,
}

impl Default for DefinitionData {
    fn default() -> DefinitionData {
        // A field not written keeps what the Rust constructor gives it; the
        // kind and the name chosen here reach no field below.
        let base = Definition::new(DefinitionKind::Macro, "", Vec::new());
        DefinitionData {
            args: base.args,
            content: None,
            defaults: base.defaults,
            argument_number_offset: base.argument_number_offset,
            block_role: base.block_role,
            expand: base.expand,
            overrides: base.overrides,
            docs: base.docs,
        }
    }
}

impl DefinitionData {
    /// Returns the definitions the entry declares as a definition of kind
    /// `kind` with the name `name`: one definition that applies in both modes
    /// when the content is one text or is not written; for a content given per
    /// mode, one definition per mode written, the one for text mode first, each
    /// applying in its mode alone (as [`Definition::text`] and
    /// [`Definition::math`] declare).
    pub fn definitions(self, kind: DefinitionKind, name: impl Into<Name>) -> Vec<Definition> {
        let base = Definition {
            defaults: self.defaults,
            argument_number_offset: self.argument_number_offset,
            block_role: self.block_role,
            expand: self.expand,
            overrides: self.overrides,
            ..Definition::new(kind, name, self.args)
        }
        .docs(self.docs);
        match self.content {
            None => vec![base],
            Some(content) => content.definitions(base),
        }
    }
}

impl DefinitionContent {
    /// Returns the definitions this content declares from `base`, a
    /// definition complete except for its template: for one text, one
    /// definition that applies in both modes (in each mode where the text
    /// parses); for a content given per mode, one definition per mode written,
    /// the one for text mode first, each applying in its mode alone (as
    /// [`Definition::text`] and [`Definition::math`] declare).
    ///
    /// Every feature that reads a definition's `content` key from data calls
    /// this function, so that the key has the same meaning wherever it is
    /// written.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::lang::rewrites::userdefs::DefinitionModes;
    /// use flm_core::stdcore::features::userdefs::{Definition, DefinitionContent};
    ///
    /// let base = Definition::macro_("ket", Vec::new());
    /// let one = DefinitionContent::Both(String::from("x")).definitions(base.clone());
    /// assert_eq!(one.len(), 1);
    /// assert_eq!(one[0].modes, DefinitionModes::Both);
    ///
    /// let by_mode: DefinitionContent = serde_json::from_str(r#"{"text": "t", "math": "m"}"#).unwrap();
    /// let two = by_mode.definitions(base);
    /// let modes: Vec<DefinitionModes> = two.iter().map(|definition| definition.modes).collect();
    /// assert_eq!(modes, [DefinitionModes::Text, DefinitionModes::Math]);
    /// ```
    pub fn definitions(self, base: Definition) -> Vec<Definition> {
        match self {
            DefinitionContent::Both(template) => vec![base.content(template)],
            DefinitionContent::ByMode(templates) => {
                let mut definitions = Vec::new();
                if let Some(template) = templates.text {
                    definitions.push(base.clone().text(template));
                }
                if let Some(template) = templates.math {
                    definitions.push(base.math(template));
                }
                definitions
            }
        }
    }
}

/// The `content` key of a [`DefinitionData`]: one template for both parsing
/// modes, or one template per mode.
///
/// In data, it is a text or a map with the keys `text` and `math`. A map that
/// gives neither key, and a value of any other form, are refused when the
/// settings are read.
#[derive(Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[schemars(untagged)]
pub enum DefinitionContent {
    /// One template for text mode and mathematics mode; the definition applies
    /// in each mode where it parses.
    Both(String),
    /// One template per mode, at least one of the two given.
    ByMode(ModeTemplates),
}

impl<'de> Deserialize<'de> for DefinitionContent {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        match Value::deserialize(deserializer)? {
            Value::String(template) => Ok(DefinitionContent::Both(template)),
            value @ Value::Object(_) => {
                let templates: ModeTemplates = serde_json::from_value(value).map_err(D::Error::custom)?;
                if templates.text.is_none() && templates.math.is_none() {
                    return Err(D::Error::custom(
                        "a `content` map must give `text`, `math` or both",
                    ));
                }
                Ok(DefinitionContent::ByMode(templates))
            }
            other => Err(D::Error::custom(format!(
                "`content` must be a text or a map with the keys `text` and `math`, not `{other}`"
            ))),
        }
    }
}

/// The templates of a content given per mode ([`DefinitionContent::ByMode`]);
/// at least one is given.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(extend("minProperties" = 1))]
pub struct ModeTemplates {
    /// The template for text mode.
    #[schemars(with = "String")]
    pub text: Option<String>,
    /// The template for mathematics mode.
    #[schemars(with = "String")]
    pub math: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::digest::digest_of;

    fn config(template: &str) -> UserdefsConfig {
        UserdefsConfig::new()
            .definition(Definition::macro_("foo", vec![ArgDef::mandatory("x")]).text(template))
    }

    #[test]
    fn the_default_limit_is_the_passes() {
        assert_eq!(UserdefsConfig::new().recursion_limit, DEFAULT_RECURSION_LIMIT);
        assert!(UserdefsConfig::new().definitions.is_empty());
    }

    #[test]
    fn the_rendering_sees_the_template_text() {
        assert_ne!(config("#1").digest_bytes(), config("#1!").digest_bytes());
        assert_eq!(config("#1").digest_bytes(), config("#1").digest_bytes());
    }

    #[test]
    fn the_rendering_sees_the_parsing_modes() {
        let in_math = UserdefsConfig::new()
            .definition(Definition::macro_("foo", vec![ArgDef::mandatory("x")]).math("#1"));
        let in_both = UserdefsConfig::new().definition(
            Definition::macro_("foo", vec![ArgDef::mandatory("x")]).content("#1"),
        );
        // `config` declares the same template text in text mode alone.
        assert_ne!(config("#1").digest_bytes(), in_math.digest_bytes());
        assert_ne!(config("#1").digest_bytes(), in_both.digest_bytes());
        assert_ne!(in_math.digest_bytes(), in_both.digest_bytes());
    }

    #[test]
    fn the_rendering_sees_the_argument_number_offset() {
        let shifted = UserdefsConfig::new().definition(
            Definition::macro_("foo", vec![ArgDef::mandatory("x")])
                .text("#1")
                .argument_number_offset(1),
        );
        assert_ne!(config("#1").digest_bytes(), shifted.digest_bytes());
    }

    #[test]
    fn the_rendering_sees_the_depth_limit() {
        assert_ne!(
            config("#1").digest_bytes(),
            config("#1").recursion_limit(3).digest_bytes()
        );
    }

    #[test]
    fn the_rendering_sees_an_arguments_mode_layout_and_scoped_packages() {
        let with = |arg: ArgDef| {
            UserdefsConfig::new().definition(Definition::macro_("foo", vec![arg]).text("#1"))
        };
        let plain = with(ArgDef::mandatory("x")).digest_bytes();
        let in_math = with(ArgDef::mandatory("x").mode(techy::latexlike::Mode::Math));
        let as_blocks = with(ArgDef::mandatory("x").layout(ArgLayout::Blocks));
        let scoped = with(ArgDef::mandatory("x").scoped(crate::model::names::Name::new("some.package")));
        assert_ne!(plain, in_math.digest_bytes());
        assert_ne!(plain, as_blocks.digest_bytes());
        assert_ne!(plain, scoped.digest_bytes());
        assert_eq!(plain, with(ArgDef::mandatory("x")).digest_bytes());
    }

    #[test]
    fn documentation_is_not_semantic() {
        let documented = UserdefsConfig::new().definition(
            Definition::macro_("foo", vec![ArgDef::mandatory("x")]).text("#1").docs("a foo"),
        );
        assert_eq!(config("#1").digest_bytes(), documented.digest_bytes());
    }

    #[test]
    fn the_hash_is_the_rendering() {
        assert_eq!(digest_of(&config("#1")), digest_of(&config("#1")));
        assert_ne!(digest_of(&config("#1")), digest_of(&config("#1 ")));
    }
}
