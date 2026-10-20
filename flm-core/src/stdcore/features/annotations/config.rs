//! The configuration of the `annotations` feature, the annotators, each with
//! the command they write and the initials shown at the start of their
//! annotations; and the render option that hides every annotation.
//!
//! The configuration ([`AnnotationsConfig`]) is the value the feature is
//! constructed from, fixed when the registry is built; the render option
//! ([`AnnotationsRenderOptions`]) is set per render request
//! ([settings, options and configuration](crate::guide::concepts_overview#settings-options-and-configuration)).

/// One annotator: a reviewer who highlights passages and writes remarks in a
/// document, with the command they write and the initials shown at the start
/// of each of their annotations.
///
/// - [`command`](Annotator::command) — the name of the annotator's command,
///   without the backslash: `phf` makes `\phf{…}` (a highlight), `\phf[…]` (a
///   comment) and `\phf … \endphf` (a highlight). It must be made of ASCII
///   letters only, like every command name of the language written with
///   letters, and must be neither `begin` nor `end`, which open and close
///   environments ([`Annotator::validate`]).
/// - [`initials`](Annotator::initials) — the short text shown where each of
///   the annotator's highlights and comments starts (in the margin, on an
///   output that has one), such as `PhF`; `None` for an annotator whose
///   annotations show no initials. The text is shown as written, never read
///   as markup.
///
/// An annotator's **number** is its position in
/// [`AnnotationsConfig::annotators`], counting from zero. The configuration
/// specifies no color: an output that shows colors chooses one per number
/// (the HTML stylesheet `flm.annotations.style` and the `phfcc` LaTeX package
/// give the first eight numbers their own colors).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::annotations::Annotator;
///
/// let reviewer = Annotator::new("phf").initials("PhF");
/// assert_eq!(reviewer.command, "phf");
/// assert_eq!(reviewer.initials.as_deref(), Some("PhF"));
/// assert_eq!(Annotator::new("abc").initials, None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Annotator {
    /// The command's name, without the backslash: `phf`. In settings, it is
    /// written as the key of the annotator's entry, not as a field.
    #[schemars(skip)]
    pub command: String,
    /// The initials shown at the start of the annotator's annotations, such as
    /// `PhF`; `None` for none.
    pub initials: Option<String>,
}

impl Annotator {
    /// Returns an annotator whose command is `command` (without the
    /// backslash), with no initials.
    pub fn new(command: impl Into<String>) -> Annotator {
        Annotator { command: command.into(), initials: None }
    }

    /// Builder: sets the initials shown at the start of the annotator's
    /// annotations, and returns the annotator.
    pub fn initials(mut self, initials: impl Into<String>) -> Annotator {
        self.initials = Some(initials.into());
        self
    }

    /// Returns the name of the command that closes a passage the annotator
    /// highlights in the third form, `\phf … \endphf`: `end` followed by the
    /// command's name, `endphf`.
    pub fn end_command(&self) -> String {
        format!("end{}", self.command)
    }

    /// Checks the annotator on its own.
    ///
    /// # Errors
    ///
    /// Returns a sentence that identifies the annotator and states the
    /// problem: a command that is empty, holds a character other than an
    /// ASCII letter, or is `begin` or `end` (none of which the language could
    /// read as the annotator's command); or initials that are empty or made
    /// only of whitespace (an annotator whose annotations show no initials has
    /// `None`).
    pub fn validate(&self) -> Result<(), String> {
        let command = self.command.as_str();
        if command.is_empty() || !command.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Err(format!(
                "the annotator command name `{command}` must be one or more ASCII letters, \
                 without the backslash"
            ));
        }
        if command == "begin" || command == "end" {
            return Err(format!(
                "the annotator command name may not be `{command}`, since `\\{command}` starts or ends \
                 an environment"
            ));
        }
        // Refused rather than read as none: blank initials show nothing,
        // and a host meaning "no initials" writes `None`.
        // See decision rationale at [§flm-dd-dr:std-annotations-config].
        if self.initials.as_deref().is_some_and(|initials| initials.trim().is_empty()) {
            return Err(format!(
                "the initials of the annotator `\\{command}` may not be empty or only \
                 whitespace (leave them unset for no initials)"
            ));
        }
        Ok(())
    }
}

/// The configuration of the `annotations` feature: the annotators, in order.
///
/// Each annotator declares one command, written in three forms: `\phf{…}`
/// and `\phf … \endphf` mark a passage (a **highlight**), and `\phf[…]`
/// writes a remark (a **comment**). The order numbers the annotators from
/// zero; an output that shows colors chooses one per number. The default
/// configuration declares **no annotator**, and the feature then declares no
/// command at all: `\phf` is an unknown command until a host configures it.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::annotations::{AnnotationsConfig, Annotator};
///
/// let config = AnnotationsConfig::new()
///     .annotator(Annotator::new("phf").initials("PhF"))
///     .annotator(Annotator::new("abc"));
/// assert_eq!(config.annotators.len(), 2);
/// assert_eq!(config.number_of("abc"), Some(1));
/// assert_eq!(config.validate(), Ok(()));
/// assert!(AnnotationsConfig::new().annotators.is_empty());
/// ```
///
/// The value is what
/// [`AnnotationsFeature::new`](super::AnnotationsFeature::new) takes; a host
/// using the standard package passes it with
/// [`StdcoreConfig::annotations`](crate::stdcore::StdcoreConfig::annotations).
/// The configuration is fixed when the registry is built, before any document
/// is parsed
/// ([configuration](crate::guide::concepts_overview#settings-options-and-configuration)),
/// because the commands are part of the language. The feature therefore
/// feeds [`digest_bytes`](AnnotationsConfig::digest_bytes) into the registry
/// digest ([digests](crate::model::digest)), the fingerprint that decides
/// whether a cached result may be reused.
///
/// The feature checks the configuration when the registry is built
/// ([`AnnotationsConfig::validate`]): every annotator on its own
/// ([`Annotator::validate`]), then two annotators with the same command are
/// refused. A command that another feature also declares, such as `emph`,
/// makes the registry build fail, as any two declarations of one name do.
///
/// The configuration refers to no output, HTML tag or class. Whether
/// annotations are shown at all is decided per render, not by the
/// configuration: [`AnnotationsRenderOptions`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AnnotationsConfig {
    /// The annotators, in order: the first is annotator 0. In settings, a map
    /// from each annotator's command name to its other fields.
    #[serde(deserialize_with = "crate::core::settings::named::deserialize")]
    #[schemars(
        transform = crate::core::settings::named::map_schema,
        with = "std::collections::BTreeMap<String, Annotator>"
    )]
    pub annotators: Vec<Annotator>,
}

impl AnnotationsConfig {
    /// Returns the default configuration: no annotator.
    pub fn new() -> AnnotationsConfig {
        AnnotationsConfig::default()
    }

    /// Builder: adds `annotator` after the annotators already declared, and
    /// returns the configuration; the new annotator's number is the count of
    /// those already declared.
    pub fn annotator(mut self, annotator: Annotator) -> AnnotationsConfig {
        self.annotators.push(annotator);
        self
    }

    /// Returns the number of the annotator whose command is `command`, or
    /// `None` when there is none.
    pub fn number_of(&self, command: &str) -> Option<usize> {
        self.annotators.iter().position(|annotator| annotator.command == command)
    }

    /// Checks the configuration, as the feature does when it registers.
    ///
    /// # Errors
    ///
    /// Returns a sentence that states the first problem found: one of
    /// [`Annotator::validate`]'s, or two annotators with the same command.
    pub fn validate(&self) -> Result<(), String> {
        for (at, annotator) in self.annotators.iter().enumerate() {
            annotator.validate()?;
            if self.annotators[..at].iter().any(|earlier| earlier.command == annotator.command) {
                return Err(format!(
                    "the command `\\{}` may not be declared for two annotators",
                    annotator.command
                ));
            }
        }
        Ok(())
    }

    /// Returns the canonical byte form of the configuration, which the feature
    /// feeds into the registry digest: every annotator in order, every field
    /// of it, each text preceded by its length. Two configurations therefore
    /// have the same byte form exactly when they are equal.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        push_len(&mut out, self.annotators.len());
        for annotator in &self.annotators {
            // Taken apart by name, so a field added and not written here fails
            // to compile.
            let Annotator { command, initials } = annotator;
            push_text(&mut out, command);
            match initials {
                None => out.push(0),
                Some(initials) => {
                    out.push(1);
                    push_text(&mut out, initials);
                }
            }
        }
        out
    }
}

/// Writes a count as eight little-endian bytes.
fn push_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&(len as u64).to_le_bytes());
}

/// Writes a text as its length and its bytes.
fn push_text(out: &mut Vec<u8>, text: &str) {
    push_len(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// The render options of the `annotations` feature: an option type whose
/// values are set on a render request ([`crate::model::options`]), in the
/// plan request's overrides ([`Overrides`](crate::core::plan::Overrides)), as
/// the example shows.
///
/// When [`hidden`](AnnotationsRenderOptions::hidden) is true, every annotation is
/// left out of the output on every backend: a highlight is shown as its
/// passage alone and a comment as nothing. This gives a clean copy of an
/// annotated document without any change to its source.
///
/// # Examples
///
/// ```
/// use flm_core::core::plan::Overrides;
/// use flm_core::model::options::OptionValues;
/// use flm_core::stdcore::features::annotations::AnnotationsRenderOptions;
///
/// let clean_copy = Overrides {
///     render_options: OptionValues::new().with(AnnotationsRenderOptions { hidden: true }),
///     ..Overrides::none()
/// };
/// # let _ = clean_copy;
/// assert!(!AnnotationsRenderOptions::default().hidden);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AnnotationsRenderOptions {
    /// Whether every annotation is left out of the output, each highlight
    /// shown as its passage alone and each comment not at all. `false` by
    /// default.
    pub hidden: bool,
}


/// In settings, an annotator is written under its command's name, as one
/// entry of the map [`AnnotationsConfig::annotators`].
impl crate::core::settings::named::NamedEntry for Annotator {
    fn read_entry(name: &str, data: serde_json::Value) -> Result<Annotator, String> {
        crate::core::settings::named::read_with_name("command", name, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_ascii_letters_and_no_environment_word() {
        for command in ["", "ph f", "phf1", "é", "p-f", "begin", "end"] {
            assert!(Annotator::new(command).validate().is_err(), "{command}");
        }
        assert_eq!(Annotator::new("PhF").validate(), Ok(()));
        assert_eq!(Annotator::new("endphf").validate(), Ok(()));
    }

    #[test]
    fn blank_initials_are_refused_and_two_annotators_of_one_command_too() {
        for initials in ["", " ", "\t"] {
            let error = Annotator::new("phf").initials(initials).validate().unwrap_err();
            assert!(error.contains("`\\phf`"), "{initials:?}: {error}");
        }
        assert_eq!(Annotator::new("phf").initials(" P ").validate(), Ok(()));
        let config = AnnotationsConfig::new()
            .annotator(Annotator::new("phf"))
            .annotator(Annotator::new("phf").initials("X"));
        assert!(config.validate().expect_err("refused").contains("`\\phf`"));
    }

    #[test]
    fn the_digest_tells_every_field_apart() {
        let one = AnnotationsConfig::new().annotator(Annotator::new("phf"));
        let with_initials = AnnotationsConfig::new().annotator(Annotator::new("phf").initials("PhF"));
        let other = AnnotationsConfig::new().annotator(Annotator::new("abc"));
        let two = one.clone().annotator(Annotator::new("abc"));
        let reordered = AnnotationsConfig::new().annotator(Annotator::new("abc")).annotator(Annotator::new("phf"));
        let digests = [
            AnnotationsConfig::new().digest_bytes(),
            one.digest_bytes(),
            with_initials.digest_bytes(),
            other.digest_bytes(),
            two.digest_bytes(),
            reordered.digest_bytes(),
        ];
        for (at, digest) in digests.iter().enumerate() {
            for other in &digests[..at] {
                assert_ne!(digest, other);
            }
        }
        assert_eq!(one.digest_bytes(), AnnotationsConfig::new().annotator(Annotator::new("phf")).digest_bytes());
    }
}
