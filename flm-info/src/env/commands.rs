//! The commands an author may write: their declarations as views
//! ([`Environment::commands`], [`Environment::command`]) and the scoped packages
//! ([`Environment::scoped_packages`], [`Environment::scoped_package`]).

use std::sync::Arc;

use flm_core::lang::mint::{
    ArgDef, ArgLayout, ArgStdKind, BodyDef, BodyLayout, CallableKind, ElementDef, ScopedPackageDef,
    SyntaxDef, TackOnField,
};
use flm_core::model::element::ElementSpec;
use serde::Serialize;

use super::{Environment, is_empty, is_false, summary_of, written_name};

impl Environment {
    /// The commands that the feature `feature` declares directly, grouped by kind, each
    /// group in the order the feature declared them. The commands of its scoped packages
    /// are not among them: see [`scoped_packages`](Environment::scoped_packages). A feature
    /// that declares no command, or that the registry does not hold, gives empty groups.
    pub fn commands(&self, feature: &str) -> CommandsView {
        CommandsView::of(self.registry.declarations().elements_of(feature).map(|def| self.command_view(def, None)))
    }

    /// Every declaration, across features, of the command `name` of the kind `kind`
    /// (written without the backslash; for specials, the trigger), in the order in which
    /// the language stacks the features' definitions: in a given parsing mode, the last
    /// declaration available in that mode is the one in force. Only the declarations that
    /// features made directly are searched; a command of a scoped package is found through
    /// [`scoped_package`](Environment::scoped_package) or [`lookup`](Environment::lookup).
    /// Empty when no feature declares the command.
    pub fn command(&self, kind: CallableKind, name: &str) -> Vec<CommandView> {
        self.registry.declarations().named(kind, name).into_iter().map(|def| self.command_view(def, None)).collect()
    }

    /// The scoped packages that the feature `feature` declares, in the order it declared
    /// them.
    pub fn scoped_packages(&self, feature: &str) -> Vec<ScopedPackageView> {
        self.registry
            .declarations()
            .scoped_packages_of(feature)
            .map(|package| self.scoped_package_view(package))
            .collect()
    }

    /// The scoped package called `name`, or `None` when no feature declares one.
    pub fn scoped_package(&self, name: &str) -> Option<ScopedPackageView> {
        self.registry.declarations().scoped_package(name).map(|package| self.scoped_package_view(package))
    }

    fn scoped_package_view(&self, package: &ScopedPackageDef) -> ScopedPackageView {
        ScopedPackageView {
            name: package.name.to_string(),
            owner: package.owner.to_string(),
            docs: package.docs.clone(),
            summary: summary_of(&package.docs).to_string(),
            commands: CommandsView::of(package.elements.iter().map(|def| self.command_view(def, Some(package)))),
            available_in: self.use_sites(&package.name),
        }
    }

    /// Every place that makes the scoped package `name` available.
    fn use_sites(&self, name: &str) -> Vec<UseSiteView> {
        self.registry
            .declarations()
            .scoped_package_uses(name)
            .into_iter()
            .map(|site| {
                let def = site.declaration();
                let (part, field) = match &site {
                    flm_core::core::registry::ScopedPackageUse::Body { .. } => (String::from("body"), None),
                    flm_core::core::registry::ScopedPackageUse::Argument(argument) => (
                        argument_label(argument.arg, argument.index),
                        argument.field.map(|field| field.name.clone()),
                    ),
                };
                UseSiteView {
                    feature: def.owner.to_string(),
                    kind: def.syntax.kind(),
                    command: written_name(def),
                    within: site.within().map(|package| package.name.to_string()),
                    field,
                    part,
                }
            })
            .collect()
    }

    /// The view of the declaration `def`, held by the scoped package `within` if any.
    pub(crate) fn command_view(&self, def: &ElementDef, within: Option<&ScopedPackageDef>) -> CommandView {
        let synthesized = matches!(def.syntax, SyntaxDef::Synthesized);
        CommandView {
            feature: def.owner.to_string(),
            names: def.names.iter().map(ToString::to_string).collect(),
            kind: def.syntax.kind(),
            trigger: match &def.syntax {
                SyntaxDef::Specials { trigger, .. } => Some(trigger.clone()),
                _ => None,
            },
            modes: if synthesized { Vec::new() } else { def.modes.iter().map(|mode| mode_name(*mode)).collect() },
            docs: def.docs.clone(),
            summary: summary_of(&def.docs).to_string(),
            arguments: def.syntax.args().iter().map(argument_view).collect(),
            body: match &def.syntax {
                SyntaxDef::Environment { body, .. } => Some(body_view(body)),
                _ => None,
            },
            element: self.element_view(&def.element),
            within: within.map(|package| package.name.to_string()),
            available_in: within.map(|package| self.use_sites(&package.name)).unwrap_or_default(),
            synthesized,
        }
    }

    /// The view of the element value `element`.
    pub(crate) fn element_view(&self, element: &Arc<ElementSpec>) -> ElementView {
        ElementView {
            name: element.name.to_string(),
            docs: element.docs.clone(),
            summary: summary_of(&element.docs).to_string(),
            aspects: element.aspects.entries().map(|(_, name, _)| aspect_name(name).to_string()).collect(),
            number: self.registry.elem_idx(element).map(|idx| idx.0),
        }
    }
}

/// The name of an aspect's trait as the views write it: the Rust path of the trait, as the
/// compiler gives the type name of a trait object (`std::any::type_name`), without its
/// `dyn ` prefix.
fn aspect_name(type_name: &str) -> &str {
    type_name.strip_prefix("dyn ").unwrap_or(type_name)
}

/// How a place names an argument: its name, or `#N` (its position, counting from 1) for an
/// argument without a name.
fn argument_label(arg: &ArgDef, index: usize) -> String {
    match &arg.name {
        Some(name) => name.clone(),
        None => format!("#{}", index + 1),
    }
}

/// The spelling of a parsing mode in the views: `text` or `math`, as its serialized form
/// gives it.
pub(crate) fn mode_name(mode: flm_core::techy::latexlike::Mode) -> String {
    match serde_json::to_value(mode) {
        Ok(serde_json::Value::String(name)) => name,
        _ => format!("{mode:?}").to_lowercase(),
    }
}

fn argument_view(arg: &ArgDef) -> ArgumentView {
    ArgumentView {
        form: arg.code().map(str::to_string).unwrap_or_else(|| arg.kind.description()),
        syntax: arg.kind.std_kind().cloned(),
        written: written_form(arg),
        name: arg.name.clone(),
        layout: arg.layout,
        mode: arg.mode.map(mode_name),
        scoped: arg.scoped.iter().map(ToString::to_string).collect(),
        docs: arg.docs.clone(),
        summary: summary_of(&arg.docs).to_string(),
        role: arg.role.as_ref().map(ToString::to_string),
        opaque_wrapper: arg.opaque_wrapper,
        fields: arg.kind.tack_on_fields().unwrap_or_default().iter().map(field_view).collect(),
    }
}

/// How the argument `arg` is written at a use site, with its name standing for its content
/// (`{title}`, `[options]`, `*`); `…` stands for the content of an argument without a name.
///
/// An argument of a standard kind (every argument declared with an argument code, and every
/// argument declared with a kind of [`ArgStdKind`]) is written as its kind says
/// ([`written_from_kind`]). An argument whose parser is a feature's own is written as its
/// fields when the parser declares tack-on fields, and otherwise from the parser's
/// description ([`written_from_description`]).
fn written_form(arg: &ArgDef) -> String {
    let name = arg.name.as_deref().unwrap_or("…");
    match arg.kind.std_kind() {
        Some(kind) => written_from_kind(kind, name),
        None => match arg.kind.tack_on_fields() {
            Some(fields) => written_fields(fields),
            None => written_from_description(&arg.kind.description(), name),
        },
    }
}

/// The written form of an argument of the standard kind `kind`, its content standing as
/// `name` (see [`written_form`]):
///
/// - a group whose use site writes braces (mandatory, braced only, read as characters, in
///   any of the pairs, verbatim between any pair) as `{name}`;
/// - an optional group in brackets (in any of the pairs, of key-value pairs) as `[name]`;
/// - a marker as its character (`*`, `!`);
/// - a group between a prescribed pair, mandatory or optional, and a verbatim argument
///   between a prescribed pair, as `<open>name<close>`;
/// - embellishments as each marker followed by `{…}`;
/// - a tack-on argument as its fields, each a macro followed by the written forms of its own
///   arguments (`\label{target}`);
/// - a terminated argument as its opener, `name`, and its terminator, in the shortest form
///   that reads: the repeated character once on each side when the terminator holds nothing
///   else, and not at all otherwise.
fn written_from_kind(kind: &ArgStdKind, name: &str) -> String {
    match kind {
        ArgStdKind::Mandatory
        | ArgStdKind::AnyDelimitedGroup
        | ArgStdKind::BracedOnly
        | ArgStdKind::CharsGroup
        | ArgStdKind::Verbatim { delimiters: None } => format!("{{{name}}}"),
        ArgStdKind::Optional | ArgStdKind::OptionalAnyDelimitedGroup | ArgStdKind::KeyVals => {
            format!("[{name}]")
        }
        ArgStdKind::Star => String::from("*"),
        ArgStdKind::Marker { marker } => marker.to_string(),
        ArgStdKind::DelimitedGroup { open, close }
        | ArgStdKind::OptionalDelimitedGroup { open, close }
        | ArgStdKind::Verbatim { delimiters: Some((open, close)) } => format!("{open}{name}{close}"),
        ArgStdKind::Embellishments { markers } => markers.iter().map(|marker| format!("{marker}{{…}}")).collect(),
        ArgStdKind::TackOn { fields } => written_fields(fields),
        ArgStdKind::Terminated { terminator, repeat_char, opener, head, raw: _ } => {
            let run = match repeat_char {
                Some(repeat_char) if head.is_empty() && terminator.is_empty() => repeat_char.to_string(),
                _ => String::new(),
            };
            format!("{run}{opener}{name}{head}{run}{terminator}")
        }
    }
}

/// The written form of a tack-on argument with the fields `fields`: each field's macro
/// followed by the written forms of its own arguments.
fn written_fields(fields: &[TackOnField]) -> String {
    fields
        .iter()
        .map(|field| {
            let args: String = field.args.iter().map(written_form).collect();
            format!("\\{}{args}", field.name)
        })
        .collect()
}

/// The written form of an argument whose parser, a feature's own, describes itself as
/// `description`, its content standing as `name` (see [`written_form`]): the first code span
/// of the description (the text between its first pair of backticks: `{…}` in "a `{…}` group
/// read as an inline formula"), the content between its first and last characters replaced
/// by the name when the span holds `…`; braces around the name when the description holds no
/// code span.
fn written_from_description(description: &str, name: &str) -> String {
    let Some(span) = description.split('`').nth(1).filter(|_| description.matches('`').count() >= 2) else {
        return format!("{{{name}}}");
    };
    let chars: Vec<char> = span.chars().collect();
    if chars.len() >= 3 && span.contains('…') {
        format!("{}{name}{}", chars[0], chars[chars.len() - 1])
    } else {
        span.to_string()
    }
}

fn field_view(field: &TackOnField) -> FieldView {
    FieldView {
        name: field.name.clone(),
        repeatable: field.repeatable,
        docs: field.docs.clone(),
        summary: summary_of(&field.docs).to_string(),
        arguments: field.args.iter().map(argument_view).collect(),
    }
}

fn body_view(body: &BodyDef) -> BodyView {
    BodyView {
        layout: body.layout,
        mode: body.mode.map(mode_name),
        scoped: body.scoped.iter().map(ToString::to_string).collect(),
        opaque_wrapper: body.opaque_wrapper,
        reader: body.parser.as_ref().map(|parser| parser.description()),
    }
}

/// The commands of one feature or of one scoped package, grouped by kind, as
/// [`Environment::commands`] and [`ScopedPackageView::commands`] give them.
#[derive(Debug, Clone, PartialEq, Default, Serialize, schemars::JsonSchema)]
pub struct CommandsView {
    /// The macros, written `\name…`, file-inclusion macros among them.
    pub macros: Vec<CommandView>,
    /// The environments, written `\begin{name}…\end{name}`.
    pub environments: Vec<CommandView>,
    /// The special character sequences, recognized wherever they appear (`~`, `---`).
    pub specials: Vec<CommandView>,
    /// The declarations without syntax: element values that only a stage creates, which
    /// an author never writes.
    pub synthesized: Vec<CommandView>,
}

impl CommandsView {
    fn of(views: impl Iterator<Item = CommandView>) -> CommandsView {
        let mut out = CommandsView::default();
        for view in views {
            match view.kind {
                Some(CallableKind::Macro) => out.macros.push(view),
                Some(CallableKind::Environment) => out.environments.push(view),
                Some(CallableKind::Specials) => out.specials.push(view),
                None => out.synthesized.push(view),
            }
        }
        out
    }
}

/// One declaration of a command: how it is written, what its arguments and body hold, the
/// element its nodes carry, and where it is available.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct CommandView {
    /// The feature that declares it.
    pub feature: String,
    /// The names it is written under, without the backslash; empty for specials and for a
    /// declaration without syntax.
    pub names: Vec<String>,
    /// Its kind; absent for a declaration without syntax.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<CallableKind>,
    /// For specials, the character sequence that triggers it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    /// The parsing modes it is available in (`text`, `math`); empty for a declaration
    /// without syntax.
    pub modes: Vec<String>,
    /// Its documentation text.
    #[serde(skip_serializing_if = "is_empty")]
    pub docs: String,
    /// The first paragraph of [`docs`](CommandView::docs).
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
    /// Its arguments, in the order they are written.
    pub arguments: Vec<ArgumentView>,
    /// For an environment, its body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<BodyView>,
    /// The element value its nodes carry.
    pub element: ElementView,
    /// The scoped package that holds the declaration, for a command defined only inside
    /// certain environments or arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
    /// For a command of a scoped package, every place that makes the package available;
    /// absent for a command a feature declares directly, which is available everywhere in
    /// its modes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub available_in: Vec<UseSiteView>,
    /// Whether the declaration has no syntax: its element value is created only by a
    /// stage, and an author never writes it.
    #[serde(skip_serializing_if = "is_false")]
    pub synthesized: bool,
}

/// One argument of a command or of a tack-on field.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct ArgumentView {
    /// How the argument is written: the argument code it was declared with (`m`, `[`), or,
    /// for an argument declared otherwise (with a structured syntax or a parser), the
    /// parser's description (`a mandatory {…} group`).
    pub form: String,
    /// The argument's structured syntax: its standard kind written as data, in the form
    /// settings declare an argument with (`{"syntax": "delimited-group", "delimiters":
    /// ["(", ")"]}`; see [`ArgStdKind`](ArgStdKind#the-kind-as-data)), whatever form the
    /// argument was declared in, a tack-on argument included (its kind with its fields,
    /// which settings cannot declare). Absent for an argument whose parser is a feature's
    /// own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<ArgStdKind>,
    /// How the argument is written at a use site, with its name standing for its content:
    /// `{title}`, `[options]`, `*`, or, for a tack-on argument, its fields (`\label{target}`).
    /// `…` stands for the content of an argument without a name. The form of an argument of
    /// a standard kind (every argument declared with an argument code among them) follows
    /// from its kind; for an argument whose parser is a feature's own, it is read from the
    /// parser's description and may leave out what the description says beyond its first
    /// code span.
    pub written: String,
    /// The argument's name, by which a feature reads it; absent for an argument read by
    /// position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// How its content is laid out: `inline`, `blocks` or `inline-or-blocks`.
    pub layout: ArgLayout,
    /// The parsing mode its content is read in, when the declaration sets one: `text` or
    /// `math`. Absent when the content is read in the mode the command is written in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// The scoped packages available inside it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scoped: Vec<String>,
    /// Its documentation text.
    #[serde(skip_serializing_if = "is_empty")]
    pub docs: String,
    /// The first paragraph of [`docs`](ArgumentView::docs).
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
    /// Its argument role: the registered name of what it holds (a label, a file path).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Whether the declaration places its content under an opaque wrapper, which keeps the
    /// content as written: no whitespace is collapsed and no typographic substitution is
    /// made in it. A characters-only group is kept as written whatever the declaration
    /// says, as its [`form`](ArgumentView::form) states.
    #[serde(skip_serializing_if = "is_false")]
    pub opaque_wrapper: bool,
    /// For a tack-on argument, the fields that may be written after the command.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldView>,
}

/// One tack-on field: a macro written directly after a command (such as `\label` after
/// `\section`) that the command reads as data.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct FieldView {
    /// The field's name, without the backslash.
    pub name: String,
    /// Whether it may be written more than once.
    #[serde(skip_serializing_if = "is_false")]
    pub repeatable: bool,
    /// Its documentation text.
    #[serde(skip_serializing_if = "is_empty")]
    pub docs: String,
    /// The first paragraph of [`docs`](FieldView::docs).
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
    /// Its arguments, in the order they are written.
    pub arguments: Vec<ArgumentView>,
}

/// The body of an environment.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct BodyView {
    /// How the body is laid out: `inline`, `blocks` or `from-role`.
    pub layout: BodyLayout,
    /// The parsing mode it is read in, when the declaration sets one: `text` or `math`.
    /// Absent when the body is read in the mode the environment is written in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// The scoped packages available inside it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scoped: Vec<String>,
    /// Whether the declaration places the body's content under an opaque wrapper, which
    /// keeps the content as written: no whitespace is collapsed and no typographic
    /// substitution is made in it.
    #[serde(skip_serializing_if = "is_false")]
    pub opaque_wrapper: bool,
    /// The description of the body's reader, for a body not read the ordinary way.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reader: Option<String>,
}

/// The element value that the nodes of a command carry.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct ElementView {
    /// The element type's name.
    pub name: String,
    /// The value's documentation text.
    #[serde(skip_serializing_if = "is_empty")]
    pub docs: String,
    /// The first paragraph of [`docs`](ElementView::docs).
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
    /// The aspects attached to the value (the traits through which stages and rules read
    /// it), in the order attached, each as the Rust path of its trait
    /// (`flm_core::model::aspects::Referencing`). The paths are given for information: the
    /// compiler writes them, and the exact form of such a path may change with the
    /// compiler's version.
    pub aspects: Vec<String>,
    /// The value's number in the registry; absent for a value the registry does not hold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
}

/// One scoped package: a set of commands defined only inside certain environment bodies or
/// arguments, such as `\item` inside a list.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct ScopedPackageView {
    /// The package's dotted name.
    pub name: String,
    /// The feature that declares it.
    pub owner: String,
    /// Its documentation text.
    #[serde(skip_serializing_if = "is_empty")]
    pub docs: String,
    /// The first paragraph of [`docs`](ScopedPackageView::docs).
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
    /// Its commands, by kind.
    pub commands: CommandsView,
    /// Every place that makes it available: the environment bodies and the arguments that
    /// list it.
    pub available_in: Vec<UseSiteView>,
}

/// One place that makes a scoped package available: an environment's body or an argument
/// of a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct UseSiteView {
    /// The feature that declares the command.
    pub feature: String,
    /// The command's kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<CallableKind>,
    /// The command's name (its first name, or its trigger).
    pub command: String,
    /// The scoped package that holds the command, when the command is itself defined only
    /// inside certain environments or arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
    /// For an argument of a tack-on field, the field's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// `body` for an environment's body; otherwise the argument's name, or `#N` (its
    /// position, counting from 1) for an argument without a name.
    pub part: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminated(terminator: &str, repeat_char: Option<char>, opener: &str, head: &str) -> ArgStdKind {
        ArgStdKind::Terminated {
            terminator: terminator.to_string(),
            repeat_char,
            opener: opener.to_string(),
            head: head.to_string(),
            raw: true,
        }
    }

    #[test]
    fn a_standard_kind_is_written_as_its_kind_says() {
        let cases = [
            (ArgStdKind::Mandatory, "{x}"),
            (ArgStdKind::KeyVals, "[x]"),
            (ArgStdKind::Marker { marker: '!' }, "!"),
            (ArgStdKind::Verbatim { delimiters: None }, "{x}"),
            (ArgStdKind::Verbatim { delimiters: Some(('{', '}')) }, "{x}"),
            (ArgStdKind::Verbatim { delimiters: Some(('|', '|')) }, "|x|"),
            (ArgStdKind::OptionalDelimitedGroup { open: '<', close: '>' }, "<x>"),
            (ArgStdKind::Embellishments { markers: vec!['_', '^'] }, "_{…}^{…}"),
            // The terminator alone ends the shortest form; the repeated character is written
            // when the terminator holds nothing else.
            (terminated("\\endphf", None, "", ""), "x\\endphf"),
            (terminated("`", Some('`'), "", ""), "x`"),
            (terminated("", Some('+'), "", ""), "+x+"),
            (terminated("]", Some('='), "[", "]"), "[x]]"),
        ];
        for (kind, expected) in cases {
            assert_eq!(written_from_kind(&kind, "x"), expected, "{kind:?}");
        }
        let label = TackOnField::new("label", vec![ArgDef::named("target", ArgStdKind::CharsGroup)]);
        assert_eq!(written_from_kind(&ArgStdKind::TackOn { fields: vec![label] }, "fields"), "\\label{target}");
    }

    #[test]
    fn a_parser_of_a_feature_s_own_is_written_from_its_description() {
        assert_eq!(written_from_description("an optional `<…>` group of style tokens", "styles"), "<styles>");
        assert_eq!(written_from_description("a size command, or nothing", "size"), "{size}");
    }
}
