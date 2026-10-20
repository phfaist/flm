//! The content handler that the [`graphics`](super) feature contributes to the
//! `floats` feature: `includegraphics`, which accepts exactly one image.
//!
//! A float type lists by name the **content handlers** it accepts. A content
//! handler ([`FloatContentHandler`]) is a value that accepts a float's content
//! or refuses it, contributed to the service slot
//! [`CONTENT_SLOT`](crate::stdcore::features::floats::CONTENT_SLOT)
//! (`flm.floats.content`) by the feature that owns that kind of content. This
//! feature owns images, so it contributes [`HANDLER`] (`includegraphics`,
//! [`IncludeGraphicsContent`]): it accepts a float's content where the content
//! is exactly one node of the image element [`IMAGE_ELEMENT`] (the body of a
//! `figure` holding `\includegraphics{plot.png}` beside its caption and
//! labels), and refuses it otherwise with the words `exactly one image`, which
//! complete the float's Error ("a ‘figure’ must hold exactly one image").
//!
//! The contribution is a peer contribution: in a registry without the
//! `floats` feature, no feature declares the slot, the registry build drops
//! the contribution with one Note, and the `graphics` feature builds and works
//! as it does on its own.

use techy::core::node::NodeRef;

use crate::lang::Flm;
use crate::model::element::element_of;
use crate::stdcore::features::floats::FloatContentHandler;

use super::IMAGE_ELEMENT;

/// The name of the feature's content handler, `includegraphics`: the name a
/// float type lists among its handlers to accept one image
/// ([`FloatType::handlers`](crate::stdcore::features::floats::FloatType::handlers)).
pub const HANDLER: &str = "includegraphics";

/// The content handler [`HANDLER`] (`includegraphics`): it accepts exactly one
/// image, a node of [`IMAGE_ELEMENT`], and refuses anything else: two images,
/// text, an image beside a paragraph, or no content at all. See the [module
/// documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct IncludeGraphicsContent;

impl FloatContentHandler for IncludeGraphicsContent {
    fn name(&self) -> &str {
        HANDLER
    }

    fn admit(&self, content: &[NodeRef<'_, Flm>]) -> Result<(), String> {
        match content {
            [one] if element_of(one).name.as_str() == IMAGE_ELEMENT => Ok(()),
            _ => Err(String::from("exactly one image")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::features::floats::INCLUDEGRAPHICS_HANDLER;

    #[test]
    fn the_handler_is_the_one_the_default_float_types_name() {
        assert_eq!(IncludeGraphicsContent.name(), HANDLER);
        assert_eq!(HANDLER, INCLUDEGRAPHICS_HANDLER);
    }

    #[test]
    fn no_content_is_refused() {
        assert_eq!(IncludeGraphicsContent.admit(&[]), Err(String::from("exactly one image")));
    }
}
