//! The extension `profile-tty`: the workflow `tty`, which the profile `tty` runs, and its
//! output `tty`, the document as text for a terminal: the printed text of the output `text`
//! with the escape sequences that show its bold, italic and underlined runs, its colors and
//! its hyperlinks.
//!
//! The extension is written against the guest client `flm_ext::guest`. The `flm` program
//! carries it compiled in (the engine `native`); `extensions/rust/build-wasm.sh` builds the
//! same code as a WebAssembly module (the engine `wasm`).
//!
//! Its package `_` declares the workflow `tty` (extending `text`, with the
//! output `tty` in place of `text`, so that it runs every check and every lowering choice of
//! `text`, and with the settings `text.table_frame=box` and `text.heading_rules=unicode`,
//! which frame the tables and draw the rules under the headings with box-drawing
//! characters, and the graphics feature's strategy `content` for the text output, with a
//! conversion of `svg` and `pdf` images to `png`), and holds the feature `tty`. The workflow, the feature and the output are named
//! after the profile, and the names of the table and the stage begin with `tty.`: the
//! extension layer lets an extension whose stem is `profile-X` declare its names under `X`
//! as well as under its stem. The feature declares:
//!
//! - the option type `tty.render`, set on a render request (`-O tty.KEY=VALUE`) or by a
//!   workflow's settings, with the key `colors`: `none`, `standard` (the default) or
//!   `rich`, which colors the painter writes (the module `paint`), any other value being an
//!   error of the stage, which then paints without colors; and the keys
//!   `graphics_cell_width_pt` and `graphics_cell_height_pt`, the size of a terminal cell in
//!   points (6 and 12 by default), by which an image's shown size is divided (the module
//!   `graphics`), a value that is not a positive number being an error of the stage
//!   (`tty.invalid-cell-size`) and the default applying;
//! - the table `tty.document`, of the type `ArtifactSet`, which holds the painted text;
//! - the output `tty` (media type `text/plain`, extension `ansi`);
//! - the stage `tty.paint` (effect `Compute`, once per document), which reads the table
//!   `flm.output.text` (the printed text and its style marks), paints it (the module `paint`) and
//!   writes the result to `tty.document`; under colors other than `none`, it also colors the
//!   images the rule below drew, from the tables `flm.lowered.text` and
//!   `flm.graphics.content.text` (the module `drawn`);
//! - the lowering rule `tty.graphics.image` of the text backend, which the workflow `tty`
//!   prefers (the module `image_rule`): an image shown as a block becomes a block of the role
//!   `tty.graphics` holding its label and its drawing in half-block characters, read from the
//!   table `flm.graphics.content.text`.
//!
//! The text backend lays the text out and records which ranges of it are bold, italic,
//! links and so on; the extension only writes those ranges with escape sequences, so the
//! layout of the output `tty` is the layout of the output `text`, set by the same render
//! options (`text.width`, `text.link_addresses`, `text.table_frame`, `text.heading_rules`). The workflow `tty` extends `text` and the
//! stage needs the table `flm.output.text`, so a document that uses the extension cannot
//! switch the feature `text` off.

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::data::table::TableValue;
use flm_ext::guest::core::lowering::{DefaultMark, LoweringDecl, LoweringRule};
use flm_ext::guest::core::registry::{FeatureBuilder, OutputDef, StandardTable};
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_ext::guest::stdlib::backends::pres;
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::{Map, Value, json};

mod drawn;
mod graphics;
mod image_rule;
mod paint;

/// The name of the profile `tty`, which loads the extension `profile-tty` and runs its
/// workflow: the name of the extension's workflow, its feature and its output, and the
/// beginning of the name of its table and its stage.
const PROFILE: &str = "tty";

/// The extension's output.
const OUTPUT: &str = PROFILE;

/// The table holding the painted text: the output's table and capability.
const DOCUMENT: &str = "tty.document";

/// The table of the text backend holding the printed text and its style marks, which the
/// stage reads.
const TEXT_OUTPUT: &str = "flm.output.text";

/// The option type of the feature `tty`, set on a render request: its keys [`COLORS`],
/// [`CELL_WIDTH`] and [`CELL_HEIGHT`].
const RENDER: &str = "tty.render";

/// The key of [`RENDER`] that chooses which colors the painter writes, the setting
/// `tty.colors`.
const COLORS: &str = "colors";

/// The values of [`COLORS`], in the order the declaration and the message list them.
const COLORS_VALUES: [&str; 3] = ["none", "standard", "rich"];

/// The diagnostic for a value of [`COLORS`] that is none of [`COLORS_VALUES`].
const INVALID_COLORS: &str = "tty.invalid-colors";

/// The key of [`RENDER`] that gives the width of a terminal cell in points, the setting
/// `tty.graphics_cell_width_pt`.
const CELL_WIDTH: &str = "graphics_cell_width_pt";

/// The key of [`RENDER`] that gives the height of a terminal cell in points, the setting
/// `tty.graphics_cell_height_pt`.
const CELL_HEIGHT: &str = "graphics_cell_height_pt";

/// The diagnostic for a value of [`CELL_WIDTH`] or [`CELL_HEIGHT`] that is not a positive
/// number.
const INVALID_CELL_SIZE: &str = "tty.invalid-cell-size";

/// The media type of the output.
const MEDIA_TYPE: &str = "text/plain";

/// The file extension of the output.
const EXTENSION: &str = "ansi";

/// The extension's code, which its row of the table of code of `flm-extensions` holds: the
/// function that declares its packages, and the extension's name and version.
pub static FLM_EXTENSION: flm_ext::guest::CompiledExtension =
    flm_ext::guest::CompiledExtension::new(extension, "profile-tty", env!("CARGO_PKG_VERSION"));

fn extension(ext: &mut Extension) {
    // A terminal shows box-drawing characters, so the workflow frames the tables and draws
    // the rules under the headings with them; `-O text.table_frame=none` and `-O
    // text.heading_rules=ascii` give the plain look back. The text backend's publish stage
    // keeps every image's content (the strategy `content`), a vector image converted to PNG,
    // for the rule `tty.graphics.image`, which draws it; `-O graphics.strategy.text=reference`
    // leaves the images' labels alone.
    let workflow = json!({
        "extends": "text",
        "outputs": [OUTPUT],
        "settings": {
            "text": { "table_frame": "box", "heading_rules": "unicode" },
            "graphics": {
                "strategy": { "text": "content" },
                "convert": { "text": [{ "from": ["svg", "pdf"], "to": "png" }] },
            },
        },
        "docs": "The document as text for a terminal: the text output with escape sequences for bold, italic, \
                 underline, colors and hyperlinks, its tables framed and its headings ruled with box-drawing \
                 characters, and its images drawn with half-block characters.",
    });
    ext.package(
        "_",
        Package::new().flm(json!({ "workflows": { PROFILE: workflow } })).feature(
            PROFILE,
            Feature::new()
                .title("Terminal text output")
                .docs(
                    "Makes the output `tty`: the printed text of the output `text`, with the escape sequences that \
                     show its bold, italic and underlined runs, its colors and its links to addresses outside the \
                     document.",
                )
                .settings(settings())
                .register(register),
        ),
    );
}

/// The feature's settings, declared as data: the option type [`RENDER`], set on a render
/// request, with its keys [`COLORS`], [`CELL_WIDTH`] and [`CELL_HEIGHT`].
fn settings() -> Value {
    json!({
        "options": {
            RENDER: {
                "bind": "render",
                "docs": "How the output `tty` is painted.",
                "schema": {
                    "type": "object",
                    "properties": {
                        COLORS: {
                            "type": "string",
                            "enum": COLORS_VALUES,
                            "default": "standard",
                            "description": "The colors written: `none` writes none (bold, italics, underlining and \
                                            hyperlinks remain), `standard` the sixteen standard colors of the \
                                            terminal, whose values its theme chooses, and `rich` also writes a \
                                            color given by its red, green and blue values as it is, for a \
                                            terminal that shows 24-bit colors.",
                        },
                        CELL_WIDTH: {
                            "type": "number",
                            "default": 6,
                            "description": "The width of a terminal cell in points, a positive number: an image \
                                            drawn with half-block characters takes as many columns as cells of \
                                            this width cover its shown width. The default, 6, is the width the \
                                            text output gives a column.",
                        },
                        CELL_HEIGHT: {
                            "type": "number",
                            "default": 12,
                            "description": "The height of a terminal cell in points, a positive number: an image \
                                            drawn with half-block characters takes as many lines as cells of this \
                                            height cover its shown height. The default, 12, is twice the default \
                                            width, as a terminal line is about twice as tall as a column is wide.",
                        },
                    },
                },
            },
        },
    })
}

fn register(b: &mut FeatureBuilder, _settings: &Value) -> Result<(), ExtError> {
    b.table_named(
        DOCUMENT,
        StandardTable::ArtifactSet,
        "The printed text with its terminal escape sequences, as a one-artifact set.",
    )?;
    b.output(
        OutputDef::new(OUTPUT, DOCUMENT, DOCUMENT, MEDIA_TYPE, EXTENSION)
            .docs("The document as text for a terminal: the text output with its styles shown by escape sequences."),
    )?;
    b.stage(Arc::new(Paint))?;
    // The rule lowers the image nodes the graphics feature's rule stages for the text
    // backend; only the workflow `tty` selects it.
    for element in [pres::IMAGE, pres::BLOCK, pres::VERBATIM] {
        b.peer_element(element, "A presentation element of the text backend that an image is lowered to or from.")?;
    }
    b.peer_backend("text", "The text backend, whose output the output `tty` paints.")?;
    b.lowering_rule(
        LoweringRule::new(
            image_rule::RULE,
            pres::IMAGE,
            LoweringDecl::new().reads(image_rule::CONTENT).option(RENDER).produces(pres::BLOCK).produces(pres::VERBATIM),
            Arc::new(image_rule::ImageRule),
            DefaultMark::Never,
        )?
        .docs(
            "Draws an image shown as a block with half-block characters, under its label, from the content the \
             text backend's publish stage keeps; any other image becomes its label.",
        )?,
    )?;
    b.workflow_lowering(PROFILE, &json!({ "text": { "prefer": [image_rule::RULE] } }))
}

/// The stage `tty.paint`: the printed text of `flm.output.text` painted, written to
/// `tty.document`.
struct Paint;

impl Stage for Paint {
    fn decl(&self) -> StageDecl {
        StageDecl::new("tty.paint", Effect::Compute, Scope::Document)
            .writes(DOCUMENT)
            .docs(
                "writes the printed text of the output `text` with the escape sequences that show its style marks \
                 and the colors of the images drawn in it",
            )
    }

    fn needs(&self) -> Needs {
        Needs::new().reads(TEXT_OUTPUT).reads(drawn::LOWERED).reads(image_rule::CONTENT).option(RENDER)
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        // Without the table, the text backend made no text, and reported why.
        let projection = match cx.get(TEXT_OUTPUT)? {
            None => return Ok(()),
            Some(TableValue::Projection(projection)) => projection,
            Some(_) => return Err(not_the_text_output()),
        };
        let styled = paint::StyledText::from_projection(&projection).ok_or_else(not_the_text_output)?;
        let options = cx.option(RENDER)?;
        let colors = colors(&options, out)?;
        // The rule `tty.graphics.image` reads the cell size, and applies the default where a
        // value is invalid; the stage, which runs once per render, reports it.
        for (key, given) in cell_size(&options).1 {
            out.diags(Diagnostic::error(
                INVALID_CELL_SIZE,
                format!("the setting `{PROFILE}.{key}` must be a positive number, not `{given}`"),
            ))?;
        }
        // The colors of the drawn images, which the printed text does not carry.
        let cells = match colors {
            paint::Colors::None => Vec::new(),
            paint::Colors::Standard | paint::Colors::Rich => drawn::cells(cx, &projection, &styled)?,
        };
        out.table(DOCUMENT)?.push(artifact(paint::paint(&styled, &cells, colors)))
    }
}

/// The value of the setting `tty.colors` in `options`, the value of [`RENDER`] for this render: the default when it is not set;
/// for a value that is none of the three, [`Colors::None`](paint::Colors::None), after
/// reporting it.
fn colors(options: &Value, out: &mut StageOut) -> Result<paint::Colors, ExtError> {
    let given = match options.get(COLORS) {
        None | Some(Value::Null) => return Ok(paint::Colors::default()),
        Some(given) => given,
    };
    if let Some(colors) = given.as_str().and_then(paint::Colors::named) {
        return Ok(colors);
    }
    let given = given.as_str().map_or_else(|| given.to_string(), str::to_string);
    let [none, standard, rich] = COLORS_VALUES;
    out.diags(Diagnostic::error(
        INVALID_COLORS,
        format!("the setting `{PROFILE}.{COLORS}` must be `{none}`, `{standard}` or `{rich}`, not `{given}`"),
    ))?;
    Ok(paint::Colors::None)
}

/// The cell size that `options`, the value of [`RENDER`] for a render, gives: its keys
/// [`CELL_WIDTH`] and [`CELL_HEIGHT`], each where it is a positive number, else the default;
/// and each key whose value is not a positive number, with that value's text.
fn cell_size(options: &Value) -> (graphics::CellSize, Vec<(&'static str, String)>) {
    let mut cell = graphics::CellSize::default();
    let mut invalid = Vec::new();
    for (key, size) in [(CELL_WIDTH, &mut cell.width_pt), (CELL_HEIGHT, &mut cell.height_pt)] {
        match options.get(key) {
            None | Some(Value::Null) => {}
            Some(given) => match given.as_f64().filter(|value| value.is_finite() && *value > 0.0) {
                Some(value) => *size = value,
                None => invalid.push((key, given.as_str().map_or_else(|| given.to_string(), str::to_string))),
            },
        }
    }
    (cell, invalid)
}

/// The error for a table `flm.output.text` that is not read as the printed text and its
/// style marks.
fn not_the_text_output() -> ExtError {
    ExtError::Value { detail: format!("the table `{TEXT_OUTPUT}` must be read as a printed text and its style marks") }
}

/// The entry artifact of the output `tty`, holding `painted`.
fn artifact(painted: String) -> Value {
    let mut data = Map::new();
    data.insert("bytes".to_string(), json!({ "text": painted }));
    let mut artifact = Map::new();
    artifact.insert("id".to_string(), json!({ "owner": OUTPUT, "name": "document" }));
    artifact.insert("media_type".to_string(), Value::from(MEDIA_TYPE));
    artifact.insert("entry".to_string(), Value::Bool(true));
    artifact.insert("extension".to_string(), Value::from(EXTENSION));
    artifact.insert("data".to_string(), Value::Object(data));
    Value::Object(artifact)
}
