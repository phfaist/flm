//! The `use` entry: the text that names a package in a `use` list, and its
//! grammar.
//!
//! ```text
//! ENTRY   := LOCATOR ["#" PACKAGE]        PACKAGE absent means "_"
//! LOCATOR := "." | SEGMENT ("/" SEGMENT)* the first SEGMENT may begin with "@"
//! SEGMENT := letters, digits, "-", "_"
//! PACKAGE := SEGMENT                      "_" is the default package
//! ```
//!
//! A locator is a name, never a path; a locator holding `:` is reserved (a
//! later scheme prefix such as `file:`). `.` is the current extension, legal
//! only in the `uses` list an engine reports: the parser reads it, and the
//! [loader](crate::loader::ExtensionLoader) refuses it when it is asked for it.
//! One parser serves the front matter, the data engine's block and the js
//! engine's lists.

use std::fmt;

/// The package an entry names when it names none: the default package.
pub const DEFAULT_PACKAGE: &str = "_";

/// The part of a `use` entry that finds an extension.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Locator {
    /// `.`: the current extension; legal only in the `uses` list an engine
    /// reports.
    Current,
    /// `SEGMENT ("/" SEGMENT)*`, the first segment possibly beginning with
    /// `@`, as written.
    Named(String),
}

impl Locator {
    /// The stem: the last segment (`supermathjax` for
    /// `@acme/math/supermathjax`); `None` for `.`.
    pub fn stem(&self) -> Option<&str> {
        match self {
            Locator::Current => None,
            Locator::Named(text) => Some(text.rsplit('/').next().unwrap_or(text)),
        }
    }
}

impl fmt::Display for Locator {
    /// `.`, or the locator's text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Locator::Current => f.write_str("."),
            Locator::Named(text) => f.write_str(text),
        }
    }
}

/// One entry of a `use` list, `LOCATOR#PACKAGE`; `PACKAGE` absent means `_`.
///
/// `myext` and `myext#_` parse to one entry, so they are one package.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UseEntry {
    /// The part that finds the extension.
    pub locator: Locator,
    /// The package within the extension; `_` is the default package.
    pub package: String,
}

impl UseEntry {
    /// Parses `text` by the grammar of the [module documentation](self).
    ///
    /// # Errors
    ///
    /// An [`EntryError`] naming the entry as written and the rule it breaks:
    /// an empty entry; a locator that is a path (an empty segment, a `.` or
    /// `..` segment other than the whole locator `.`, a leading `/`, a `\`);
    /// a locator holding `:`; a segment with another character than letters,
    /// digits, `-` and `_` (the first may begin with `@`); a package that is
    /// not one such segment.
    pub fn parse(text: &str) -> Result<UseEntry, EntryError> {
        let error = |detail: String| EntryError { entry: text.to_string(), detail };
        if text.is_empty() {
            return Err(error("an entry of `use` must not be empty".to_string()));
        }
        let (locator, package) = match text.split_once('#') {
            Some((locator, package)) => (locator, Some(package)),
            None => (text, None),
        };
        if locator.contains(':') {
            return Err(error(format!("the entry `{text}` of `use` must not hold `:`, which is reserved")));
        }
        let locator = if locator == "." {
            Locator::Current
        } else {
            let is_path = locator.starts_with('/')
                || locator.contains('\\')
                || locator.split('/').any(|segment| segment.is_empty() || segment == "." || segment == "..");
            if is_path {
                return Err(error(format!(
                    "the entry `{text}` of `use` must be a name (`LOCATOR` or `LOCATOR#PACKAGE`, segments of \
                     letters, digits, `-` and `_` joined by `/`), not a path; a document's extensions are found \
                     in `_flmextensions` and the other folders of the search path"
                )));
            }
            let well_formed = locator.split('/').enumerate().all(|(index, segment)| {
                let segment = if index == 0 { segment.strip_prefix('@').unwrap_or(segment) } else { segment };
                is_segment(segment)
            });
            if !well_formed {
                return Err(error(format!(
                    "the locator `{locator}` of the entry `{text}` of `use` must be segments of letters, digits, \
                     `-` and `_` joined by `/`, the first of which may begin with `@`"
                )));
            }
            Locator::Named(locator.to_string())
        };
        let package = match package {
            None => DEFAULT_PACKAGE.to_string(),
            Some(package) if is_segment(package) => package.to_string(),
            Some(package) => {
                return Err(error(format!(
                    "the package `{package}` of the entry `{text}` of `use` must be letters, digits, `-` and `_`"
                )));
            }
        };
        Ok(UseEntry { locator, package })
    }

    /// The stem of the locator; `None` for `.`.
    pub fn stem(&self) -> Option<&str> {
        self.locator.stem()
    }

    /// The name the catalog resolves: for `.`, the factory name
    /// `<current>#<package>`, `current` being the extension's identity; any
    /// other entry its [`Display`](fmt::Display), so that a compiled package's
    /// name (`stdcore`) comes back unchanged.
    pub fn catalog_name(&self, current: &str) -> String {
        match self.locator {
            Locator::Current => format!("{current}#{}", self.package),
            Locator::Named(_) => self.to_string(),
        }
    }
}

impl fmt::Display for UseEntry {
    /// The short form: `LOCATOR` for the package `_`, `LOCATOR#PACKAGE`
    /// otherwise.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.package == DEFAULT_PACKAGE {
            write!(f, "{}", self.locator)
        } else {
            write!(f, "{}#{}", self.locator, self.package)
        }
    }
}

/// A `use` entry that breaks the grammar: the entry as written and the rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryError {
    /// The entry, as written.
    pub entry: String,
    /// The rule it breaks, as text for a person.
    pub detail: String,
}

impl fmt::Display for EntryError {
    /// The detail.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for EntryError {}

/// Whether `text` is one segment: non-empty, of ASCII letters, digits, `-`
/// and `_`.
pub(crate) fn is_segment(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(locator: &str, package: &str) -> UseEntry {
        UseEntry { locator: Locator::Named(locator.to_string()), package: package.to_string() }
    }

    #[test]
    fn names_nested_names_and_packages_parse() {
        assert_eq!(UseEntry::parse("myext"), Ok(named("myext", "_")));
        assert_eq!(UseEntry::parse("myext#_"), Ok(named("myext", "_")));
        assert_eq!(UseEntry::parse("myext#core"), Ok(named("myext", "core")));
        assert_eq!(UseEntry::parse("@acme/math/supermathjax"), Ok(named("@acme/math/supermathjax", "_")));
        assert_eq!(UseEntry::parse("my-ext_2/sub#svg-out"), Ok(named("my-ext_2/sub", "svg-out")));
        assert_eq!(UseEntry::parse("@acme/math/supermathjax").unwrap().stem(), Some("supermathjax"));
        assert_eq!(UseEntry::parse("myext").unwrap().stem(), Some("myext"));
    }

    #[test]
    fn dot_is_the_current_extension() {
        let entry = UseEntry::parse(".").unwrap();
        assert_eq!(entry, UseEntry { locator: Locator::Current, package: "_".to_string() });
        assert_eq!(entry.stem(), None);
        let core = UseEntry::parse(".#core").unwrap();
        assert_eq!(core.locator, Locator::Current);
        assert_eq!(core.catalog_name("_flmextensions/myext.yaml"), "_flmextensions/myext.yaml#core");
        assert_eq!(entry.catalog_name("_flmextensions/myext.yaml"), "_flmextensions/myext.yaml#_");
        assert_eq!(UseEntry::parse("stdcore").unwrap().catalog_name("x"), "stdcore");
    }

    #[test]
    fn display_writes_the_package_always() {
        // The package is written whenever it is not the default one, which
        // the short form leaves out.
        for (text, shown) in [
            ("myext", "myext"),
            ("myext#_", "myext"),
            ("myext#core", "myext#core"),
            (".", "."),
            (".#core", ".#core"),
            ("@acme/math", "@acme/math"),
        ] {
            assert_eq!(UseEntry::parse(text).unwrap().to_string(), shown, "{text}");
        }
    }

    #[test]
    fn each_rule_refuses_with_the_entry_as_written() {
        let refused = |text: &str| UseEntry::parse(text).expect_err(text);
        // E1
        assert_eq!(refused("").detail, "an entry of `use` must not be empty");
        // E2
        for text in ["./x", "../x", "/x", "a//b", "a/./b", "a\\b", "..", "a/", "./shared/mystyle"] {
            let error = refused(text);
            assert_eq!(error.entry, text);
            assert_eq!(
                error.detail,
                format!(
                    "the entry `{text}` of `use` must be a name (`LOCATOR` or `LOCATOR#PACKAGE`, segments of \
                     letters, digits, `-` and `_` joined by `/`), not a path; a document's extensions are found \
                     in `_flmextensions` and the other folders of the search path"
                ),
                "{text}"
            );
        }
        // E3, checked before the path rule
        assert_eq!(refused("file:mystyle").detail, "the entry `file:mystyle` of `use` must not hold `:`, which is reserved");
        assert_eq!(refused("file:./x").detail, "the entry `file:./x` of `use` must not hold `:`, which is reserved");
        // E4
        assert_eq!(
            refused("my ext#core").detail,
            "the locator `my ext` of the entry `my ext#core` of `use` must be segments of letters, digits, `-` and \
             `_` joined by `/`, the first of which may begin with `@`"
        );
        assert!(refused("a/@b").detail.starts_with("the locator `a/@b` of the entry `a/@b`"));
        assert!(refused("@").detail.starts_with("the locator `@` of the entry `@`"));
        // E5
        assert_eq!(
            refused("myext#a#b").detail,
            "the package `a#b` of the entry `myext#a#b` of `use` must be letters, digits, `-` and `_`"
        );
        assert_eq!(refused("myext#").detail, "the package `` of the entry `myext#` of `use` must be letters, digits, `-` and `_`");
        assert_eq!(refused("myext#a/b").entry, "myext#a/b");
    }
}
