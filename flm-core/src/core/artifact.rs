//! Artifacts: one file's worth of output held as a value, the artifact set
//! that makes up one output, and the naming service a host supplies to turn
//! an artifact's identity into a path.
//!
//! An artifact is one file's worth of output held as a value, identified by
//! its producer and a name and never by a path: the bytes of a page, a
//! stylesheet, a copied image; a *folder artifact* holds the files of a whole
//! folder instead (see [A folder artifact](#a-folder-artifact) below). An
//! artifact set is the artifacts of one output, at most one of which is the
//! entry artifact, the one that represents the output as a whole (the HTML
//! page, the text, the LaTeX document). An output is a name a caller asks
//! for, such as `text` or `html`
//! ([`OutputDef`]); the output's table holds
//! its artifact set. A backend's final stage puts the set into that table,
//! and the caller reads it from there. For the place of outputs in the
//! processing of a document, see [backends, lowering, and
//! outputs](crate::guide::concepts_overview#backends-lowering-and-outputs).
//!
//! The items of this module:
//!
//! - [`Artifact`], one artifact, and [`ArtifactData`], its content: the bytes,
//!   a [`FileRef`] to a file that holds them, or the files of a folder;
//! - [`ArtifactId`], an artifact's identity, and [`RelPath`], a relative path;
//! - [`ArtifactSet`], the artifacts of one output, with
//!   [`ArtifactSetViolation`] for the two ways a set can be malformed, and the
//!   two conditions reported for them, [`DuplicateArtifact`] and
//!   [`SecondEntryArtifact`];
//! - [`ArtifactNamer`], the naming service that turns an identity into a
//!   path, with its two inputs: [`ArtifactRef`], what is named, and
//!   [`NamingScope`] ([`ScopedOutput`]), the outputs of the command;
//! - [`path_of`], the path at which a host writes an artifact: its fixed path,
//!   else the naming service's answer;
//! - [`media_type_of`], the media type of a file by its extension.
//!
//! # Identity, not paths
//!
//! An artifact is identified by an [`ArtifactId`]: the feature or backend that
//! contributed it, and a name that this producer chose. This library writes
//! no file, so an artifact has no path of its own. A host that writes the
//! artifacts out asks its [`ArtifactNamer`] for the path of each identity.
//! Because the naming service computes the path from the identity and a few
//! facts known in advance (the entry role, the media type, the extension, the
//! outputs of the command), a producer can write a link to another artifact
//! before that artifact's bytes exist. [`path_of`] states the whole rule: an
//! artifact's [fixed path](Artifact::fixed_path) when it has one, else the
//! naming service's answer.
//!
//! # A folder artifact
//!
//! An artifact whose data is [`ArtifactData::Files`] is a *folder artifact*:
//! the files of one folder, each by its path relative to the folder, shipped
//! together under one identity (a compose template's folder of icons, for
//! instance). The naming service names the folder ([`ArtifactRef::folder`]);
//! a host that writes it writes each file under that folder with its own
//! path. A folder artifact has no extension and no media type of its own
//! ([`Artifact::folder`]); each of its files takes the media type its own
//! extension gives.
//!
//! # The entry artifact
//!
//! At most one artifact of a set is its **entry artifact**: the one that
//! represents the output as a whole, such as the page a reader opens or the
//! file a build system is given. A one-file output is a set of one artifact,
//! which is the entry artifact. The registry records what is known about an
//! output's entry artifact before anything is rendered, its media type and
//! file extension, in [`OutputEntry`](crate::core::registry::OutputEntry).
//!
//! Artifacts enter a set in two ways, which treat a violation of the
//! one-entry rule differently because only one of them can refuse.
//! [`ArtifactSet::insert`] is what a producer calls; it returns a [`Result`]
//! and refuses a second entry artifact or a repeated identity.
//! [`Table::absorb`], which combines the table pieces that several stage
//! invocations wrote, cannot fail (no table's `absorb` can), so it keeps the
//! first artifact, turns a second entry artifact into an ordinary artifact,
//! and records each violation in [`ArtifactSet::violations`].
//!
//! # The same artifact twice
//!
//! When table pieces are combined, an artifact equal in every field to one
//! the set already holds (the same identity, content, media type, entry role,
//! fixed path and links) is the same artifact contributed twice. It is
//! dropped and nothing is recorded: when two trees of a document show one
//! image, both contribute its copy, and the document is not at fault. Two
//! artifacts that differ under one identity are a violation, as above.
//!
//! # Making an output's set, and reporting a malformed one
//!
//! The stage that finishes an output (the backend's [final
//! stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)
//! or a host's template stage) makes its set with
//! [`ArtifactSet::insert_entry_with`]: the entry artifact first, then the
//! artifacts of the tables of files written beside it. That function returns
//! one Error diagnostic for every violation the finished set holds
//! ([`DuplicateArtifact`], [`SecondEntryArtifact`]), whichever of those stages
//! calls it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::core::data::table::{PieceClass, ProjectCx, Table};
use crate::core::registry::{FlmRegistry, OutputDef};
use crate::lang::origin::nowhere;
use crate::lang::{FlmDiagnostic, FlmOrigin};
use crate::model::names::{CapName, Name, OutputName};

/// A relative path, written with forward slashes and no leading slash.
///
/// An [`ArtifactNamer`] returns one for each artifact, and a producer may set
/// one as an artifact's [`fixed_path`](Artifact::fixed_path). The path is
/// relative to the place where the host writes the output; this library
/// writes no file and gives the text no meaning beyond that form. The
/// `graphics` feature, for example, writes the path the naming service
/// returns for a copied image as the image's address, and the recomposed LaTeX
/// sets the fixed path of its style file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

impl RelPath {
    /// Returns a relative path holding `path` as written, unchecked.
    pub fn new(path: impl Into<String>) -> RelPath {
        RelPath(path.into())
    }

    /// Returns the path's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An artifact's identity: the feature or backend that contributed it, and a
/// name that this producer chose.
///
/// The name is the producer's own (`document`, `style`, `fig-3`); this library
/// never interprets it. A producer must choose the same name for the same
/// artifact in every run, so that a link written in one artifact still points
/// at the right one. Everything else refers to an artifact by its identity:
/// an [`ArtifactSet`] is keyed by it, one artifact links to another through
/// it ([`Artifact::refs`]), and an [`ArtifactNamer`] turns it into a path.
///
/// Its [`Display`](fmt::Display) form is `owner:name`.
///
/// # Examples
///
/// ```
/// use flm_core::core::artifact::ArtifactId;
///
/// let id = ArtifactId::new("text", "document");
/// assert_eq!(id.to_string(), "text:document");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactId {
    /// The name of the feature or backend that contributed the artifact.
    pub owner: Name,
    /// The name the producer chose, which no other artifact of that owner
    /// has.
    pub name: String,
}

impl ArtifactId {
    /// Returns the identity of the artifact `name` contributed by `owner`.
    pub fn new(owner: impl Into<Name>, name: impl Into<String>) -> ArtifactId {
        ArtifactId { owner: owner.into(), name: name.into() }
    }
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.owner, self.name)
    }
}

/// The location of a file holding an artifact's bytes, written the way the
/// host that produced the file writes paths.
///
/// A file reference keeps a large artifact out of memory: a host that ran an
/// external converter, or that keeps intermediate files in a working folder,
/// passes on the location instead of the bytes. This library treats the text
/// as opaque: it is not a [`RelPath`], and nothing in this library opens it.
///
/// No feature of this library produces a file reference: every artifact the
/// standard backends make holds its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileRef {
    /// Where the file is, as the host that produced it spells it.
    pub location: String,
}

/// An artifact's content: the bytes themselves, a reference to a file
/// holding them, or the files of a folder.
///
/// [`Artifact::as_bytes`] returns the bytes in the first case and `None` in
/// the other two.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ArtifactData {
    /// The bytes, in memory.
    Bytes(Vec<u8>),
    /// A [`FileRef`] to a file holding the bytes, for an artifact too large to
    /// hold in memory.
    FileRef(FileRef),
    /// The files of a folder artifact (see the [module
    /// documentation](self#a-folder-artifact)), each by its path relative to
    /// the folder and its bytes, in the order the producer gives. Each path is
    /// written with forward slashes, with no leading slash and no `.` or `..`
    /// part, and no two are equal.
    Files(Vec<(RelPath, Vec<u8>)>),
}

impl fmt::Debug for ArtifactData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactData::Bytes(bytes) => write!(f, "Bytes({} bytes)", bytes.len()),
            ArtifactData::FileRef(file) => write!(f, "FileRef({:?})", file.location),
            ArtifactData::Files(files) => {
                let paths: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
                write!(f, "Files({paths:?})")
            }
        }
    }
}

/// One file's worth of output held as a value: an identity, a media type, the
/// content, and whether it is the entry artifact of its set. A folder artifact
/// ([`Artifact::folder`]) holds the files of a folder as its content.
///
/// A stage builds an artifact with [`Artifact::bytes`] and puts it into an
/// [`ArtifactSet`]. This library writes nothing to disk; the host decides
/// whether and where the bytes become a file (see the [module
/// documentation](self)).
///
/// # Examples
///
/// ```
/// use flm_core::core::artifact::{Artifact, ArtifactId};
///
/// let artifact = Artifact::bytes(
///     ArtifactId::new("text", "document"),
///     "text/plain",
///     b"Hello.".to_vec(),
/// )
/// .as_entry();
///
/// assert!(artifact.entry);
/// assert_eq!(artifact.as_bytes(), Some(&b"Hello."[..]));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// The artifact's identity.
    pub id: ArtifactId,
    /// Whether this artifact is the entry artifact of the set it belongs to
    /// (see the [module documentation](self#the-entry-artifact)).
    pub entry: bool,
    /// The media type, such as `text/plain` or `text/html`; the empty text for
    /// a folder artifact, whose files each take the media type of their own
    /// extension.
    pub media_type: String,
    /// The bytes, a reference to the file holding them, or the files of a
    /// folder artifact.
    pub data: ArtifactData,
    /// A path the producer requires for this artifact. A host that writes the
    /// artifact uses this path as given instead of asking its
    /// [`ArtifactNamer`]. The recomposed LaTeX sets it on its style file,
    /// whose file name the LaTeX document refers to.
    pub fixed_path: Option<RelPath>,
    /// The other artifacts this artifact's bytes link to. The HTML and
    /// recomposed LaTeX final stages set it on the entry artifact from the
    /// [`Regions::refs`](crate::core::region::Regions::refs) of their output.
    pub refs: Vec<ArtifactId>,
    /// The file extension the producer gives, without the dot (`sty`, `png`);
    /// `None` leaves it to the host's table over media types. A host that
    /// writes the artifact passes it to its [`ArtifactNamer`]
    /// ([`ArtifactRef::extension`]). Set it with [`Artifact::with_extension`].
    /// It is `None` for a folder artifact, whose path has no extension.
    pub extension: Option<String>,
}

impl Artifact {
    /// Returns an artifact of identity `id` and media type `media_type`
    /// holding `bytes`: not the entry artifact, with no fixed path, no links
    /// and no extension of its own.
    ///
    /// Mark it as the entry artifact with [`Artifact::as_entry`].
    pub fn bytes(id: ArtifactId, media_type: impl Into<String>, bytes: Vec<u8>) -> Artifact {
        Artifact {
            id,
            entry: false,
            media_type: media_type.into(),
            data: ArtifactData::Bytes(bytes),
            fixed_path: None,
            refs: Vec::new(),
            extension: None,
        }
    }

    /// Returns the folder artifact of identity `id` holding `files`, each by
    /// its path relative to the folder ([`ArtifactData::Files`]): not the
    /// entry artifact, with no fixed path, no links, no extension and the
    /// empty text as its media type (see the [module
    /// documentation](self#a-folder-artifact)).
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::core::artifact::{Artifact, ArtifactData, ArtifactId, RelPath};
    ///
    /// let icons = Artifact::folder(
    ///     ArtifactId::new("compose", "icons"),
    ///     vec![(RelPath::new("check.svg"), b"<svg/>".to_vec())],
    /// );
    /// assert!(matches!(icons.data, ArtifactData::Files(_)));
    /// assert_eq!((icons.media_type.as_str(), icons.extension), ("", None));
    /// ```
    pub fn folder(id: ArtifactId, files: Vec<(RelPath, Vec<u8>)>) -> Artifact {
        Artifact {
            id,
            entry: false,
            media_type: String::new(),
            data: ArtifactData::Files(files),
            fixed_path: None,
            refs: Vec::new(),
            extension: None,
        }
    }

    /// Sets the artifact's file extension to `extension`, written without the
    /// dot (`sty`), and returns it. See [`Artifact::extension`].
    pub fn with_extension(mut self, extension: impl Into<String>) -> Artifact {
        self.extension = Some(extension.into());
        self
    }

    /// Marks the artifact as the entry artifact of its set, and returns it.
    ///
    /// A set holds at most one entry artifact; see the [module
    /// documentation](self#the-entry-artifact) for what happens to a second
    /// one.
    pub fn as_entry(mut self) -> Artifact {
        self.entry = true;
        self
    }

    /// Returns the bytes, or `None` when the content is a [`FileRef`] or the
    /// files of a folder.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match &self.data {
            ArtifactData::Bytes(bytes) => Some(bytes),
            ArtifactData::FileRef(_) | ArtifactData::Files(_) => None,
        }
    }
}

/// One of the two ways an [`ArtifactSet`] can be malformed: two artifacts with
/// one identity, or two entry artifacts.
///
/// [`ArtifactSet::insert`] returns this error instead of adding the artifact.
/// [`Table::absorb`] cannot refuse anything, so it repairs the set as
/// described on each variant and records the violation in
/// [`ArtifactSet::violations`]; [`ArtifactSet::insert_entry_with`] then reports
/// each recorded violation as an Error diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactSetViolation {
    /// Two artifacts have the same identity. The artifact already in the set
    /// is kept and the later one is dropped.
    DuplicateId(ArtifactId),
    /// A second artifact is marked as the entry artifact. The first keeps that
    /// role, and the second is kept as an ordinary artifact.
    SecondEntry {
        /// The artifact that stays the entry artifact.
        first: ArtifactId,
        /// The artifact refused the role of entry artifact.
        second: ArtifactId,
    },
}

impl fmt::Display for ArtifactSetViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactSetViolation::DuplicateId(id) => {
                write!(f, "two different files of an output must not share the identity `{id}`")
            }
            ArtifactSetViolation::SecondEntry { first, second } => write!(
                f,
                "an output has only one main file, `{first}`, so `{second}` must not also be \
                 marked as the main file"
            ),
        }
    }
}

impl std::error::Error for ArtifactSetViolation {}

/// An artifact set: the artifacts of one output, at most one of which is the
/// entry artifact, the one that represents the output as a whole.
///
/// This is the value an output's table holds
/// ([`OutputDef`]), so producing an output
/// means writing one of these; see the [module documentation](self). The set
/// keeps its artifacts in identity order, the order of [`ArtifactSet::iter`].
///
/// It is a [`Table`]: a stage that runs once per tree may write one table
/// piece per tree, and the runner combines them ([`Table::absorb`]). The
/// standard backends' final stages run once for the whole document and make
/// the finished set with [`ArtifactSet::insert_entry_with`].
///
/// # Examples
///
/// ```
/// use flm_core::core::artifact::{Artifact, ArtifactId, ArtifactSet, ArtifactSetViolation};
///
/// let page = |name: &str| {
///     Artifact::bytes(ArtifactId::new("html", name), "text/html", b"<p>x</p>".to_vec())
/// };
///
/// let mut set = ArtifactSet::new();
/// set.insert(page("index").as_entry()).unwrap();
/// set.insert(page("chapter-1")).unwrap();
///
/// assert_eq!(set.len(), 2);
/// assert_eq!(set.entry().map(|a| a.id.name.as_str()), Some("index"));
///
/// // A second entry is refused, and the set is left as it was.
/// let refused = set.insert(page("chapter-2").as_entry()).unwrap_err();
/// assert!(matches!(refused, ArtifactSetViolation::SecondEntry { .. }));
/// assert_eq!(set.len(), 2);
/// ```
#[derive(Debug, Clone, Default)]
pub struct ArtifactSet {
    artifacts: BTreeMap<ArtifactId, Artifact>,
    entry: Option<ArtifactId>,
    violations: Vec<ArtifactSetViolation>,
}

impl ArtifactSet {
    /// Returns an empty set: no artifact and no entry artifact.
    pub fn new() -> ArtifactSet {
        ArtifactSet::default()
    }

    /// Adds `artifact` to the set.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactSetViolation::DuplicateId`] when the set already
    /// holds an artifact of that identity, even an equal one, and
    /// [`ArtifactSetViolation::SecondEntry`] when the artifact is marked as
    /// the entry artifact and the set already has one. In both cases the set
    /// is left unchanged.
    pub fn insert(&mut self, artifact: Artifact) -> Result<(), ArtifactSetViolation> {
        if self.artifacts.contains_key(&artifact.id) {
            return Err(ArtifactSetViolation::DuplicateId(artifact.id));
        }
        if artifact.entry {
            if let Some(first) = &self.entry {
                return Err(ArtifactSetViolation::SecondEntry {
                    first: first.clone(),
                    second: artifact.id,
                });
            }
            self.entry = Some(artifact.id.clone());
        }
        self.artifacts.insert(artifact.id.clone(), artifact);
        Ok(())
    }

    /// Adds an output's entry artifact `entry`, then every artifact of each
    /// set in `beside`, in order (the files the output's stages write for the
    /// entry artifact to link to), and returns one Error diagnostic for every
    /// violation the set then holds.
    ///
    /// The stage that finishes an output (the backend's final stage or a
    /// host's template stage) makes its set with this function. The artifacts of `beside` are combined as [`Table::absorb`]
    /// combines table pieces: an artifact equal to one already in the set is
    /// dropped, and a repeated identity or a second entry artifact is recorded
    /// among the set's [violations](ArtifactSet::violations). Every recorded
    /// violation, including those the sets of `beside` already held, is then
    /// returned as one Error diagnostic ([`DuplicateArtifact`],
    /// [`SecondEntryArtifact`]), which the caller adds to its stage's
    /// diagnostics. See the [module
    /// documentation](self#making-an-outputs-set-and-reporting-a-malformed-one).
    ///
    /// # Errors
    ///
    /// Returns the violation of [`ArtifactSet::insert`] when `entry` itself
    /// cannot be added; nothing of `beside` is added then.
    #[must_use = "the returned Error diagnostics report the violations of the artifact set; \
                  the caller must add them to its diagnostics"]
    pub fn insert_entry_with(
        &mut self,
        entry: Artifact,
        beside: &[&ArtifactSet],
    ) -> Result<Vec<FlmDiagnostic>, ArtifactSetViolation> {
        self.insert(entry)?;
        for set in beside {
            self.absorb((*set).clone());
        }
        let mut reports = Vec::new();
        for violation in &self.violations {
            let at = nowhere(FlmOrigin::synthetic(REPORT_LABEL));
            reports.push(match violation {
                ArtifactSetViolation::DuplicateId(id) => {
                    Diagnostic::error(DuplicateArtifact { id: id.to_string() }, at)
                }
                ArtifactSetViolation::SecondEntry { first, second } => Diagnostic::error(
                    SecondEntryArtifact { first: first.to_string(), second: second.to_string() },
                    at,
                ),
            });
        }
        Ok(reports)
    }

    /// Returns the entry artifact, or `None` when no artifact of the set is
    /// marked as the entry artifact.
    pub fn entry(&self) -> Option<&Artifact> {
        self.entry.as_ref().and_then(|id| self.artifacts.get(id))
    }

    /// Returns the artifact of identity `id`, or `None` when the set holds
    /// none.
    pub fn get(&self, id: &ArtifactId) -> Option<&Artifact> {
        self.artifacts.get(id)
    }

    /// Returns every artifact of the set, in identity order.
    pub fn iter(&self) -> impl Iterator<Item = &Artifact> + '_ {
        self.artifacts.values()
    }

    /// Returns the number of artifacts in the set.
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    /// Returns whether the set holds no artifact.
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    /// Returns the violations recorded while table pieces of this set were
    /// combined, in the order they were found.
    ///
    /// [`ArtifactSet::insert_entry_with`] reports each one as an Error
    /// diagnostic; see [`ArtifactSetViolation`]. An artifact refused by
    /// [`ArtifactSet::insert`] is not recorded here, since that call returned
    /// the violation to its caller.
    pub fn violations(&self) -> &[ArtifactSetViolation] {
        &self.violations
    }
}

impl Table for ArtifactSet {
    /// Combines the table piece `piece` into this set, keeping the violations
    /// the table piece had already recorded.
    ///
    /// An artifact equal in every field to one the set holds is dropped, and
    /// nothing is recorded: it is the same artifact contributed twice.
    /// Otherwise a second entry artifact becomes an ordinary artifact, and an
    /// artifact with a repeated identity is dropped; either way the violation
    /// is recorded in [`ArtifactSet::violations`], because this operation
    /// cannot fail.
    fn absorb(&mut self, piece: ArtifactSet) {
        self.violations.extend(piece.violations);
        for (_, mut artifact) in piece.artifacts {
            if self.artifacts.get(&artifact.id) == Some(&artifact) {
                continue;
            }
            if artifact.entry {
                if let Some(first) = &self.entry {
                    self.violations.push(ArtifactSetViolation::SecondEntry {
                        first: first.clone(),
                        second: artifact.id.clone(),
                    });
                    artifact.entry = false;
                }
            }
            if let Err(violation) = self.insert(artifact) {
                self.violations.push(violation);
            }
        }
    }

    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        let artifacts = self
            .artifacts
            .values()
            .map(|artifact| {
                SerialValue::Map(vec![
                    ("id".to_string(), SerialValue::Str(artifact.id.to_string())),
                    ("entry".to_string(), SerialValue::Bool(artifact.entry)),
                    ("media_type".to_string(), SerialValue::Str(artifact.media_type.clone())),
                    (
                        "data".to_string(),
                        match &artifact.data {
                            ArtifactData::Bytes(bytes) => SerialValue::Bytes(bytes.clone()),
                            ArtifactData::FileRef(file) => SerialValue::Str(file.location.clone()),
                            ArtifactData::Files(files) => SerialValue::Map(
                                files
                                    .iter()
                                    .map(|(path, bytes)| (path.to_string(), SerialValue::Bytes(bytes.clone())))
                                    .collect(),
                            ),
                        },
                    ),
                ])
            })
            .collect();
        SerialValue::Map(vec![
            ("artifacts".to_string(), SerialValue::List(artifacts)),
            (
                "violations".to_string(),
                SerialValue::List(
                    self.violations.iter().map(|v| SerialValue::Str(v.to_string())).collect(),
                ),
            ),
        ])
    }

    fn piece_class(&self) -> PieceClass {
        if self.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

/// The label of the source at which the reports of a malformed set are
/// located: they concern the files of an output, which have no place in the
/// document.
const REPORT_LABEL: &str = "the files of the output";

/// Condition `flm.artifacts.duplicate`, an Error: two different files of an
/// output were given one identity, so one of them is missing from the output
/// ([`ArtifactSetViolation::DuplicateId`]).
///
/// [`ArtifactSet::insert_entry_with`] reports it. Recovery: the first file is
/// kept.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.artifacts.duplicate",
    message = "two different files of the output must not share the identity `{id}`"
)]
pub struct DuplicateArtifact {
    /// The shared identity, `owner:name`.
    pub id: String,
}

/// Condition `flm.artifacts.second-entry`, an Error: two files of an output
/// are both marked as the entry artifact, the file that represents the whole
/// output ([`ArtifactSetViolation::SecondEntry`]).
///
/// [`ArtifactSet::insert_entry_with`] reports it. Recovery: the first file
/// keeps that role, and the second is kept as an ordinary file.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.artifacts.second-entry",
    message = "an output has only one main file, `{first}`, so `{second}` must not also be \
               marked as the main file"
)]
pub struct SecondEntryArtifact {
    /// The identity of the file that stays the main file, `owner:name`.
    pub first: String,
    /// The identity of the file refused that role, `owner:name`.
    pub second: String,
}

/// What an [`ArtifactNamer`] is asked to name: an artifact's identity and the
/// facts about it that are known before its bytes are made.
///
/// A host's write stage builds one from each [`Artifact`] it places; a
/// producer that needs the path of its own artifact during the run (the
/// `graphics` feature's copy of an image, the recomposed LaTeX's style file, a
/// compose template's own files) builds one from what it is about to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactRef<'a> {
    /// The artifact's identity.
    pub id: &'a ArtifactId,
    /// Whether the artifact is its output's entry artifact.
    pub entry: bool,
    /// The artifact's media type, such as `text/html`.
    pub media_type: &'a str,
    /// The file extension the producer gives (without the dot), when it gives
    /// one; the namer derives one from the media type otherwise.
    pub extension: Option<&'a str>,
    /// The output the artifact is placed under, when the caller knows it: the
    /// host's write stage does; a producer naming its own artifact during the
    /// run gives `None`.
    pub output: Option<&'a str>,
    /// Whether the artifact is a folder artifact ([`ArtifactData::Files`]; see
    /// the [module documentation](self#a-folder-artifact)). The namer then
    /// names a folder: a path with no extension (and no trailing slash), under
    /// which the host writes each of the folder's files with its own path.
    /// [`extension`](ArtifactRef::extension) is `None` and
    /// [`media_type`](ArtifactRef::media_type) the empty text for a folder.
    pub folder: bool,
}

/// One output of a [`NamingScope`]: its name and the file extension of its
/// entry artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedOutput {
    /// The output's name, such as `html`.
    pub name: OutputName,
    /// The file extension of the output's entry artifact, without the dot, as
    /// the registry declares it
    /// ([`OutputEntry::extension`](crate::core::registry::OutputEntry::extension)).
    pub extension: String,
}

/// The outputs a command produces: what the paths of its files may depend on.
///
/// An [`ArtifactNamer`] receives it at every call, so that a path can depend on
/// the outputs of the command (for instance on their extensions) while the
/// namer stays a function of its arguments: the namer is contributed when the
/// registry is built, and the outputs of a command are known only once a plan
/// is made. Every caller of one run builds it the same way,
/// [`NamingScope::of`] over the registry and the plan's demands, so it is the
/// same for every call of the run.
///
/// # Examples
///
/// ```
/// use flm_core::core::artifact::{NamingScope, ScopedOutput};
///
/// let scope = NamingScope {
///     outputs: vec![ScopedOutput { name: "html".into(), extension: "html".to_string() }],
/// };
/// assert_eq!(scope.outputs[0].extension, "html");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamingScope {
    /// Each output's name and the file extension of its entry artifact, in the
    /// registry's order.
    pub outputs: Vec<ScopedOutput>,
}

impl NamingScope {
    /// Returns the outputs of `registry` whose capability is among `demands`
    /// (a plan's demands, [`Plan::demands`](crate::core::plan::Plan::demands)),
    /// each with the extension of its entry artifact, in the registry's order.
    /// Nothing demanded gives an empty scope.
    pub fn of(registry: &FlmRegistry, demands: &BTreeSet<CapName>) -> NamingScope {
        NamingScope {
            outputs: registry
                .outputs()
                .filter(|output| demands.contains(&output.def.capability))
                .map(|output| ScopedOutput {
                    name: output.def.name.clone(),
                    extension: output.def.entry.extension.clone(),
                })
                .collect(),
        }
    }
}

/// The naming service: the service a host supplies to decide where each
/// artifact is written, as a function from what an artifact is
/// ([`ArtifactRef`]) and the outputs of the command ([`NamingScope`]) to a
/// relative path.
///
/// The path must depend only on the arguments and on the namer's own
/// configuration, never on which artifacts exist or on how far a run has
/// progressed, so that a producer can write a link to an artifact before that
/// artifact's bytes are made. The scope is the same for every call of one run
/// ([`NamingScope::of`] over the registry and the plan's demands), so a
/// producer and the host's write stage get the same path for one artifact. A
/// host that writes files uses the namer to turn the artifacts of an
/// [`ArtifactSet`] into paths, except for an artifact that has a
/// [`fixed_path`](Artifact::fixed_path). A stage may also call it while a run
/// is under way: the `graphics` feature's publish stage computes the address
/// of a copied image this way, the recomposed LaTeX names its style file, and
/// a compose template's composer names the files the template ships.
///
/// This library ships no implementation. A host contributes its naming scheme
/// to the service slot `flm.artifacts.namer`
/// ([`NAMER_SLOT`](crate::stdcore::backends::lowering::NAMER_SLOT)), which the
/// standard `lowering` feature declares; see [`crate::core::services`] for
/// service slots. When no host contributes a namer, the slot is empty, and a
/// stage that needs the path of an artifact cannot obtain one.
pub trait ArtifactNamer: Send + Sync {
    /// Returns the relative path at which the artifact `artifact` is written,
    /// for a command producing the outputs of `scope`. For a folder artifact
    /// ([`ArtifactRef::folder`]), it is the path of the folder, with no
    /// extension and no trailing slash.
    fn url_of(&self, artifact: &ArtifactRef<'_>, scope: &NamingScope) -> RelPath;
}

/// Returns the path at which a host writes the artifact `artifact`, relative to the place
/// where it writes the output: the artifact's [`fixed_path`](Artifact::fixed_path) when it
/// has one, else the path that the naming service `namer` gives it for a command producing
/// the outputs of `scope`.
///
/// `output` is the declaration of the output whose artifact set holds the artifact, when
/// the caller knows it. The namer then receives the output's name
/// ([`ArtifactRef::output`]), and an entry artifact is named with the file extension that
/// the output declares for its entry artifact
/// ([`OutputEntry::extension`](crate::core::registry::OutputEntry::extension)), whatever
/// extension the artifact gives itself. Every other artifact, and an entry artifact when
/// `output` is `None`, is named with the extension it gives ([`Artifact::extension`]). A
/// folder artifact is named as a folder ([`ArtifactRef::folder`]); each of its files is
/// written under that path with its own path within the folder.
///
/// This is the one statement of where a file of an output is written: a host's write stage
/// places each artifact with it, and a stage that hands the files of an output to another
/// program asks it for the path each file must have so that the links between the files
/// hold. Both build `scope` with [`NamingScope::of`] over the registry and the plan's
/// demands, so they obtain the same path.
///
/// # Examples
///
/// ```
/// use flm_core::core::artifact::{
///     path_of, Artifact, ArtifactId, ArtifactNamer, ArtifactRef, NamingScope, RelPath,
/// };
/// use flm_core::core::registry::OutputDef;
///
/// /// Names an entry artifact `paper.EXT` and any other `paper-NAME.EXT`.
/// struct Paper;
/// impl ArtifactNamer for Paper {
///     fn url_of(&self, artifact: &ArtifactRef<'_>, _: &NamingScope) -> RelPath {
///         let extension = artifact.extension.unwrap_or("bin");
///         match artifact.entry {
///             true => RelPath::new(format!("paper.{extension}")),
///             false => RelPath::new(format!("paper-{}.{extension}", artifact.id.name)),
///         }
///     }
/// }
///
/// let output = OutputDef::new("html", "flm.artifacts.html", "flm.artifacts.html", "text/html", "html");
/// let page = Artifact::bytes(ArtifactId::new("html", "page"), "text/html", Vec::new()).as_entry();
/// let figure = Artifact::bytes(ArtifactId::new("graphics", "fig"), "image/png", Vec::new())
///     .with_extension("png");
/// let scope = NamingScope::default();
/// assert_eq!(path_of(&page, Some(&output), &Paper, &scope).as_str(), "paper.html");
/// assert_eq!(path_of(&figure, Some(&output), &Paper, &scope).as_str(), "paper-fig.png");
/// ```
pub fn path_of(
    artifact: &Artifact,
    output: Option<&OutputDef>,
    namer: &dyn ArtifactNamer,
    scope: &NamingScope,
) -> RelPath {
    // See decision rationale at [§flm-dd-dr:ext-artifact-set-read].
    if let Some(path) = &artifact.fixed_path {
        return path.clone();
    }
    let extension = match output {
        Some(output) if artifact.entry => Some(output.entry.extension.as_str()),
        _ => artifact.extension.as_deref(),
    };
    let named = ArtifactRef {
        id: &artifact.id,
        entry: artifact.entry,
        media_type: &artifact.media_type,
        extension,
        output: output.map(|output| output.name.as_str()),
        folder: matches!(artifact.data, ArtifactData::Files(_)),
    };
    namer.url_of(&named, scope)
}

/// Returns the media type of a file by its extension `extension`, written
/// without the dot, or `None` for an extension this table does not list.
///
/// The table lists `html`, `css`, `js`, `txt`, `json`, `svg`, `png`, `jpg` and
/// `jpeg`, `gif`, `webp`, `pdf`, `tex`, `sty`, `bib`, `woff`, `woff2` and
/// `ico`; the extension is compared without regard to case. A compose
/// template's file whose description gives no media type takes the one this
/// table gives.
///
/// # Examples
///
/// ```
/// use flm_core::core::artifact::media_type_of;
///
/// assert_eq!(media_type_of("css"), Some("text/css"));
/// assert_eq!(media_type_of("PNG"), Some("image/png"));
/// assert_eq!(media_type_of("webmanifest"), None);
/// ```
pub fn media_type_of(extension: &str) -> Option<&'static str> {
    Some(match extension.to_ascii_lowercase().as_str() {
        "html" => "text/html",
        "css" => "text/css",
        "js" => "text/javascript",
        "txt" => "text/plain",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "tex" | "sty" => "application/x-latex",
        "bib" => "application/x-bibtex",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ico" => "image/vnd.microsoft.icon",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(name: &str) -> Artifact {
        Artifact::bytes(ArtifactId::new("text", name), "text/plain", b"x".to_vec())
    }

    #[test]
    fn a_second_entry_is_refused_at_insert_and_recorded_at_absorb() {
        let mut set = ArtifactSet::new();
        set.insert(artifact("document").as_entry()).unwrap();
        let refused = set.insert(artifact("other").as_entry()).unwrap_err();
        assert!(matches!(refused, ArtifactSetViolation::SecondEntry { .. }));
        let mut piece = ArtifactSet::new();
        piece.insert(artifact("also").as_entry()).unwrap();
        set.absorb(piece);
        assert_eq!(set.violations().len(), 1);
        assert_eq!(set.entry().map(|a| a.id.name.as_str()), Some("document"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn the_same_artifact_twice_is_one_and_two_under_one_identity_are_reported() {
        let mut first = ArtifactSet::new();
        first.insert(artifact("copy")).unwrap();
        let mut again = ArtifactSet::new();
        again.insert(artifact("copy")).unwrap();
        let mut other = ArtifactSet::new();
        other.insert(Artifact::bytes(ArtifactId::new("text", "copy"), "text/plain", b"y".to_vec())).unwrap();
        let mut set = ArtifactSet::new();
        let reports = set.insert_entry_with(artifact("document").as_entry(), &[&first, &again]).unwrap();
        assert!(reports.is_empty());
        assert_eq!(set.len(), 2);
        let mut set = ArtifactSet::new();
        let reports = set.insert_entry_with(artifact("document").as_entry(), &[&first, &other]).unwrap();
        assert_eq!(set.violations(), [ArtifactSetViolation::DuplicateId(ArtifactId::new("text", "copy"))]);
        assert_eq!(reports.len(), 1);
        assert_eq!(set.get(&ArtifactId::new("text", "copy")).and_then(Artifact::as_bytes), Some(&b"x"[..]));
    }

    /// Writes what it is asked: `NAME.EXT`, `NAME/` for a folder, `!` before the name of an
    /// entry artifact and `@OUTPUT/` before an artifact of a known output.
    struct Echo;

    impl ArtifactNamer for Echo {
        fn url_of(&self, artifact: &ArtifactRef<'_>, _: &NamingScope) -> RelPath {
            let output = artifact.output.map(|output| format!("@{output}/")).unwrap_or_default();
            let entry = if artifact.entry { "!" } else { "" };
            let tail = match (artifact.folder, artifact.extension) {
                (true, _) => String::from("/"),
                (false, Some(extension)) => format!(".{extension}"),
                (false, None) => String::new(),
            };
            RelPath::new(format!("{output}{entry}{}{tail}", artifact.id.name))
        }
    }

    #[test]
    fn the_path_of_an_artifact_is_its_fixed_path_else_the_namer_s_answer() {
        let output = OutputDef::new("paper", "x.paper", "x.paper", "application/pdf", "pdf");
        let scope = NamingScope::default();
        let path = |artifact: &Artifact, output: Option<&OutputDef>| path_of(artifact, output, &Echo, &scope).to_string();
        let entry = Artifact::bytes(ArtifactId::new("x", "doc"), "text/plain", Vec::new()).with_extension("txt").as_entry();
        // An entry artifact takes its output's extension; without an output, its own.
        assert_eq!(path(&entry, Some(&output)), "@paper/!doc.pdf");
        assert_eq!(path(&entry, None), "!doc.txt");
        let other = Artifact::bytes(ArtifactId::new("x", "fig"), "image/png", Vec::new()).with_extension("png");
        assert_eq!(path(&other, Some(&output)), "@paper/fig.png");
        let folder = Artifact::folder(ArtifactId::new("x", "icons"), vec![(RelPath::new("a.svg"), Vec::new())]);
        assert_eq!(path(&folder, None), "icons/");
        let mut fixed = other.clone();
        fixed.fixed_path = Some(RelPath::new("fixed/fig.png"));
        assert_eq!(path(&fixed, Some(&output)), "fixed/fig.png");
    }
}
