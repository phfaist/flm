//! The extension `math-techxt`: a math processor that renders each formula as
//! Unicode text with techxt, written in Rust against the guest client
//! `flm_ext::guest`.
//!
//! The extension holds one package, the package `_` (the package a `use`
//! entry `math-techxt` gives, and the one a document's `math: techxt`
//! chooses), which holds one feature, `math-techxt`.
//!
//! # What a formula may hold
//!
//! The feature declares in math mode, by name alone, the macros and
//! environments of techxt's notation that a formula may use: those of its
//! categories `mathcore`, `mathenvs`, `fontstyles`, `symbols_extra` and
//! `accents` visible in math mode, and the entries of its category `base` that
//! mathematics uses (the line break `\\`, the escaped characters `\{`, `\}`,
//! `\$`, `\&`, `\#`, `\_`, `\%`, the spacing commands, the decorations, the
//! phantoms, `\smash` and `\boxed`), except the names that the `math` feature
//! gives a meaning of its own (its text commands, `\tag`, `\nonumber`,
//! `\label` and `\ensuremath`), and except the names that techxt's whole
//! library gives to a category of its document library (`symbols_extra` also
//! holds `\eqref`, `\ref`, `\cite`, … as placeholders).
//! A command of techxt's document library, such as `\eqref`, `\ref` or
//! `\cite`, is therefore an unknown command inside a formula. The macros are
//! elements `math-techxt.macro`, the environments elements
//! `math-techxt.environment`, each carrying its category as its data.
//!
//! The line break `\\` ends a row of a matrix, `cases`, `aligned`, `split` or
//! `array` written in a math group. Directly in the body of a display-math
//! environment (`align`, `gather`, …) it is the `math` feature's, and ends the
//! equation line.
//!
//! # The converter
//!
//! The feature builds one converter when it registers, from techxt's seven
//! notation categories (`symbols_extra`, `base`, `accents`, `fontstyles`,
//! `mathcore`, `mathenvs`, `subsuperscripts`) and nothing of its document
//! library, in techxt's `Fancy` mode (Unicode symbols, scripts and alphabets
//! with TeX's spacing) and without a wrap width. The parse language is techy's
//! plain `Latexlike`, under which a defining command defines nothing; most
//! defining commands are not declared inside a formula at all (see
//! [What a formula may hold](#what-a-formula-may-hold)).
//!
//! # The rendering and the check
//!
//! The stage `math-techxt.check` is the feature's check of the formulas: it
//! provides `flm.math.content` and `math-techxt.check`, a member of
//! `flm.validated`, so it runs on every workflow that validates a document. It
//! reports `math-techxt.empty-line` (an Error): an equation line holds no
//! mathematics.
//!
//! The stage `math-techxt.render` renders every formula of a tree, nested ones
//! included, and writes one row per math group or display-math environment
//! into the table `math-techxt.rendered`, keyed by the node's address: a math
//! group's row holds its formula, an environment's row one formula per
//! equation line, each line rendered alone. It is in a plan only where a
//! selected rule reads that table, which the rules of the text and HTML
//! outputs do; what it reports is therefore reported on those runs only, not
//! on the LaTeX outputs nor by `flm check`. It reports:
//!
//! - `math-techxt.conversion`: what techxt reports about a formula, at
//!   techxt's severity, techxt's message in the message and its identifier in
//!   the data (`condition`);
//! - `math-techxt.conversion-failed` (an Error): techxt cannot read a formula;
//! - `math-techxt.private-use-character` (an Error): a formula holds a
//!   private-use character `U+E000` to `U+F8FF` outside its text commands;
//! - `math-techxt.too-many-text-commands` (an Error): a formula holds more
//!   than 6400 text commands.
//!
//! A formula that techxt cannot read, or that holds a private-use character or
//! too many text commands, gets no row.
//!
//! # The element `math-techxt.rendered-as-text`
//!
//! A formula rendered as Unicode text, carried from the rule of the formula to
//! the rule that writes it on the backend at hand. Its payload, written in
//! JSON, is of one of three kinds, named under `kind`:
//!
//! - `formula`, a math group: `display` (`inline` or `display`) and `lines`,
//!   the rendered lines in order, each a list of segments: a text, printed as
//!   given, or the index `i` of a text command of the formula (a *hole*),
//!   whose content, the command lowered through the rules, is in the content
//!   slot `hole-<i>`;
//! - `environment`, a display-math environment: `env`, the environment's
//!   name, and no lines; its body holds one element of the kind `line` per
//!   equation line;
//! - `line`, an equation line: its `lines`, its slots `hole-<i>`, its `anchor`
//!   where it has one, and a slot `tag` holding what the line shows at its tag
//!   where it shows something.
//!
//! A text command is written into the TeX techxt reads as `\text{…}` around
//! one private-use character, `U+E000` plus its index, which techxt carries
//! through unchanged; the rendered text is split there.
//!
//! # The settings
//!
//! `font_url` (a non-empty text): the address of the font file that the HTML
//! stylesheet loads for the formulas; by default JuliaMono 0.63.2 on cdnjs.
//!
//! ```yaml
//! ---
//! flm:
//!   features:
//!     math-techxt:
//!       font_url: fonts/JuliaMono-Regular.woff2
//! ---
//! ```
//!
//! # Lowering
//!
//! On the backends `text` and `html`, a math group lowers to the element
//! `math-techxt.rendered-as-text`, its text commands lowered through the
//! rules into its slots; a display-math environment lowers to one such
//! element whose body holds one element per equation line, each with the
//! line's anchor and, in its slot `tag`, the line's number or tag. Another
//! round of lowering writes the element in the backend's own elements:
//!
//! - on `text`, an inline formula as text within its line; a displayed
//!   formula on lines of its own, indented by two columns, its spaces kept so
//!   that a drawing over several lines stays aligned; an equation line's tag
//!   after its last printed line;
//! - on `html`, `<span>` elements of the extension's own classes
//!   (`math-techxt`, `math-techxt-display`, `math-techxt-line`,
//!   `math-techxt-text`, `math-techxt-tag`), each equation line carrying its
//!   anchor as its `id`, the outermost span of a formula marked as a formula
//!   for the page's stylesheets and scripts (the class `flm-math-formula`,
//!   `data-flm-display`, and `data-flm-env` for an environment), with the
//!   stylesheet `math-techxt.style`: the font
//!   JuliaMono loaded from `font_url`, the white space kept, a displayed
//!   formula as a block, the tag at the right edge.
//!
//! A text command's content is shown as the backend shows that content
//! anywhere: a reference as its number, linked on `html`. A formula without a
//! row (the stage reported why) lowers to `flm.pres.math-passthrough` over its
//! TeX, each text command lowered through the rules.
//!
//! A macro or an environment of techxt's reached outside a formula lowers to
//! its source, on every backend.

mod check;
mod feature;
mod render;
mod rules;
mod settings;
mod vocabulary;

use flm_ext::guest::{Extension, Feature, Package};

/// The extension's code, which its row of the table of code of `flm-extensions` holds: the
/// function that declares its packages, and the extension's name and version.
pub static FLM_EXTENSION: flm_ext::guest::CompiledExtension =
    flm_ext::guest::CompiledExtension::new(define, "math-techxt", env!("CARGO_PKG_VERSION"));

/// The feature's name, `math-techxt`: the name under which a document gives the
/// feature's settings (in its front matter under `flm`, `features`,
/// `math-techxt`), and the prefix of every name the feature declares.
pub const FEATURE: &str = "math-techxt";

/// The element of every macro of techxt's that the feature declares:
/// `math-techxt.macro`.
pub const MACRO: &str = "math-techxt.macro";

/// The element of every environment of techxt's that the feature declares:
/// `math-techxt.environment`.
pub const ENVIRONMENT: &str = "math-techxt.environment";

/// The presentation element of a formula rendered as Unicode text:
/// `math-techxt.rendered-as-text`.
pub const RENDERED_AS_TEXT: &str = "math-techxt.rendered-as-text";

/// Declares the extension's one package, `_`: it holds the feature
/// `math-techxt`.
fn define(ext: &mut Extension) {
    ext.package(
        "_",
        Package::new().feature(
            FEATURE,
            Feature::new()
                .title("Formulas rendered as Unicode text")
                .docs(
                    "Mathematics rendered as Unicode text by techxt: the commands of techxt's notation \
                     inside formulas (Greek letters, operators, fractions, roots, matrices, the math \
                     alphabets, accents and decorations), each formula written as Unicode symbols, scripts \
                     and alphabets with TeX's spacing, and the text inside a formula lowered by FLM.",
                )
                .settings(settings::declaration())
                .read(settings::read)
                .register(feature::register),
        ),
    );
}
