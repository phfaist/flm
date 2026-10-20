//! The cell styles of the tables of cells: what a style token written in
//! the angle brackets of `\cell<…>` or `\celldata<…>` states about a cell.
//!
//! Every style token an author writes refers by name to a **cell style**, a
//! declaration ([`CellStyle`]) contributed to the feature's service slot
//! [`STYLES_SLOT`]. A service slot is the declaration, by the feature that
//! owns an interaction, of a dotted name, the type of the values the slot
//! takes and its arity; any feature, and the program embedding the library,
//! may contribute values to it (see [services](crate::core::services)). A
//! cell style says what the
//! token states about the cell, one of four [`Effect`]s, each a fact that
//! holds regardless of the output format, and lists the definitions a
//! backend needs where a cell uses the style ([`CellStyle::requires`]): for
//! example a style definition on HTML and a definition piece on the
//! recomposed LaTeX. A
//! cell style carries no code, so no cell style can change a cell's content
//! or the table's model.
//!
//! A token is a name, optionally followed by a value in braces: `r`,
//! `align{right}`. A cell style of the fixed form ([`StyleForm::Fixed`])
//! takes no value; one of the structural form ([`StyleForm::Structural`])
//! takes exactly one, which its [`Parameter`] reads. The feature ships
//! ([`shipped`]):
//!
//! - three structural styles: `align{left|center|right}`, `head{column|row}`
//!   and `rule{above|below|left|right}` (a rule: a line drawn along an edge
//!   of the cell);
//! - their short forms: `l`, `c`, `r`, `H` (a column header), `rH` (a row
//!   header), `lvert` and `rvert` (a rule at the left or the right edge);
//! - four custom styles, `green`, `blue`, `yellow` and `red`, to which the
//!   feature's HTML style definition and its LaTeX definition piece give a
//!   background.
//!
//! The [guide on extending FLM](crate::guide::extending#a-cell-style-of-your-own)
//! shows how a feature declares a cell style of its own.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use crate::core::registry::{BuildError, FeatureBuilder, Finalizer};
use crate::model::names::{BackendName, Name};
use crate::stdcore::backends::pres::{CellAlign, CellEdge, CellHeader};
use crate::stdcore::backends::{html, latex_recomposed, latex_rendered};

use super::{LATEX_PIECE, STYLE, STYLES_SLOT};

/// What a style token states about a cell: a fact that holds regardless of
/// the output format, which each backend renders in its own way or
/// ignores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Effect {
    /// The cell's alignment. Of the tokens of one cell that give an
    /// alignment, the first decides.
    Align(CellAlign),
    /// The cell is a header of its column or of its row. Of the tokens of one
    /// cell that give a header kind, the first decides.
    Header(CellHeader),
    /// A rule is drawn at this edge of the cell. Every edge that a token lists is drawn.
    Rule(CellEdge),
    /// The cell carries the style by its name. Each backend writes the name
    /// in a form of its own (the attribute `data-flm-style-<name>` on HTML,
    /// the table command `\flmCellsSet{<name>}` at the start of the cell's
    /// text on the recomposed LaTeX), and the definitions that the style
    /// requires on that backend give the name its meaning; the text backend
    /// shows nothing. Every custom style a cell's tokens refer to is carried,
    /// in the order of the tokens.
    Custom,
}

/// The parameter of a structural style: which values its token takes, and the
/// effect of each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Parameter {
    /// `left`, `center` or `right`: the cell's alignment.
    Align,
    /// `column` or `row`: the cell heads its column or its row.
    Head,
    /// `above`, `below`, `left` or `right`: a rule at that edge.
    Rule,
}

impl Parameter {
    /// The values the parameter takes, in order.
    pub fn values(self) -> &'static [&'static str] {
        match self {
            Parameter::Align => &["left", "center", "right"],
            Parameter::Head => &["column", "row"],
            Parameter::Rule => &["above", "below", "left", "right"],
        }
    }

    /// The effect of the value `value`, when the parameter takes it.
    pub fn effect(self, value: &str) -> Option<Effect> {
        match self {
            Parameter::Align => CellAlign::parse(value).map(Effect::Align),
            Parameter::Head => CellHeader::parse(value).map(Effect::Header),
            Parameter::Rule => CellEdge::parse(value).map(Effect::Rule),
        }
    }
}

/// How a style token is written: with no value, or with one value in braces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StyleForm {
    /// A token with no value, stating one effect: `l`, `H`, `green`.
    Fixed(Effect),
    /// A token with one value in braces, which the parameter reads:
    /// `align{left}`. The feature's own three styles have this form.
    Structural(Parameter),
}

/// One definition that a backend needs where a cell carries a cell style:
/// the backend, and the identifier of the definition it requires there (a
/// style or script definition on HTML, a definition piece or a package
/// request on the recomposed LaTeX).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StyleRequirement {
    /// The backend the definition belongs to (`html`, `latex-recomposed`).
    pub backend: BackendName,
    /// The definition's identifier.
    pub id: Name,
}

/// A cell style: the name an author writes in a cell's angle brackets, what
/// it states about the cell, and the definitions a backend needs where a cell
/// carries it.
///
/// A feature contributes one to the service slot [`STYLES_SLOT`] with
/// `b.peer_slot::<CellStyle>(STYLES_SLOT, Arc::new(style))`: a peer
/// contribution, a contribution to a service slot another feature owns,
/// which the registry build drops with one Note when that feature is not
/// registered, so that the contributing feature still builds in a registry
/// without the `cells` feature. Two cell styles of one name are a registry
/// build error, whoever contributes them, so a cell style the feature ships
/// cannot be declared again. What a shipped custom style shows is changed
/// through its definitions instead: on HTML, the custom properties of the
/// feature's stylesheet (`--flm-cells-green` and the three others); on the
/// recomposed LaTeX, the command `\flmCellsKeys<name>` (see the
/// [rendering guide](crate::guide::rendering#tables-of-cells-in-the-recomposed-latex)).
///
/// # Examples
///
/// A custom style `orange`, which a style definition `host.orange` colors
/// on HTML and a definition piece `host.latex.orange` on the recomposed
/// LaTeX:
///
/// ```
/// use flm_core::stdcore::backends::{html, latex_recomposed};
/// use flm_core::stdcore::features::cells::{CellStyle, Effect, StyleForm};
///
/// let orange = CellStyle::custom("orange", "an orange background")
///     .requires(html::BACKEND, ["host.orange"])
///     .requires(latex_recomposed::BACKEND, ["host.latex.orange"]);
/// assert_eq!(orange.form, StyleForm::Fixed(Effect::Custom));
/// assert_eq!(orange.requirements_on(html::BACKEND).count(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellStyle {
    /// The token's name: ASCII letters and digits, beginning with a letter.
    /// A custom style's name is lowercase, since it becomes part of the name
    /// of an HTML attribute, which HTML reads without regard to case.
    pub name: Name,
    /// What the style does, as a sentence, for generated reference
    /// documentation. The constructors ([`fixed`](CellStyle::fixed),
    /// [`custom`](CellStyle::custom), [`structural`](CellStyle::structural))
    /// leave it empty when the crate is built without its cargo feature `docs`.
    pub docs: String,
    /// How the token is written and what it states.
    pub form: StyleForm,
    /// The definitions each backend needs where a cell carries the style.
    /// The rule that lowers a table requires those of the backend it runs on,
    /// and none of any other backend.
    pub requires: Vec<StyleRequirement>,
}

impl CellStyle {
    /// A cell style of the fixed form, stating `effect` and requiring
    /// nothing.
    pub fn fixed(name: impl Into<Name>, docs: impl Into<String>, effect: Effect) -> CellStyle {
        CellStyle {
            name: name.into(),
            docs: crate::kept_docs(docs),
            form: StyleForm::Fixed(effect),
            requires: Vec::new(),
        }
    }

    /// A custom style: a cell style of the fixed form that the cell carries
    /// by its name ([`Effect::Custom`]), requiring nothing yet; add its
    /// definitions with [`requires`](CellStyle::requires).
    pub fn custom(name: impl Into<Name>, docs: impl Into<String>) -> CellStyle {
        CellStyle::fixed(name, docs, Effect::Custom)
    }

    /// A cell style of the structural form, which reads its value through
    /// `parameter`.
    pub fn structural(name: impl Into<Name>, docs: impl Into<String>, parameter: Parameter) -> CellStyle {
        CellStyle {
            name: name.into(),
            docs: crate::kept_docs(docs),
            form: StyleForm::Structural(parameter),
            requires: Vec::new(),
        }
    }

    /// Builder: adds the definitions `ids`, which `backend` needs where a
    /// cell carries the style.
    pub fn requires(
        mut self,
        backend: impl Into<BackendName>,
        ids: impl IntoIterator<Item = impl Into<Name>>,
    ) -> CellStyle {
        let backend = backend.into();
        self.requires
            .extend(ids.into_iter().map(|id| StyleRequirement { backend: backend.clone(), id: id.into() }));
        self
    }

    /// The identifiers of the definitions `backend` needs where a cell
    /// carries the style, in order.
    pub fn requirements_on<'s>(&'s self, backend: &'s str) -> impl Iterator<Item = &'s Name> + 's {
        self.requires.iter().filter(move |r| r.backend.as_str() == backend).map(|r| &r.id)
    }
}

/// Whether `name` is a style's name: an ASCII letter, then ASCII letters and
/// digits.
pub(super) fn is_style_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric())
}

/// The cell styles the feature ships, in the order it contributes them:
/// `align`, `head`, `rule`; `l`, `c`, `r`, `H`, `rH`, `lvert`, `rvert`; and the
/// four custom styles `green`, `blue`, `yellow`, `red`, each requiring the
/// feature's style definition [`STYLE`] on HTML and on the rendered LaTeX
/// (where the feature's piece delivers it) and its definition piece
/// [`LATEX_PIECE`] on the recomposed LaTeX.
pub fn shipped() -> Vec<CellStyle> {
    let mut styles = vec![
        CellStyle::structural("align", "the cell's alignment: `align{left}`, `align{center}`, `align{right}`", Parameter::Align),
        CellStyle::structural("head", "the cell heads its column or its row: `head{column}`, `head{row}`", Parameter::Head),
        CellStyle::structural(
            "rule",
            "a line drawn along an edge of the cell: `rule{above}`, `rule{below}`, `rule{left}`, \
             `rule{right}`",
            Parameter::Rule,
        ),
        CellStyle::fixed("l", "the cell aligned left, as `align{left}`", Effect::Align(CellAlign::Left)),
        CellStyle::fixed("c", "the cell centered, as `align{center}`", Effect::Align(CellAlign::Center)),
        CellStyle::fixed("r", "the cell aligned right, as `align{right}`", Effect::Align(CellAlign::Right)),
        CellStyle::fixed("H", "the cell heads its column, as `head{column}`", Effect::Header(CellHeader::Column)),
        CellStyle::fixed("rH", "the cell heads its row, as `head{row}`", Effect::Header(CellHeader::Row)),
        CellStyle::fixed(
            "lvert",
            "a line drawn along the cell's left edge, as `rule{left}`",
            Effect::Rule(CellEdge::Left),
        ),
        CellStyle::fixed(
            "rvert",
            "a line drawn along the cell's right edge, as `rule{right}`",
            Effect::Rule(CellEdge::Right),
        ),
    ];
    for color in ["green", "blue", "yellow", "red"] {
        styles.push(
            CellStyle::custom(color, format!("a {color} background, which sets the cell apart"))
                .requires(html::BACKEND, [STYLE])
                .requires(latex_recomposed::BACKEND, [LATEX_PIECE])
                .requires(latex_rendered::BACKEND, [STYLE]),
        );
    }
    styles
}

/// The cell styles of a registry, by name: every contribution to the service
/// slot [`STYLES_SLOT`], collected by the feature's build hook
/// ([`Finalizer`]) once the slot holds them all. The feature's rules hold
/// this collection, because a rule has no access to the registry's services
/// while it runs.
#[derive(Default)]
pub struct Styles {
    by_name: OnceLock<BTreeMap<Name, Arc<CellStyle>>>,
}

impl Styles {
    /// A collection filled from `styles` directly, for a caller that reads a
    /// table of cells outside a registry, such as a test of the reader
    /// ([`read_grid`](super::read_grid)).
    pub fn of(styles: impl IntoIterator<Item = CellStyle>) -> Styles {
        let by_name = styles.into_iter().map(|style| (style.name.clone(), Arc::new(style))).collect();
        let collected = Styles::default();
        let _ = collected.by_name.set(by_name);
        collected
    }

    /// The cell style of the name `name`, if the registry holds one.
    pub fn get(&self, name: &str) -> Option<&CellStyle> {
        self.by_name.get()?.get(name).map(|style| style.as_ref())
    }

    /// Every cell style, in the order of their names.
    pub fn iter(&self) -> impl Iterator<Item = &CellStyle> + '_ {
        self.by_name.get().into_iter().flat_map(|by_name| by_name.values().map(|style| style.as_ref()))
    }
}

impl fmt::Debug for Styles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.iter().map(|style| style.name.as_str()).collect();
        f.debug_struct("Styles").field("names", &names).finish()
    }
}

/// The feature's build hook ([`Finalizer`]): it reads every contribution to
/// the service slot [`STYLES_SLOT`] into the feature's [`Styles`], and fails
/// the registry build on a name that is not a valid style name, on a custom
/// style's name that is not lowercase, and on two cell styles of one name.
pub(super) struct CollectStyles {
    pub(super) styles: Arc<Styles>,
}

impl Finalizer for CollectStyles {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let refuse = |b: &FeatureBuilder<'_>, detail: String| BuildError::Feature {
            feature: b.feature().clone(),
            detail,
        };
        let Some(services) = b.services() else {
            return Err(refuse(b, "the styles slot must be assembled before the finalizer runs".into()));
        };
        let mut by_name: BTreeMap<Name, Arc<CellStyle>> = BTreeMap::new();
        for style in services.shared_named::<CellStyle>(STYLES_SLOT) {
            let name = style.name.as_str();
            if !is_style_name(name) {
                return Err(refuse(
                    b,
                    format!(
                        "the cell style name `{name}` must begin with an ASCII letter and hold only \
                         ASCII letters and digits"
                    ),
                ));
            }
            if style.form == StyleForm::Fixed(Effect::Custom) && name.chars().any(|ch| ch.is_ascii_uppercase()) {
                return Err(refuse(b, format!("the custom cell style name `{name}` must be lowercase")));
            }
            if by_name.insert(style.name.clone(), Arc::clone(style)).is_some() {
                return Err(refuse(b, format!("the cell style `{name}` may not be declared twice")));
            }
        }
        if self.styles.by_name.set(by_name).is_err() {
            return Err(refuse(b, "the feature may be registered at most once in a registry".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_styles_are_the_python_tokens_and_the_three_structural_ones() {
        let names: Vec<String> = shipped().iter().map(|style| style.name.to_string()).collect();
        assert_eq!(
            names,
            ["align", "head", "rule", "l", "c", "r", "H", "rH", "lvert", "rvert", "green", "blue", "yellow", "red"]
        );
        assert!(shipped().iter().all(|style| is_style_name(style.name.as_str())));
    }

    #[test]
    fn a_custom_shipped_style_requires_the_chunk_on_html_and_the_piece_on_latex() {
        let green = shipped().into_iter().find(|style| style.name.as_str() == "green").expect("green");
        assert_eq!(green.requirements_on(html::BACKEND).map(Name::as_str).collect::<Vec<_>>(), [STYLE]);
        assert_eq!(
            green.requirements_on(latex_recomposed::BACKEND).map(Name::as_str).collect::<Vec<_>>(),
            [LATEX_PIECE]
        );
        assert_eq!(
            green.requirements_on(latex_rendered::BACKEND).map(Name::as_str).collect::<Vec<_>>(),
            [STYLE]
        );
        assert_eq!(green.requirements_on("text").count(), 0);
    }

    #[test]
    fn a_parameter_reads_its_values() {
        assert_eq!(Parameter::Align.effect("right"), Some(Effect::Align(CellAlign::Right)));
        assert_eq!(Parameter::Head.effect("row"), Some(Effect::Header(CellHeader::Row)));
        assert_eq!(Parameter::Rule.effect("above"), Some(Effect::Rule(CellEdge::Above)));
        assert_eq!(Parameter::Align.effect("middle"), None);
    }
}
