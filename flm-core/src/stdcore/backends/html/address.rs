//! The address policy: which addresses the HTML backend writes into a page,
//! and which it refuses.
//!
//! An **address** here is what a link points at (`href`) or what an image is
//! loaded from (`src`, `srcset`). Both come from the document or from the host
//! that rendered it, and a browser can do more with an address than fetch it:
//! when a link is followed whose address has the **scheme** `javascript` (the
//! scheme is the part before the first colon, `https` in
//! `https://example.org`), the browser runs the rest of the address as a
//! program.
//!
//! [`check_address`] therefore accepts an address only when it is one of
//! these:
//!
//! - an **address fragment**, `#section-2`: a place in the same page;
//! - a **relative address**, `figures/plot.png`, `../index.html`, `?page=2`:
//!   one with no scheme at all, read against the address of the page it is
//!   in;
//! - an address whose scheme is one of [`ALLOWED_SCHEMES`];
//! - for an image only, a `data:` address whose media type begins with
//!   `image/` ([`DATA_SCHEME`], [`IMAGE_MEDIA_PREFIX`]) — a picture written
//!   into the page itself.
//!
//! Anything else is refused. The list is fixed data, not a setting: it is the
//! smallest list that lets the standard features write the links and images
//! they need. Because the list is fixed, this library can promise that a page
//! it produced runs no program that a document asked for.
//!
//! A refused address loses the address and nothing else: the emitter writes
//! the link without its `href` and the image without its `src`, so the link's
//! text and the image's alternative text (the content) stay in the page, and
//! it reports the refusal as `flm.html.refused-address`
//! ([`AddressRefused`](super::emit::AddressRefused)) with the refused scheme.
//!
//! The policy does not apply to a [construct node](super::construct), the node
//! a lowering rule stages (builds into the lowered tree; see
//! [`Lowerer`](crate::core::lowering::Lowerer)) to write one HTML tag of its
//! own with its own attributes: the rule that writes a tag is responsible for what it writes,
//! and its author must check the addresses it writes as this module does. Nor
//! does it apply to the address of a style or script definition, which comes
//! from a feature's author or the host ([`super::page`]).
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

use std::fmt;

/// The schemes an address may have, whether it is a link or an image:
/// `http`, `https`, `mailto`, `ftp`, `tel`.
///
/// The comparison ignores letter case. See [the module documentation](self)
/// for why the list is fixed data rather than a setting.
pub const ALLOWED_SCHEMES: [&str; 5] = ["http", "https", "mailto", "ftp", "tel"];

/// The scheme of an address that carries its own content instead of pointing
/// at something, `data`: allowed for an image whose media type begins with
/// [`IMAGE_MEDIA_PREFIX`], refused for everything else.
pub const DATA_SCHEME: &str = "data";

/// What the media type of a `data:` address must begin with for an image,
/// `image/`, comparing without regard to letter case: `data:image/png;base64,…`
/// is an image, `data:text/html,…` is a page and is refused.
pub const IMAGE_MEDIA_PREFIX: &str = "image/";

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
        let continues = !scheme.is_empty() && (ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.'));
        if ch.is_ascii_alphabetic() || continues {
            scheme.push(ch.to_ascii_lowercase());
        } else {
            return None;
        }
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

/// Checks whether `address` may be written for the use `used_as`.
///
/// It may when it is an address fragment (`#…`), when it is relative, when its scheme is
/// one of [`ALLOWED_SCHEMES`], or when it is an image's `data:` address
/// carrying a picture. The HTML emitter calls it for every link and image
/// address and writes the element without a refused address, keeping its
/// content. [The module documentation](self) states the policy and how an
/// address is read.
///
/// # Errors
///
/// [`RefusedAddress`], holding the scheme, when the address has a scheme that
/// the policy does not allow for that use.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::address::{AddressUse, check_address};
///
/// assert!(check_address("#section-2", AddressUse::Link).is_ok());
/// assert!(check_address("../figures/plot.png", AddressUse::Image).is_ok());
/// assert!(check_address("https://example.org/", AddressUse::Link).is_ok());
///
/// // A scheme that hides behind letter case and a leading control character.
/// let refused = check_address(" \u{1}JaVaScRiPt:alert(1)", AddressUse::Link)
///     .expect_err("a scheme this backend does not write");
/// assert_eq!(refused.scheme, "javascript");
///
/// // A picture written into the page is an image, not a link.
/// let png = "data:image/png;base64,iVBORw0KGgo=";
/// assert!(check_address(png, AddressUse::Image).is_ok());
/// assert!(check_address(png, AddressUse::Link).is_err());
/// ```
pub fn check_address(address: &str, used_as: AddressUse) -> Result<(), RefusedAddress> {
    let address = normalized(address);
    let Some(scheme) = scheme_of(&address) else {
        // No scheme: a fragment, or an address read against the page's own.
        return Ok(());
    };
    if ALLOWED_SCHEMES.contains(&scheme.as_str()) {
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
            assert!(check_address(address, AddressUse::Link).is_ok(), "{address:?}");
            assert!(check_address(address, AddressUse::Image).is_ok(), "{address:?}");
        }
    }

    #[test]
    fn the_listed_schemes_are_written_whatever_their_letter_case() {
        for scheme in ALLOWED_SCHEMES {
            let address = format!("{scheme}:x");
            assert!(check_address(&address, AddressUse::Link).is_ok(), "{address}");
            assert!(check_address(&address.to_uppercase(), AddressUse::Image).is_ok(), "{address}");
        }
        // A scheme's own characters do not end it.
        assert!(check_address("view-source+x:y", AddressUse::Link).is_err());
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
                check_address(address, AddressUse::Link).expect_err("a refused scheme");
            assert_eq!(refused.scheme, "javascript", "{address:?}");
        }
    }

    #[test]
    fn a_data_address_is_a_picture_for_an_image_and_nothing_for_a_link() {
        let png = "data:image/png;base64,iVBORw0KGgo=";
        assert!(check_address(png, AddressUse::Image).is_ok());
        assert!(check_address("DATA:IMAGE/PNG;base64,iVBORw0KGgo=", AddressUse::Image).is_ok());
        assert_eq!(
            check_address(png, AddressUse::Link).expect_err("not for a link").scheme,
            "data"
        );
        for address in ["data:text/html,<script>x</script>", "data:,plain", "data:image"] {
            assert_eq!(
                check_address(address, AddressUse::Image).expect_err("not a picture").scheme,
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
                check_address(address, AddressUse::Link).expect_err("a refused scheme");
            assert_eq!(refused.scheme, scheme, "{address:?}");
            assert!(refused.to_string().contains(scheme));
        }
    }

    #[test]
    fn a_scheme_written_with_character_references_is_not_decoded() {
        // The sink escapes the `&` of a reference, so what stands here is an
        // address beginning with `&`, which is no scheme at all, and the page
        // holds `&amp;#106;avascript:x`: a relative address, fetched, never run.
        assert!(check_address("&#106;avascript:alert(1)", AddressUse::Link).is_ok());
    }
}
