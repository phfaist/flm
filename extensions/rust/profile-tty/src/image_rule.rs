//! The lowering rule `tty.graphics.image` of the text backend, which the workflow `tty`
//! prefers: an image shown as a block becomes its label and the image drawn as half-block
//! characters (the module `graphics`).
//!
//! The rule lowers the node `flm.pres.image` that the graphics feature's own rule staged in
//! the round before, so that the reading of the image's path and options, its inspection,
//! its diagnostics and its publishing stay the graphics feature's. It reads the image's
//! content from the table `flm.graphics.content.text`, which the publish stage of the text
//! backend fills under the strategy `content` (the workflow's setting), the row whose
//! address is the image's.
//!
//! - An image shown as a block becomes a block of the role [`ROLE`], the block that stands
//!   for the image: it holds the label, `[image: <alternative text, or address>]` (the text
//!   backend's own form for an image), and, when the image is drawn, a verbatim block of the
//!   glyph rows, at the size the module `graphics` computes from the image's shown size and
//!   the cell size of the option type `tty.render`.
//! - The image is drawn when its content decodes. Without content (another strategy, or the
//!   publish stage reported why it read none) the block holds the label alone; with content
//!   of another format than PNG, JPEG, GIF and WebP (an SVG image that no conversion made
//!   into PNG), or content that does not decode, the label alone and the Error
//!   [`NOT_SHOWN`].
//! - An image shown inline becomes its label alone, in the line.

use flm_ext::ExtError;
use flm_ext::guest::core::data::table::TableValue;
use flm_ext::guest::core::lowering::{Lower, LowerCx, Lowerer, Lowering};
use flm_ext::guest::stdlib::backends::pres;
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::techy::node::Node;
use serde_json::{Value, json};

use crate::profile_tty::graphics::{self, NotDecoded};

/// The rule's name.
pub const RULE: &str = "tty.graphics.image";

/// The table of the images' content, which the text backend's publish stage writes.
pub const CONTENT: &str = "flm.graphics.content.text";

/// The role of the block that stands for an image shown as a block: it holds the image's
/// label and, when the image is drawn, the verbatim block of its glyph rows.
pub const ROLE: &str = "tty.graphics";

/// The diagnostic for an image whose content cannot be drawn.
pub const NOT_SHOWN: &str = "tty.graphics-not-shown";

/// The body of the rule [`RULE`].
pub struct ImageRule;

impl Lower for ImageRule {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let payload =
            node.payload()?.ok_or_else(|| ExtError::Value { detail: "an image node must carry its payload".into() })?;
        let src = payload["src"].as_str().unwrap_or_default();
        let label = label(&payload);
        if payload["display"] != "block" {
            return Ok(Lowering::Emit(vec![lw.text(cx, &label)?].into()));
        }
        let mut children = vec![lw.text(cx, &label)?];
        if let Some(rows) = drawing(lw, &payload, src)? {
            let content = lw.text(cx, &rows)?;
            children.push(lw.pres(cx, pres::VERBATIM, &json!({"kind": "text", "display": "block"}), vec![content].into())?);
        }
        let mut block = json!({ "role": ROLE });
        if let Some(anchor) = payload.get("anchor").filter(|anchor| anchor.is_string()) {
            block["anchor"] = anchor.clone();
        }
        Ok(Lowering::Emit(vec![lw.pres(cx, pres::BLOCK, &block, children.into())?].into()))
    }
}

/// The label of the image whose payload is `payload`, as the text backend writes an image:
/// `[image: <alternative text, or address>]`, on one line.
fn label(payload: &Value) -> String {
    let alt = payload["alt"].as_str().filter(|alt| !alt.trim().is_empty());
    let shown = alt.or_else(|| payload["src"].as_str()).unwrap_or_default();
    let shown = shown.replace(['\n', '\r', '\t', '\u{000B}', '\u{000C}', '\u{0085}', '\u{2028}', '\u{2029}'], " ");
    format!("[image: {shown}]")
}

/// The glyph rows of the image at `src` whose payload is `payload`, joined by newlines;
/// `None` when the image has no content to draw, after reporting why when its content does
/// not decode.
fn drawing(lw: &mut Lowerer, payload: &Value, src: &str) -> Result<Option<String>, ExtError> {
    let Some((media_type, content)) = content_of(lw, src)? else {
        return Ok(None);
    };
    let image = match graphics::decode(&media_type, &content) {
        Ok(image) => image,
        Err(why) => {
            let message = match why {
                NotDecoded::Format => format!(
                    "the terminal output shows an image in the PNG, JPEG, GIF or WebP format; `{src}` is of the \
                     type `{media_type}`"
                ),
                NotDecoded::Invalid(detail) => format!(
                    "the terminal output shows an image in the PNG, JPEG, GIF or WebP format; `{src}` is not a \
                     valid image of the type `{media_type}`: {detail}"
                ),
            };
            lw.diagnose(Diagnostic::error(NOT_SHOWN, message))?;
            return Ok(None);
        }
    };
    let cell = crate::profile_tty::cell_size(&lw.option(crate::profile_tty::RENDER)?).0;
    let length = |key: &str| payload[key].as_str().and_then(graphics::points);
    let (columns, rows) = graphics::grid(length("width"), length("height"), (image.width(), image.height()), cell);
    Ok(Some(graphics::render(&image, columns, rows).glyphs.join("\n")))
}

/// The media type and the content of the image at `src`, from the table [`CONTENT`];
/// `None` when the table holds no row for it.
fn content_of(lw: &Lowerer, src: &str) -> Result<Option<(String, Vec<u8>)>, ExtError> {
    let rows = content_rows(lw.get(CONTENT)?)?;
    rows.iter().find(|row| row["src"] == src).map(row_content).transpose()
}

/// The rows of the table [`CONTENT`] as `table`, its value read, holds them: none when the
/// table holds no value.
pub fn content_rows(table: Option<TableValue>) -> Result<Vec<Value>, ExtError> {
    match table {
        None => Ok(Vec::new()),
        Some(TableValue::Projection(Value::Array(rows))) => Ok(rows),
        Some(_) => Err(not_the_content_table()),
    }
}

/// The media type and the content of `row`, a row of the table [`CONTENT`].
pub fn row_content(row: &Value) -> Result<(String, Vec<u8>), ExtError> {
    let media_type = row["media_type"].as_str().ok_or_else(not_the_content_table)?;
    let content = graphics::content_bytes(&row["content"]).ok_or_else(not_the_content_table)?;
    Ok((media_type.to_string(), content))
}

/// The error for a table [`CONTENT`] that is not read as the rows of the images' content.
fn not_the_content_table() -> ExtError {
    ExtError::Value { detail: format!("the table `{CONTENT}` must be read as the rows of the images' content") }
}
