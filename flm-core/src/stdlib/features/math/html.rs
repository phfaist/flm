//! The mark of a formula on the HTML backend: what tells a stylesheet or a
//! script of the page that an element is a formula, whichever processing
//! feature wrote it.
//!
//! Each processing feature (the module documentation of [`math`](super),
//! *Processing mathematics*) writes a formula into the page its own way: the
//! TeX that a renderer in the reader's browser typesets, MathML, text, a
//! drawing. Whatever element it writes as a formula's **outermost element**,
//! that element carries the mark, and nothing is written around it for the
//! mark's sake:
//!
//! - the class of the role [`FORMULA_ROLE`], `flm-math-formula`, after the
//!   feature's own classes;
//! - `data-flm-display`, `inline` or `display`;
//! - for a display-math environment, `data-flm-env`, the environment's name.
//!
//! A stylesheet selects a formula as `.flm-math-formula` and a displayed one as
//! `.flm-math-formula[data-flm-display="display"]`; a script finds the formula
//! around an element with `closest('.flm-math-formula')`. The mark says what
//! an element is and nothing of how it is written: whether a script of the
//! page still has to transform the formula is that script's business, under the
//! conventions for a page updated in place ([a page updated in
//! place](crate::guide::rendering#a-page-updated-in-place)).
//!
//! A feature that writes the outermost element as HTML tags
//! ([`flm.pres.html.tag`](crate::stdlib::backends::html::TAG)) gives it the
//! attributes [`formula_attributes`] answers. The formulas of
//! [`math-passthrough`](crate::math_passthrough) are
//! `flm.pres.math-passthrough` nodes whose payload carries the role, and the
//! HTML contract writes the role's class and the payload's `display` and `env`
//! as these same attributes.

use crate::stdlib::backends::html::{class_of_role, data_attribute};
use crate::stdlib::backends::pres::MathDisplay;

/// The role of a formula's outermost element, `flm.math.formula`, whose class
/// `flm-math-formula` marks a formula in an HTML page.
pub const FORMULA_ROLE: &str = "flm.math.formula";

/// The attributes that mark the outermost element of a formula written as
/// HTML tags: `class`, holding `classes` (the writer's own classes, separated
/// by spaces; empty for none) followed by the class of [`FORMULA_ROLE`];
/// `data-flm-display`, the name of `display`; and, when `env` names the
/// display-math environment the formula is written as, `data-flm-env`.
///
/// ```
/// use flm_core::stdlib::backends::pres::MathDisplay;
/// use flm_core::stdlib::features::math::html::formula_attributes;
///
/// assert_eq!(
///     formula_attributes("my-formula", MathDisplay::Display, Some("align")),
///     [
///         (String::from("class"), String::from("my-formula flm-math-formula")),
///         (String::from("data-flm-display"), String::from("display")),
///         (String::from("data-flm-env"), String::from("align")),
///     ],
/// );
/// ```
pub fn formula_attributes(classes: &str, display: MathDisplay, env: Option<&str>) -> Vec<(String, String)> {
    let role = class_of_role(FORMULA_ROLE);
    let class = if classes.trim().is_empty() { role } else { format!("{} {role}", classes.trim()) };
    let mut attributes = vec![
        (String::from("class"), class),
        (data_attribute("display"), String::from(display.as_str())),
    ];
    if let Some(env) = env {
        attributes.push((data_attribute("env"), String::from(env)));
    }
    attributes
}
