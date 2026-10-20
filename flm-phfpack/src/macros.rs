//! One macro of the feature `phfqit`, built-in or declared in the feature's
//! settings, as the feature registers it: a [user definition](Definition) (a
//! macro defined by substitution) together with the list of its arguments that
//! are size slots.
//!
//! A *size slot* is an argument through which a use asks for the size of the
//! macro's delimiters: a size after a backtick, a star, or a size in brackets
//! ([`SlotKind`]). [`Macro::new`] declares on every definition the four
//! placeholders whose content the feature computes at each use (`DelimLeft`,
//! `DelimMiddle`, `DelimRight` and [`CONFIG_PLACEHOLDER`]), so that the
//! macro's templates may write them; the module [`crate::feature`] gives them
//! their content.

use flm_core::lang::mint::{arg_def_from_code, ArgCodeError, ArgDef};
use flm_core::lang::rewrites::userdefs::{Definition, DefinitionKind};

use crate::sizing::{SlotKind, SIDES};
use crate::tables::{Row, RowArg, RowKind};

/// The name of the placeholder with which a template reads a named value of
/// the settings: `#[group.name]{config}`, for example `#[spaceKets.Bar]{config}`.
pub const CONFIG_PLACEHOLDER: &str = "config";

/// One macro of the feature: its user definition, and which of its arguments
/// are size slots.
///
/// Build one with [`Macro::new`] from a list of arguments, or with
/// [`Macro::from_row`] from a row of the built-in tables; the settings build the
/// declared macros ([`PhfqitConfig`](crate::settings::PhfqitConfig)).
#[derive(Debug, Clone)]
pub struct Macro {
    /// The user definition: the name, the arguments, the template, the default
    /// values of optional arguments and the parsing modes.
    pub definition: Definition,
    /// The size slots: each one's argument index (counted from zero) and kind,
    /// in argument order.
    pub slots: Vec<(usize, SlotKind)>,
}

/// One argument of a macro, as passed to [`Macro::new`]: a size slot, or an
/// ordinary argument declaration.
#[derive(Debug, Clone)]
pub enum MacroArg {
    /// A size slot of the given kind.
    Size(SlotKind),
    /// An ordinary argument, declared as [`ArgDef`] (usually read from an
    /// argument code by [`arg_def_from_code`]).
    Arg(ArgDef),
}

impl Macro {
    /// Returns the macro of kind `kind` (a macro or a special character
    /// sequence) called `name`, taking the arguments `args`, with no template
    /// yet. Each size slot becomes the argument its [`SlotKind::arg`] declares.
    ///
    /// The definition declares the four placeholders the feature computes
    /// (`DelimLeft`, `DelimMiddle`, `DelimRight` and `config`) with
    /// [`Definition::extra_placeholder`], each written with a substitution
    /// argument (`#[…]{DelimLeft}`).
    ///
    /// # Errors
    ///
    /// The [`ArgCodeError`] of reading the argument code of a star or bracket
    /// size slot. These codes (`*` and `[`) are the library's own, so the error
    /// does not occur.
    pub fn new(kind: DefinitionKind, name: &str, args: Vec<MacroArg>) -> Result<Macro, ArgCodeError> {
        let mut slots = Vec::new();
        let mut defs = Vec::with_capacity(args.len());
        for (index, arg) in args.into_iter().enumerate() {
            match arg {
                MacroArg::Size(kind) => {
                    slots.push((index, kind));
                    defs.push(kind.arg()?);
                }
                MacroArg::Arg(def) => defs.push(def),
            }
        }
        let mut definition = Definition::new(kind, name, defs)
            .extra_placeholder(CONFIG_PLACEHOLDER, true);
        for side in &SIDES {
            definition = definition.extra_placeholder(side.placeholder, true);
        }
        Ok(Macro { definition, slots })
    }

    /// Returns the macro that a row of the built-in tables declares, with the
    /// row's template, parsing modes, argument number offset and default
    /// values. Each argument code of the row is read with
    /// [`arg_def_from_code`].
    ///
    /// # Errors
    ///
    /// The [`ArgCodeError`] of a code the row writes that is not an argument
    /// code: a fault of the tables, which a test of the crate rules out.
    pub fn from_row(row: &Row) -> Result<Macro, ArgCodeError> {
        let args = row
            .args
            .iter()
            .map(|arg| match *arg {
                RowArg::Size(kind) => Ok(MacroArg::Size(kind)),
                RowArg::Code(code) => arg_def_from_code(code).map(MacroArg::Arg),
                RowArg::Named(name, code) => arg_def_from_code(code).map(|mut def| {
                    def.name = Some(name.to_string());
                    MacroArg::Arg(def)
                }),
            })
            .collect::<Result<Vec<MacroArg>, ArgCodeError>>()?;
        let kind = match row.kind {
            RowKind::Macro => DefinitionKind::Macro,
            RowKind::Specials => DefinitionKind::Specials,
        };
        let mut m = Macro::new(kind, row.name, args)?;
        m.definition =
            m.definition.content(row.template).modes(row.modes).argument_number_offset(row.offset);
        for (key, text) in row.defaults {
            m.definition = m.definition.default(*key, *text);
        }
        Ok(m)
    }
}
