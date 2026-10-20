//! Lengths spelled for LaTeX: one function, [`latex_length`], used for every
//! length the `latex-rendered` backend writes (a line's indentations, an
//! image's width and height).

use crate::stdcore::backends::pres::{Length, LengthUnit};

/// The LaTeX spelling of `length`: a tab as a multiple of `\flmxTabWidth`
/// (`2\flmxTabWidth`, `0.5\flmxTabWidth`), an em as `<n>em`, a point as
/// `<n>bp` (FLM's point is CSS's, 1/72 inch, which is LaTeX's `bp`), and
/// millimeters, centimeters and inches as they stand. The amount is the
/// length's shortest decimal that reads back to it, as the length's own
/// printing writes it.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_rendered::length::latex_length;
/// use flm_core::stdcore::backends::pres::Length;
///
/// assert_eq!(latex_length(&Length::tabs(2)), "2\\flmxTabWidth");
/// assert_eq!(latex_length(&Length::parse("12pt").unwrap()), "12bp");
/// assert_eq!(latex_length(&Length::parse("4.5cm").unwrap()), "4.5cm");
/// ```
pub fn latex_length(length: &Length) -> String {
    let amount = length.amount_decimal();
    match length.unit {
        LengthUnit::Tabs => format!("{amount}\\flmxTabWidth"),
        LengthUnit::Ems => format!("{amount}em"),
        LengthUnit::Points => format!("{amount}bp"),
        LengthUnit::Millimeters => format!("{amount}mm"),
        LengthUnit::Centimeters => format!("{amount}cm"),
        LengthUnit::Inches => format!("{amount}in"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_unit_is_spelled_for_latex() {
        let spelled = |text: &str| latex_length(&Length::parse(text).expect("a length"));
        assert_eq!(spelled("1tab"), "1\\flmxTabWidth");
        assert_eq!(spelled("0.5tab"), "0.5\\flmxTabWidth");
        assert_eq!(spelled("1.5em"), "1.5em");
        assert_eq!(spelled("10pt"), "10bp");
        assert_eq!(spelled("3mm"), "3mm");
        assert_eq!(spelled("2cm"), "2cm");
        assert_eq!(spelled("1in"), "1in");
        assert_eq!(spelled("-2em"), "-2em");
        assert_eq!(spelled("0tab"), "0\\flmxTabWidth");
    }
}
