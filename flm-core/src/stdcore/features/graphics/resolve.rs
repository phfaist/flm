//! Finding and reading the file of an image: the graphics resolver that a
//! host supplies ([`GraphicsResolver`]), what it returns ([`ImageFile`],
//! [`ResolvedImage`]), and the formats of image files ([`ImageFormat`]).
//!
//! # The graphics resolver
//!
//! The graphics resolver ([`GraphicsResolver`]) is the service that finds the
//! file an image's path refers to and reads its content. A host that can read
//! image files contributes one to the service slot [`RESOLVER_SLOT`]
//! (`flm.graphics.resolver`, one value at most) when it builds its registry;
//! the feature never requires one. The library itself opens no file. The
//! converters ([`GraphicsConverter`](super::GraphicsConverter)) are given
//! content and open nothing. The inspector
//! ([`GraphicsInspector`](super::GraphicsInspector)) is given the file's
//! content where a graphics resolver is registered and has read it; otherwise
//! it receives only the path and the origin
//! ([`GraphicsInspectRequest`](super::GraphicsInspectRequest)) and finds the
//! file itself.
//!
//! The feature asks the resolver in two steps:
//!
//! 1. [`resolve`](GraphicsResolver::resolve) returns, for the path as written
//!    and the origin of the source the image is written in, an [`ImageFile`]:
//!    the file's **identity** in the host's own form (two images that refer to
//!    one file have the same identity), its **format** as a media type, and
//!    the **address** an output writes to reach the file where it is. The
//!    inspect stage asks it for every image.
//! 2. [`read`](GraphicsResolver::read) returns the file's content. Only a
//!    stage that needs the content asks for it: the inspect stage where an
//!    inspector is registered to measure the content, and the publish stage
//!    where the strategy copies the file or writes it into the page.
//!
//! When the resolver returns an error, the stage that asked reports it
//! ([`ResolveFailed`](super::inspect::ResolveFailed)), and the image is
//! published by the strategy `reference`: at the path as written where the
//! file was not found.
//!
//! # A path that begins with a scheme
//!
//! A path that begins with a scheme, such as `https://example.org/a.png` or
//! `data:image/png;base64,…`, is not a file: it is never given to the resolver
//! or the inspector, and every output writes it as written, regardless of the
//! strategy ([`names_scheme`]).

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use crate::lang::FlmOrigin;

/// The name of the graphics resolver's service slot, `flm.graphics.resolver`:
/// it takes one [`GraphicsResolver`] at most, which a host contributes and
/// the feature never requires.
pub const RESOLVER_SLOT: &str = "flm.graphics.resolver";

/// The format of an image file that a conversion rule can list: one of the
/// six kinds of image the library knows by name.
///
/// A file of another kind is still published, under its own media type, but
/// no conversion rule can list it and no converter is asked about it. In
/// settings, a format is written by its short name
/// ([`name`](ImageFormat::name)): `png`, `jpeg`, `gif`, `webp`, `svg`,
/// `pdf`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::graphics::ImageFormat;
///
/// assert_eq!(ImageFormat::Pdf.media_type(), "application/pdf");
/// assert_eq!(ImageFormat::from_media_type("image/svg+xml"), Some(ImageFormat::Svg));
/// assert_eq!(ImageFormat::from_name("jpeg"), Some(ImageFormat::Jpeg));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    /// A PNG image, `image/png`: a grid of pixels.
    Png,
    /// A JPEG image, `image/jpeg`: a grid of pixels.
    Jpeg,
    /// A GIF image, `image/gif`: a grid of pixels, possibly animated.
    Gif,
    /// A WebP image, `image/webp`: a grid of pixels.
    Webp,
    /// An SVG drawing, `image/svg+xml`.
    Svg,
    /// A PDF document, `application/pdf`, whose first page is the drawing.
    Pdf,
}

impl ImageFormat {
    /// The six formats, in the order of the enumeration's variants.
    pub const ALL: [ImageFormat; 6] = [
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::Gif,
        ImageFormat::Webp,
        ImageFormat::Svg,
        ImageFormat::Pdf,
    ];

    /// Returns the format's media type, such as `image/png`.
    pub fn media_type(self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Webp => "image/webp",
            ImageFormat::Svg => "image/svg+xml",
            ImageFormat::Pdf => "application/pdf",
        }
    }

    /// Returns the format's short name, as settings write it, such as `png`.
    pub fn name(self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpeg",
            ImageFormat::Gif => "gif",
            ImageFormat::Webp => "webp",
            ImageFormat::Svg => "svg",
            ImageFormat::Pdf => "pdf",
        }
    }

    /// Returns the format whose media type is `media_type`, if it is one of
    /// the six.
    pub fn from_media_type(media_type: &str) -> Option<ImageFormat> {
        ImageFormat::ALL.into_iter().find(|format| format.media_type() == media_type)
    }

    /// Returns the format whose short name is `name`, if it is one of the
    /// six.
    pub fn from_name(name: &str) -> Option<ImageFormat> {
        ImageFormat::ALL.into_iter().find(|format| format.name() == name)
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What the graphics resolver returns about the file of an image: its
/// identity, its format and the address that reaches it where it is. It
/// holds no content: the stage that needs the content asks the resolver for
/// it ([`GraphicsResolver::read`]).
///
/// The record of each image holds it
/// ([`GraphicsRecord::file`](super::GraphicsRecord::file)).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ImageFile {
    /// The file's identity, in the host's own form: two images that refer to
    /// one file have the same identity, regardless of their paths as written.
    /// The resolver reads the file by it.
    pub identity: String,
    /// The file's format, as a media type (`image/png`), which the resolver
    /// reads from the content where it can, else from the file's name. See
    /// [`ImageFormat::from_media_type`] for the formats a conversion rule can
    /// list.
    pub media_type: String,
    /// The address an output writes to reach the file where it is, under
    /// the strategy `reference`.
    pub address: String,
}

impl ImageFile {
    /// Returns the file of identity `identity`, of the media type
    /// `media_type`, reached at `address`.
    pub fn new(
        identity: impl Into<String>,
        media_type: impl Into<String>,
        address: impl Into<String>,
    ) -> ImageFile {
        ImageFile {
            identity: identity.into(),
            media_type: media_type.into(),
            address: address.into(),
        }
    }

    /// Returns the file's format, where it is one of the six that a
    /// conversion rule can list.
    pub fn format(&self) -> Option<ImageFormat> {
        ImageFormat::from_media_type(&self.media_type)
    }
}

/// A file found and read by the graphics resolver: what the inspector is
/// given
/// ([`GraphicsInspectRequest::resolved`](super::GraphicsInspectRequest::resolved)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImage {
    /// The file, as the resolver returned it.
    pub file: ImageFile,
    /// The file's content.
    pub content: Arc<[u8]>,
}

/// What the feature asks a graphics resolver about one image.
///
/// New fields may be added in later versions: a resolver reads the fields it
/// needs, and a request is built with [`new`](GraphicsResolveRequest::new).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GraphicsResolveRequest {
    /// The path, as the author wrote it.
    pub path: String,
    /// The origin of the source the image is written in, from which a
    /// relative path is found, as for
    /// [`GraphicsInspectRequest::origin`](super::GraphicsInspectRequest::origin).
    pub origin: FlmOrigin,
}

impl GraphicsResolveRequest {
    /// Returns the request for the path `path`, written in the source of
    /// origin `origin`.
    pub fn new(path: impl Into<String>, origin: FlmOrigin) -> GraphicsResolveRequest {
        GraphicsResolveRequest { path: path.into(), origin }
    }
}

/// Why a graphics resolver could not find or read a file, in the resolver's
/// own words, which the Error the feature reports shows
/// ([`ResolveFailed`](super::inspect::ResolveFailed)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsResolveError {
    /// What went wrong, as the resolver says it, such as `no file named
    /// plot.png in the folders figures, .`.
    pub detail: String,
}

impl GraphicsResolveError {
    /// Returns the error with the text `detail`.
    pub fn new(detail: impl Into<String>) -> GraphicsResolveError {
        GraphicsResolveError { detail: detail.into() }
    }
}

impl fmt::Display for GraphicsResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for GraphicsResolveError {}

/// The **graphics resolver**: a service that finds and reads image files,
/// which a host contributes to the service slot [`RESOLVER_SLOT`]. See the
/// [module documentation](self).
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use flm_core::core::registry::RegistryBuilder;
/// use flm_core::stdcore::features::graphics::{
///     GraphicsResolveError, GraphicsResolveRequest, GraphicsResolver, ImageFile, RESOLVER_SLOT,
/// };
///
/// /// Every path refers to the same one-byte PNG file.
/// struct OneFile;
///
/// impl GraphicsResolver for OneFile {
///     fn resolve(&self, request: &GraphicsResolveRequest) -> Result<ImageFile, GraphicsResolveError> {
///         Ok(ImageFile::new("the-file", "image/png", request.path.clone()))
///     }
///
///     fn read(&self, _file: &ImageFile) -> Result<Vec<u8>, GraphicsResolveError> {
///         Ok(vec![0])
///     }
/// }
///
/// let mut builder = RegistryBuilder::new();
/// builder.contribute_to(RESOLVER_SLOT, Arc::new(OneFile) as Arc<dyn GraphicsResolver>);
/// ```
pub trait GraphicsResolver: Send + Sync {
    /// Returns the file that `request.path` refers to, found from
    /// `request.origin`.
    ///
    /// # Errors
    ///
    /// A [`GraphicsResolveError`] in the resolver's own words when no file is
    /// found or the file cannot be read.
    fn resolve(&self, request: &GraphicsResolveRequest)
    -> Result<ImageFile, GraphicsResolveError>;

    /// Returns the content of `file`, a file this resolver returned. The
    /// feature calls it again whenever it needs the content.
    ///
    /// # Errors
    ///
    /// A [`GraphicsResolveError`] in the resolver's own words when the file
    /// can no longer be read.
    fn read(&self, file: &ImageFile) -> Result<Vec<u8>, GraphicsResolveError>;
}

/// Returns whether `path` begins with a scheme, as an address on the web does
/// (`https://…`, `data:…`, `file:…`): a letter, then letters, digits, `+`, `-`
/// or `.`, then a colon, the scheme being at least two characters long. A
/// single letter before a colon is a drive (`C:\figures\plot.png`), not a
/// scheme. Such a path is not a file: see the [module
/// documentation](self#a-path-that-begins-with-a-scheme).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::graphics::names_scheme;
///
/// assert!(names_scheme("https://example.org/a.png"));
/// assert!(names_scheme("data:image/png;base64,AAAA"));
/// assert!(!names_scheme("figures/plot.png"));
/// assert!(!names_scheme("C:/figures/plot.png"));
/// ```
pub fn names_scheme(path: &str) -> bool {
    let Some((scheme, _)) = path.split_once(':') else { return false };
    let mut chars = scheme.chars();
    scheme.len() >= 2
        && chars.next().is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Returns the file's stem, with which the name of a copy begins: the last
/// segment of `path` without its extension, every character other than an
/// ASCII letter, a digit, `-` and `_` written `_`, and `image` where nothing
/// is left.
pub(crate) fn stem_of(path: &str) -> Cow<'static, str> {
    let last = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = match last.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => last,
    };
    if stem.is_empty() {
        return Cow::Borrowed("image");
    }
    Cow::Owned(
        stem.chars()
            .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scheme_is_two_characters_at_least_and_a_drive_is_none() {
        assert!(names_scheme("http://x"));
        assert!(names_scheme("svn+ssh://x"));
        assert!(!names_scheme("c:/x"));
        assert!(!names_scheme("1a:x"));
        assert!(!names_scheme("fig ures:x"));
        assert!(!names_scheme("plot.png"));
    }

    #[test]
    fn the_stem_is_the_last_segment_without_its_extension() {
        assert_eq!(stem_of("figures/plot.png"), "plot");
        assert_eq!(stem_of("a b/my plot.v2.pdf"), "my_plot_v2");
        assert_eq!(stem_of("figures/.hidden"), "_hidden");
        assert_eq!(stem_of("dir/"), "image");
    }

    #[test]
    fn every_format_reads_back_from_its_names() {
        for format in ImageFormat::ALL {
            assert_eq!(ImageFormat::from_name(format.name()), Some(format));
            assert_eq!(ImageFormat::from_media_type(format.media_type()), Some(format));
        }
    }
}
