//! The escaping of verbatim text for the `latex-rendered` backend.
//!
//! Verbatim text is never written under LaTeX's verbatim catcodes, so that it
//! may stand anywhere: in a list item, a footnote, a table cell, a template's
//! argument. [`escape_verbatim`] writes it as ordinary LaTeX text in which every
//! character stands for itself.

/// `text` escaped for an argument of `\flmxVerbatim` or `\flmxVerbLine`: the
/// ten special characters as the LaTeX family's text escaping writes them
/// (`\{`, `\}`, `\%`, `\&`, `\#`, `\_`, `\$`, `\textbackslash{}`,
/// `\textasciitilde{}`, `\textasciicircum{}`), a space as `\ ` (a control
/// space, so that runs of spaces keep their width), a tab as four of them,
/// and every other character as it is. A newline is written as it is; the
/// emitter splits a block's text into lines before escaping each.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_rendered::verbatim::escape_verbatim;
///
/// assert_eq!(escape_verbatim("let x = {1};"), "let\\ x\\ =\\ \\{1\\};");
/// assert_eq!(escape_verbatim("a\\b~c^d"), "a\\textbackslash{}b\\textasciitilde{}c\\textasciicircum{}d");
/// ```
pub fn escape_verbatim(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 4);
    for ch in text.chars() {
        match ch {
            '{' | '}' | '%' | '&' | '#' | '_' | '$' => {
                out.push('\\');
                out.push(ch);
            }
            '\\' => out.push_str("\\textbackslash{}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            ' ' => out.push_str("\\ "),
            '\t' => out.push_str("\\ \\ \\ \\ "),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_special_character_a_space_and_a_tab_are_escaped() {
        assert_eq!(
            escape_verbatim("{}%&#_$\\~^"),
            "\\{\\}\\%\\&\\#\\_\\$\\textbackslash{}\\textasciitilde{}\\textasciicircum{}"
        );
        assert_eq!(escape_verbatim("a  b"), "a\\ \\ b");
        assert_eq!(escape_verbatim("\tx"), "\\ \\ \\ \\ x");
        assert_eq!(escape_verbatim("é→"), "é→");
    }
}
