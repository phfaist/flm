//! The exports: two outputs that write the records of the citation keys a document cites
//! into one file, in BibTeX ([`BIBTEX`], `flm render -p bibtex`) or in CSL-JSON
//! ([`CSL_JSON`], `flm render -p csl-json`).
//!
//! The feature `citations-standard` ([`crate::stdpkg::citations_standard`]) registers the
//! two outputs, each with a workflow of the same name, and three stages:
//!
//! - the stage [`RECORDS_STAGE`] reads the cited citation keys from the library's table
//!   [`KEYS`] and obtains the record of each, in order of first citation, through the
//!   standard provider ([`StandardProvider`]), and so through the same cache file, into
//!   the table [`RECORDS`] ([`CitedRecords`]);
//! - the stages [`BIBTEX_STAGE`] and [`CSL_JSON_STAGE`] each read [`RECORDS`] and write one
//!   file, the only artifact of their output's artifact set ([`BIBTEX_DOCUMENT`],
//!   [`CSL_JSON_DOCUMENT`]).
//!
//! In both files a record is identified by its citation key as the author wrote it. The
//! recomposed LaTeX output writes each citation key the same way, in its citation command
//! `\flmCite{…}`. A work cited under two spellings of its citation key therefore has two
//! records holding the same details. A cited
//! citation key with no record is reported as [`NoRecord`] and left out of the file.

use std::fmt;
use std::sync::Arc;

use autocitefetch::CslValue;
use flm_core::core::artifact::{Artifact, ArtifactId, ArtifactSet};
use flm_core::core::data::store::StageOut;
use flm_core::core::data::table::{PieceClass, ProjectCx, Table, TableName};
use flm_core::core::plan::PlanCx;
use flm_core::core::run::StageCx;
use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_core::lang::{FlmOrigin, FlmSpan};
use flm_core::stdcore::features::citations::KEYS;
use flm_core::techy::error::{Diagnostic, DiagnosticInfo};
use flm_core::techy::serialize::SerialValue;
use flm_core::techy::source::{Source, SourceSpan};

use super::format::{TextPart, text_parts, with_family_names};
use super::provider::StandardProvider;

/// The table of the records of the cited citation keys, `flm.citations-standard.records`
/// ([`CitedRecords`]), written by the stage [`RECORDS_STAGE`] and read by the stages that
/// write the exports.
pub const RECORDS: TableName<CitedRecords> = TableName::new("flm.citations-standard.records");

/// The name of the stage `flm.citations-standard.records`, which fills the table
/// [`RECORDS`] (the table has the same name): it obtains the record of every cited citation
/// key through the standard provider.
pub const RECORDS_STAGE: &str = "flm.citations-standard.records";

/// The name of the output `bibtex`, a BibTeX file of the records of the cited citation keys,
/// and of the workflow that produces it.
pub const BIBTEX: &str = "bibtex";

/// The name of the output `csl-json`, a CSL-JSON file of the records of the cited citation
/// keys, and of the workflow that produces it.
pub const CSL_JSON: &str = "csl-json";

/// The media type of a BibTeX file, `application/x-bibtex`.
pub const BIBTEX_MEDIA_TYPE: &str = "application/x-bibtex";

/// The media type of a CSL-JSON file, `application/vnd.citationstyles.csl+json`.
pub const CSL_JSON_MEDIA_TYPE: &str = "application/vnd.citationstyles.csl+json";

/// The table of the output `bibtex`, `flm.output.bibtex.document`: an artifact set whose
/// only artifact is the BibTeX file, with the file extension `bib`.
pub const BIBTEX_DOCUMENT: TableName<ArtifactSet> = TableName::new("flm.output.bibtex.document");

/// The table of the output `csl-json`, `flm.output.csl-json.document`: an artifact set
/// whose only artifact is the CSL-JSON file, with the file extension `json`.
pub const CSL_JSON_DOCUMENT: TableName<ArtifactSet> = TableName::new("flm.output.csl-json.document");

/// The name of the stage that writes the table [`BIBTEX_DOCUMENT`] from the table
/// [`RECORDS`], `flm.citations-standard.bibtex`.
pub const BIBTEX_STAGE: &str = "flm.citations-standard.bibtex";

/// The name of the stage that writes the table [`CSL_JSON_DOCUMENT`] from the table
/// [`RECORDS`], `flm.citations-standard.csl-json`.
pub const CSL_JSON_STAGE: &str = "flm.citations-standard.csl-json";

/// The content of the table [`RECORDS`]: every citation key that the document cites, in
/// order of first citation, with the place of its first citation and its record, or the
/// reason it has none.
///
/// A program reads it after a run that included the stage [`RECORDS_STAGE`], with
/// [`Run::table`](flm_core::core::run::Run::table) and the key [`RECORDS`], then
/// [`iter`](CitedRecords::iter), for example to write the records in a format of its own.
#[derive(Debug, Clone, Default)]
pub struct CitedRecords {
    records: Vec<(String, FlmSpan, Result<CslValue, String>)>,
}

impl CitedRecords {
    /// Returns every citation key, in order of first citation, with the place of its first
    /// citation and its record, or the reason it has none.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &FlmSpan, Result<&CslValue, &str>)> {
        self.records.iter().map(|(key, span, record)| (key.as_str(), span, record.as_ref().map_err(String::as_str)))
    }
}

impl Table for CitedRecords {
    /// Adds the citation keys of the table piece `piece` that this table does not hold yet,
    /// after this table's own.
    fn absorb(&mut self, piece: CitedRecords) {
        for record in piece.records {
            if !self.records.iter().any(|(key, _, _)| *key == record.0) {
                self.records.push(record);
            }
        }
    }

    /// Returns the serialized form of the table: a list in order of first citation, with
    /// `{key, id}` (the record's `id`) for a citation key with a record, and
    /// `{key, missing}` (the reason) for one without.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.iter()
                .map(|(key, _, record)| {
                    let (field, value) = match record {
                        Ok(record) => ("id", record.get("id").and_then(CslValue::as_str).unwrap_or_default().to_string()),
                        Err(why) => ("missing", why.to_string()),
                    };
                    SerialValue::Map(vec![
                        (String::from("key"), SerialValue::Str(key.to_string())),
                        (String::from(field), SerialValue::Str(value)),
                    ])
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.records.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

/// Condition `flm.citations-standard.no-record`, an Error: a cited citation key has no
/// record to write into an export, because no record source of the provider handles it,
/// or its record could not be obtained. Reported by the stage that writes the export
/// ([`BIBTEX_STAGE`], [`CSL_JSON_STAGE`]), at the citation key's first citation; the
/// citation key is left out of the file. A citation key that another citation provider
/// answers has no record here, and is reported too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoRecord {
    /// The citation key, as the author wrote it.
    pub key: String,
    /// The reason there is no record.
    pub detail: String,
}

impl fmt::Display for NoRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the citation key `{}` must have a bibliography record to be exported: {}", self.key, self.detail)
    }
}

impl DiagnosticInfo for NoRecord {
    const IDENTIFIER: &'static str = "flm.citations-standard.no-record";
}

/// Condition `flm.citations-standard.export-failed`, an Error: a stage of the exports
/// ([`RECORDS_STAGE`], [`BIBTEX_STAGE`], [`CSL_JSON_STAGE`]) could not do its work, because a
/// table could not be read or written, or the file could not be added to its artifact set.
/// Reported once by that stage, at no place of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFailed {
    /// The name of the stage.
    pub stage: String,
    /// What failed, in the words of the part that failed.
    pub detail: String,
}

impl fmt::Display for ExportFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the stage `{}` failed: {}", self.stage, self.detail)
    }
}

impl DiagnosticInfo for ExportFailed {
    const IDENTIFIER: &'static str = "flm.citations-standard.export-failed";
}

/// Reports [`ExportFailed`] for the stage `stage` with `detail`, at no place of the
/// document.
fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    let source = Arc::new(Source::new("").with_origin(FlmOrigin::synthetic(stage)));
    let failed = ExportFailed { stage: stage.to_string(), detail: detail.to_string() };
    out.diags.push(Diagnostic::error(failed, SourceSpan::new(&source, 0..0)));
}

/// The stage [`RECORDS_STAGE`]: it obtains the record of every cited citation key through
/// the standard provider ([`StandardProvider::records`]), and writes them into the table
/// [`RECORDS`].
pub(crate) struct RecordsStage {
    provider: Arc<StandardProvider>,
}

impl RecordsStage {
    pub(crate) fn new(provider: Arc<StandardProvider>) -> RecordsStage {
        RecordsStage { provider }
    }
}

/// The reason recorded for a cited citation key that no record source of the provider
/// handles; the library's reason for a citation key that no citation provider claims has
/// the same words.
const NO_SOURCE: &str = "none of the record sources has a record for it";

impl Stage for RecordsStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(RECORDS_STAGE, Effect::Compute, Scope::Document).writes(RECORDS).docs(
            "obtains the record of every citation key that the document cites, in order of first citation, \
             through the citation provider of `citations-standard` and its cache file, into the table \
             `flm.citations-standard.records`",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(KEYS)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let cited = match cx.get(KEYS) {
            Ok(Some(cited)) => cited,
            Ok(None) => return,
            Err(error) => return stage_failed(out, RECORDS_STAGE, error),
        };
        let keys: Vec<&str> = cited.iter().map(|(key, _)| key).collect();
        let records = self.provider.records(&keys);
        let mut table = CitedRecords::default();
        for ((key, span), record) in cited.iter().zip(records) {
            let record = record.map_or_else(|| Err(String::from(NO_SOURCE)), |fetched| fetched.record);
            table.records.push((key.to_string(), span.clone(), record));
        }
        match out.table(RECORDS) {
            Ok(written) => *written = table,
            Err(error) => stage_failed(out, RECORDS_STAGE, error),
        }
    }
}

/// Which export an [`ExportStage`] writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Export {
    /// The BibTeX file.
    Bibtex,
    /// The CSL-JSON file.
    CslJson,
}

impl Export {
    pub(crate) fn stage(self) -> &'static str {
        match self {
            Export::Bibtex => BIBTEX_STAGE,
            Export::CslJson => CSL_JSON_STAGE,
        }
    }

    pub(crate) fn table(self) -> TableName<ArtifactSet> {
        match self {
            Export::Bibtex => BIBTEX_DOCUMENT,
            Export::CslJson => CSL_JSON_DOCUMENT,
        }
    }

    pub(crate) fn media_type(self) -> &'static str {
        match self {
            Export::Bibtex => BIBTEX_MEDIA_TYPE,
            Export::CslJson => CSL_JSON_MEDIA_TYPE,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Export::Bibtex => BIBTEX,
            Export::CslJson => CSL_JSON,
        }
    }

    /// Returns the bytes of the file holding `records`, in order, each identified by its
    /// citation key.
    fn file(self, records: &[(&str, &CslValue)]) -> Vec<u8> {
        match self {
            Export::Bibtex => bibtex_file(records).into_bytes(),
            Export::CslJson => csl_json_file(records),
        }
    }
}

/// The stage that writes one export from the table [`RECORDS`]: the file, as the only
/// artifact of the output's artifact set. A cited citation key with no record is reported
/// as [`NoRecord`] and left out of the file.
pub(crate) struct ExportStage(pub(crate) Export);

impl Stage for ExportStage {
    fn decl(&self) -> StageDecl {
        let export = self.0;
        StageDecl::new(export.stage(), Effect::Compute, Scope::Document)
            .writes(export.table())
            .docs(match export {
                Export::Bibtex => "writes the records of the cited citation keys as a BibTeX file, the only \
                                   artifact of the artifact set `flm.output.bibtex.document`",
                Export::CslJson => "writes the records of the cited citation keys as a CSL-JSON file, the only \
                                    artifact of the artifact set `flm.output.csl-json.document`",
            })
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(RECORDS)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let export = self.0;
        let cited = match cx.get(RECORDS) {
            Ok(Some(cited)) => cited,
            Ok(None) => return,
            Err(error) => return stage_failed(out, export.stage(), error),
        };
        let mut records = Vec::new();
        for (key, span, record) in cited.iter() {
            match record {
                Ok(record) => records.push((key, record)),
                Err(detail) => out
                    .diags
                    .push(Diagnostic::error(NoRecord { key: key.to_string(), detail: detail.to_string() }, span.clone())),
            }
        }
        let id = ArtifactId::new(export.name(), "document");
        let entry = Artifact::bytes(id, export.media_type(), export.file(&records)).as_entry();
        match out.table(export.table()) {
            Ok(set) => match set.insert_entry_with(entry, &[]) {
                Ok(reports) => reports.into_iter().for_each(|report| out.diags.push(report)),
                Err(violation) => stage_failed(out, export.stage(), violation),
            },
            Err(error) => stage_failed(out, export.stage(), error),
        }
    }
}

/// Returns `record` as it is exported under the citation key `key`: a person written with
/// `name` alone given that text as family name ([`with_family_names`]), the field `id` set
/// to the citation key as written, and the field `type` present, `null` where the record
/// has none.
fn exported(key: &str, record: &CslValue) -> CslValue {
    let mut record = with_family_names(record);
    if let Some(fields) = record.as_object_mut() {
        fields.insert(String::from("id"), CslValue::String(key.to_string()));
        fields.entry("type").or_insert(CslValue::Null);
    }
    record
}

/// Returns the CSL-JSON file of `records`: a list of the records, in order, each with its
/// citation key as its `id` ([`exported`]), indented by four spaces and ending with a
/// line break.
fn csl_json_file(records: &[(&str, &CslValue)]) -> Vec<u8> {
    let list: Vec<CslValue> = records.iter().map(|(key, record)| exported(key, record)).collect();
    let mut bytes = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut writer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
    // A list of values read from JSON is written back without fail.
    if serde::Serialize::serialize(&list, &mut writer).is_ok() {
        bytes.push(b'\n');
    }
    bytes
}

/// Returns the BibTeX file of `records`: one BibTeX entry per record, in order, each with
/// its citation key as its BibTeX key, the BibTeX entries separated by an empty line.
fn bibtex_file(records: &[(&str, &CslValue)]) -> String {
    let entries: Vec<String> = records.iter().map(|(key, record)| bibtex_entry(key, record)).collect();
    entries.join("\n")
}

/// Returns the BibTeX entry type of a record of the CSL-JSON type `csl`: `article`, `book`,
/// `inbook`, `inproceedings`, `phdthesis`, `techreport`, `unpublished`, or `misc` for every
/// other type.
fn bibtex_type(csl: &str) -> &'static str {
    match csl {
        "bill" | "book" | "graphic" | "legal_case" | "legislation" | "motion_picture" | "song" => "book",
        "chapter" => "inbook",
        "article" | "article-journal" | "journal-article" | "article-magazine" | "article-newspaper" => "article",
        "thesis" => "phdthesis",
        "manuscript" => "unpublished",
        "paper-conference" => "inproceedings",
        "report" => "techreport",
        _ => "misc",
    }
}

/// Returns the BibTeX entry of `record` under the BibTeX key `key`. The year and the month
/// come from the date `issued`, or from `published` where the record has no `issued`; a
/// list of values, such as a list of ISSNs, is joined by spaces.
///
/// The entry types and the fields are those of the BibTeX export of FLM's earlier Python
/// implementation, with two differences: a record of type `report` is a `@techreport` with
/// an `institution`, where that export wrote a `@book`; and `number` holds the record's
/// `issue`, else its `number`, where that export wrote the two run together. The month is
/// BibTeX's macro for it (`month = mar`), which a bibliography style writes as the month's
/// name. Each field that the record has is written, in the order of the list `fields` in
/// this function's code: as LaTeX text
/// ([`latex_text`]), except the identifiers and the address (`doi`, `eprint`, `arxivid`,
/// `url`), which BibTeX passes to LaTeX unchanged and which are written character for
/// character.
// See decision rationale at [§flm-dd-dr:host-citations-exports]
fn bibtex_entry(key: &str, record: &CslValue) -> String {
    let record = with_family_names(record);
    let csl_type = record.get("type").and_then(CslValue::as_str).unwrap_or_default();
    let text = |name: &str| -> Option<FieldValue> {
        let written = match record.get(name)? {
            CslValue::String(text) if !text.is_empty() => latex_text(text),
            CslValue::Number(number) => number.to_string(),
            CslValue::Array(items) => {
                let items: Vec<String> = items.iter().filter_map(CslValue::as_str).map(latex_text).collect();
                if items.is_empty() {
                    return None;
                }
                items.join(" ")
            }
            _ => return None,
        };
        Some(FieldValue::Braced(written))
    };
    // An identifier or an address, which BibTeX hands to LaTeX as it is: written character
    // for character.
    let verbatim = |name: &str| -> Option<FieldValue> {
        let value = record.get(name).and_then(CslValue::as_str).filter(|value| !value.is_empty())?;
        Some(FieldValue::Braced(value.to_string()))
    };
    let date = date_parts(&record, "issued").or_else(|| date_parts(&record, "published"));
    let publisher_field = match csl_type {
        "thesis" => "school",
        "report" => "institution",
        _ => "publisher",
    };
    let container_field = match csl_type {
        "chapter" | "paper-conference" => "booktitle",
        _ => "journal",
    };
    let fields = [
        ("author", names(&record, &["author"]).map(FieldValue::Braced)),
        ("editor", names(&record, &["editor", "translator"]).map(FieldValue::Braced)),
        ("title", text("title")),
        (container_field, text("container-title")),
        ("series", text("collection-title")),
        ("collection", text("collection-title")),
        ("edition", text("edition")),
        ("chapter", text("chapter-number")),
        ("type", text("genre")),
        ("volume", text("volume")),
        ("number", text("issue").or_else(|| text("number"))),
        ("pages", text("page").or_else(|| text("article-number"))),
        (publisher_field, text("publisher")),
        ("address", text("publisher-place")),
        ("year", date.as_ref().and_then(|parts| parts.first()).map(|year| FieldValue::Braced(year.to_string()))),
        ("month", date.as_ref().and_then(|parts| parts.get(1)).and_then(|month| month_macro(*month)).map(FieldValue::Macro)),
        ("isbn", text("ISBN")),
        ("issn", text("ISSN")),
        ("doi", verbatim("DOI")),
        ("eprint", verbatim("arxivid")),
        ("arxivid", verbatim("arxivid")),
        ("url", verbatim("URL")),
        ("note", text("note")),
        ("keywords", text("keyword")),
        ("language", text("language")),
        ("callNumber", text("call-number")),
        ("archiveLocation", text("archive_location")),
        ("annote", text("annote")),
        ("abstractNote", text("abstract")),
    ];
    let mut entry = format!("@{}{{{key},\n", bibtex_type(csl_type));
    for (field, value) in fields {
        match value {
            Some(FieldValue::Braced(value)) => entry.push_str(&format!("  {field} = {{{value}}},\n")),
            Some(FieldValue::Macro(name)) => entry.push_str(&format!("  {field} = {name},\n")),
            None => {}
        }
    }
    entry.push_str("}\n");
    entry
}

/// The value of a field of a BibTeX entry, as it is written after `=`.
enum FieldValue {
    /// A text, written between braces.
    Braced(String),
    /// The name of a macro BibTeX defines, such as a month's (`mar`), written bare.
    Macro(&'static str),
}

/// Returns the macro that BibTeX defines for the month numbered `month`, from 1 to 12:
/// `jan` to `dec`; `None` for any other number.
fn month_macro(month: i64) -> Option<&'static str> {
    const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    usize::try_from(month).ok().and_then(|month| month.checked_sub(1)).and_then(|index| MONTHS.get(index).copied())
}

/// Returns `text` written as LaTeX inside a BibTeX value, split as [`text_parts`] splits it:
/// each literal part with every character special to LaTeX written as its command, so that
/// the value shows the record's characters; each formula as it is, between its two dollar
/// signs, and an empty one as `${}$` (two dollar signs in a row would open a displayed
/// formula).
fn latex_text(text: &str) -> String {
    let mut written = String::new();
    for part in text_parts(text) {
        match part {
            TextPart::Literal(characters) => escape_into(characters, &mut written),
            TextPart::Formula("") => written.push_str("${}$"),
            TextPart::Formula(formula) => {
                written.push('$');
                written.push_str(formula);
                written.push('$');
            }
        }
    }
    written
}

/// Appends `text` to `written`, each character special to LaTeX (`\`, `{`, `}`, `~`, `^`,
/// `%`, `&`, `#`, `_`, `$`) written as the command or the escape that shows it.
fn escape_into(text: &str, written: &mut String) {
    for character in text.chars() {
        match character {
            '\\' => written.push_str("\\textbackslash{}"),
            '{' => written.push_str("\\textbraceleft{}"),
            '}' => written.push_str("\\textbraceright{}"),
            '~' => written.push_str("\\textasciitilde{}"),
            '^' => written.push_str("\\textasciicircum{}"),
            '%' | '&' | '#' | '_' | '$' => {
                written.push('\\');
                written.push(character);
            }
            other => written.push(other),
        }
    }
}

/// Returns the persons of the lists `fields` of `record`, as BibTeX writes them: `Family,
/// Given`, joined by ` and `; a literal name, or a family name with no given name (an
/// institution's), in braces, so that BibTeX reads it as one name. `None` when there is no
/// person.
fn names(record: &CslValue, fields: &[&str]) -> Option<String> {
    let mut written = Vec::new();
    for person in fields.iter().filter_map(|field| record.get(*field)).filter_map(CslValue::as_array).flatten() {
        let part = |name: &str| person.get(name).and_then(CslValue::as_str).filter(|part| !part.is_empty());
        let family: Vec<&str> = [part("non-dropping-particle"), part("family")].into_iter().flatten().collect();
        let family = latex_text(&family.join(" "));
        let name = match (part("literal"), family.is_empty(), part("given")) {
            (Some(literal), _, _) => format!("{{{}}}", latex_text(literal)),
            (None, false, Some(given)) => format!("{family}, {}", latex_text(given)),
            (None, false, None) => format!("{{{family}}}"),
            (None, true, Some(given)) => latex_text(given),
            (None, true, None) => continue,
        };
        written.push(name);
    }
    (!written.is_empty()).then(|| written.join(" and "))
}

/// Returns the numbers of the date `name` of `record` (year, month, day), from the first
/// date of its `date-parts`; `None` when there is none.
fn date_parts(record: &CslValue, name: &str) -> Option<Vec<i64>> {
    let first = record.get(name)?.get("date-parts")?.get(0)?.as_array()?;
    let parts: Vec<i64> = first
        .iter()
        .map_while(|part| part.as_i64().or_else(|| part.as_str().and_then(|text| text.parse().ok())))
        .collect();
    (!parts.is_empty()).then_some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_formula_is_written_as_it_is() {
        assert_eq!(latex_text("a $x^{2$ b"), "a $x^{2$ b");
        assert_eq!(latex_text("$$ & $\\alpha_1$, 5$"), "${}$ \\& $\\alpha_1$, 5\\$");
    }

    #[test]
    fn an_exported_record_is_named_by_its_key_and_states_its_type() {
        let record = serde_json::json!({"id": "bib:x", "author": [{"name": "A Group"}]});
        let exported = exported("BIB:x", &record);
        assert_eq!(exported["id"], "BIB:x");
        assert_eq!(exported["type"], CslValue::Null);
        assert_eq!(exported["author"][0]["family"], "A Group");
    }
}
