//! The host's external converter, which runs ImageMagick's program `magick` to turn an
//! image of any format into any other.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use cachette::{BlobName, BlobSelection, Cost, PutOptions, Record, ValueSort, ValuesSession};
use flm_core::progress::Scope;
use flm_core::stdlib::features::graphics::{
    ConversionOptions, ConversionRequest, GraphicsConvertError, GraphicsConverter, ImageFormat,
};
use sha2::{Digest, Sha256};

use crate::cache::{Instance, ServiceCaches, ValuesDeclaration, ValuesEngine};

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
/// variable `MAGICK` where that variable is set. The value is a name with no folder or a
/// path, written as the host writes paths ([`crate::files`], `/` between names), and
/// converted to the system's form when the program is started
/// ([`to_system`](crate::files::system::to_system)): a name with no folder is looked for
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
/// **What it keeps.** The converter keeps the result of each conversion in the cache
/// [`MagickConverter::CACHE`], which it opens through the handle of the caches it is made
/// with ([`ServiceCaches`]); it keeps nothing of its own. A value of the cache is the
/// converted image, under a key that names the program, both formats, the options and the
/// content (see [`MagickConverter::CACHE`]), so that a result found is the result that the
/// program would give again. The program is identified by the first line that `PROGRAM
/// -version` writes on its standard output, read once, at the converter's first conversion
/// with a cache. When that line cannot be read, the converter makes every conversion
/// without the cache. Over a manager that keeps nothing, the converter never runs
/// `-version`, and runs the program for every conversion. A conversion that failed is not
/// stored: asking again runs the program again. The same conversion asked twice at once
/// runs the program once, and both get its result.
///
/// **Progress.** The converter declares no row of its own on the progress sheet
/// ([`flm_core::progress`]): the row that the graphics stage declares for each conversion
/// stands for it. A conversion under way is not interrupted when the build is cancelled.
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::features::graphics::{GraphicsConverter, ImageFormat};
/// use flm_host::cache::ServiceCaches;
/// use flm_host::graphics::MagickConverter;
///
/// let magick = MagickConverter::new("magick", ServiceCaches::default());
/// assert_eq!(magick.name(), "magick");
/// assert!(magick.offers(ImageFormat::Pdf, ImageFormat::Png));
/// assert!(magick.offers(ImageFormat::Svg, ImageFormat::Pdf));
/// ```
#[derive(Debug)]
pub struct MagickConverter {
    program: String,
    caches: ServiceCaches,
    /// The identity of the program, read at the first conversion with a cache; `None` when
    /// it could not be read.
    identity: OnceLock<Option<String>>,
    /// The conversions under way, under the digest of their conversion: each entry is
    /// locked while the program runs, and holds its result for the callers that waited.
    running: Mutex<HashMap<String, Arc<Running>>>,
}

/// A conversion under way: empty until the program has succeeded, and locked while it runs.
type Running = Mutex<Option<Vec<u8>>>;

/// The name of the one blob of a value of [`MagickConverter::CACHE`]: the converted image.
const IMAGE_BLOB: &str = "image";

impl MagickConverter {
    /// The program run where the embedding program gives no other: `magick`, which the
    /// operating system looks for in the folders it searches for programs.
    pub const DEFAULT_PROGRAM: &'static str = "magick";

    /// The cache of the converted images, `flm.graphics-files.magick.converted`: one
    /// instance shared by every document, whose values are kept by the engine `Files`,
    /// stable, and removed when unused for 90 days.
    ///
    /// A value has the record `null` and one blob, `image`, the converted image. Its cost is
    /// the time that its conversion took; the declared default cost, for a value stored
    /// without one, is 500 ms. Its key is the SHA-256 digest, in lowercase
    /// hexadecimal digits, of the identity of the program, the names of both formats, each
    /// option with whether it is given, and the content with its length.
    pub const CACHE: ValuesDeclaration = ValuesDeclaration {
        name: Cow::Borrowed("flm.graphics-files.magick.converted"),
        version: 1,
        engine: ValuesEngine::Files,
        blobs: true,
        sort: ValueSort::Stable { idle_lifetime: Duration::from_secs(90 * 24 * 60 * 60) },
        default_cost: Cost::new(500).unwrap(),
    };

    /// Returns the converter that runs `program`: a name that the operating system looks
    /// for in the folders it searches for programs, or a path, converted to the system's
    /// form when the program is started. The program is not looked for here: the operating
    /// system looks for it each time a conversion starts it. The converter keeps its results
    /// in the cache [`MagickConverter::CACHE`], which it opens through `caches`.
    pub fn new(program: impl Into<String>, caches: ServiceCaches) -> MagickConverter {
        MagickConverter {
            program: program.into(),
            caches,
            identity: OnceLock::new(),
            running: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the cache for a conversion, or `None` when the conversion is made without
    /// it: a session without a cache, or a program that could not be identified. The
    /// identity of the program is read at the first conversion that has a cache.
    fn cache(&self) -> Option<Cached<'_>> {
        if matches!(self.identity.get(), Some(None)) {
            return None;
        }
        let session = self.caches.open_values(&MagickConverter::CACHE, &Instance::Shared(serde_json::Value::Null)).ok()?;
        if !session.has_cache() {
            return None;
        }
        let identity = self.identity.get_or_init(|| self.read_identity()).as_deref()?;
        Some(Cached { session, identity })
    }

    /// Returns the first line that `PROGRAM -version` writes on its standard output, or
    /// `None` when the program cannot be started, ends with a status other than success, or
    /// writes no line of text.
    fn read_identity(&self) -> Option<String> {
        let system = crate::files::system::to_system(&self.program).ok()?;
        let output = Command::new(system)
            .arg("-version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let first = output.stdout.split(|byte| *byte == b'\n').next()?;
        let line = std::str::from_utf8(first).ok()?.trim_end();
        (!line.is_empty()).then(|| line.to_string())
    }

    /// Returns the key of `request` in the cache, for the program identified by `identity`:
    /// the SHA-256 digest, in lowercase hexadecimal digits, of the identity, the names of
    /// both formats, each option with whether it is given, and the content. Each text and
    /// the content are preceded by their length, and each option by one byte saying whether
    /// it is given, so that two different conversions never give the same sequence of bytes.
    fn key(identity: &str, request: &ConversionRequest<'_>) -> String {
        let ConversionOptions { dpi, transparent_bg } = *request.options;
        let mut digest = Sha256::new();
        for bytes in [identity.as_bytes(), request.from.name().as_bytes(), request.to.name().as_bytes()] {
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        digest.update([u8::from(dpi.is_some())]);
        digest.update(dpi.unwrap_or(0).to_le_bytes());
        digest.update([u8::from(transparent_bg.is_some()), u8::from(transparent_bg == Some(true))]);
        digest.update((request.content.len() as u64).to_le_bytes());
        digest.update(request.content);
        digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
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
        // Only the first page or frame is converted: the result is read from the standard
        // output, which holds one file, and the first page is the one the inspector measures.
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
        let program = &self.program;
        let could_not_start = |detail: &dyn std::fmt::Display| {
            GraphicsConvertError::new(format!("the program `{program}` could not be started: {detail}"))
        };
        let system = crate::files::system::to_system(program).map_err(|reason| could_not_start(&reason))?;
        let mut child = Command::new(system)
            .args(MagickConverter::arguments(request))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| could_not_start(&error))?;
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

    fn convert(&self, request: &ConversionRequest<'_>, _progress: &Scope) -> Result<Vec<u8>, GraphicsConvertError> {
        let cache = self.cache();
        // Without the cache, the program is the same for the converter's whole existence, so
        // a key with no identity tells the conversions under way apart.
        let key = MagickConverter::key(cache.as_ref().map_or("", |cache| cache.identity), request);
        // One entry per conversion under way, locked while the program runs, so that the
        // same conversion asked twice at once runs the program once.
        let entry = Arc::clone(lock(&self.running).entry(key.clone()).or_default());
        let mut result = lock(&entry);
        let converted = if let Some(converted) = result.clone() {
            Ok(converted)
        } else if let Some(found) = cache.as_ref().and_then(|cache| cache.get(&key)) {
            Ok(found)
        } else {
            let started = Instant::now();
            let converted = self.run(request);
            if let (Ok(converted), Some(cache)) = (&converted, &cache) {
                cache.put(&key, converted, started.elapsed());
            }
            converted
        };
        if let Ok(converted) = &converted {
            *result = Some(converted.clone());
        }
        let mut running = lock(&self.running);
        if running.get(&key).is_some_and(|under_way| Arc::ptr_eq(under_way, &entry)) {
            running.remove(&key);
        }
        converted
    }
}

/// The cache of a conversion: the session, and the identity of the program.
struct Cached<'a> {
    session: Arc<ValuesSession>,
    identity: &'a str,
}

impl Cached<'_> {
    /// Returns the image stored under `key`, if there is one.
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        let name = BlobName::new(IMAGE_BLOB).ok()?;
        let found = self.session.get_with_blobs(key, BlobSelection::Named(std::slice::from_ref(&name)))?;
        found.bytes.into_iter().find(|(blob, _)| *blob == name).map(|(_, bytes)| bytes)
    }

    /// Stores the image `converted` under `key`, with the time `took` that its conversion
    /// took as its cost.
    fn put(&self, key: &str, converted: &[u8], took: Duration) {
        let Ok(name) = BlobName::new(IMAGE_BLOB) else { return };
        let millis = u64::try_from(took.as_millis()).unwrap_or(u64::MAX);
        let options = PutOptions { cost: Cost::new(millis.max(1)), lifetime: None };
        self.session.put(key, Record::null(), vec![(name, converted.to_vec())], options);
    }
}

/// Locks `mutex`, also after a panic of another holder: the data it guards stays valid.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
