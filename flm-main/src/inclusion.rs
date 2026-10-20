//! The host's input resolver: finds and reads, for the library, the file that an `\input`
//! command refers to.
//!
//! The library parses `\input{reference}` but reads no file and no front matter itself:
//! for each `\input` it asks an [input resolver](InputResolver) for the file. [`Inclusion`]
//! is the host's input resolver. [`session::build`](crate::session::build) makes a new one
//! for each build, over that build's file reader ([`BuildFiles`]), so that every
//! included file is recorded on the build's list of files read. It reads through
//! [`crate::files`] and [`crate::seed`], and records, for each file it reads, the file's
//! content and the byte offset at which it starts ([`Inclusion::content_offsets`]), so that
//! a place in that content can be counted from the top of the file ([`crate::report`]).
//!
//! For one reference, [`Inclusion`]:
//!
//! - resolves the reference against the folder of the file that holds the command, and
//!   normalizes the result ([`fold`]): `\input{chapter.flm}` written in `docs/paper.flm` is
//!   the file `docs/chapter.flm`. That path is the file's name, which messages print and the
//!   list of files read records;
//! - reads the file, splits off its front matter and reads it ([`seed::read_included`]); a
//!   problem in the front matter is returned with its place in the file, and the content is
//!   still included, except when the front matter has no closing `---` line: the file is
//!   then included with no content;
//! - for a heading input (a file whose front matter holds `flm.input_as`, so that it opens
//!   with a heading), returns the heading: the heading command given by `input_as.heading`,
//!   the file's `title` (read as a primary file's metadata is read, with its place), the
//!   label `input_as.label` and whether the heading is numbered (`input_as.numbered`);
//! - when the file writes parsing settings under `flm.parsing`, returns them applied over
//!   the parsing settings that apply where the command is written, which the library passes
//!   in.
//!
//! The library keeps track of which file includes which, and of the parsing settings each
//! file is read under; [`Inclusion`] keeps no other memory of earlier calls. The library's
//! documentation of [`InputResolver`] and of the module [`flm_core::lang::input`] describes
//! the whole exchange, including the checks the library makes on a heading input's command
//! and label.

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use flm_core::lang::{FlmParsing, IncludedFile, IncludedFileProblem, InputError, InputHeading, InputResolver};

use crate::files::{BuildFiles, fold};
use crate::report::ContentOffsets;
use crate::seed::{self, Node, SeedError};

/// The host's [input resolver](InputResolver), reading through the file reader of one build:
/// see the [module documentation](self).
pub struct Inclusion {
    files: BuildFiles,
    /// The content of each file read, with the byte offset at which it starts.
    offsets: Mutex<ContentOffsets>,
}

impl Inclusion {
    /// Returns the input resolver that reads through `files`, the file reader of one build.
    pub fn new(files: BuildFiles) -> Inclusion {
        Inclusion { files, offsets: Mutex::new(ContentOffsets::new()) }
    }

    /// Returns, for each file this resolver has read, by the file's name, its content and the
    /// byte offset at which the content starts: after its front matter, when it has one
    /// ([`Split::content_offset`](seed::Split::content_offset)).
    pub fn content_offsets(&self) -> ContentOffsets {
        self.offsets.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl InputResolver for Inclusion {
    fn resolve(&self, reference: &str, including: Option<&str>, parsing: &FlmParsing) -> Result<IncludedFile, InputError> {
        let folder = including.and_then(|name| Path::new(name).parent()).unwrap_or(Path::new(""));
        // See decision rationale at [§flm-dd-dr:host-input-names].
        let path = fold(&BuildFiles::resolve(folder, Path::new(reference)));
        let name = path.display().to_string();
        let text = self.files.read_text(Path::new(""), &path).map_err(|error| InputError::new(error.to_string()))?;
        let mut problems = Vec::new();
        let (content, first_line, front_matter) = match seed::split(&text) {
            Ok(split) => {
                let mut offsets = self.offsets.lock().unwrap_or_else(PoisonError::into_inner);
                offsets.insert(name.clone(), &split);
                let front_matter = split.front_matter.map(|fm| (fm, split.front_matter_line));
                (split.content.to_string(), split.content_line, front_matter)
            }
            Err(error) => {
                problems.push(problem(error));
                (String::new(), 1, None)
            }
        };
        let mut own_parsing = None;
        let mut heading = None;
        if let Some((front_matter, line)) = front_matter {
            match seed::read_front_matter(front_matter, line) {
                Ok(root) => {
                    let (included, errors) = seed::read_included(&root);
                    problems.extend(errors.into_iter().map(problem));
                    own_parsing = included.parsing;
                    if let Some(input_as) = included.input_as {
                        heading = Some(input_heading(input_as, &included.metadata, &name, &mut problems));
                    }
                }
                Err(error) => problems.push(problem(error)),
            }
        }
        let parsing = own_parsing.and_then(|node| match over(parsing, &node) {
            Ok(settings) => Some(settings),
            Err(error) => {
                problems.push(problem(error));
                None
            }
        });
        Ok(IncludedFile { name, text: content, first_line: first_line as usize, parsing, problems, heading })
    }
}

/// Returns the heading of a heading input: what `input_as` gives, with the file's title,
/// read from its metadata as a primary file's is ([`seed::read_metadata`]); a file without
/// a title gives an empty title. A problem in the metadata is added to `problems`.
fn input_heading(
    input_as: seed::InputAs,
    metadata: &[(String, Node)],
    name: &str,
    problems: &mut Vec<IncludedFileProblem>,
) -> InputHeading {
    let title = match seed::read_metadata(metadata, name) {
        Ok(meta) => meta.title,
        Err(errors) => {
            problems.extend(errors.into_iter().map(problem));
            None
        }
    };
    let (title, title_at) = match title {
        Some(title) => {
            let at = title.place.map(|place| (place.line as usize, place.column as usize));
            (title.text, at)
        }
        None => (String::new(), None),
    };
    InputHeading {
        command: input_as.heading,
        title,
        title_at,
        label: input_as.label,
        numbered: input_as.numbered,
    }
}

/// Returns `base` with the keys written in `node` replacing its values; `node` is a
/// `parsing` map that the block step ([`flm_ext::block::read`]) read without a problem.
///
/// # Errors
///
/// A [`SeedError`] at `node` when the combined settings cannot be read. The checks of
/// the block step rule this out: each key they accept holds a value of its
/// setting's type, regardless of the other keys.
fn over(base: &FlmParsing, node: &Node) -> Result<FlmParsing, SeedError> {
    let mut value = serde_json::to_value(base).expect("the parsing settings are plain data");
    if let (Some(target), serde_json::Value::Object(written)) = (value.as_object_mut(), node.to_json()) {
        target.extend(written);
    }
    serde_json::from_value(value)
        .map_err(|error| SeedError::at(node.place, format!("each setting under `parsing` must hold a value of its type: {error}")))
}

/// Returns a problem of the front matter as the library takes it, at its place in the
/// file (line 1, column 1 when the place is not known).
fn problem(error: SeedError) -> IncludedFileProblem {
    let (line, column) = error.place.map_or((1, 1), |place| (place.line as usize, place.column as usize));
    IncludedFileProblem { line, column, message: error.detail }
}
