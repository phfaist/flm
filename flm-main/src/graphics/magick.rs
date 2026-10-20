//! The host's external converter, which runs ImageMagick's program `magick` to turn an
//! image of any format into any other.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};

use flm_core::stdcore::features::graphics::{
    ConversionOptions, ConversionRequest, GraphicsConvertError, GraphicsConverter, ImageFormat,
};

/// The converter's name, `magick`, by which the `via` list of a conversion rule refers to
/// it.
pub const MAGICK_CONVERTER: &str = "magick";

/// The external converter, named [`MAGICK_CONVERTER`]: it turns an image of one format
/// into another by running ImageMagick's program `magick`.
///
/// **Which program.** The converter runs the program it is made with
/// ([`MagickConverter::new`]), which the program embedding the host gives
/// ([`Request::magick_program`](crate::session::Request::magick_program)); the program is
/// never a setting of a document. It is [`MagickConverter::DEFAULT_PROGRAM`] unless the
/// embedding program gives another; the program `flm` takes it from the environment
/// variable `MAGICK` where that variable is set. The value is given to the operating
/// system as written: a name with no folder is looked for, when the program is started,
/// in the folders that the system searches for programs; a path is taken as any path
/// typed at a command line, a relative one from the current folder. The converter does
/// not check that the program exists when it is made. A program that cannot be started is
/// an error of the conversion that needs it, so a document with no image to convert never
/// needs the program.
///
/// **What it offers.** Every pair of formats, since ImageMagick reads and writes all six.
/// Contributed after the converter that works in the program's own code, it is therefore
/// the one used for every pair that the first converter does not offer: a PDF or SVG
/// drawing made into a PNG image for a web page, an SVG drawing made into a PDF file for
/// LaTeX.
///
/// **How it runs the program.** The converter writes the image's content to the program's
/// standard input and reads the converted image from its standard output; no file is
/// written. It gives the program both formats by name (`magick pdf:-[0] png:-`), and the
/// program converts only the first page or frame of the content: the page that the
/// inspector measured, and one image where a PDF file has several pages or a GIF image
/// several frames. Of the conversion rule's options ([`ConversionOptions`]), `dpi` is the
/// resolution at which a drawing is turned into pixels (`-density`); `transparent_bg`
/// keeps the drawing's background transparent where `true` (`-background none`) and fills
/// it in white where `false`; where an option is not given, the program's own default
/// applies. The conversion fails where the program cannot be started or ends with a status
/// other than success; the error then holds what the program wrote on its standard error
/// stream.
///
/// **What it keeps.** The converter keeps the result of each conversion for as long as it
/// exists (in the program `flm`, as long as the registry, which `flm watch` keeps across
/// rebuilds): the same content converted to the same format with the same options runs
/// the program once. Since the program is the same for the converter's whole existence, a
/// kept result is the same as a result computed again. A conversion that failed is not
/// kept: asking again runs the program again.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::graphics::{GraphicsConverter, ImageFormat};
/// use flm_main::graphics::MagickConverter;
///
/// let magick = MagickConverter::new("magick");
/// assert_eq!(magick.name(), "magick");
/// assert!(magick.offers(ImageFormat::Pdf, ImageFormat::Png));
/// assert!(magick.offers(ImageFormat::Svg, ImageFormat::Pdf));
/// ```
#[derive(Debug)]
pub struct MagickConverter {
    program: OsString,
    kept: Mutex<HashMap<Conversion, Arc<Kept>>>,
}

/// The result kept for one conversion: empty until the program has succeeded once, and
/// locked while the program runs.
type Kept = Mutex<Option<Vec<u8>>>;

/// One conversion, the key of a kept result: the content itself (compared whole, so that
/// two contents never share a result), its format, the target format and the options.
// See decision rationale at [§flm-dd-dr:host-magick-results]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Conversion {
    content: Vec<u8>,
    from: ImageFormat,
    to: ImageFormat,
    options: ConversionOptions,
}

impl MagickConverter {
    /// The program run where the embedding program gives no other: `magick`, which the
    /// operating system looks for in the folders it searches for programs.
    pub const DEFAULT_PROGRAM: &'static str = "magick";

    /// Returns the converter that runs `program`: a name that the operating system looks
    /// for in the folders it searches for programs, or a path, given to the operating
    /// system as written. The program is not looked for here: the operating system looks
    /// for it each time a conversion starts it.
    pub fn new(program: impl Into<OsString>) -> MagickConverter {
        MagickConverter { program: program.into(), kept: Mutex::new(HashMap::new()) }
    }

    /// Returns the arguments given to the program for `request`: the settings that apply
    /// to the content read, the content read from the standard input (its first page or
    /// frame), the operations applied to it, and the result written to the standard
    /// output.
    fn arguments(request: &ConversionRequest<'_>) -> Vec<String> {
        let mut arguments = Vec::new();
        if request.options.transparent_bg == Some(true) {
            arguments.extend(["-background", "none"].map(String::from));
        }
        if let Some(dpi) = request.options.dpi {
            arguments.extend([String::from("-density"), dpi.to_string()]);
        }
        // See decision rationale at [§flm-dd-dr:host-magick-first-frame]
        arguments.push(format!("{}:-[0]", request.from.name()));
        if request.options.transparent_bg == Some(false) {
            arguments.extend(["-background", "white", "-alpha", "remove", "-alpha", "off"].map(String::from));
        }
        arguments.push(format!("{}:-", request.to.name()));
        arguments
    }

    /// Runs the program over `request`, with its content on the standard input, and
    /// returns the result read from the standard output.
    fn run(&self, request: &ConversionRequest<'_>) -> Result<Vec<u8>, GraphicsConvertError> {
        let program = self.program.to_string_lossy();
        let mut child = Command::new(&self.program)
            .args(MagickConverter::arguments(request))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| GraphicsConvertError::new(format!("the program `{program}` could not be started: {error}")))?;
        let mut stdin = child.stdin.take().expect("the standard input is piped");
        // The content is written from a thread of its own while the output is read, so
        // that neither side waits for the other with a full pipe.
        let (written, output) = std::thread::scope(|scope| {
            let writer = scope.spawn(move || stdin.write_all(request.content));
            let output = child.wait_with_output();
            (writer.join().expect("writing to the program does not panic"), output)
        });
        let output = output.map_err(|error| {
            GraphicsConvertError::new(format!("the output of the program `{program}` could not be read: {error}"))
        })?;
        if !output.status.success() {
            let said = String::from_utf8_lossy(&output.stderr);
            let said = said.trim();
            let mut detail = match output.status.code() {
                Some(code) => format!("the program `{program}` failed with the exit status {code}"),
                None => format!("the program `{program}` failed ({})", output.status),
            };
            if !said.is_empty() {
                detail.push_str(": ");
                detail.push_str(said);
            }
            return Err(GraphicsConvertError::new(detail));
        }
        written.map_err(|error| {
            GraphicsConvertError::new(format!("the image could not be handed to the program `{program}`: {error}"))
        })?;
        Ok(output.stdout)
    }
}

impl GraphicsConverter for MagickConverter {
    fn name(&self) -> &str {
        MAGICK_CONVERTER
    }

    fn offers(&self, _from: ImageFormat, _to: ImageFormat) -> bool {
        true
    }

    fn convert(&self, request: &ConversionRequest<'_>) -> Result<Vec<u8>, GraphicsConvertError> {
        let conversion = Conversion {
            content: request.content.to_vec(),
            from: request.from,
            to: request.to,
            options: *request.options,
        };
        // One entry per conversion, locked while the program runs, so that the same
        // conversion asked twice at once runs the program once.
        let entry = Arc::clone(self.kept.lock().unwrap_or_else(PoisonError::into_inner).entry(conversion).or_default());
        let mut result = entry.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(converted) = result.as_ref() {
            return Ok(converted.clone());
        }
        let converted = self.run(request)?;
        *result = Some(converted.clone());
        Ok(converted)
    }
}
