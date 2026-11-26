//! The address policy: which addresses the HTML backend writes into a page,
//! and which it refuses.
//!
//! An **address** here is a URL that the page references: what a link points
//! at (`href`) or what an image is loaded from (`src`, `srcset`). Both come
//! from the document or from the host that rendered it, and a browser can do
//! more with an address than fetch it: when a link is followed whose address
//! has the **scheme** `javascript` (the scheme is the part before the first
//! colon, `https` in `https://example.org`), the browser runs the rest of the
//! address as a program.
//!
//! [`check_address`] therefore accepts an address only when it is one of
//! these:
//!
//! - an **address fragment**, `#section-2`: a place in the same page;
//! - a **relative address**, `figures/plot.png`, `../index.html`, `?page=2`:
//!   one with no scheme at all, read against the address of the page it is
//!   in;
//! - an address whose scheme the render option [`HtmlAllowedUrlSchemes`]
//!   allows: by default, one of [`DEFAULT_ALLOWED_URL_SCHEMES`];
//! - for an image only, a `data:` address whose media type begins with
//!   `image/` ([`DATA_SCHEME`], [`IMAGE_MEDIA_PREFIX`]) — a picture written
//!   into the page itself.
//!
//! Anything else is refused. A refused address loses the address and nothing
//! else: the emitter writes the link without its `href` and the image without
//! its `src`, so the link's text and the image's alternative text (the
//! content) stay in the page, and it reports the refusal as
//! `flm.html.refused-address`
//! ([`AddressRefused`](super::emit::AddressRefused)) with the refused scheme.
//!
//! # Who decides which schemes are allowed
//!
//! The default list is the smallest that lets the standard features write the
//! links and images they need, and it holds no scheme that a browser runs as a
//! program. Under it, a page this library produced runs no program that a
//! document asked for.
//!
//! A host that needs more, such as a scheme of its own that a script of its
//! page handles (`app:open/42`), sets the render option
//! [`HtmlAllowedUrlSchemes`]: either the list of allowed schemes, which
//! replaces the default list, or [`HtmlAllowedUrlSchemes::All`], under which
//! every address is written as given.
//!
//! The option is set only by Rust code: on the render request
//! ([`PlanRequest::render_options`](crate::core::plan::PlanRequest::render_options())),
//! or by a workflow defined in Rust
//! ([`WorkflowDef::render_options`](crate::core::registry::WorkflowDef#structfield.render_options)).
//! It is not one of the `html` feature's settings keys
//! ([`crate::core::settings`]), so no settings written as data (a document's
//! front matter, a command line, the settings of a package) can set it.
//! Whether a page may reference an address that a browser runs is the decision
//! of the program that serves the page, not of a document or of the
//! configuration written beside it.
//!
//! # Where the policy does not apply
//!
//! The policy does not apply to a [construct node](super::construct), the node
//! a lowering rule stages (builds into the lowered tree; see
//! [`Lowerer`](crate::core::lowering::Lowerer)) to write one HTML tag of its
//! own with its own attributes: the rule that writes a tag is responsible for
//! what it writes, and its author must check the addresses it writes as this
//! module does, with [`check_address`] under the value of
//! [`HtmlAllowedUrlSchemes`] that the rule declares a read of
//! ([`LoweringDecl::option`](crate::core::lowering::LoweringDecl::option)).
//! Nor does it apply to the address of a style or script definition, which
//! comes from a feature's author or the host ([`super::page`]).
//!
//! # How a browser reads the scheme of an address
//!
//! A browser does not read the characters of an address exactly as written.
//! Before it looks for the scheme it removes every tab, line feed and carriage
//! return from the address, wherever they are, and ignores leading and
//! trailing spaces and control characters. `java\nscript:alert(1)` and
//! `  JaVaScRiPt:alert(1)` are therefore both the `javascript` scheme.
//! [`check_address`] does the same before it reads the scheme, and compares
//! the scheme ignoring letter case.
//!
//! It does **not** decode character references. `&#106;avascript:` is not a
//! hidden scheme here: the sink escapes every `&` it writes into an
//! attribute value as `&amp;`
//! ([`escape_attr_into`](super::escape::escape_attr_into)), so the `&` of a
//! reference never reaches the page as a `&`, and the address the browser
//! reads is the one this module read. Decoding references before the check
//! would therefore not make the check stricter; it would only make it disagree
//! with what the page will hold.

use std::collections::BTreeSet;
use std::fmt;

/// The schemes an address may have by default, whether it is a link or an
/// image: `http`, `https`, `mailto`, `ftp`, `tel`.
///
/// They are the schemes of the default value of the render option
/// [`HtmlAllowedUrlSchemes`] ([`HtmlAllowedUrlSchemes::default_schemes`]).
/// The comparison ignores letter case.
pub const DEFAULT_ALLOWED_URL_SCHEMES: [&str; 5] = ["http", "https", "mailto", "ftp", "tel"];

/// The scheme of an address that carries its own content instead of pointing
/// at something, `data`: allowed for an image whose media type begins with
/// [`IMAGE_MEDIA_PREFIX`], refused for everything else unless the render
/// option [`HtmlAllowedUrlSchemes`] allows the scheme `data` itself.
pub const DATA_SCHEME: &str = "data";

/// What the media type of a `data:` address must begin with for an image,
/// `image/`, comparing without regard to letter case: `data:image/png;base64,…`
/// is an image, `data:text/html,…` is a page and is refused.
pub const IMAGE_MEDIA_PREFIX: &str = "image/";

/// A URL scheme, such as `https`: the part of a URL before its first colon,
/// without the colon, held in lower case.
///
/// A scheme is an ASCII letter followed by ASCII letters, digits, `+`, `-` and
/// `.`; [`UrlScheme::new`] refuses any other name, so a value of this type is
/// always a scheme that an address can have.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UrlScheme(String);

impl UrlScheme {
    /// Returns the scheme `name`, held in lower case: `UrlScheme::new("HTTPS")`
    /// is the scheme `https`.
    ///
    /// # Errors
    ///
    /// [`InvalidUrlScheme`] when `name` is not a scheme: it is empty, it does
    /// not begin with an ASCII letter, or it holds a character other than an
    /// ASCII letter, a digit, `+`, `-` and `.` (the colon that ends a scheme in
    /// an address included: `app:` is not a scheme, `app` is).
    pub fn new(name: &str) -> Result<UrlScheme, InvalidUrlScheme> {
        let mut chars = name.chars();
        if chars.next().is_some_and(begins_scheme) && chars.all(continues_scheme) {
            Ok(UrlScheme(name.to_ascii_lowercase()))
        } else {
            Err(InvalidUrlScheme { name: String::from(name) })
        }
    }

    /// Returns the scheme's name, in lower case and without a colon.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UrlScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The error [`UrlScheme::new`] returns for a name that is not a URL scheme,
/// holding the name as given.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InvalidUrlScheme {
    /// The name as given.
    pub name: String,
}

impl fmt::Display for InvalidUrlScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not a URL scheme: a scheme is an ASCII letter followed by ASCII letters, \
             digits, `+`, `-` and `.`, given without its colon",
            self.name
        )
    }
}

impl std::error::Error for InvalidUrlScheme {}

/// The render option that decides which URL schemes the page may reference:
/// the schemes allowed in the URLs that the HTML backend writes into a link's
/// `href` and an image's `src` and `srcset`.
///
/// A URL with no scheme (a relative address, an address fragment) is always
/// allowed, whatever the value; [the module documentation](self) states the
/// whole policy. The default value is
/// [`Listed`](HtmlAllowedUrlSchemes::Listed) with
/// [`DEFAULT_ALLOWED_URL_SCHEMES`].
///
/// The option is bound to the render request
/// ([`Bind::Render`](crate::model::options::Bind::Render)) and is set only by
/// Rust code: it is declared by the `html` feature beside its settings, not
/// among them, so no settings written as data can set it ([who decides which
/// schemes are allowed](self#who-decides-which-schemes-are-allowed)). It is
/// an option type of its own rather than a field of
/// [`HtmlRenderOptions`](super::HtmlRenderOptions) for that reason, and so
/// that a workflow's value of it is never replaced when settings written as
/// data set a key of [`HtmlRenderOptions`](super::HtmlRenderOptions).
///
/// # Examples
///
/// A host that lets the page reference its own scheme `app`, beside the
/// default ones, and sets the value on its render request:
///
/// ```
/// use flm_core::model::options::OptionValues;
/// use flm_core::stdlib::backends::html::{
///     AddressUse, HtmlAllowedUrlSchemes, UrlScheme, check_address,
/// };
///
/// let mut schemes = HtmlAllowedUrlSchemes::default_schemes();
/// schemes.insert(UrlScheme::new("app").expect("a scheme"));
/// let allowed = HtmlAllowedUrlSchemes::Listed(schemes);
/// assert!(check_address("app:open/42", AddressUse::Link, &allowed).is_ok());
/// assert!(check_address("javascript:alert(1)", AddressUse::Link, &allowed).is_err());
///
/// // Given to `PlanRequest::render_options`.
/// let render_options = OptionValues::new().with(allowed);
/// # let _ = render_options;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HtmlAllowedUrlSchemes {
    /// Only the URLs whose scheme is one of these, beside the URLs with no
    /// scheme and, for an image, a `data:` URL carrying a picture. The list
    /// replaces the default list: a host that adds a scheme to the default
    /// ones starts from [`default_schemes`](HtmlAllowedUrlSchemes::default_schemes).
    Listed(BTreeSet<UrlScheme>),
    /// Every URL, written as given whatever its scheme, `javascript:`
    /// included: a link whose address is `javascript:…` then runs its program
    /// in the reader's browser when it is followed. Nothing is refused.
    All,
}

impl HtmlAllowedUrlSchemes {
    /// Returns the default schemes, [`DEFAULT_ALLOWED_URL_SCHEMES`], as the
    /// set that the default value lists.
    pub fn default_schemes() -> BTreeSet<UrlScheme> {
        DEFAULT_ALLOWED_URL_SCHEMES.iter().map(|name| UrlScheme(String::from(*name))).collect()
    }

    /// Returns the addresses this value allows, in words, as the report of a
    /// refused address gives them
    /// ([`AddressRefused`](super::emit::AddressRefused)).
    pub(super) fn allowed_addresses(&self) -> String {
        const PICTURE: &str = "for an image, `data:image/…`";
        let HtmlAllowedUrlSchemes::Listed(schemes) = self else {
            return String::from("every address");
        };
        let names: Vec<String> = schemes.iter().map(|scheme| format!("`{scheme}`")).collect();
        match names.split_last() {
            None => format!("those without a scheme and, {PICTURE}"),
            Some((last, [])) => {
                format!("those with the scheme {last}, those without a scheme, and, {PICTURE}")
            }
            Some((last, others)) => format!(
                "those with the scheme {} or {last}, those without a scheme, and, {PICTURE}",
                others.join(", ")
            ),
        }
    }
}

impl Default for HtmlAllowedUrlSchemes {
    fn default() -> HtmlAllowedUrlSchemes {
        HtmlAllowedUrlSchemes::Listed(HtmlAllowedUrlSchemes::default_schemes())
    }
}

/// How an address is used: as the target of a link, or as the source of an
/// image. [`check_address`] takes it.
///
/// The two differ in one point only — an image may be a `data:` address
/// carrying a picture, a link may not (see [the module
/// documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressUse {
    /// The `href` of a link.
    Link,
    /// The `src` or one candidate of the `srcset` of an image.
    Image,
}

/// The error [`check_address`] returns for an address the policy refuses,
/// holding the refused scheme.
///
/// The scheme is the address's own, in lower case and without its colon
/// (`javascript`); the emitter's report
/// ([`AddressRefused`](super::emit::AddressRefused)) gives it, so that a reader
/// of the report can see what the document asked for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefusedAddress {
    /// The scheme, in lower case, without the colon.
    pub scheme: String,
}

impl fmt::Display for RefusedAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the address scheme `{}` is not allowed in HTML output", self.scheme)
    }
}

/// Returns whether `ch` can begin a scheme: an ASCII letter.
fn begins_scheme(ch: char) -> bool {
    ch.is_ascii_alphabetic()
}

/// Returns whether `ch` can stand in a scheme after its first character: an
/// ASCII letter, a digit, `+`, `-` or `.`.
fn continues_scheme(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.')
}

/// Returns the address as a browser reads it before looking for its scheme: without
/// the tabs, line feeds and carriage returns it holds anywhere, and without
/// leading and trailing spaces and control characters.
fn normalized(address: &str) -> String {
    address
        .trim_matches(|ch: char| ch == ' ' || (ch as u32) <= 0x1F)
        .chars()
        .filter(|ch| !matches!(ch, '\t' | '\n' | '\r'))
        .collect()
}

/// Returns the scheme of a normalized address, in lower case and without its colon, or
/// `None` when the address has none and is therefore relative.
///
/// A scheme is an ASCII letter followed by ASCII letters, digits, `+`, `-` and
/// `.`, and ends at the first colon. Anything else before the first colon
/// means there is no scheme: the colon belongs to a path (`a/b:c`), to a
/// fragment (`#see:this`) or to a query, and the address is read against the
/// page's own.
fn scheme_of(address: &str) -> Option<String> {
    let mut scheme = String::new();
    for ch in address.chars() {
        if ch == ':' {
            return if scheme.is_empty() { None } else { Some(scheme) };
        }
        let fits = if scheme.is_empty() { begins_scheme(ch) } else { continues_scheme(ch) };
        if !fits {
            return None;
        }
        scheme.push(ch.to_ascii_lowercase());
    }
    None
}

/// Returns whether `address`, whose scheme is `data`, carries a picture: the
/// text after the colon begins with [`IMAGE_MEDIA_PREFIX`], in any letter
/// case.
fn is_image_data(address: &str) -> bool {
    match address.split_once(':') {
        Some((_, rest)) => rest
            .get(..IMAGE_MEDIA_PREFIX.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(IMAGE_MEDIA_PREFIX)),
        None => false,
    }
}

/// Checks whether `address` may be written for the use `used_as` under the
/// value `allowed` of the render option [`HtmlAllowedUrlSchemes`].
///
/// It may when it is an address fragment (`#…`), when it is relative, when
/// `allowed` allows its scheme, or when it is an image's `data:` address
/// carrying a picture; under [`HtmlAllowedUrlSchemes::All`], every address
/// may be written. The HTML emitter calls it for every link and image address
/// and writes the element without a refused address, keeping its content.
/// [The module documentation](self) states the policy and how an address is
/// read.
///
/// # Errors
///
/// [`RefusedAddress`], holding the scheme, when the address has a scheme that
/// `allowed` does not allow for that use.
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::backends::html::address::{
///     AddressUse, HtmlAllowedUrlSchemes, check_address,
/// };
///
/// let allowed = HtmlAllowedUrlSchemes::default();
/// assert!(check_address("#section-2", AddressUse::Link, &allowed).is_ok());
/// assert!(check_address("../figures/plot.png", AddressUse::Image, &allowed).is_ok());
/// assert!(check_address("https://example.org/", AddressUse::Link, &allowed).is_ok());
///
/// // A scheme that hides behind letter case and a leading control character.
/// let refused = check_address(" \u{1}JaVaScRiPt:alert(1)", AddressUse::Link, &allowed)
///     .expect_err("a scheme this backend does not write");
/// assert_eq!(refused.scheme, "javascript");
///
/// // A picture written into the page is an image, not a link.
/// let png = "data:image/png;base64,iVBORw0KGgo=";
/// assert!(check_address(png, AddressUse::Image, &allowed).is_ok());
/// assert!(check_address(png, AddressUse::Link, &allowed).is_err());
/// ```
pub fn check_address(
    address: &str,
    used_as: AddressUse,
    allowed: &HtmlAllowedUrlSchemes,
) -> Result<(), RefusedAddress> {
    let HtmlAllowedUrlSchemes::Listed(schemes) = allowed else {
        return Ok(());
    };
    let address = normalized(address);
    let Some(scheme) = scheme_of(&address) else {
        // No scheme: a fragment, or an address read against the page's own.
        return Ok(());
    };
    if schemes.iter().any(|listed| listed.as_str() == scheme) {
        return Ok(());
    }
    if used_as == AddressUse::Image && scheme == DATA_SCHEME && is_image_data(&address) {
        return Ok(());
    }
    Err(RefusedAddress { scheme })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The check under the default value of the option.
    fn check(address: &str, used_as: AddressUse) -> Result<(), RefusedAddress> {
        check_address(address, used_as, &HtmlAllowedUrlSchemes::default())
    }

    #[test]
    fn an_address_with_no_scheme_is_written_as_it_stands() {
        for address in [
            "#section-2",
            "",
            "figures/plot.png",
            "../index.html",
            "/absolute/path",
            "//example.org/page",
            "?page=2",
            "a/b:c",          // the colon belongs to the path
            "#see:this",      // and here to the fragment
            "9lives:x",       // a scheme cannot begin with a digit
            "my scheme:x",    // nor hold a space
        ] {
            assert!(check(address, AddressUse::Link).is_ok(), "{address:?}");
            assert!(check(address, AddressUse::Image).is_ok(), "{address:?}");
        }
    }

    #[test]
    fn the_listed_schemes_are_written_whatever_their_letter_case() {
        for scheme in DEFAULT_ALLOWED_URL_SCHEMES {
            let address = format!("{scheme}:x");
            assert!(check(&address, AddressUse::Link).is_ok(), "{address}");
            assert!(check(&address.to_uppercase(), AddressUse::Image).is_ok(), "{address}");
        }
        // A scheme's own characters do not end it.
        assert!(check("view-source+x:y", AddressUse::Link).is_err());
    }

    #[test]
    fn a_scheme_hidden_by_case_whitespace_or_control_characters_is_still_refused() {
        for address in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "  javascript:alert(1)",
            "\u{1}\u{2}javascript:alert(1)",
            "\tjava\nscript:alert(1)",
            "java\r\nscript:alert(1)",
            "  \n javascript:alert(1)  \n",
        ] {
            let refused =
                check(address, AddressUse::Link).expect_err("a refused scheme");
            assert_eq!(refused.scheme, "javascript", "{address:?}");
        }
    }

    #[test]
    fn a_data_address_is_a_picture_for_an_image_and_nothing_for_a_link() {
        let png = "data:image/png;base64,iVBORw0KGgo=";
        assert!(check(png, AddressUse::Image).is_ok());
        assert!(check("DATA:IMAGE/PNG;base64,iVBORw0KGgo=", AddressUse::Image).is_ok());
        assert_eq!(
            check(png, AddressUse::Link).expect_err("not for a link").scheme,
            "data"
        );
        for address in ["data:text/html,<script>x</script>", "data:,plain", "data:image"] {
            assert_eq!(
                check(address, AddressUse::Image).expect_err("not a picture").scheme,
                "data",
                "{address:?}"
            );
        }
    }

    #[test]
    fn every_other_scheme_is_refused_and_names_itself() {
        for (address, scheme) in [
            ("vbscript:x", "vbscript"),
            ("file:///etc/passwd", "file"),
            ("about:blank", "about"),
            ("blob:https://example.org/x", "blob"),
        ] {
            let refused =
                check(address, AddressUse::Link).expect_err("a refused scheme");
            assert_eq!(refused.scheme, scheme, "{address:?}");
        }
    }

    #[test]
    fn a_scheme_written_with_character_references_is_not_decoded() {
        // The sink escapes the `&` of a reference, so what stands here is an
        // address beginning with `&`, which is no scheme at all, and the page
        // holds `&amp;#106;avascript:x`: a relative address, fetched, never run.
        assert!(check("&#106;avascript:alert(1)", AddressUse::Link).is_ok());
    }

    #[test]
    fn a_hosts_list_replaces_the_default_and_all_writes_every_address() {
        let only_app =
            HtmlAllowedUrlSchemes::Listed(BTreeSet::from([UrlScheme::new("APP").unwrap()]));
        assert!(check_address("app:open/42", AddressUse::Link, &only_app).is_ok());
        let refused = check_address("https://example.org/", AddressUse::Link, &only_app)
            .expect_err("a scheme the list leaves out");
        assert_eq!(refused.scheme, "https");
        // A picture written into the page is not a scheme of the list.
        assert!(check_address("data:image/png;base64,x", AddressUse::Image, &only_app).is_ok());
        for address in ["javascript:alert(1)", "data:text/html,x", "app:x"] {
            assert!(check_address(address, AddressUse::Link, &HtmlAllowedUrlSchemes::All).is_ok());
        }
        // A name that is not a scheme cannot be listed.
        for name in ["app:", "", "9p", "my scheme"] {
            assert!(UrlScheme::new(name).is_err(), "{name:?}");
        }
    }
}
