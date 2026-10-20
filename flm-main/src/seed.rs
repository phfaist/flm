//! Splitting a file into its front matter and its content, and reading the front matter.
//!
//! The front matter is the block of YAML between a first line `---` and the next `---` line
//! of a file: its root keys are the author's metadata (title, authors, date), and the
//! reserved key `flm` holds the document's settings (`use`, `initial_use`, `features`,
//! `parsing`, `feature_order`, `layout`, `default_profile`, `math`). A file that does not
//! begin with a `---` line has no front matter.
//! The root is a map; every root key other than `flm` is metadata, which the library's `meta`
//! feature reads ([`DocMeta`]). The module is named after [`Seed`], the value a front matter
//! is read into.
//!
//! | Key under `flm` | What it holds |
//! |---|---|
//! | `use` | the packages the document uses: one entry, or a list of entries (`LOCATOR` or `LOCATOR#PACKAGE`; [`crate::packages`]) |
//! | `initial_use` | the packages used before those of `use`, in place of the host's initial list: one package's name, or a list; the list is used exactly as written ([`read_initial_use`], [`crate::packages`]) |
//! | `features` | a map from a feature's name to its settings, or to `false` to switch the feature off ([`crate::settings`]) |
//! | `parsing` | how the content is read: `comments` (whether comments are recognized), `comment_prefix` (what starts a comment) and `dollar_math` (whether `$` delimits mathematics) |
//! | `feature_order` | the order of features: one chain `NAME < NAME < … < NAME` of features' names, or a list of chains, each placing every feature it names after the one before it |
//! | `layout` | how the content is read at its top level: `blocks`, `inline` or `auto` |
//! | `default_profile` | the profile to build with when the request gives none: a profile's name, which runs the workflow of that name and uses the extension `profile-NAME` when one exists ([`read_default_profile`], [`DefaultProfile`]) |
//! | `math` | the math processor, the package that processes the document's mathematics: a processor's name `NAME`, which stands for the package `math-NAME`, or `false` for no processor; without the key, the host's default processor ([`read_math`], [`MathChoice`]) |
//! | `input_as` | in a file included with `\input` only: the file opens with a heading (see [`read_input_as`]) |
//!
//! [`split`] splits the text of any file and gives the line at which the content starts, so
//! that a position in the content is counted from the top of the file. [`read_front_matter`]
//! reads the YAML into a [`Node`], a value that records where in the file each of its parts
//! is written. [`read_seed`] reads the keys of a document's front matter into a [`Seed`]:
//! the library's block step ([`flm_ext::block::read`]) reads `use`, `features`, `parsing`
//! and `feature_order`, by the one rule that reads the `flm` block of an extension written as data
//! too, and a problem it finds is placed at the key it names; [`read_initial_use`],
//! [`read_layout`], [`read_default_profile`] and [`read_math`] read the four keys only a
//! document's front matter writes. [`read_packages_seed`] reads only the keys that decide
//! the packages (`use`, `initial_use`, `math`, `default_profile`), by the same rules, and
//! leaves the rest of the front matter unread.
//! [`read_metadata`] reads the root keys into the document's metadata. [`read_included`]
//! reads the front matter of a file included with `\input`, which may set only `parsing`
//! and `input_as` under `flm`.
//!
//! The JSON Schema of a front matter ([`front_matter_schema`]) is built from the same
//! readers' parts ([`use_schema`], [`initial_use_schema`], [`math_schema`], [`features_schema`],
//! [`parsing_schema`], [`feature_order_schema`], [`input_as_schema`]); its descriptions are what an editor shows to a person writing a
//! front matter. The schemas of `use`, `initial_use` and `math` suggest values, which an
//! editor offers as completions and which do not restrict what may be written: the packages
//! and extensions that the host lists ([`crate::packages::suggestions`]) and, for `math`,
//! the processors among them.
//!
//! This module is given text and reads no file.

use std::fmt;

use flm_core::core::registry::PackageFactory;
use flm_core::lang::FlmParsing;
use flm_core::model::doc::FragmentLayout;
use flm_core::stdcore::features::meta::{DocMeta, MetaPath, MetaPlace, metadata_schema};
use flm_ext::block::{self, Block, ChainBlock};
use saphyr::{LoadableYamlNode, MarkedYaml, Marker, Scalar, YamlData};
use serde_json::{Map, Number, Value};

/// A place in a file: its line and its column, both counted from 1, the column in bytes. It
/// is displayed as `line:column`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Place {
    /// The line, counted from 1 at the top of the file.
    pub line: u32,
    /// The column, counted from 1 at the start of the line, in bytes.
    pub column: u32,
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// A problem found in a front matter (or in an extension's manifest, read the same way): its
/// place and its message. Its `Display` text is [`detail`](SeedError::detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedError {
    /// Where in the file the problem is, when it is known.
    pub place: Option<Place>,
    /// The problem, as a text for a reader.
    pub detail: String,
}

impl SeedError {
    /// Returns the problem `detail`, at `place`.
    pub fn at(place: Place, detail: impl Into<String>) -> SeedError {
        SeedError { place: Some(place), detail: detail.into() }
    }
}

impl fmt::Display for SeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for SeedError {}

/// A file's text split into its front matter and its content, as [`split`] returns it.
///
/// The four slices joined in order give back the file's text, byte for byte: `opening`, the
/// front matter's text (empty when there is none), `closing`, then `content`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split<'a> {
    /// The front matter's opening `---` line, its trailing spaces and line end included;
    /// empty when the file has no front matter.
    pub opening: &'a str,
    /// The front matter's text, without the two `---` lines, if the file has one.
    pub front_matter: Option<&'a str>,
    /// The front matter's closing `---` line, its trailing spaces and line end included;
    /// empty when the file has no front matter.
    pub closing: &'a str,
    /// The line of the file at which the front matter's text starts, counted from 1: the
    /// line after the first `---` (1 when the file has no front matter).
    pub front_matter_line: u32,
    /// The content: everything after the front matter's closing line, or the whole text.
    pub content: &'a str,
    /// The line of the file at which the content starts, counted from 1.
    pub content_line: u32,
}

impl Split<'_> {
    /// Returns the byte offset in the file at which the content starts: the number of bytes
    /// of the opening line, the front matter's text and the closing line together, 0 when
    /// the file has no front matter. The library parses the content alone, so a byte offset
    /// in the parsed content plus this offset is a byte offset in the file.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_main::seed::split;
    ///
    /// let text = "---\ntitle: Notes\n---\nHello.\n";
    /// let parts = split(text).unwrap();
    /// assert_eq!(parts.content_offset(), 21);
    /// assert_eq!(&text[parts.content_offset()..], parts.content);
    /// assert_eq!(split("Hello.\n").unwrap().content_offset(), 0);
    /// ```
    pub fn content_offset(&self) -> usize {
        self.opening.len() + self.front_matter.map_or(0, str::len) + self.closing.len()
    }
}

/// Splits `text`, the text of a file, into its front matter and its content.
///
/// A front matter starts with a first line that is `---` (spaces after it allowed) and
/// ends with the next such line. A file whose first line is anything else has no front
/// matter, and its content is the whole text.
///
/// # Errors
///
/// A [`SeedError`] at line 1, column 1 when the front matter has no closing line.
///
/// # Examples
///
/// ```
/// use flm_main::seed::split;
///
/// let split = split("---\ntitle: Notes\n---\nHello.\n").unwrap();
/// assert_eq!(split.front_matter, Some("title: Notes\n"));
/// assert_eq!(split.content, "Hello.\n");
/// assert_eq!(split.content_line, 4);
/// assert_eq!(split.front_matter_line, 2);
/// ```
pub fn split(text: &str) -> Result<Split<'_>, SeedError> {
    let is_fence = |line: &str| line.trim_end_matches(['\r', '\n']).trim_end() == "---";
    let mut lines = text.split_inclusive('\n');
    match lines.next() {
        Some(first) if is_fence(first) => {}
        _ => {
            return Ok(Split {
                opening: "",
                front_matter: None,
                closing: "",
                front_matter_line: 1,
                content: text,
                content_line: 1,
            });
        }
    }
    let start = text.split_inclusive('\n').next().map_or(0, str::len);
    let mut offset = start;
    for (line_number, line) in (2..).zip(lines) {
        if is_fence(line) {
            return Ok(Split {
                opening: &text[..start],
                front_matter: Some(&text[start..offset]),
                closing: line,
                front_matter_line: 2,
                content: &text[offset + line.len()..],
                content_line: line_number + 1,
            });
        }
        offset += line.len();
    }
    Err(SeedError::at(Place { line: 1, column: 1 }, "the front matter must end with a `---` line"))
}

/// A value read from YAML ([`read_front_matter`]), with the place in the file where it is
/// written; the parts of a list or a map are nodes with places of their own.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The value.
    pub value: NodeValue,
    /// Where the value is written.
    pub place: Place,
}

/// The value of a [`Node`]: one of the shapes of plain data.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeValue {
    /// No value (`null`, `~`, or nothing).
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number.
    Number(Number),
    /// A text.
    Text(String),
    /// A list of values.
    List(Vec<Node>),
    /// A map from a key to a value, in the order written.
    Map(Vec<(String, Node)>),
}

impl Node {
    /// Returns the value as JSON data, without the places.
    pub fn to_json(&self) -> Value {
        match &self.value {
            NodeValue::Null => Value::Null,
            NodeValue::Bool(value) => Value::Bool(*value),
            NodeValue::Number(value) => Value::Number(value.clone()),
            NodeValue::Text(value) => Value::String(value.clone()),
            NodeValue::List(items) => Value::Array(items.iter().map(Node::to_json).collect()),
            NodeValue::Map(entries) => Value::Object(
                entries.iter().map(|(key, node)| (key.clone(), node.to_json())).collect::<Map<_, _>>(),
            ),
        }
    }

    /// Returns the entries of a map, in the order written, or `None` for a value of
    /// another shape.
    pub fn entries(&self) -> Option<&[(String, Node)]> {
        match &self.value {
            NodeValue::Map(entries) => Some(entries),
            _ => None,
        }
    }

    /// Returns the value under `key`, when the node is a map that holds that key.
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.entries()?.iter().find(|(name, _)| name == key).map(|(_, node)| node)
    }

    /// Returns the value at `path`, a list of keys followed through nested maps.
    pub fn at<'n>(&'n self, path: &[&str]) -> Option<&'n Node> {
        path.iter().try_fold(self, |node, key| node.get(key))
    }

    /// Returns the text, when the value is a text.
    pub fn as_text(&self) -> Option<&str> {
        match &self.value {
            NodeValue::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// Reads `text`, YAML that starts at line `first_line` of its file (the front matter's
/// text, or a whole manifest of an extension), into a [`Node`]. Empty text reads as a null value.
///
/// Each value's place is where its text begins; for a text written between quotes, it is
/// the first character after the quote. Its column counts bytes from the start of the line,
/// as every column the host reports does (the YAML reader counts characters, and this
/// function converts each column it gives). A place inside a text, such as the place of a
/// problem in the document's title, is counted from the text's place, byte by byte, which
/// is exact for a text written on one line, plain or between quotes, up to its first escape
/// sequence. It is not exact after an escape sequence in a text between double quotes
/// (`\\` is two characters in the file and one in the text) or after a doubled quote in a
/// text between single quotes, nor inside a text written over several lines or as a block
/// (`|`, `>`), whose lines YAML joins or re-indents.
///
/// # Errors
///
/// A [`SeedError`] when the text is not valid YAML, placed where the YAML reader found the
/// problem, and one for a part this function does not accept: an alias (`*name`), and a
/// number that is not finite (`.nan`, `.inf`), which JSON data cannot hold. A tag
/// (`!name`) is ignored: the value is read as if it had none. Only the first YAML document
/// of the text is read.
pub fn read_front_matter(text: &str, first_line: u32) -> Result<Node, SeedError> {
    // See decision rationale at [§flm-dd-dr:host-yaml-reader].
    let read = ReadText::new(text);
    let documents = MarkedYaml::load_from_str(text).map_err(|error| SeedError {
        place: Some(read.place(error.marker(), first_line - 1)),
        detail: format!("the text must be valid YAML: {}", error.info()),
    })?;
    match documents.first() {
        None => Ok(Node { value: NodeValue::Null, place: Place { line: first_line, column: 1 } }),
        Some(document) => node_of(document, first_line - 1, &read),
    }
}

/// A count of the YAML reader's as a line or a column. The reader counts lines from 1
/// and columns from 0; the caller adds 1 to a column.
fn narrow(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// The text that [`read_front_matter`] reads, with the byte offset of each of its
/// characters, to turn a place the YAML reader gives into a [`Place`] whose column counts
/// bytes.
struct ReadText<'t> {
    text: &'t str,
    /// The byte offset of each character of `text`, then the length of `text`.
    offsets: Vec<usize>,
}

impl<'t> ReadText<'t> {
    fn new(text: &'t str) -> ReadText<'t> {
        let offsets = text.char_indices().map(|(offset, _)| offset).chain([text.len()]).collect();
        ReadText { text, offsets }
    }

    /// The place of the reader's `marker`, its line moved down by `lines` and its column
    /// counted from 1 in bytes.
    fn place(&self, marker: &Marker, lines: u32) -> Place {
        // The reader gives the marker's position in the text and its column, both counted
        // in characters from 0: the line starts that many characters before the marker. A
        // marker outside the text, which the reader does not give, keeps its column in
        // characters.
        let column = marker
            .index()
            .checked_sub(marker.col())
            .and_then(|start| Some(self.offsets.get(marker.index())? - self.offsets.get(start)?));
        Place { line: narrow(marker.line()) + lines, column: narrow(column.unwrap_or(marker.col())) + 1 }
    }

    /// Whether the character at the reader's `marker` is a quote, `'` or `"`.
    fn quote_at(&self, marker: &Marker) -> bool {
        self.offsets.get(marker.index()).is_some_and(|&offset| self.text[offset..].starts_with(['\'', '"']))
    }
}

/// The node of a value the YAML reader gives, its lines moved down by `lines`; `read` is
/// the text read.
///
/// The place of a text written between quotes is that of its first character, after the
/// quote, where the reader gives the quote's.
fn node_of(yaml: &MarkedYaml<'_>, lines: u32, read: &ReadText<'_>) -> Result<Node, SeedError> {
    let quoted = matches!(yaml.data, YamlData::Value(Scalar::String(_))) && read.quote_at(&yaml.span.start);
    let mut place = read.place(&yaml.span.start, lines);
    if quoted {
        // The quote is one byte.
        place.column = place.column.saturating_add(1);
    }
    let value = match &yaml.data {
        YamlData::Value(scalar) => scalar_of(scalar, place)?,
        YamlData::Representation(text, _, _) => NodeValue::Text(text.to_string()),
        YamlData::Sequence(items) => {
            NodeValue::List(items.iter().map(|item| node_of(item, lines, read)).collect::<Result<_, _>>()?)
        }
        YamlData::Mapping(entries) => {
            let mut map = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                let key = match &key.data {
                    YamlData::Value(Scalar::String(text)) => text.to_string(),
                    _ => node_of(key, lines, read)?.to_json().to_string(),
                };
                map.push((key, node_of(value, lines, read)?));
            }
            NodeValue::Map(map)
        }
        // A tag is dropped, and the value read as if it had none.
        YamlData::Tagged(_, inner) => return node_of(inner, lines, read),
        YamlData::Alias(_) | YamlData::BadValue => {
            return Err(SeedError::at(place, "a value must be plain data: a text, a number, `true`, `false`, an empty value, a list or a map; \
                 a YAML alias (`*name`) is not accepted"));
        }
    };
    Ok(Node { value, place })
}

/// The value of a YAML scalar written at `place`.
fn scalar_of(scalar: &Scalar<'_>, place: Place) -> Result<NodeValue, SeedError> {
    Ok(match scalar {
        Scalar::Null => NodeValue::Null,
        Scalar::Boolean(value) => NodeValue::Bool(*value),
        Scalar::Integer(value) => NodeValue::Number((*value).into()),
        Scalar::FloatingPoint(value) => match Number::from_f64(value.into_inner()) {
            Some(number) => NodeValue::Number(number),
            None => return Err(SeedError::at(place, "a number must be finite")),
        },
        Scalar::String(text) => NodeValue::Text(text.to_string()),
    })
}

/// The value of `flm.default_profile`, as [`read_default_profile`] reads it: the name of the
/// profile to build with when the request gives none, whatever output the request gives.
/// The profile `NAME` uses the extension `profile-NAME` when one exists, after the
/// document's other packages, and runs the workflow `NAME` (see [`crate::session`], "Which
/// profile runs, and its workflow"). The front matter's profile is looked for, and its
/// workflow checked, only when it is the profile that runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultProfile {
    /// The profile's name, as written.
    pub name: String,
    /// Where the name is written.
    pub place: Place,
}

/// The prefix that a math processor's name takes to give the name of the processor's
/// package: `math-`, so that the processor `mathjax` is the package `math-mathjax`
/// ([`MathChoice::package`]).
pub const MATH_PACKAGE_PREFIX: &str = "math-";

/// A choice of math processor: the value of `flm.math`, as [`read_math`] reads it, and the
/// choice that a request gives ([`Request::default_math`](crate::session::Request::default_math),
/// [`Request::math`](crate::session::Request::math)).
///
/// A *math processor* is a package whose feature processes the mathematics of a document:
/// the feature provides the capability `flm.math.content`
/// ([`CONTENT`](flm_core::stdcore::features::math::CONTENT)) with a check stage of its own,
/// which it adds to the checks every standard workflow runs. A processor is chosen by
/// its name, `NAME`, which stands for the package `math-NAME` ([`MathChoice::package`]):
/// `mathjax` for the bundled extension `math-mathjax`, which typesets formulas with MathJax,
/// and `passthrough` for the library's package `math-passthrough`
/// ([`flm_core::math_passthrough`]), which carries each formula to the output as the author
/// wrote it. The chosen processor's package is used after the packages of the initial list
/// and before those of `use`, and is found as an entry of `use` is: among the compiled
/// packages first, then on the search path ([`crate::packages`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathChoice {
    /// The processor of this name, as written (`mathjax`), whose package is `math-NAME`.
    Processor(String),
    /// No processor: no package is added to those of the initial list and of `use`, which
    /// provide a processor when the document needs one. A processor whose package's name
    /// does not have the form `math-NAME` is used this way, listed under `use`.
    NoProcessor,
}

impl MathChoice {
    /// Returns the choice of the processor `name` (`mathjax`, `passthrough`).
    pub fn processor(name: impl Into<String>) -> MathChoice {
        MathChoice::Processor(name.into())
    }

    /// Returns the name of the package that the choice adds to the packages a document
    /// uses: `math-NAME` for the processor `NAME` ([`MATH_PACKAGE_PREFIX`]), `None` for
    /// [`NoProcessor`](MathChoice::NoProcessor).
    pub fn package(&self) -> Option<String> {
        match self {
            MathChoice::Processor(name) => Some(format!("{MATH_PACKAGE_PREFIX}{name}")),
            MathChoice::NoProcessor => None,
        }
    }
}

/// A document's front matter as read by [`read_seed`]: the author's metadata and the
/// settings under `flm`. A file without a front matter gives the default, empty value.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Seed {
    /// The root keys other than `flm`, in the order written: the metadata, which
    /// [`read_metadata`] reads.
    pub metadata: Vec<(String, Node)>,
    /// The entries of `flm.use`, each as the package catalog is asked for it (the entry's
    /// short form, [`UseEntry`](flm_ext::UseEntry)'s `Display`) with its place.
    pub uses: Vec<(String, Place)>,
    /// The entries of `flm.initial_use`, each as the package catalog is asked for it (as
    /// for `uses`, the short form of a valid entry) with its place, when the front matter
    /// writes the key ([`read_initial_use`]); `None` when it does not.
    pub initial_use: Option<Vec<(String, Place)>>,
    /// The block of `flm`, as the block step reads it: the entries of `use`, the settings of
    /// `features` and the keys of `parsing`, which the settings combine
    /// ([`add_block`](crate::settings::add_block)).
    pub block: Block,
    /// The chains of `flm.feature_order`, each with its place: the registry build takes
    /// them, each with an origin naming the file and the line.
    pub feature_order: Vec<(ChainBlock, Place)>,
    /// The node of `flm`, as written: the places of the block's keys.
    pub flm: Option<Node>,
    /// `flm.layout`, read, with its place.
    pub layout: Option<(FragmentLayout, Place)>,
    /// `flm.default_profile`, read.
    pub default_profile: Option<DefaultProfile>,
    /// `flm.math`, read ([`read_math`]), with the place of its value: the package of the
    /// processor it chooses is placed there when it cannot be found.
    pub math: Option<(MathChoice, Place)>,
}

/// The root key, `flm`, under which a front matter holds the document's settings.
pub const RESERVED_KEY: &str = "flm";

/// Reads the root of a document's front matter, as [`read_front_matter`] returns it, into a
/// [`Seed`]. A null root (an empty front matter) gives the default, empty value.
///
/// # Errors
///
/// Every problem found: a root that is not a map; a value of `flm` that is not a map; under
/// `flm`, a key other than the eight of the [module documentation](self) (`input_as` is
/// refused here: only an included file may write it, [`read_included`]; `workflows` too,
/// which only an extension writes); each problem the block step finds, at the key it names;
/// and each problem of `initial_use`, `layout`, `default_profile` and `math`.
pub fn read_seed(root: &Node) -> Result<Seed, Vec<SeedError>> {
    let mut seed = Seed::default();
    let entries = match &root.value {
        NodeValue::Null => return Ok(seed),
        NodeValue::Map(entries) => entries,
        _ => return Err(vec![SeedError::at(root.place, "the front matter must be a map from keys to values")]),
    };
    let mut errors = Vec::new();
    let mut collect = |result: Result<(), Vec<SeedError>>| {
        if let Err(found) = result {
            errors.extend(found);
        }
    };
    for (key, node) in entries {
        if key != RESERVED_KEY {
            seed.metadata.push((key.clone(), node.clone()));
            continue;
        }
        let Some(settings) = node.entries() else {
            if node.value != NodeValue::Null {
                collect(Err(vec![SeedError::at(node.place, "the value of `flm` must be a map")]));
            }
            continue;
        };
        match block::read(&node.to_json()) {
            Ok(block) => {
                seed.uses = uses_of(&block, node);
                seed.feature_order =
                    block.feature_order.iter().map(|chain| (chain.clone(), place_of_path(node, &chain.path))).collect();
                seed.block = block;
                seed.flm = Some(node.clone());
            }
            Err(found) => collect(Err(found
                .into_iter()
                .map(|error| SeedError::at(place_of_path(node, &error.path), error.detail))
                .collect())),
        }
        for (name, value) in settings {
            match name.as_str() {
                // The block step read these.
                "use" | "features" | "parsing" | "feature_order" => {}
                "initial_use" => {
                    collect(read_initial_use(value, "initial_use").map(|names| seed.initial_use = Some(names)))
                }
                "layout" => collect(read_layout(value).map(|layout| seed.layout = Some((layout, value.place)))),
                "default_profile" => {
                    collect(read_default_profile(value).map(|profile| seed.default_profile = Some(profile)))
                }
                "math" => collect(read_math(value, "math").map(|choice| seed.math = Some((choice, value.place)))),
                _ => collect(Err(vec![SeedError::at(
                    value.place,
                    format!(
                        "`flm.{name}` must be one of the settings that a document's own file may \
                         write under `flm`: `use`, `initial_use`, `features`, `parsing`, `feature_order`, \
                         `layout`, `default_profile` and `math`"
                    ),
                )])),
            }
        }
    }
    if errors.is_empty() { Ok(seed) } else { Err(errors) }
}

/// Reads, from the root of a document's front matter as [`read_front_matter`] returns it,
/// the four keys under `flm` that decide which packages the document uses: `use`,
/// `initial_use`, `math` and `default_profile`, each by the rule [`read_seed`] reads it
/// with. The [`Seed`] returned holds these four keys and is otherwise empty. Every other
/// part of the front matter is left unread: the metadata, the settings, any other key under
/// `flm`, and a root or a value of `flm` that is not a map, whatever problems they have.
///
/// [`session::document_packages`](crate::session::document_packages) reads a front matter
/// this way, so that the packages of a document can be found while the rest of its front
/// matter has problems.
///
/// # Errors
///
/// Every problem of the four keys, as [`read_seed`] reports it: a `use` that is neither an
/// entry nor a list of entries, or an entry that is not a text or breaks the entry grammar
/// (the block step's rule, [`flm_ext::block::read`]); and each problem of `initial_use`
/// ([`read_initial_use`]), `math` ([`read_math`]) and `default_profile`
/// ([`read_default_profile`]).
pub fn read_packages_seed(root: &Node) -> Result<Seed, Vec<SeedError>> {
    let mut seed = Seed::default();
    let Some(flm) = root.get(RESERVED_KEY) else { return Ok(seed) };
    let mut errors = Vec::new();
    if let Some(node) = flm.get("use") {
        // The block step's rule for `use`, applied to that key alone.
        let block = Value::Object(Map::from_iter([(String::from("use"), node.to_json())]));
        match block::read(&block) {
            Ok(block) => seed.uses = uses_of(&block, flm),
            Err(found) => errors
                .extend(found.into_iter().map(|error| SeedError::at(place_of_path(flm, &error.path), error.detail))),
        }
    }
    if let Some(node) = flm.get("initial_use") {
        match read_initial_use(node, "initial_use") {
            Ok(names) => seed.initial_use = Some(names),
            Err(found) => errors.extend(found),
        }
    }
    if let Some(node) = flm.get("math") {
        match read_math(node, "math") {
            Ok(choice) => seed.math = Some((choice, node.place)),
            Err(found) => errors.extend(found),
        }
    }
    if let Some(node) = flm.get("default_profile") {
        match read_default_profile(node) {
            Ok(profile) => seed.default_profile = Some(profile),
            Err(found) => errors.extend(found),
        }
    }
    if errors.is_empty() { Ok(seed) } else { Err(errors) }
}

/// The entries of `use` that the block step read into `block`, each as the package catalog
/// is asked for it (the entry's short form) with its place in `flm`, the node of `flm`.
fn uses_of(block: &Block, flm: &Node) -> Vec<(String, Place)> {
    block
        .uses
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.to_string(), place_of_path(flm, &use_path(flm, index))))
        .collect()
}

/// The key path of the `use` entry `index` of the block `flm`: `use[I]` for a list, `use`
/// for one entry written alone.
fn use_path(flm: &Node, index: usize) -> String {
    match flm.get("use").map(|node| &node.value) {
        Some(NodeValue::List(_)) => format!("use[{index}]"),
        _ => String::from("use"),
    }
}

/// The place of the node that the key path `path` of the block step (`features.text`,
/// `use[2]`, `workflows.draft.settings.html`) designates in `node`: at a map, the longest
/// key that is a prefix of the rest of the path followed by `.`, `[` or the end (a key may
/// hold a dot); at a list, the index in brackets. The place of the deepest node reached,
/// `node`'s own at least.
fn place_of_path(node: &Node, path: &str) -> Place {
    let mut current = node;
    let mut rest = path;
    while !rest.is_empty() {
        let next = if let Some(after) = rest.strip_prefix('[') {
            let Some((index, tail)) = after.split_once(']') else { break };
            let item = match (&current.value, index.parse::<usize>()) {
                (NodeValue::List(items), Ok(index)) => items.get(index),
                _ => None,
            };
            item.map(|item| (item, tail))
        } else {
            current
                .entries()
                .unwrap_or_default()
                .iter()
                .filter(|(key, _)| {
                    rest.strip_prefix(key.as_str()).is_some_and(|tail| tail.is_empty() || tail.starts_with(['.', '[']))
                })
                .max_by_key(|(key, _)| key.len())
                .map(|(key, child)| (child, &rest[key.len()..]))
        };
        let Some((child, tail)) = next else { break };
        current = child;
        rest = tail.strip_prefix('.').unwrap_or(tail);
    }
    current.place
}

/// The front matter of a file included with `\input`, as [`read_included`] reads it: its
/// metadata and the two settings an included file may write under `flm`: `parsing`, how its
/// own content is read, and `input_as`, which makes it a heading input (a file that opens
/// with a heading).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IncludedSeed {
    /// The root keys other than `flm`, as written.
    pub metadata: Vec<(String, Node)>,
    /// `flm.parsing`, as written, once the block step has read it without a problem.
    pub parsing: Option<Node>,
    /// `flm.input_as`, read: present when the file opens with a heading.
    pub input_as: Option<InputAs>,
}

/// The value of `flm.input_as` in an included file: the file opens with a heading written
/// with the command `heading` and the file's title as its argument. The library writes the
/// heading and checks it ([`InputHeading`](flm_core::lang::InputHeading)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputAs {
    /// The heading command, without its backslash: `section`. A backslash written before
    /// the name in the front matter is removed.
    pub heading: String,
    /// The label the heading carries, if one is given; the library refuses a label that
    /// holds a brace.
    pub label: Option<String>,
    /// Whether the heading is numbered; `true` when `numbered` is not written.
    pub numbered: bool,
}

/// The keys an included file may write under `flm`.
const INCLUDED_KEYS: [&str; 2] = ["parsing", "input_as"];

/// Reads the value of `input_as`: a map holding `heading`, the name of a heading command
/// (such as `section`), and optionally `label`, a text, and `numbered`, `true` or `false`.
///
/// # Errors
///
/// A [`SeedError`] for a value that is not a map, a missing `heading`, a value of the wrong
/// type, and a key other than these three.
pub fn read_input_as(node: &Node) -> Result<InputAs, Vec<SeedError>> {
    let Some(entries) = node.entries() else {
        return Err(vec![SeedError::at(node.place, "`input_as` must be a map holding `heading`")]);
    };
    let mut heading = None;
    let mut label = None;
    let mut numbered = true;
    let mut errors = Vec::new();
    for (key, value) in entries {
        match (key.as_str(), &value.value) {
            ("heading", NodeValue::Text(name)) => heading = Some(name.trim_start_matches('\\').to_string()),
            ("heading", _) => errors.push(SeedError::at(value.place, "`input_as.heading` must be the name of a heading command")),
            ("label", NodeValue::Text(text)) => label = Some(text.clone()),
            ("label", _) => errors.push(SeedError::at(value.place, "`input_as.label` must be a text")),
            ("numbered", NodeValue::Bool(value)) => numbered = *value,
            ("numbered", _) => errors.push(SeedError::at(value.place, "`input_as.numbered` must be `true` or `false`")),
            _ => errors.push(SeedError::at(
                value.place,
                format!("`input_as.{key}` must be one of `heading`, `label` and `numbered`"),
            )),
        }
    }
    match heading {
        Some(heading) if errors.is_empty() => Ok(InputAs { heading, label, numbered }),
        None => {
            errors.push(SeedError::at(node.place, "`input_as` must hold `heading`, the name of a heading command"));
            Err(errors)
        }
        Some(_) => Err(errors),
    }
}

/// Returns the JSON Schema of `input_as`, as [`read_input_as`] reads it.
pub fn input_as_schema() -> Value {
    serde_json::json!({
        "description": "In a file included with `\\input` only: the file opens with a heading. The \
            heading is the command `heading` (such as `section`) applied to the file's `title`; it \
            carries the label `label` when one is given, and it is numbered unless `numbered` is \
            `false`.",
        "type": "object",
        "properties": {
            "heading": { "description": "The name of a heading command, such as `section`, written without a \
                backslash (a leading backslash is removed).", "type": "string" },
            "label": { "description": "The label the heading carries, as written inside `\\label{…}`; it must \
                not hold a brace.", "type": "string" },
            "numbered": { "description": "Whether the heading is numbered; `true` unless written.", "type": "boolean" },
        },
        "required": ["heading"],
        "additionalProperties": false,
    })
}

/// Reads the root of an included file's front matter, as [`read_front_matter`] returns it:
/// the metadata, and under `flm` the two settings an included file may write, `parsing` and
/// `input_as` ([`read_input_as`]). The settings of the document as a whole (`use`,
/// `initial_use`, `features`, `feature_order`, `layout`, `default_profile`, `math`) may be
/// written only in the primary file's front matter.
///
/// Returns what could be read together with every problem found, because the file's
/// content is included even when its front matter has problems: a root that is not a map,
/// a value of `flm` that is not a map, a key under `flm` that an included file may not
/// write, and each problem that the block step ([`flm_ext::block::read`]) finds in
/// `parsing` or [`read_input_as`] finds (that setting is then left out).
pub fn read_included(root: &Node) -> (IncludedSeed, Vec<SeedError>) {
    let mut seed = IncludedSeed::default();
    let mut errors = Vec::new();
    let entries = match &root.value {
        NodeValue::Null => return (seed, errors),
        NodeValue::Map(entries) => entries,
        _ => {
            errors.push(SeedError::at(root.place, "the front matter must be a map from keys to values"));
            return (seed, errors);
        }
    };
    for (key, node) in entries {
        if key != RESERVED_KEY {
            seed.metadata.push((key.clone(), node.clone()));
            continue;
        }
        let Some(settings) = node.entries() else {
            if node.value != NodeValue::Null {
                errors.push(SeedError::at(node.place, "the value of `flm` must be a map"));
            }
            continue;
        };
        for (name, value) in settings {
            match name.as_str() {
                "parsing" => {
                    let mut block = Map::new();
                    block.insert(String::from("parsing"), value.to_json());
                    match block::read(&Value::Object(block)) {
                        Ok(_) => seed.parsing = Some(value.clone()),
                        Err(found) => errors.extend(
                            found.into_iter().map(|error| SeedError::at(place_of_path(node, &error.path), error.detail)),
                        ),
                    }
                }
                "input_as" => match read_input_as(value) {
                    Ok(input_as) => seed.input_as = Some(input_as),
                    Err(found) => errors.extend(found),
                },
                _ => errors.push(SeedError::at(
                    value.place,
                    format!(
                        "`flm.{name}` must not be written in an included file; an included file \
                         may set `flm.{}` only",
                        INCLUDED_KEYS.join("`, `flm.")
                    ),
                )),
            }
        }
    }
    (seed, errors)
}

/// Reads the metadata of the file `file` (the root keys of its front matter other than
/// `flm`, as [`Seed::metadata`] holds them) into the document's metadata, [`DocMeta`].
///
/// The library's reader [`DocMeta::from_data`] states what each key may hold; this function
/// adds the place where each value is written, as [`read_front_matter`] records it (exact
/// up to the first escape sequence of a text on one line, and not exact inside a text
/// written over several lines or as a block).
///
/// # Errors
///
/// A [`SeedError`] for each problem the library's reader finds, at the place of the
/// value at fault.
pub fn read_metadata(metadata: &[(String, Node)], file: &str) -> Result<DocMeta, Vec<SeedError>> {
    let node_at = |path: &MetaPath| -> Option<&Node> {
        let node = metadata.iter().find(|(key, _)| *key == path.key).map(|(_, node)| node)?;
        match (path.index, &node.value) {
            (Some(index), NodeValue::List(items)) => items.get(index),
            (Some(_), _) => None,
            (None, _) => Some(node),
        }
    };
    let data: Map<String, Value> = metadata.iter().map(|(key, node)| (key.clone(), node.to_json())).collect();
    DocMeta::from_data(&data, |path| {
        node_at(path).map(|node| MetaPlace::new(file, node.place.line, node.place.column))
    })
    .map_err(|errors| {
        errors
            .into_iter()
            .map(|error| SeedError { place: node_at(&error.path).map(|node| node.place), detail: error.detail })
            .collect()
    })
}

/// Reads the value of `initial_use`, the packages used before those of `use`: one
/// package's name, or a list of names, each with its place, in the order written. `name`
/// is what the user wrote the value under, which a message quotes: `initial_use` in a
/// front matter, or an option of a command line. Each name is given to the package catalog
/// as a `use` entry is: in the entry's short form ([`UseEntry`](flm_ext::UseEntry)'s
/// `Display`, so that `myext#_` names the package `myext` names) when the text is a valid
/// entry, else as written; a name no package has is reported at its entry
/// ([`crate::packages`]).
///
/// # Errors
///
/// A [`SeedError`] for a value that is neither a text nor a list (`null` included), and
/// one for each entry of a list that is not a text, naming the entry by its value.
pub fn read_initial_use(node: &Node, name: &str) -> Result<Vec<(String, Place)>, Vec<SeedError>> {
    let short = |text: &str| flm_ext::UseEntry::parse(text).map_or_else(|_| text.to_string(), |entry| entry.to_string());
    match &node.value {
        NodeValue::Text(text) => Ok(vec![(short(text), node.place)]),
        NodeValue::List(items) => {
            let mut names = Vec::new();
            let mut errors = Vec::new();
            for item in items {
                match item.as_text() {
                    Some(text) => names.push((short(text), item.place)),
                    None => errors.push(SeedError::at(
                        item.place,
                        format!("the entry `{}` of `{name}` must be a package's name, written as a text", item.to_json()),
                    )),
                }
            }
            if errors.is_empty() { Ok(names) } else { Err(errors) }
        }
        _ => Err(vec![SeedError::at(node.place, format!("`{name}` must be a package's name or a list of packages' names"))]),
    }
}

/// Reads the value of `layout`, how the content is read at its top level: `blocks` (the
/// content is paragraphs and blocks), `inline` (the content is one run of text) or `auto`
/// (blocks when the content holds a paragraph break or a block, inline otherwise).
///
/// # Errors
///
/// A [`SeedError`] for any other value.
pub fn read_layout(node: &Node) -> Result<FragmentLayout, Vec<SeedError>> {
    let found = LAYOUTS.iter().find(|(name, _)| node.as_text() == Some(*name));
    found.map(|(_, layout)| *layout).ok_or_else(|| {
        let names: Vec<String> = LAYOUTS.iter().map(|(name, _)| format!("`{name}`")).collect();
        let (last, rest) = names.split_last().expect("three spellings");
        vec![SeedError::at(node.place, format!("`layout` must be {} or {last}", rest.join(", ")))]
    })
}

/// The values of `layout` that [`read_layout`] accepts, each with the layout it stands for;
/// the schema of a front matter lists the same values.
const LAYOUTS: [(&str, FragmentLayout); 3] = [
    ("blocks", FragmentLayout::Blocks),
    ("inline", FragmentLayout::Inline),
    ("auto", FragmentLayout::InlineOrBlocks),
];

/// Reads the value of `default_profile`: a profile's name ([`DefaultProfile`]). Whether an
/// extension or a workflow of that name exists is checked by
/// [`prepare`](crate::session::prepare), when the profile is the one that runs.
///
/// # Errors
///
/// A [`SeedError`] for a value that is not a text.
pub fn read_default_profile(node: &Node) -> Result<DefaultProfile, Vec<SeedError>> {
    match node.as_text() {
        Some(name) => Ok(DefaultProfile { name: name.to_string(), place: node.place }),
        None => Err(vec![SeedError::at(node.place, "`default_profile` must be a profile's name")]),
    }
}

/// Reads the value of `math`, the math processor ([`MathChoice`]): a processor's name,
/// a text of letters, digits, `-` and `_` that stands for the package `math-NAME`, or
/// `false` for no processor. `name` is what the user wrote the value under, which a
/// message quotes: `math` in a front matter, or an option of a command line. Whether a
/// package of that name exists is checked when the packages are found
/// ([`crate::packages`]).
///
/// # Errors
///
/// A [`SeedError`] for a value of another shape (`null`, `true`, a number, a list, a map,
/// an empty text), and one for a text that holds a character other than letters, digits,
/// `-` and `_` (`a/b`, `@acme/fancy`).
pub fn read_math(node: &Node, name: &str) -> Result<MathChoice, Vec<SeedError>> {
    match &node.value {
        NodeValue::Bool(false) => Ok(MathChoice::NoProcessor),
        NodeValue::Text(text) if !text.is_empty() => {
            if is_processor_name(text) {
                Ok(MathChoice::Processor(text.clone()))
            } else {
                Err(vec![SeedError::at(
                    node.place,
                    format!(
                        "`{name}` must be a math processor's name, made of letters, digits, `-` and `_` only, \
                         not `{text}`"
                    ),
                )])
            }
        }
        _ => Err(vec![SeedError::at(
            node.place,
            format!("`{name}` must be a math processor's name, such as `mathjax` or `passthrough`, or `false`"),
        )]),
    }
}

/// Whether `text` is a math processor's name, as [`read_math`] reads one: one or more
/// letters, digits, `-` and `_`.
fn is_processor_name(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The version of JSON Schema in which the schemas of this crate are written, as the
/// value of a schema's `$schema` key.
pub const JSON_SCHEMA_VERSION: &str = "https://json-schema.org/draft/2020-12/schema";

/// Returns the JSON Schema of a front matter, for the features of the package factories
/// `factories`: a complete schema, with the types it refers to under `$defs`.
///
/// The root is a map of the author's metadata, as the library's `meta` feature describes it
/// ([`flm_core::stdcore::features::meta::metadata_schema`]), and admits any other key.
/// Under [`RESERVED_KEY`] it describes the eight keys [`read_seed`] reads (`features` as
/// [`features_schema`] describes it) and `input_as`, the key that only an included file
/// writes ([`read_included`]). `use` and `initial_use` suggest the values `suggestions`
/// ([`crate::packages::suggestions`]), and `math` the processors among them. The `flm`
/// program writes this schema with `flm schema`.
pub fn front_matter_schema(factories: &[&PackageFactory], suggestions: &[String]) -> Value {
    let mut defs = Map::new();
    let mut root = metadata_schema();
    let flm = serde_json::json!({
        "description": "The settings of the document.",
        "type": ["object", "null"],
        "properties": {
            "use": use_schema(suggestions),
            "initial_use": initial_use_schema(suggestions),
            "math": math_schema(suggestions),
            "features": features_schema(factories, true, &mut defs),
            "parsing": parsing_schema(),
            "feature_order": feature_order_schema(),
            "input_as": input_as_schema(),
            "layout": {
                "description": "How the content is read at its top level: `blocks` (paragraphs and blocks), \
                    `inline` (one run of text), or `auto` (blocks when the content holds a paragraph break \
                    or a block, inline otherwise).",
                "enum": LAYOUTS.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            },
            "default_profile": {
                "description": "The profile to build with when no other is given: a profile's name. The \
                    profile `NAME` uses the extension `profile-NAME` when one exists, after the document's \
                    other packages, and runs the workflow `NAME`.",
                "type": "string",
            },
        },
        "additionalProperties": false,
    });
    if let Value::Object(object) = &mut root
        && let Some(Value::Object(properties)) = object.get_mut("properties")
    {
        properties.insert(RESERVED_KEY.into(), flm);
    }
    document(root, defs)
}

/// Returns `schema` as a complete schema: the `$schema` key with [`JSON_SCHEMA_VERSION`]
/// first, then the keys of `schema`, and `defs`, the types it refers to, under `$defs` when
/// there are any.
pub fn document(schema: Value, defs: Map<String, Value>) -> Value {
    let mut root = Map::new();
    root.insert("$schema".into(), Value::from(JSON_SCHEMA_VERSION));
    if let Value::Object(object) = schema {
        root.extend(object);
    }
    if !defs.is_empty() {
        root.insert("$defs".into(), Value::Object(defs));
    }
    Value::Object(root)
}

/// The sentence of the descriptions of `use` and `initial_use` that says what their
/// suggested values are.
const SUGGESTED_PACKAGES: &str = "The values suggested are the packages compiled into the program and the \
    extensions found on the search path when this schema was made, each by its locator; any other may be \
    written.";

/// Returns the JSON Schema of a text, with `examples`, the values an editor suggests, when
/// there are any; the schema restricts nothing.
fn suggested_text(examples: &[String]) -> Value {
    let mut schema = serde_json::json!({ "type": "string" });
    if !examples.is_empty() {
        schema["examples"] = Value::from(examples.to_vec());
    }
    schema
}

/// Returns the JSON Schema of `use`, as the block step ([`flm_ext::block::read`]) reads it,
/// suggesting the values `suggestions` ([`crate::packages::suggestions`]) for an entry.
pub fn use_schema(suggestions: &[String]) -> Value {
    serde_json::json!({
        "description": format!("The packages the document uses: one entry, or a list. An entry is `LOCATOR` or \
            `LOCATOR#PACKAGE`: `LOCATOR` names a package compiled into the program or an extension found \
            on the search path (segments of letters, digits, `-` and `_` joined by `/`, the first of which \
            may begin with `@`, which YAML requires quoting); `PACKAGE` names one of the extension's \
            packages, `_` (its default package) when absent. A path is not an entry. {SUGGESTED_PACKAGES}"),
        "anyOf": [
            suggested_text(suggestions),
            { "type": "array", "items": suggested_text(suggestions) },
            { "type": "null" },
        ],
    })
}

/// Returns the JSON Schema of `initial_use`, as [`read_initial_use`] reads it: one
/// package's name, or a list; the values `suggestions` ([`crate::packages::suggestions`])
/// are suggested for a name.
pub fn initial_use_schema(suggestions: &[String]) -> Value {
    serde_json::json!({
        "description": format!("The packages used before those of `use`, in place of the list the program starts \
            from: one package's name, or a list. The list is used exactly as written: a package it does not \
            name is not used unless `use` lists it. {SUGGESTED_PACKAGES}"),
        "anyOf": [
            suggested_text(suggestions),
            { "type": "array", "items": suggested_text(suggestions) },
        ],
    })
}

/// The names of the math processors among `suggestions`, the values suggested for an entry
/// of `use` ([`crate::packages::suggestions`]), in their order: `NAME` for each suggestion
/// `math-NAME` ([`MATH_PACKAGE_PREFIX`]) whose `NAME` [`read_math`] reads as a processor's
/// name. A scoped locator (`@acme/math-x`) gives none.
fn math_names(suggestions: &[String]) -> Vec<String> {
    suggestions
        .iter()
        .filter_map(|suggestion| suggestion.strip_prefix(MATH_PACKAGE_PREFIX))
        .filter(|name| is_processor_name(name))
        .map(str::to_string)
        .collect()
}

/// Returns the JSON Schema of `math`, as [`read_math`] reads it: a math processor's name, or
/// `false`; the names of the processors among `suggestions` ([`crate::packages::suggestions`])
/// are suggested for a name, `NAME` for each suggestion `math-NAME`.
pub fn math_schema(suggestions: &[String]) -> Value {
    serde_json::json!({
        "description": "The math processor, the package that processes the document's mathematics: a \
            processor's name, `NAME`, which uses the package `math-NAME` (`mathjax` for `math-mathjax`, \
            `passthrough` for `math-passthrough`) after the packages of `initial_use` and before those of \
            `use`; or `false`, which uses no processor, for a document whose packages under `use` provide \
            one. Without this key, the program's default processor is used. That default is not guaranteed \
            to stay the same: a document that needs a stable result specifies its processor here. The names \
            suggested are those of the processors among the packages compiled into the program and the \
            extensions found on the search path when this schema was made, `NAME` for each one named \
            `math-NAME`; any other name may be written.",
        "anyOf": [
            suggested_text(&math_names(suggestions)),
            { "const": false },
        ],
    })
}

/// Returns the JSON Schema of `feature_order`, as the block step ([`flm_ext::block::read`])
/// reads it: one chain or a list of chains.
pub fn feature_order_schema() -> Value {
    serde_json::json!({
        "description": "The order of features: one chain, or a list. A chain is `NAME < NAME < … < NAME`, \
            two or more features' names (not packages'), each placed after the one before it; the chains of \
            every source add up. A feature that is not in use is left out of a chain. Chains that place \
            features in a circle are an error. Where several features define one command, the chains choose \
            the definition that applies: that of the feature they place after the others.",
        "anyOf": [
            { "type": "string" },
            { "type": "array", "items": { "type": "string" } },
            { "type": "null" },
        ],
    })
}

/// Returns the JSON Schema of `parsing`, as the block step ([`flm_ext::block::read`]) reads
/// it: the library's parsing settings ([`FlmParsing`]).
pub fn parsing_schema() -> Value {
    let mut schema = flm_core::core::settings::schema_of::<FlmParsing>();
    if let Value::Object(object) = &mut schema {
        object.insert("description".into(), Value::from("How the content is read: `comments`, whether comments are recognized (default `true`); \
            `comment_prefix`, what starts a comment (default `%%`); `dollar_math`, whether `$…$` and `$$…$$` \
            delimit mathematics (default `false`, and a `$` is then an error)."));
    }
    schema
}

/// Returns the JSON Schema of a `features` map, as the block step ([`flm_ext::block::read`])
/// reads it, for the features of the package factories `factories`.
///
/// Each feature's name is a property, and no other name is accepted. A feature's value may
/// be empty (no settings), `false` (the feature switched off; only when `switch_off` is
/// true, since a workflow's settings cannot switch a feature off), or the feature's
/// settings as its declaration describes them
/// ([`SettingsDecl::json_schema`](flm_core::core::settings::SettingsDecl::json_schema)), in
/// which an entry of a key merged entry by entry may also be `false`, which removes that
/// entry ([`crate::settings`]). Each feature's schema is added to `defs` under the
/// feature's name, and the types it refers to under the feature's name, a `.` and the
/// type's name; the schema returned refers to them.
pub fn features_schema(factories: &[&PackageFactory], switch_off: bool, defs: &mut Map<String, Value>) -> Value {
    let mut properties = Map::new();
    for feature in factories.iter().flat_map(|factory| factory.features()) {
        let name = feature.name.to_string();
        let settings = match &feature.settings {
            Some(decl) => {
                let mut schema = decl.json_schema();
                // In a file of settings, an entry of a key merged entry by entry may be
                // `false`, which removes it ([`crate::settings`]).
                for mark in decl.marks().iter().filter(|mark| mark.merge_entries) {
                    if let Some(Value::Object(key)) = schema.pointer_mut(&format!("/properties/{}", pointer_escape(&mark.path)))
                        && let Some(entry) = key.remove("additionalProperties")
                    {
                        let removal = serde_json::json!({
                            "const": false,
                            "description": "`false` removes the entry of this name that the feature's defaults or a package used here give.",
                        });
                        key.insert("additionalProperties".into(), serde_json::json!({ "anyOf": [removal, entry] }));
                    }
                }
                if let Value::Object(object) = &mut schema
                    && let Some(Value::Object(inner)) = object.remove("$defs")
                {
                    for (type_name, definition) in inner {
                        defs.insert(format!("{name}.{type_name}"), refer_into(definition, &name));
                    }
                }
                defs.insert(name.clone(), refer_into(schema, &name));
                serde_json::json!({ "$ref": format!("#/$defs/{}", pointer_escape(&name)) })
            }
            None => serde_json::json!({
                "description": "This feature takes no settings.",
                "type": "object",
                "additionalProperties": false,
            }),
        };
        let mut forms = vec![serde_json::json!({ "type": "null" }), settings];
        if switch_off {
            forms.insert(0, serde_json::json!({ "const": false }));
        }
        properties.insert(name, serde_json::json!({ "anyOf": forms }));
    }
    let description = if switch_off {
        "The features' settings: a map from a feature's name to its settings, or to `false` to switch \
        the feature off."
    } else {
        "The features' settings: a map from a feature's name to its settings."
    };
    serde_json::json!({
        "description": description,
        "type": ["object", "null"],
        "properties": properties,
        "additionalProperties": false,
    })
}

/// Returns `schema`, a part of the schema of the feature `feature`, with every reference to
/// a type under `$defs` changed into a reference to that type under the feature's name
/// ([`features_schema`]); the values that are data (`default`, `const`, `enum`, `examples`)
/// are left unchanged.
fn refer_into(schema: Value, feature: &str) -> Value {
    match schema {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(keyword, value)| {
                    let value = match (keyword.as_str(), value) {
                        ("$ref", Value::String(reference)) => match reference.strip_prefix("#/$defs/") {
                            Some(name) => {
                                let name = name.replace("~1", "/").replace("~0", "~");
                                Value::from(format!("#/$defs/{}", pointer_escape(&format!("{feature}.{name}"))))
                            }
                            None => Value::String(reference),
                        },
                        ("default" | "const" | "enum" | "examples", value) => value,
                        (_, value) => refer_into(value, feature),
                    };
                    (keyword, value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(|item| refer_into(item, feature)).collect()),
        other => other,
    }
}

/// Returns the value that the front matter of `text`, a file's text, writes under
/// `flm.features`, as JSON data: `null` when the file has no front matter or its front
/// matter has no such key. It is only what the front matter writes, not the settings a
/// build uses, which the other settings sources complete. The live preview's *Settings*
/// tool uses it.
///
/// # Errors
///
/// A [`SeedError`] when the front matter cannot be read: [`split`]'s and
/// [`read_front_matter`]'s.
pub fn written_features(text: &str) -> Result<Value, SeedError> {
    let split = split(text)?;
    let Some(front_matter) = split.front_matter else {
        return Ok(Value::Null);
    };
    let root = read_front_matter(front_matter, split.front_matter_line)?;
    Ok(root.at(&[RESERVED_KEY, "features"]).map_or(Value::Null, Node::to_json))
}

/// Returns `name` escaped as one segment of a JSON pointer: `~` written `~0` and `/`
/// written `~1`.
fn pointer_escape(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "---\ntitle: A \\emph{title}\nauthor: [A, B]\nflm:\n  use: [std]\n  \
                        features:\n    html: {heading_tags: {section: h2}}\n    theorems: false\n  \
                        parsing: {dollar_math: true}\n  layout: blocks\n  \
                        default_profile: text\n---\nHello.\n";

    fn seed(text: &str) -> Result<Seed, Vec<SeedError>> {
        let split = split(text).unwrap();
        read_seed(&read_front_matter(split.front_matter.unwrap(), split.front_matter_line).unwrap())
    }

    #[test]
    fn a_front_matter_is_split_from_the_content() {
        let split = split(FILE).unwrap();
        assert!(split.front_matter.unwrap().starts_with("title:"));
        assert_eq!(split.content, "Hello.\n");
        assert_eq!(split.content_line, 13);
        let text = "---  \r\ntitle: x\r\n--- \r\nHello.\r\n";
        let split = super::split(text).unwrap();
        assert_eq!((split.opening, split.closing), ("---  \r\n", "--- \r\n"));
        assert_eq!(format!("{}{}{}{}", split.opening, split.front_matter.unwrap(), split.closing, split.content), text);
    }

    #[test]
    fn a_file_with_no_front_matter_is_all_content() {
        let split = split("Hello.\n---\n").unwrap();
        assert_eq!(split.front_matter, None);
        assert_eq!(split.content, "Hello.\n---\n");
        assert_eq!((split.opening, split.closing), ("", ""));
        assert_eq!(split.content_line, 1);
    }

    #[test]
    fn each_spelling_of_layout_is_read_and_another_is_refused_naming_them() {
        let layout = |text: &str| read_layout(&read_front_matter(text, 1).unwrap());
        assert_eq!(layout("blocks").unwrap(), FragmentLayout::Blocks);
        assert_eq!(layout("inline").unwrap(), FragmentLayout::Inline);
        assert_eq!(layout("auto").unwrap(), FragmentLayout::InlineOrBlocks);
        let errors = layout("sideways").unwrap_err();
        assert_eq!(errors[0].detail, "`layout` must be `blocks`, `inline` or `auto`");
    }

    #[test]
    fn a_front_matter_without_its_closing_line_is_refused() {
        assert!(split("---\ntitle: x\n").is_err());
    }

    #[test]
    fn each_of_the_five_keys_is_read_and_the_root_keys_are_metadata() {
        let seed = seed(FILE).unwrap();
        let keys: Vec<&str> = seed.metadata.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys, ["title", "author"]);
        assert_eq!(seed.metadata[0].1.place, Place { line: 2, column: 8 });
        assert_eq!(seed.uses, vec![(String::from("std"), Place { line: 5, column: 9 })]);
        assert_eq!(seed.block.features.len(), 2);
        assert!(matches!(seed.block.features[1].1, block::FeatureBlock::Off));
        let flm = seed.flm.as_ref().expect("the node of flm");
        assert_eq!(flm.at(&["features", "html", "heading_tags", "section"]).unwrap().place.line, 7);
        assert_eq!(seed.block.parsing.unwrap()["dollar_math"], Value::Bool(true));
        assert_eq!(seed.layout.unwrap().0, FragmentLayout::Blocks);
        let profile = seed.default_profile.expect("the default profile");
        assert_eq!((profile.name.as_str(), profile.place.line), ("text", 11));
    }

    #[test]
    fn default_profile_is_one_profile_s_name_and_a_map_there_is_refused() {
        let errors = seed("---\nflm:\n  default_profile: {html: html, _: text}\n---\n").unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].place.map(|place| place.line), Some(3));
    }

    #[test]
    fn an_unknown_key_under_flm_and_a_root_that_is_not_a_map_are_refused() {
        let errors = seed("---\nflm:\n  nope: 1\n---\n").unwrap_err();
        assert_eq!(errors[0].place, Some(Place { line: 3, column: 9 }));
        assert!(seed("---\n- a\n---\n").is_err());
    }

    #[test]
    fn math_is_a_processor_s_name_or_false_with_its_place() {
        let read = seed("---\nflm:\n  math: mathjax\n---\n").unwrap();
        assert_eq!(read.math, Some((MathChoice::processor("mathjax"), Place { line: 3, column: 9 })));
        assert_eq!(read.math.unwrap().0.package().as_deref(), Some("math-mathjax"));
        let read = seed("---\nflm:\n  math: false\n---\n").unwrap();
        assert_eq!(read.math, Some((MathChoice::NoProcessor, Place { line: 3, column: 9 })));
        assert_eq!(MathChoice::NoProcessor.package(), None);
        assert_eq!(seed("---\nflm:\n  math: my_math-2\n---\n").unwrap().math.unwrap().0.package().as_deref(), Some("math-my_math-2"));
        assert_eq!(seed("---\nflm:\n  use: [std]\n---\n").unwrap().math, None);
    }

    #[test]
    fn a_value_of_math_that_is_not_a_processor_s_name_is_refused_at_its_place() {
        let shape = "`math` must be a math processor's name, such as `mathjax` or `passthrough`, or `false`";
        for value in ["null", "true", "3", "[mathjax]", "{a: 1}", "''"] {
            let errors = seed(&format!("---\nflm:\n  math: {value}\n---\n")).unwrap_err();
            assert_eq!(errors.len(), 1, "{value}");
            assert_eq!(errors[0].detail, shape, "{value}");
            assert_eq!(errors[0].place.map(|place| place.line), Some(3), "{value}");
        }
        let errors = seed("---\nflm:\n  math: a/b\n---\n").unwrap_err();
        assert_eq!(errors[0].detail, "`math` must be a math processor's name, made of letters, digits, `-` and `_` only, not `a/b`");
        assert_eq!(errors[0].place, Some(Place { line: 3, column: 9 }));
        let errors = seed("---\nflm:\n  math: '@acme/fancy'\n---\n").unwrap_err();
        assert!(errors[0].detail.ends_with("not `@acme/fancy`"), "{errors:?}");
        // The name a message quotes is the caller's.
        let node = read_front_matter("true", 1).unwrap();
        assert_eq!(
            read_math(&node, "--math").unwrap_err()[0].detail,
            "`--math` must be a math processor's name, such as `mathjax` or `passthrough`, or `false`"
        );
    }

    #[test]
    fn an_included_file_may_not_set_math() {
        let root = read_front_matter("flm:\n  math: passthrough\n", 2).unwrap();
        let (_, errors) = read_included(&root);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].detail.starts_with("`flm.math` must not be written in an included file"), "{errors:?}");
    }

    #[test]
    fn a_problem_of_the_block_step_and_a_workflow_are_placed_at_their_keys() {
        let errors = seed("---\nflm:\n  use: [a, ./b]\n  features:\n    text: 3\n  workflows: {}\n---\n").unwrap_err();
        let places: Vec<Option<Place>> = errors.iter().map(|error| error.place).collect();
        assert_eq!(
            places,
            [Some(Place { line: 3, column: 12 }), Some(Place { line: 5, column: 11 }), Some(Place { line: 6, column: 14 })],
            "{errors:?}"
        );
        assert!(errors[2].detail.starts_with("`flm.workflows` must be one of the settings"), "{errors:?}");
    }

    #[test]
    fn each_chain_of_feature_order_is_read_with_its_place() {
        let listed = seed("---\nflm:\n  use: [std]\n  feature_order:\n    - math-mathjax < phfqit\n    - headings<toc\n---\n")
            .unwrap();
        let read: Vec<(String, String, u32)> = listed
            .feature_order
            .iter()
            .map(|(chain, place)| (chain.path.clone(), chain.chain.to_string(), place.line))
            .collect();
        assert_eq!(
            read,
            [
                (String::from("feature_order[0]"), String::from("math-mathjax < phfqit"), 5),
                (String::from("feature_order[1]"), String::from("headings < toc"), 6),
            ]
        );
        assert_eq!(listed.block.feature_order.len(), 2);
        let alone = seed("---\nflm:\n  feature_order: a < b\n---\n").unwrap();
        assert_eq!(alone.feature_order[0].0.path, "feature_order");
        assert_eq!(alone.feature_order[0].1, Place { line: 3, column: 18 });
    }

    #[test]
    fn a_chain_that_is_not_one_is_refused_at_its_place() {
        let errors = seed("---\nflm:\n  feature_order:\n    - a < b\n    - a > b\n---\n").unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].place, Some(Place { line: 5, column: 7 }));
        assert_eq!(
            errors[0].detail,
            "the chain `a > b` must have the form `NAME < NAME < … < NAME`, with two or more feature names"
        );
    }

    #[test]
    fn an_included_file_may_not_set_feature_order() {
        let text = "---\nflm:\n  feature_order: a < b\n---\n";
        let split = split(text).unwrap();
        let (_, errors) = read_included(&read_front_matter(split.front_matter.unwrap(), split.front_matter_line).unwrap());
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].place, Some(Place { line: 3, column: 18 }));
        assert_eq!(
            errors[0].detail,
            "`flm.feature_order` must not be written in an included file; an included file may set `flm.parsing`, \
             `flm.input_as` only"
        );
    }

    #[test]
    fn initial_use_is_one_name_or_a_list_each_with_its_place() {
        let seed_of = |value: &str| seed(&format!("---\nflm:\n  initial_use: {value}\n---\n"));
        assert_eq!(seed("---\nflm: {use: [std]}\n---\n").unwrap().initial_use, None);
        assert_eq!(seed_of("std").unwrap().initial_use, Some(vec![(String::from("std"), Place { line: 3, column: 16 })]));
        assert_eq!(
            seed_of("[stdcore, myext#svg]").unwrap().initial_use,
            Some(vec![
                (String::from("stdcore"), Place { line: 3, column: 17 }),
                (String::from("myext#svg"), Place { line: 3, column: 26 }),
            ])
        );
        assert_eq!(seed_of("[]").unwrap().initial_use, Some(Vec::new()));
    }

    #[test]
    fn initial_use_of_another_shape_is_refused_at_its_place() {
        let errors = seed("---\nflm:\n  initial_use: {a: 1}\n---\n").unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].place, Some(Place { line: 3, column: 16 }));
        assert_eq!(errors[0].detail, "`initial_use` must be a package's name or a list of packages' names");
        let errors = seed("---\nflm:\n  initial_use: [std, 3]\n---\n").unwrap_err();
        assert_eq!(errors[0].place, Some(Place { line: 3, column: 22 }));
        assert_eq!(errors[0].detail, "the entry `3` of `initial_use` must be a package's name, written as a text");
    }

    #[test]
    fn an_entry_of_initial_use_is_given_in_the_short_form_of_a_use_entry() {
        let seed_of = |value: &str| seed(&format!("---\nflm:\n  initial_use: {value}\n---\n"));
        let names = |value: &str| -> Vec<String> {
            seed_of(value).unwrap().initial_use.unwrap().into_iter().map(|(name, _)| name).collect()
        };
        assert_eq!(names("[stdcore#_, myext#svg, ../x]"), ["stdcore", "myext#svg", "../x"]);
        assert_eq!(names("myext#_"), ["myext"]);
    }

    #[test]
    fn an_included_file_may_not_set_initial_use() {
        let text = "---\nflm:\n  initial_use: [std]\n---\n";
        let split = split(text).unwrap();
        let (seed, errors) =
            read_included(&read_front_matter(split.front_matter.unwrap(), split.front_matter_line).unwrap());
        assert_eq!(seed, IncludedSeed::default());
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].place, Some(Place { line: 3, column: 16 }));
        assert_eq!(
            errors[0].detail,
            "`flm.initial_use` must not be written in an included file; an included file may set `flm.parsing`, \
             `flm.input_as` only"
        );
    }

    #[test]
    fn the_schema_of_a_front_matter_describes_initial_use() {
        let schema = front_matter_schema(&[], &[]);
        let initial_use = &schema["properties"][RESERVED_KEY]["properties"]["initial_use"];
        assert_eq!(*initial_use, initial_use_schema(&[]));
        assert!(initial_use["description"].as_str().unwrap().contains("exactly as written"));
    }

    #[test]
    fn the_schema_of_a_front_matter_describes_math_as_a_name_or_false() {
        let schema = front_matter_schema(&[], &[]);
        let math = &schema["properties"][RESERVED_KEY]["properties"]["math"];
        assert_eq!(*math, math_schema(&[]));
        assert_eq!(math["anyOf"], serde_json::json!([{ "type": "string" }, { "const": false }]));
        assert!(math["description"].as_str().unwrap().contains("not guaranteed"));
    }

    #[test]
    fn a_path_of_the_block_step_finds_its_node() {
        let node = read_front_matter("use: [a, b]\nfeatures:\n  a.b: {x: 1}\n  a: {y: 2}\nw:\n  d:\n    s:\n      h: 1\n", 1).unwrap();
        let at = |path: &str| place_of_path(&node, path);
        assert_eq!(at("use[1]"), Place { line: 1, column: 10 });
        assert_eq!(at("use[7]"), node.get("use").unwrap().place);
        assert_eq!(at("features.a.b.x"), Place { line: 3, column: 12 });
        assert_eq!(at("features.a.y"), Place { line: 4, column: 10 });
        assert_eq!(at("w.d.s.h"), Place { line: 8, column: 10 });
        assert_eq!(at("features.nosuch"), node.get("features").unwrap().place);
        assert_eq!(at(""), node.place);
    }

    #[test]
    fn a_number_that_is_not_finite_is_refused_at_its_place() {
        for value in [".nan", ".inf", "-.inf"] {
            let error = read_front_matter(&format!("a: 1\nb: {value}\n"), 2).unwrap_err();
            assert_eq!(error.place, Some(Place { line: 3, column: 4 }), "{value}");
        }
    }

    #[test]
    fn a_tag_is_dropped_and_its_value_read() {
        let node = read_front_matter("title: !custom Hello\n", 1).unwrap();
        assert_eq!(node.get("title").and_then(Node::as_text), Some("Hello"));
    }

    #[test]
    fn text_that_is_not_yaml_is_placed_in_the_file() {
        let split = split("---\ntitle: [a\n---\n").unwrap();
        let error = read_front_matter(split.front_matter.unwrap(), split.front_matter_line).unwrap_err();
        assert!(error.place.unwrap().line >= 2);
    }

    #[test]
    fn a_place_s_column_counts_bytes_on_a_line_with_characters_beyond_ascii() {
        // `é`, `à` and `ç` are one character and two bytes each.
        let node = read_front_matter("title: Café\nflm: {features: {}, éé: 1}\nsee: [\"àb\", 'ç', x]\n", 2).unwrap();
        assert_eq!(node.get("title").unwrap().place, Place { line: 2, column: 8 });
        assert_eq!(node.at(&["flm", "éé"]).unwrap().place, Place { line: 3, column: 27 });
        let NodeValue::List(items) = &node.get("see").unwrap().value else { panic!("a list") };
        let places: Vec<Place> = items.iter().map(|item| item.place).collect();
        assert_eq!(places, [Place { line: 4, column: 8 }, Place { line: 4, column: 15 }, Place { line: 4, column: 20 }]);
        // A problem the YAML reader finds is placed the same way.
        let error = read_front_matter("a: 1\nb: [é, }\n", 1).unwrap_err();
        assert_eq!(error.place, Some(Place { line: 2, column: 9 }));
    }

    #[test]
    fn the_features_written_are_read_as_data_and_null_without_a_front_matter() {
        assert_eq!(written_features(FILE).unwrap(), serde_json::json!({ "html": { "heading_tags": { "section": "h2" } }, "theorems": false }));
        assert_eq!(written_features("Hello.\n").unwrap(), Value::Null);
        assert_eq!(written_features("---\ntitle: x\n---\n").unwrap(), Value::Null);
        assert!(written_features("---\ntitle: [unclosed\n---\n").is_err());
    }
}
