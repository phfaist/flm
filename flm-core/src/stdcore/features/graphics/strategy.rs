//! How each output publishes its images: the strategies ([`Strategy`]), and
//! the render options ([`GraphicsRenderOptions`]) that choose, per output, a
//! strategy, the conversion rules and the magnification of images
//! ([`Magnification`]).
//!
//! **Publishing** an image is deciding the address an output writes to reach
//! it. A **strategy** says how the address is decided:
//!
//! | strategy | the address | what the output holds |
//! |---|---|---|
//! | `reference` | the address of the file where it is | the entry artifact alone |
//! | `copy` | the address of a copy of the file, made part of the output | the entry artifact and one copy per file |
//! | `data-uri` | the file's content, in an address beginning with `data:` | the entry artifact alone |
//!
//! The entry artifact is the artifact that represents an output as a whole,
//! such as the HTML page ([`ArtifactSet`](crate::core::artifact::ArtifactSet)).
//! Not every output can use every strategy: `copy` needs an output that can
//! hold several files (not the plain-text output), and `data-uri` an output
//! that can show such an address (the HTML page only). A strategy set for an
//! output that cannot use it is reported at each image, which is then
//! published by `reference` ([`OutputImages`](super::OutputImages)).
//!
//! The strategy and the conversion rules are chosen per output, by the
//! output's name. An output that the render options do not list keeps
//! `reference` and no conversion rule, so a caller that sets nothing, or a
//! host that supplies no graphics resolver, gets every image's path written
//! as the author wrote it. The publish stages ([`publish`](super::publish))
//! carry the strategies out.
//!
//! The **magnification** of an output multiplies the size every image is
//! shown at on that output, by one factor for raster images and one for vector
//! images ([`Magnification`]); the image rules apply it where they decide an
//! image's size ([`shown_size`](super::rules::shown_size)).

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;

use crate::stdcore::backends::pres::I48F16;

use super::convert::ConversionRule;

/// How an output publishes its images: one of the three strategies. In
/// settings, written `reference`, `copy` or `data-uri`. See the [module
/// documentation](self).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Strategy {
    /// The address of the file where it is: the graphics resolver's address
    /// where a resolver found the file, else the path as written. Nothing is
    /// read, copied or converted. The default.
    #[default]
    Reference,
    /// A copy of the file, converted where a conversion rule says so, is made
    /// part of the output, and the address is the copy's, as the host's
    /// artifact namer gives it. Needs a graphics resolver to read the file, an
    /// artifact namer, and an output that can hold several files (not the
    /// plain-text output).
    Copy,
    /// The file's content, converted where a conversion rule says so, is
    /// written into the address itself, `data:<media type>;base64,<content>`,
    /// so that the output needs no other file. Needs a graphics resolver to
    /// read the file, and an output that can show such an address (the HTML
    /// page only).
    DataUri,
}

impl Strategy {
    /// Returns whether the strategy makes new content from the file (`copy`
    /// and `data-uri`): such a strategy needs the file's content, and the
    /// conversion rules apply to it.
    pub fn needs_content(self) -> bool {
        !matches!(self, Strategy::Reference)
    }

    /// Returns the strategy's name, as settings write it: `reference`, `copy`
    /// or `data-uri`.
    pub fn name(self) -> &'static str {
        match self {
            Strategy::Reference => "reference",
            Strategy::Copy => "copy",
            Strategy::DataUri => "data-uri",
        }
    }
}

/// The render options of the `graphics` feature: for each output, how its
/// images are published, which conversion rules apply, and by how much its
/// images are magnified.
///
/// An option type is a plain Rust struct a feature declares once; its values
/// are set on a document or on a render request
/// ([`crate::model::options`]). This one is set on the render request. Each
/// field is a map from an output's name (`html`, `latex-recomposed`, `text`)
/// to that output's value; an output that a map does not list takes the
/// default. When a host combines several sources of settings, a source that
/// writes one output's entry leaves the other outputs' entries as the
/// lower-ranked sources wrote them. The publish stage of a backend reads the
/// entries of the output of the backend's name: each standard backend makes
/// one output, named after the backend.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::graphics::{GraphicsRenderOptions, ImageFormat, Strategy};
///
/// let options: GraphicsRenderOptions = serde_json::from_value(serde_json::json!({
///     "strategy": { "html": "data-uri", "latex-recomposed": "copy" },
///     "convert": { "html": [ { "from": "pdf", "to": "png", "options": { "dpi": 192 } } ] },
///     "magnification": { "html": { "raster": 1.5, "vector": 1.25 } },
/// }))
/// .unwrap();
/// assert_eq!(options.strategy_for("html"), Strategy::DataUri);
/// assert_eq!(options.strategy_for("text"), Strategy::Reference);
/// assert_eq!(options.rules_for("html")[0].to, ImageFormat::Png);
/// assert!(options.rules_for("latex-recomposed").is_empty());
/// assert_eq!(options.magnification_for("html").raster.value().to_string(), "1.5");
/// assert_eq!(options.magnification_for("text").vector.value().to_string(), "1");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct GraphicsRenderOptions {
    /// How each output publishes its images, by the output's name:
    /// `reference` (the address of the file where it is, the default), `copy`
    /// (a copy of the file beside the output) or `data-uri` (the file's
    /// content written into the address). `copy` and `data-uri` need a way to
    /// read image files, which the program must supply.
    pub strategy: BTreeMap<String, Strategy>,
    /// The conversion rules of each output, by the output's name, applied
    /// under `copy` and `data-uri` and never under `reference`. Each rule has
    /// `from` (a format or a list of formats among `png`, `jpeg`, `gif`,
    /// `webp`, `svg`, `pdf`), `to` (one format), `via` (the names of the
    /// converters allowed; any converter where empty) and `options` (`dpi`,
    /// `transparent_bg`). For each image, the first rule whose `from` lists
    /// the image's format applies. No rule by default.
    pub convert: BTreeMap<String, Vec<ConversionRule>>,
    /// The magnification of each output, by the output's name: `raster` and
    /// `vector`, each a number greater than zero (1 where not given), by
    /// which every dimension of a raster or a vector image shown on that
    /// output is multiplied, including the `width` and `height` the author
    /// gave. An image whose type is not known uses `vector`. No
    /// magnification by default.
    pub magnification: BTreeMap<String, Magnification>,
}

impl GraphicsRenderOptions {
    /// Returns the strategy of the output `output`: its entry, else
    /// `reference`.
    pub fn strategy_for(&self, output: &str) -> Strategy {
        self.strategy.get(output).copied().unwrap_or_default()
    }

    /// Returns the conversion rules of the output `output`: its entry, else
    /// none.
    pub fn rules_for(&self, output: &str) -> &[ConversionRule] {
        self.convert.get(output).map(Vec::as_slice).unwrap_or_default()
    }

    /// Returns the magnification of the output `output`: its entry, else a
    /// factor of 1 for both types of images.
    pub fn magnification_for(&self, output: &str) -> Magnification {
        self.magnification.get(output).copied().unwrap_or_default()
    }
}

/// The magnification of one output: the factor by which the size of each
/// image shown on it is multiplied, one for raster images (a grid of pixels,
/// such as a PNG or JPEG file) and one for vector images (a drawing, such as
/// an SVG or PDF file). In settings, `{raster: 1.5, vector: 1.25}`; a factor
/// not given is 1.
///
/// The two factors let an output show images at another size relative to its
/// text than the other outputs do, such as larger on a web page than in a
/// printed document. See [`GraphicsRenderOptions::magnification`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Magnification {
    /// The factor of a raster image.
    pub raster: Factor,
    /// The factor of a vector image, and of an image whose type is not known.
    pub vector: Factor,
}

/// A magnification factor: an exact number greater than zero, 1 by default.
///
/// The factor is held exactly, as the fixed-point number with sixteen
/// fractional bits that a length's amount is ([`I48F16`]), so that render
/// options holding it compare equal and hash alike (an option value is part of
/// what identifies a plan) and an image's size is computed identically on
/// every platform. In settings it is written as a number, which is rounded to
/// the nearest 1/65536 when it is read: `1.5` and `1.25` are held exactly, and
/// `1.1` as the nearest multiple of 1/65536.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(serde::Deserialize)]
#[serde(try_from = "f64")]
pub struct Factor(I48F16);

impl Factor {
    /// The factor 1, which leaves a size as it is.
    pub const ONE: Factor = Factor(I48F16::ONE);

    /// Returns the factor `value`, or `None` where `value` is not greater than
    /// zero.
    pub fn new(value: I48F16) -> Option<Factor> {
        (value > I48F16::ZERO).then_some(Factor(value))
    }

    /// Returns the factor's value.
    pub fn value(self) -> I48F16 {
        self.0
    }
}

impl Default for Factor {
    fn default() -> Factor {
        Factor::ONE
    }
}

impl TryFrom<f64> for Factor {
    type Error = FactorError;

    fn try_from(value: f64) -> Result<Factor, FactorError> {
        if value.is_nan() || value <= 0.0 {
            return Err(FactorError::NotPositive(value));
        }
        I48F16::checked_from_num(value)
            .and_then(Factor::new)
            .ok_or(FactorError::OutOfRange(value))
    }
}

/// Why a number is not a magnification [`Factor`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FactorError {
    /// The number is zero or negative.
    NotPositive(f64),
    /// The number is too small to differ from zero once rounded to the
    /// nearest 1/65536, or too large to hold.
    OutOfRange(f64),
}

impl fmt::Display for FactorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FactorError::NotPositive(value) => {
                write!(f, "a magnification factor must be greater than zero, not `{value}`")
            }
            FactorError::OutOfRange(value) => write!(
                f,
                "a magnification factor must lie between 1/65536 and 2^47, not `{value}`"
            ),
        }
    }
}

impl std::error::Error for FactorError {}

impl schemars::JsonSchema for Factor {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Factor")
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "number",
            "exclusiveMinimum": 0,
            "default": 1,
            "description": "A magnification factor, greater than zero.",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Factor, FactorError, GraphicsRenderOptions, I48F16, Magnification, Strategy};

    #[test]
    fn a_magnification_defaults_to_one_per_output_and_per_type() {
        let options: GraphicsRenderOptions = serde_json::from_value(serde_json::json!({
            "magnification": { "html": { "raster": 1.5 } },
        }))
        .expect("the settings read");
        let html = options.magnification_for("html");
        assert_eq!(html.raster.value(), I48F16::from_num(1.5));
        assert_eq!(html.vector, Factor::ONE);
        assert_eq!(options.magnification_for("text"), Magnification::default());
        assert_eq!(Magnification::default().raster, Factor::ONE);
    }

    #[test]
    fn a_factor_is_greater_than_zero() {
        assert_eq!(Factor::try_from(0.0), Err(FactorError::NotPositive(0.0)));
        assert_eq!(Factor::try_from(-2.0), Err(FactorError::NotPositive(-2.0)));
        assert_eq!(Factor::try_from(1e-9), Err(FactorError::OutOfRange(1e-9)));
        assert_eq!(Factor::try_from(1e20), Err(FactorError::OutOfRange(1e20)));
        assert!(Factor::try_from(f64::NAN).is_err());
        assert_eq!(Factor::try_from(1.25).map(Factor::value), Ok(I48F16::from_num(1.25)));
        let refused = serde_json::from_value::<GraphicsRenderOptions>(serde_json::json!({
            "magnification": { "html": { "vector": 0 } },
        }));
        assert!(refused.is_err());
    }

    #[test]
    fn each_strategy_s_name_reads_back_as_the_same_strategy() {
        for strategy in [Strategy::Reference, Strategy::Copy, Strategy::DataUri] {
            let read: Strategy = serde_json::from_value(serde_json::Value::from(strategy.name()))
                .expect("the name is the settings' spelling");
            assert_eq!(read, strategy);
        }
    }
}
