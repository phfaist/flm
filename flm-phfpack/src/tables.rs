//! The built-in macros of the feature `phfqit`, as data: one row per macro,
//! the delimiters of the backtick form, and the named values with their
//! default values.
//!
//! A row ([`Row`]) gives a macro's name, its arguments, its argument number
//! offset, its template, its parsing modes and the default values of its
//! optional arguments. Each row becomes the same kind of macro as a macro
//! declared in the settings ([`crate::settings`]), through
//! [`Macro::from_row`](crate::macros::Macro::from_row); no code reads a
//! particular row. [`TABLES`] lists the tables in the order their macros are
//! registered.

use flm_core::lang::rewrites::userdefs::DefinitionModes;

use crate::sizing::SlotKind;

/// One argument of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowArg {
    /// A size slot of the given kind.
    Size(SlotKind),
    /// An argument read by its argument code (`{`, `r()`, …).
    Code(&'static str),
    /// An argument read by its argument code, under a name the template
    /// writes as `#{name}`: the name first, then the code.
    Named(&'static str, &'static str),
}

/// Whether a row declares a macro (`\name`) or a special character sequence
/// (characters that trigger the definition, such as the backtick of the
/// backtick form).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// A macro, written `\name`.
    Macro,
    /// A special character sequence.
    Specials,
}

/// One built-in macro.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    /// Whether the row declares a macro or a special character sequence.
    pub kind: RowKind,
    /// The macro's name without the backslash, or the characters of the special
    /// character sequence.
    pub name: &'static str,
    /// The arguments, in the order a use writes them.
    pub args: &'static [RowArg],
    /// How many leading arguments the numbered placeholders skip: `#1` is the
    /// argument at this index, counted from zero.
    pub offset: usize,
    /// The template, in the template language of user definitions.
    pub template: &'static str,
    /// The parsing modes the macro is declared for: both, for every macro, so
    /// that it is available in each mode where its template parses; formulas
    /// only, for the backtick form, because a backtick in running text is a
    /// quotation mark.
    pub modes: DefinitionModes,
    /// The default values of optional arguments, each keyed by the argument's
    /// name or number.
    pub defaults: &'static [(&'static str, &'static str)],
}

use RowArg::{Code, Named, Size};
use SlotKind::{Backtick, Bracket, Star, Token};

/// The three size slots of a delimited macro, then one braced argument.
const SIZED_1: &[RowArg] = &[Size(Backtick), Size(Star), Size(Bracket), Code("{")];
/// The three size slots, then two braced arguments.
const SIZED_2: &[RowArg] = &[Size(Backtick), Size(Star), Size(Bracket), Code("{"), Code("{")];
/// The three size slots, then three braced arguments.
const SIZED_3: &[RowArg] =
    &[Size(Backtick), Size(Star), Size(Bracket), Code("{"), Code("{"), Code("{")];
/// One braced argument.
const BRACED: &[RowArg] = &[Code("{")];
/// One argument in parentheses.
const PARENS: &[RowArg] = &[Code("r()")];

/// Returns the row of a delimited macro with the arguments `args` (the three
/// size slots, then the braced arguments) and the template `template`: an
/// offset of 3, so that `#1` is the first braced argument, both parsing
/// modes, and no default value.
const fn delimited(name: &'static str, args: &'static [RowArg], template: &'static str) -> Row {
    Row {
        kind: RowKind::Macro,
        name,
        args,
        offset: 3,
        template,
        modes: DefinitionModes::Both,
        defaults: &[],
    }
}

/// Returns the row of a macro with the arguments `args`, no size slot, and the
/// template `template`, in both parsing modes.
const fn plain(name: &'static str, args: &'static [RowArg], template: &'static str) -> Row {
    Row {
        kind: RowKind::Macro,
        name,
        args,
        offset: 0,
        template,
        modes: DefinitionModes::Both,
        defaults: &[],
    }
}

/// The delimited macros: the three size slots, then the braced arguments that
/// are written between the delimiters.
pub const DELIMITED: &[Row] = &[
    delimited("abs", SIZED_1, r"#[{\lvert}]{DelimLeft}{#1}#[{\rvert}]{DelimRight}"),
    delimited("norm", SIZED_1, r"#[{\lVert}]{DelimLeft}{#1}#[{\rVert}]{DelimRight}"),
    delimited("avg", SIZED_1, r"#[{\langle}]{DelimLeft}{#1}#[{\rangle}]{DelimRight}"),
    delimited("ket", SIZED_1, r"#[{\lvert}]{DelimLeft}{#1}#[{\rangle}]{DelimRight}"),
    delimited("bra", SIZED_1, r"#[{\langle}]{DelimLeft}{#1}#[{\rvert}]{DelimRight}"),
    delimited(
        "braket",
        SIZED_2,
        r"#[{\langle}]{DelimLeft}{#1}#[spaceKets.Bar]{config}#[\vert]{DelimMiddle}#[spaceKets.Bar]{config}{#2}#[{\rangle}]{DelimRight}",
    ),
    delimited(
        "ketbra",
        SIZED_2,
        r"#[{\lvert}]{DelimLeft}{#1}#[\rangle]{DelimMiddle} #[spaceKets.RLAngle]{config}#[\langle]{DelimMiddle}{#2}#[{\rvert}]{DelimRight}",
    ),
    delimited(
        "proj",
        SIZED_1,
        r"#[{\lvert}]{DelimLeft}{#1}#[\rangle]{DelimMiddle} #[spaceKets.RLAngle]{config}#[\langle]{DelimMiddle}{#1}#[{\rvert}]{DelimRight}",
    ),
    delimited(
        "matrixel",
        SIZED_3,
        r"#[{\langle}]{DelimLeft}{#1}#[spaceKets.Bar]{config}#[\vert]{DelimMiddle} #[spaceKets.Bar]{config}{#2}#[spaceKets.Bar]{config}#[\vert]{DelimMiddle} #[spaceKets.Bar]{config}{#3}#[{\rangle}]{DelimRight}",
    ),
    delimited(
        "dmatrixel",
        SIZED_2,
        r"#[{\langle}]{DelimLeft}{#1}#[spaceKets.Bar]{config}#[\vert]{DelimMiddle} #[spaceKets.Bar]{config}{#2}#[spaceKets.Bar]{config}#[\vert]{DelimMiddle} #[spaceKets.Bar]{config}{#1}#[{\rangle}]{DelimRight}",
    ),
    delimited(
        "innerprod",
        SIZED_2,
        r"#[{\langle}]{DelimLeft}{#1}#[space.BeforeComma]{config},#[space.AfterComma]{config}{#2}#[{\rangle}]{DelimRight}",
    ),
    delimited("oket", SIZED_1, r"#[{\lvert}]{DelimLeft}{#1}#[{\rrangle}]{DelimRight}"),
    delimited("obra", SIZED_1, r"#[{\llangle}]{DelimLeft}{#1}#[{\rvert}]{DelimRight}"),
    delimited(
        "obraket",
        SIZED_2,
        r"#[{\llangle}]{DelimLeft}{#1}#[spaceOKets.Bar]{config}#[\vert]{DelimMiddle} #[spaceOKets.Bar]{config}{#2}#[{\rrangle}]{DelimRight}",
    ),
    delimited(
        "oketbra",
        SIZED_2,
        r"#[{\lvert}]{DelimLeft}{#1}#[\rrangle]{DelimMiddle} #[spaceOKets.RLAngle]{config}#[\llangle]{DelimMiddle}{#2}#[{\rvert}]{DelimRight}",
    ),
    delimited(
        "oproj",
        SIZED_1,
        r"#[{\lvert}]{DelimLeft}{#1}#[\rrangle]{DelimMiddle} #[spaceOKets.RLAngle]{config}#[\llangle]{DelimMiddle}{#1}#[{\rvert}]{DelimRight}",
    ),
    delimited(
        "omatrixel",
        SIZED_3,
        r"#[{\llangle}]{DelimLeft}{#1}#[spaceOKets.Bar]{config}#[\vert]{DelimMiddle} #[spaceOKets.Bar]{config}{#2}#[spaceOKets.Bar]{config}#[\vert]{DelimMiddle} #[spaceOKets.Bar]{config}{#3}#[{\rrangle}]{DelimRight}",
    ),
    delimited(
        "odmatrixel",
        SIZED_2,
        r"#[{\llangle}]{DelimLeft}{#1}#[spaceOKets.Bar]{config}#[\vert]{DelimMiddle} #[spaceOKets.Bar]{config}{#2}#[spaceOKets.Bar]{config}#[\vert]{DelimMiddle} #[spaceOKets.Bar]{config}{#1}#[{\rrangle}]{DelimRight}",
    ),
    delimited("intervalc", SIZED_2, r"#[{[}]{DelimLeft}{#1\mathclose{},\mathopen{}#2}#[{]}]{DelimRight}"),
    delimited("intervalo", SIZED_2, r"#[{]}]{DelimLeft}{#1\mathclose{},\mathopen{}#2}#[{[}]{DelimRight}"),
    delimited("intervalco", SIZED_2, r"#[{[}]{DelimLeft}{#1\mathclose{},\mathopen{}#2}#[{[}]{DelimRight}"),
    delimited("intervaloc", SIZED_2, r"#[{]}]{DelimLeft}{#1\mathclose{},\mathopen{}#2}#[{]}]{DelimRight}"),
];

/// The macros that write a fixed symbol or wrap their argument: a symbol, a
/// bit or bit string, a gate, a group or algebra.
pub const SUBSTITUTIONS: &[Row] = &[
    plain("Hs", &[], r"\mathscr{H}"),
    plain("Ident", &[], "\u{1D7D9}"),
    plain("bit", BRACED, r"\texttt{#1}"),
    plain("bitstring", BRACED, r"\ensuremath{\underline{\overline{\texttt{#1}}}}"),
    plain("gate", BRACED, r"\textsc{#1}"),
    plain("AND", &[], r"\textsc{and}"),
    plain("XOR", &[], r"\textsc{xor}"),
    plain("CNOT", &[], r"\textsc{c-not}"),
    plain("NOT", &[], r"\textsc{not}"),
    plain("NOOP", &[], r"\textsc{no-op}"),
    plain("uu", PARENS, r"\mathrm{u}({#1})"),
    plain("UU", PARENS, r"\mathrm{U}({#1})"),
    plain("su", PARENS, r"\mathrm{su}({#1})"),
    plain("SU", PARENS, r"\mathrm{SU}({#1})"),
    plain("so", PARENS, r"\mathrm{so}({#1})"),
    plain("SO", PARENS, r"\mathrm{SO}({#1})"),
    plain("slalg", PARENS, r"\mathrm{sl}({#1})"),
    plain("SL", PARENS, r"\mathrm{SL}({#1})"),
    plain("GL", PARENS, r"\mathrm{GL}({#1})"),
    plain("SN", PARENS, r"\mathrm{S}_{#1}"),
];

/// The operators, each written with `\operatorname`.
pub const OPERATORS: &[Row] = &[
    plain("tr", &[], r"\operatorname{tr}"),
    plain("supp", &[], r"\operatorname{supp}"),
    plain("rank", &[], r"\operatorname{rank}"),
    plain("linspan", &[], r"\operatorname{span}"),
    plain("spec", &[], r"\operatorname{spec}"),
    plain("diag", &[], r"\operatorname{diag}"),
    plain("Re", &[], r"\operatorname{Re}"),
    plain("Im", &[], r"\operatorname{Im}"),
    plain("poly", &[], r"\operatorname{poly}"),
];

/// Returns the row of a quantity macro: the arguments `args`, `#1` the
/// argument at index `offset`, the template `template`, the default values
/// `defaults`, and both parsing modes.
const fn quantity(
    name: &'static str,
    args: &'static [RowArg],
    offset: usize,
    template: &'static str,
    defaults: &'static [(&'static str, &'static str)],
) -> Row {
    Row { kind: RowKind::Macro, name, args, offset, template, modes: DefinitionModes::Both, defaults }
}

/// The arguments of an entropy: the size after a backtick, `[state]`,
/// `[epsilon]`, `{target}`, `[cond]`.
const ENTROPY: &[RowArg] = &[Size(Backtick), Code("["), Code("["), Code("{"), Code("[")];
/// The arguments of a relative entropy with a fixed subscript: `[epsilon]`,
/// the size after a backtick, `{rho}`, `{sigma}`.
const RELATIVE_ENTROPY: &[RowArg] = &[Code("["), Size(Backtick), Code("{"), Code("{")];

// The parenthesis of a quantity, with what stands in it and after it, is
// written when at least one of the parts it holds is given, as the LaTeX
// package renders it; the template language's conditional tests one
// placeholder, so the test of several parts nests one conditional per part,
// each writing the same parenthesis. These macros assemble the template of a
// row from its symbol when the crate compiles, so that each parenthesis is
// written once here. See decision rationale at
// [§flm-dd-dr:feat-phfqit-quantity-templates]

/// The parenthesis of an entropy and the state's subscript after it:
/// `#3` the target, `#4` the conditioning system, `#1` the state.
macro_rules! entropy_parenthesis {
    () => {
        r"#[(]{DelimLeft}#3\notblank{#4}{\,#[\vert]{DelimMiddle}\,#4}{}#[)]{DelimRight}\notblank{#1}{_{#1}}{}"
    };
}

/// The template of an entropy written `$symbol`: the symbol, `^{epsilon}`
/// when given, then the parenthesis when the state, the target or the
/// conditioning system is given.
macro_rules! entropy {
    ($symbol:literal) => {
        concat!(
            $symbol,
            r"\notblank{#2}{^{#2}}{}",
            r"\notblank{#1}{",
            entropy_parenthesis!(),
            r"}{\notblank{#3}{",
            entropy_parenthesis!(),
            r"}{\notblank{#4}{",
            entropy_parenthesis!(),
            r"}{}}}"
        )
    };
}

/// The parenthesis of the mutual information and the state's subscript after
/// it: `#3` and `#4` the two systems, `#5` the conditioning system, `#1` the
/// state.
macro_rules! information_parenthesis {
    () => {
        r"#[(]{DelimLeft}#3\,:\,#4\notblank{#5}{\,#[\vert]{DelimMiddle}\,#5}{}#[)]{DelimRight}\notblank{#1}{_{#1}}{}"
    };
}

/// The parenthesis of a relative entropy: `#3` and `#4` the two states.
macro_rules! relative_entropy_parenthesis {
    () => {
        r"#[(]{DelimLeft}#3\notblank{#4}{\,#[\Vert]{DelimMiddle}\,#4}{}#[)]{DelimRight}"
    };
}

/// The template of a relative entropy written `$head` (the symbol with its
/// scripts), then the parenthesis when either state is given.
macro_rules! relative_entropy {
    ($head:literal) => {
        concat!(
            $head,
            r"\notblank{#3}{",
            relative_entropy_parenthesis!(),
            r"}{\notblank{#4}{",
            relative_entropy_parenthesis!(),
            r"}{}}"
        )
    };
}

/// The quantity macros: entropies, information measures, the identity process,
/// the exponential and the trivial system. A sub- or superscript that would
/// hold nothing is not written, and the parenthesis of a quantity is left out
/// when every part it holds is blank (`\Hmaxf{}` is the symbol alone); every
/// other part is written as the LaTeX package `phfqit` renders it.
// See decision rationale at [§flm-dd-dr:feat-phfqit-quantity-templates]
pub const QUANTITIES: &[Row] = &[
    quantity("HH", ENTROPY, 1, entropy!("H"), &[]),
    quantity("Hmin", ENTROPY, 1, entropy!(r"H_{\mathrm{min}}"), &[]),
    quantity("Hzero", ENTROPY, 1, entropy!(r"H_{\mathrm{max},0}"), &[]),
    quantity("Hmaxf", ENTROPY, 1, entropy!(r"H_{\mathrm{max}}"), &[]),
    quantity(
        "Hfn",
        &[Size(Backtick), Code("r()")],
        1,
        r"H\notblank{#1}{#[(]{DelimLeft}#1#[)]{DelimRight}}{}",
        &[],
    ),
    quantity(
        "II",
        &[Size(Backtick), Code("["), Code("["), Code("{"), Code("{"), Code("[")],
        1,
        concat!(
            r"I\notblank{#2}{^{#2}}{}",
            r"\notblank{#1}{",
            information_parenthesis!(),
            r"}{\notblank{#3}{",
            information_parenthesis!(),
            r"}{\notblank{#4}{",
            information_parenthesis!(),
            r"}{\notblank{#5}{",
            information_parenthesis!(),
            r"}{}}}}"
        ),
        &[],
    ),
    quantity(
        "DD",
        &[Code("e{_^}"), Size(Backtick), Code("{"), Code("{")],
        0,
        relative_entropy!("D#1"),
        &[],
    ),
    quantity(
        "Dmax",
        RELATIVE_ENTROPY,
        0,
        relative_entropy!(r"D_{\mathrm{max}}\notblank{#1}{^{#1}}{}"),
        &[],
    ),
    quantity(
        "Dminz",
        RELATIVE_ENTROPY,
        0,
        relative_entropy!(r"D_{\mathrm{min,0}}\notblank{#1}{^{#1}}{}"),
        &[],
    ),
    quantity(
        "Dminf",
        RELATIVE_ENTROPY,
        0,
        relative_entropy!(r"D_{\mathrm{min}}\notblank{#1}{^{#1}}{}"),
        &[],
    ),
    quantity(
        "Dr",
        RELATIVE_ENTROPY,
        0,
        relative_entropy!(r"D_{\mathrm{r}}\notblank{#1}{^{#1}}{}"),
        &[],
    ),
    quantity(
        "DHyp",
        RELATIVE_ENTROPY,
        0,
        relative_entropy!(r"D_{\mathrm{H}}\notblank{#1}{^{#1}}{}"),
        &[("1", r"\eta")],
    ),
    quantity(
        "Dhyp",
        RELATIVE_ENTROPY,
        0,
        relative_entropy!(r"D_{\mathrm{h}}\notblank{#1}{^{#1}}{}"),
        &[("1", r"\eta")],
    ),
    quantity(
        "IdentProc",
        &[Size(Backtick), Code("["), Code("["), Code("{")],
        1,
        r"\operatorname{id}\notblank{#1}{_{#1\notblank{#2}{\to #2}{}}}{\notblank{#2}{_{\to #2}}{}}\notblank{#3}{#[(]{DelimLeft}#3#[)]{DelimRight}}{}",
        &[],
    ),
    quantity("ee", &[Code("e{^}")], 0, r"e#1", &[]),
    quantity("emptysystem", &[], 0, r"\emptyset", &[]),
];

/// The name of the backtick form's argument whose own delimiters the template
/// writes: a delimiter placeholder written without a delimiter in brackets
/// (`#{DelimLeft}`) writes the delimiter that corresponds to the one opening
/// this argument ([`MAIN_DELIMITERS`]).
pub const MAIN_DELIMITED_ARGUMENT: &str = "MainDelimitedArgument";

/// The backtick form, a special character sequence available in formulas
/// only: a backtick, an optional size token, and a group in braces, brackets,
/// parentheses or angle brackets, written between its own delimiters at the
/// size asked for.
pub const BACKTICK: &[Row] = &[Row {
    kind: RowKind::Specials,
    name: "`",
    args: &[Size(Token), Named(MAIN_DELIMITED_ARGUMENT, "AnyDelimited")],
    offset: 0,
    template: "#{DelimLeft}#{MainDelimitedArgument}#{DelimRight}",
    modes: DefinitionModes::Math,
    defaults: &[],
}];

/// Every table of built-in macros, in the order they are registered.
pub const TABLES: &[&[Row]] = &[DELIMITED, SUBSTITUTIONS, OPERATORS, QUANTITIES, BACKTICK];

/// The delimiters that a delimiter placeholder written without a delimiter in
/// brackets writes, by the delimiter that opens the [main delimited
/// argument](MAIN_DELIMITED_ARGUMENT): for each opening delimiter, the
/// delimiter written on the left, in the middle and on the right.
pub const MAIN_DELIMITERS: &[(&str, [&str; 3])] = &[
    ("{", [r"\{", "|", r"\}"]),
    ("[", ["[", "|", "]"]),
    ("(", ["(", "|", ")"]),
    ("<", [r"\langle", "|", r"\rangle"]),
];

/// The named values a template reads with `#[group.name]{config}`: each group
/// (a key of the settings) with its entries and their default values.
pub const VALUES: &[(&str, &[(&str, &str)])] = &[
    ("spaceKets", &[("Bar", r"\mkern 1.5mu "), ("RLAngle", r"\mkern -1.8mu ")]),
    ("spaceOKets", &[("Bar", r"\mkern 1.5mu "), ("RLAngle", r"\mkern -1.8mu ")]),
    ("space", &[("BeforeComma", ""), ("AfterComma", r"\mkern 1.5mu ")]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_hold_fifty_one_notation_macros_sixteen_quantities_and_the_backtick_form() {
        assert_eq!(DELIMITED.len() + SUBSTITUTIONS.len() + OPERATORS.len(), 51);
        assert_eq!(QUANTITIES.len(), 16);
        assert_eq!(BACKTICK.len(), 1);
        let macros = TABLES.iter().flat_map(|table| table.iter()).filter(|row| row.kind == RowKind::Macro);
        assert_eq!(macros.count(), 51 + 16);
    }
}
