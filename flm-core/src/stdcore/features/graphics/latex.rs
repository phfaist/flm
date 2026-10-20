//! The lowering rule of the [`graphics`](super) feature for the
//! `latex-recomposed` backend, which writes an image as the LaTeX hook
//! `\flmIncludeGraphics`, and the feature's definition piece, which defines
//! that hook.
//!
//! | written | LaTeX |
//! |---|---|
//! | `\includegraphics{figures/plot.png}` | `\flmIncludeGraphics{figures/plot.png}` |
//! | `\includegraphics[width=5cm]{plot.png}` | `\flmIncludeGraphics[width=5cm]{plot.png}` (with no inspector) |
//! | `\includegraphics[width=10pt]{plot.png}` | `\flmIncludeGraphics[width=10bp]{plot.png}` (with no inspector) |
//!
//! # The LaTeX hook
//!
//! A LaTeX hook is a macro that the recomposed LaTeX writes and the style
//! file defines with a plain default, which a user redefines in the
//! document's preamble ([LaTeX hooks](crate::stdcore::backends::latex::preamble);
//! [the LaTeX hooks](crate::guide::rendering#the-latex-hooks) in the rendering
//! guide). The rule [`LATEX_RULE`] ([`ImageLatexRule`]) writes an image as
//! `\flmIncludeGraphics[<keys>]{<path>}` ([`HOOK_COMMAND`]):
//!
//! - `<keys>` are `width=<length>` and `height=<length>` where the image is
//!   shown at a set size, the size being decided as on every output
//!   ([`shown_size`](super::rules::shown_size)): the options, one dimension
//!   scaled from the inspected size where only the other is given, else the
//!   inspected size, each multiplied by the output's magnification. Each length is written by [`stage_length`], the LaTeX
//!   family's only way of writing a length, which writes a point (1/72 inch)
//!   as LaTeX's big point `bp`. The optional argument is left out where there
//!   is no key.
//! - `<path>` is the published address (the path as written, under the
//!   strategy `reference`), escaped as the LaTeX family escapes an address
//!   ([`escape_url`]).
//!
//! The path is read as characters, so a user definition whose template holds
//! a placeholder in a path is not written as a LaTeX definition: each of its
//! use sites is expanded instead. The alternative text is not written,
//! because LaTeX's `\includegraphics` has no place for it.
//!
//! # The definition piece
//!
//! A definition piece ([`LatexPreambleDef`]) is a unit of LaTeX definitions
//! that a feature registers under an identifier `flm.latex.def.<name>`; a
//! lowering rule requires it by identifier, and the style file holds the text
//! of every piece the document requires. The feature's piece, [`PIECE`]
//! (`flm.latex.def.graphics`, [`definition_piece`]), defines the LaTeX hook with
//! `\ProvideDocumentCommand` as `\includegraphics[max width=\linewidth,#1]{#2}`,
//! so that an image is never wider than the line. The piece requests the
//! LaTeX packages `graphicx` and `adjustbox`, the latter with its option
//! `export`, which makes `max width` a key of `\includegraphics` ([package
//! requests](crate::stdcore::backends::latex::packages)). A user who wants
//! another default redefines the LaTeX hook, before or after loading the style
//! file. The rendering guide shows the result ([images in the recomposed
//! LaTeX](crate::guide::rendering#images-in-the-recomposed-latex)).

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{ARG_SLOT, MACRO, RAW};
use crate::stdcore::backends::latex::preamble::stage_length;
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexMacroPayload, LatexPreambleDef, LatexRawPayload, escape_url, packages,
    preamble,
};
use crate::stdcore::backends::latex_recomposed::BACKEND;
use crate::stdcore::backends::pres::Length;

use super::inspect::INFO;
use super::options::{HEIGHT_KEY, WIDTH_KEY};
use super::publish::published_table;
use super::rules::read_image;
use super::strategy::GraphicsRenderOptions;
use super::IMAGE_ELEMENT;

/// The name of the image's rule on the `latex-recomposed` backend,
/// `flm.graphics.image.lower.latex-recomposed`, whose body is
/// [`ImageLatexRule`].
pub const LATEX_RULE: &str = "flm.graphics.image.lower.latex-recomposed";

/// The name of the feature's definition piece, `graphics`; its identifier is
/// `flm.latex.def.graphics` ([`PIECE`]).
pub const PIECE_NAME: &str = "graphics";

/// The identifier of the feature's definition piece, `flm.latex.def.graphics`
/// ([`definition_piece`]).
pub const PIECE: &str = "flm.latex.def.graphics";

/// The name of the LaTeX hook an image is written as, `flmIncludeGraphics`.
pub const HOOK_COMMAND: &str = "flmIncludeGraphics";

/// Returns the feature's definition piece, `flm.latex.def.graphics`, which
/// defines the LaTeX hook `\flmIncludeGraphics[<keys>]{<path>}` with
/// `\ProvideDocumentCommand` as
/// `\includegraphics[max width=\linewidth,#1]{#2}`. It requests the LaTeX
/// packages `graphicx` ([`packages::GRAPHICX`]) and `adjustbox` with its
/// option `export` ([`packages::ADJUSTBOX`]). Redefine the LaTeX hook, before or
/// after loading the style file, to show images another way.
pub fn definition_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        PIECE_NAME,
        "\
% An image, as FLM's \\includegraphics writes it:
% \\flmIncludeGraphics[<keys>]{<path>}, <keys> being width=<length> and
% height=<length> where the image is shown at a set size, <path> an ordinary
% argument in which the characters # % \\ are written \\# \\% \\\\.
% The image never runs wider than the line (max width, a key adjustbox adds to
% \\includegraphics). Redefine the hook to show images another way.
\\ProvideDocumentCommand\\flmIncludeGraphics{O{} m}{\\includegraphics[max width=\\linewidth,#1]{#2}}
",
        "`\\flmIncludeGraphics[keys]{path}`: the LaTeX hook an image is written as, \
         `\\includegraphics` kept within the line's width",
    )
    .requires([packages::GRAPHICX, packages::ADJUSTBOX])
}

/// The body of [`LATEX_RULE`]: it stages the LaTeX hook `\flmIncludeGraphics`, with
/// the image's size as keys where the image has one, and its published
/// address escaped by [`escape_url`]. See the [module documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct ImageLatexRule;

impl Lower for ImageLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let (src, (width, height)) = {
            let Some(image) = read_image(&node, BACKEND, lw)? else {
                return Ok(Lowering::Emit(Vec::new()));
            };
            (image.published.src.clone(), image.size)
        };
        let mut shapes = Vec::new();
        let mut slots = Vec::new();
        let keys = keys(cx, lw, width.as_ref(), height.as_ref())?;
        if !keys.is_empty() {
            shapes.push(LatexArgShape::Optional);
            slots.push(keys);
        }
        shapes.push(LatexArgShape::Mandatory);
        slots.push(raw(cx, lw, &escape_url(&src))?);
        lw.require(&Name::from(PIECE));
        let hook = lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(HOOK_COMMAND, shapes),
            slots.into_iter().map(|slot| (String::from(ARG_SLOT), slot)).collect(),
        )?;
        Ok(Lowering::Emit(vec![hook]))
    }
}

/// Stages `text` as one raw node (`flm.pres.latex.raw`); stages nothing
/// where `text` is empty.
fn raw(cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, text: &str) -> Result<Vec<BuildId>, LowerError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())?])
}

/// Stages the keys of the LaTeX hook `\flmIncludeGraphics`: `width=<length>`
/// and `height=<length>` for those set, separated by a comma, each length
/// written by [`stage_length`].
fn keys(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    width: Option<&Length>,
    height: Option<&Length>,
) -> Result<Vec<BuildId>, LowerError> {
    let mut nodes = Vec::new();
    for (key, length) in [(WIDTH_KEY, width), (HEIGHT_KEY, height)] {
        let Some(length) = length else { continue };
        let lead = if nodes.is_empty() { format!("{key}=") } else { format!(",{key}=") };
        nodes.extend(raw(cx, lw, &lead)?);
        nodes.push(stage_length(cx, lw, length)?);
    }
    Ok(nodes)
}

/// Returns the image's rule on the `latex-recomposed` backend,
/// [`LATEX_RULE`], the default on that backend only. It reads the table of
/// records [`INFO`], the backend's published table and the render options
/// [`GraphicsRenderOptions`] (for the magnification).
pub fn latex_rule() -> LoweringRule {
    LoweringRule::new(
        LATEX_RULE,
        IMAGE_ELEMENT,
        LoweringDecl::new()
            .reads(INFO)
            .reads(published_table(BACKEND))
            .option::<GraphicsRenderOptions>()
            .produces(MACRO)
            .produces(RAW),
        Arc::new(ImageLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "an image lowers to the LaTeX hook `\\flmIncludeGraphics`, with its size as keys where it \
         has one, and its escaped address",
    )
}

/// Registers the image's rule on the `latex-recomposed` backend and
/// contributes the definition piece to the service slot of definition pieces
/// ([`preamble::SLOT`]), with peer declarations of the names that the LaTeX
/// features own (the backend, the macro construct and the raw construct). A
/// peer declaration ([`PeerName`](crate::core::registry::PeerName)) tells the
/// registry build that another feature owns a name, so the feature builds in
/// a registry without the LaTeX features, with its rule set aside (excluded
/// from rule selection; see [`PeerName`](crate::core::registry::PeerName)).
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made
/// outside the registration round that accepts it (see
/// [`Finalizer`](crate::core::registry::Finalizer)).
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the image's LaTeX rule is the default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the image's LaTeX rule stages for the \
         LaTeX hook `\\flmIncludeGraphics`",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw construct, whose raw nodes hold the keys and the address",
    )?;
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(definition_piece()))?;
    b.lowering_rule(latex_rule())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::latex::preamble::piece_id;

    #[test]
    fn the_piece_defines_the_hook_and_requests_the_two_packages() {
        let piece = definition_piece();
        assert_eq!(piece.id, piece_id(PIECE_NAME));
        assert_eq!(piece.id.as_str(), PIECE);
        assert_eq!(piece.requires, [Name::from(packages::GRAPHICX), Name::from(packages::ADJUSTBOX)]);
        assert!(piece.content.contains(
            "\\ProvideDocumentCommand\\flmIncludeGraphics{O{} m}{\\includegraphics[max \
             width=\\linewidth,#1]{#2}}"
        ));
    }

    #[test]
    fn the_rule_is_the_default_on_the_recomposed_backend_only() {
        let rule = latex_rule();
        assert_eq!(rule.element, IMAGE_ELEMENT);
        assert!(rule.default.is_specific_to(&BACKEND.into()));
    }
}
