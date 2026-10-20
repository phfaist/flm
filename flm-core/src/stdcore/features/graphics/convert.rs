//! Turning an image of one format into another: the converters that a host
//! or an extension supplies ([`GraphicsConverter`]), the conversion rules set
//! for an output ([`ConversionRule`]), and the choice of a converter for a
//! rule ([`choose_converter`]).
//!
//! # Converters
//!
//! A **converter** ([`GraphicsConverter`]) turns the content of an image of
//! one format into content of another. It has a **name** (`raster`,
//! `magick`), says whether it **offers** a pair of formats, and **converts**
//! content under a rule's options. It is given content and returns content;
//! the converter decides whether it does the work in the program's own code
//! or by running another program installed on the machine. Converters are
//! contributed to the service slot [`CONVERTERS_SLOT`]
//! (`flm.graphics.converters`), any number of them, kept in the order
//! contributed; the feature requires none.
//!
//! # Conversion rules
//!
//! A **conversion rule** ([`ConversionRule`]) says which formats are turned
//! into which: `from`, one format or a list; `to`, one format; `via`, an
//! optional list of the names of the converters allowed to perform it; and
//! `options` ([`ConversionOptions`]), such as the resolution at which a
//! drawing is turned into pixels. The rules of each output are a render option
//! of the feature
//! ([`GraphicsRenderOptions::convert`](super::GraphicsRenderOptions::convert)).
//! They apply where the output's strategy makes new content from the file
//! (`copy` and `data-uri`), and never under `reference`, which writes the
//! address of the file where it is.
//!
//! # Which converter performs a rule
//!
//! For an image, the first of the output's rules whose `from` lists the
//! image's format applies; an image whose format no rule lists is published as
//! it is. The converters that the rule's `via` lists (every converter, where
//! `via` is empty) are asked, in the order of precedence of their service slot
//! (the converters of the feature placed last in the feature order first, each
//! feature's in the order it contributed them, the host's last), whether they
//! offer the rule's pair of formats, and the first that does performs the
//! conversion ([`choose_converter`]). A converter is asked when a rule is applied, not
//! when the registry is built. Conversions are never chained: a rule whose
//! pair of formats no allowed converter offers is reported, and the image is
//! published as it is.

use std::fmt;

use serde::Deserialize;

use super::resolve::ImageFormat;

/// The name of the converters' service slot, `flm.graphics.converters`: it
/// takes any number of [`GraphicsConverter`]s, which a host or an extension
/// contributes and the feature never requires. The feature reads them in the
/// order of precedence
/// ([`Services::services_by_precedence_named`](crate::core::services::Services::services_by_precedence_named)):
/// the converters of the feature placed last in the feature order first, each
/// feature's in the order it contributed them, and the host's last.
pub const CONVERTERS_SLOT: &str = "flm.graphics.converters";

/// The options of a conversion rule, which the converter that performs the
/// rule reads. In settings, the rule's `options`, with the keys `dpi` and
/// `transparent_bg`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::graphics::ConversionOptions;
///
/// let options: ConversionOptions = serde_json::from_value(serde_json::json!({ "dpi": 192 })).unwrap();
/// assert_eq!(options.dpi, Some(192));
/// assert_eq!(options.transparent_bg, None);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ConversionOptions {
    /// The resolution, in dots per inch, at which a drawing (an SVG or a PDF
    /// image) is turned into pixels. Where it is not given, the converter
    /// chooses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dpi: Option<u32>,
    /// Whether a drawing turned into pixels keeps a transparent background,
    /// where the converter can do so. Where it is not given, the converter
    /// chooses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparent_bg: Option<bool>,
}

impl ConversionOptions {
    /// Returns the options with nothing given: the converter chooses
    /// everything.
    pub fn new() -> ConversionOptions {
        ConversionOptions::default()
    }

    /// Builder: sets [`dpi`](ConversionOptions::dpi).
    pub fn dpi(mut self, dpi: u32) -> ConversionOptions {
        self.dpi = Some(dpi);
        self
    }

    /// Builder: sets [`transparent_bg`](ConversionOptions::transparent_bg).
    pub fn transparent_bg(mut self, transparent: bool) -> ConversionOptions {
        self.transparent_bg = Some(transparent);
        self
    }
}

/// One format or a list of formats, as a rule's `from` is written in
/// settings.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
enum Formats {
    /// One format.
    One(ImageFormat),
    /// A list of formats.
    Many(Vec<ImageFormat>),
}

/// Reads a rule's `from`, written as one format or a list, as a list.
fn one_or_many<'de, D: serde::Deserializer<'de>>(data: D) -> Result<Vec<ImageFormat>, D::Error> {
    Ok(match Formats::deserialize(data)? {
        Formats::One(format) => vec![format],
        Formats::Many(formats) => formats,
    })
}

/// A conversion rule: which formats are turned into which, by which
/// converters, with which options. See the [module
/// documentation](self#conversion-rules).
///
/// In settings, a rule is written as a map with the keys `from`, `to`, `via`
/// and `options`; `from` and `to` are required.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::graphics::{ConversionRule, ImageFormat};
///
/// let rule: ConversionRule = serde_json::from_value(serde_json::json!({
///     "from": "pdf",
///     "to": "png",
///     "options": { "dpi": 192 },
/// }))
/// .unwrap();
/// assert_eq!(rule.from, [ImageFormat::Pdf]);
/// assert_eq!(rule.to, ImageFormat::Png);
/// assert!(rule.via.is_empty());
/// assert_eq!(rule.options.dpi, Some(192));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConversionRule {
    /// The formats the rule turns into `to`: one format (`pdf`) or a list
    /// (`[svg, pdf]`), each one of `png`, `jpeg`, `gif`, `webp`, `svg` and
    /// `pdf`.
    #[serde(deserialize_with = "one_or_many")]
    #[schemars(with = "Formats")]
    pub from: Vec<ImageFormat>,
    /// The format the rule turns them into.
    pub to: ImageFormat,
    /// The names of the converters allowed to perform the rule, in no
    /// particular order; where the list is empty (the default), every
    /// converter is allowed.
    #[serde(default)]
    pub via: Vec<String>,
    /// The options the converter reads, such as the resolution at which a
    /// drawing is turned into pixels.
    #[serde(default)]
    pub options: ConversionOptions,
}

impl ConversionRule {
    /// Returns the rule that turns each of the formats `from` into `to`, by
    /// any converter, with no option given.
    pub fn new(from: impl IntoIterator<Item = ImageFormat>, to: ImageFormat) -> ConversionRule {
        ConversionRule {
            from: from.into_iter().collect(),
            to,
            via: Vec::new(),
            options: ConversionOptions::default(),
        }
    }

    /// Builder: adds the converter named `name` to the converters allowed to
    /// perform the rule.
    pub fn via(mut self, name: impl Into<String>) -> ConversionRule {
        self.via.push(name.into());
        self
    }

    /// Builder: sets the rule's options.
    pub fn options(mut self, options: ConversionOptions) -> ConversionRule {
        self.options = options;
        self
    }

    /// Returns whether the rule applies to an image of the format `format`:
    /// whether its `from` lists the format.
    pub fn applies_to(&self, format: ImageFormat) -> bool {
        self.from.contains(&format)
    }
}

/// What a converter is asked to do: turn `content`, of the format `from`,
/// into content of the format `to`, under `options`.
///
/// New fields may be added in later versions: a converter reads the fields it
/// needs, and a request is built with [`new`](ConversionRequest::new).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConversionRequest<'a> {
    /// The content to convert.
    pub content: &'a [u8],
    /// The format of the content.
    pub from: ImageFormat,
    /// The format to turn it into.
    pub to: ImageFormat,
    /// The rule's options.
    pub options: &'a ConversionOptions,
}

impl<'a> ConversionRequest<'a> {
    /// Returns the request to turn `content`, of the format `from`, into the
    /// format `to` under `options`.
    pub fn new(
        content: &'a [u8],
        from: ImageFormat,
        to: ImageFormat,
        options: &'a ConversionOptions,
    ) -> ConversionRequest<'a> {
        ConversionRequest { content, from, to, options }
    }
}

/// Why a converter could not convert, in the converter's own words, which the
/// Error the feature reports shows
/// ([`ConversionFailed`](super::publish::ConversionFailed)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsConvertError {
    /// What went wrong, as the converter says it.
    pub detail: String,
}

impl GraphicsConvertError {
    /// Returns the error with the text `detail`.
    pub fn new(detail: impl Into<String>) -> GraphicsConvertError {
        GraphicsConvertError { detail: detail.into() }
    }
}

impl fmt::Display for GraphicsConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for GraphicsConvertError {}

/// A **converter**: a service that turns an image of one format into
/// another, which a host or an extension contributes to the service slot
/// [`CONVERTERS_SLOT`]. See the [module documentation](self).
///
/// A converter may keep the results it has computed, provided that reusing
/// one cannot be told from converting again.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use flm_core::core::registry::RegistryBuilder;
/// use flm_core::stdcore::features::graphics::{
///     CONVERTERS_SLOT, ConversionRequest, GraphicsConvertError, GraphicsConverter, ImageFormat,
/// };
///
/// /// Turns a GIF into a PNG by keeping its bytes: a stand-in for a real one.
/// struct GifToPng;
///
/// impl GraphicsConverter for GifToPng {
///     fn name(&self) -> &str {
///         "gif-to-png"
///     }
///
///     fn offers(&self, from: ImageFormat, to: ImageFormat) -> bool {
///         from == ImageFormat::Gif && to == ImageFormat::Png
///     }
///
///     fn convert(&self, request: &ConversionRequest<'_>) -> Result<Vec<u8>, GraphicsConvertError> {
///         Ok(request.content.to_vec())
///     }
/// }
///
/// let mut builder = RegistryBuilder::new();
/// builder.contribute_to(CONVERTERS_SLOT, Arc::new(GifToPng) as Arc<dyn GraphicsConverter>);
/// ```
pub trait GraphicsConverter: Send + Sync {
    /// Returns the converter's name, by which a rule's `via` lists it, such as
    /// `raster`.
    fn name(&self) -> &str;

    /// Returns whether the converter can turn content of the format `from`
    /// into the format `to`.
    fn offers(&self, from: ImageFormat, to: ImageFormat) -> bool;

    /// Returns the content `request.content`, of the format `request.from`,
    /// turned into the format `request.to` under `request.options`.
    ///
    /// # Errors
    ///
    /// A [`GraphicsConvertError`] in the converter's own words when the
    /// content cannot be read or the conversion fails.
    fn convert(&self, request: &ConversionRequest<'_>) -> Result<Vec<u8>, GraphicsConvertError>;
}

/// Why no converter can perform a rule, as [`choose_converter`] returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoConverter {
    /// The rule's `via` lists a converter that is not registered: the name
    /// as written.
    Unknown(String),
    /// No converter allowed by the rule offers its pair of formats.
    NoneOffers,
}

/// Returns the converter that performs `rule` for an image of the format
/// `from`: among `converters`, in their order, the first that the rule's
/// `via` allows (every converter, where `via` is empty) and that offers the
/// pair `from`, `rule.to`. See the [module
/// documentation](self#which-converter-performs-a-rule).
///
/// # Errors
///
/// [`NoConverter::Unknown`] for the first name in the rule's `via` that no
/// converter of `converters` has, and [`NoConverter::NoneOffers`] where no
/// allowed converter offers the pair.
pub fn choose_converter<'c>(
    rule: &ConversionRule,
    from: ImageFormat,
    converters: &[&'c dyn GraphicsConverter],
) -> Result<&'c dyn GraphicsConverter, NoConverter> {
    if let Some(unknown) =
        rule.via.iter().find(|name| !converters.iter().any(|converter| converter.name() == *name))
    {
        return Err(NoConverter::Unknown(unknown.clone()));
    }
    converters
        .iter()
        .copied()
        .filter(|converter| rule.via.is_empty() || rule.via.iter().any(|name| name == converter.name()))
        .find(|converter| converter.offers(from, rule.to))
        .ok_or(NoConverter::NoneOffers)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Named(&'static str, bool);

    impl GraphicsConverter for Named {
        fn name(&self) -> &str {
            self.0
        }
        fn offers(&self, _from: ImageFormat, _to: ImageFormat) -> bool {
            self.1
        }
        fn convert(&self, request: &ConversionRequest<'_>) -> Result<Vec<u8>, GraphicsConvertError> {
            Ok(request.content.to_vec())
        }
    }

    #[test]
    fn the_first_allowed_converter_offering_the_pair_is_chosen() {
        let (none, first, second) = (Named("none", false), Named("first", true), Named("second", true));
        let converters: [&dyn GraphicsConverter; 3] = [&none, &first, &second];
        let rule = ConversionRule::new([ImageFormat::Pdf], ImageFormat::Png);
        let chosen = |rule: &ConversionRule| {
            choose_converter(rule, ImageFormat::Pdf, &converters).map(|converter| converter.name().to_string())
        };
        assert_eq!(chosen(&rule), Ok(String::from("first")));
        assert_eq!(chosen(&rule.clone().via("second")), Ok(String::from("second")));
        assert_eq!(chosen(&rule.clone().via("none")), Err(NoConverter::NoneOffers));
        assert_eq!(chosen(&rule.clone().via("other")), Err(NoConverter::Unknown(String::from("other"))));
    }

    #[test]
    fn a_rule_reads_one_format_or_a_list_and_refuses_several_targets() {
        let one: ConversionRule =
            serde_json::from_value(serde_json::json!({ "from": "svg", "to": "pdf" })).unwrap();
        assert_eq!(one.from, [ImageFormat::Svg]);
        let many: ConversionRule =
            serde_json::from_value(serde_json::json!({ "from": ["gif", "webp"], "to": "png", "via": ["raster"] }))
                .unwrap();
        assert_eq!(many.from, [ImageFormat::Gif, ImageFormat::Webp]);
        assert_eq!(many.via, ["raster"]);
        let several: Result<ConversionRule, _> =
            serde_json::from_value(serde_json::json!({ "from": "pdf", "to": ["png", "jpeg"] }));
        assert!(several.is_err());
        let unknown: Result<ConversionRule, _> =
            serde_json::from_value(serde_json::json!({ "from": "tiff", "to": "png" }));
        assert!(unknown.is_err());
    }
}
