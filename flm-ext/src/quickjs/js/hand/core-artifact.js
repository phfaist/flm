// The hand-written part of `flm:core/artifact`, spliced into the generated
// stub `api/core/artifact.js` by `tests/api_surface.rs`: the classes of the
// `flm-core` types `ArtifactId` and `Artifact`, re-exported as plain data.
//
// The shapes, in argument order:
//
//   new ArtifactId(owner, name)
//   Artifact.bytes(id, media_type, content)    `content` a text, `{ text }` or `{ base64 }`
//   Artifact.folder(id, [[path, content], …])  each file's content likewise
//   artifact.with_extension(extension)         answers the same instance
//   artifact.as_entry()                        answers the same instance
//   media_type_of(extension)                   a text or `null`
//
// A content is a text or `{ text: text }` (the UTF-8 bytes of the text), or
// `{ base64: text }` (bytes of any kind, in base64 with the standard alphabet
// and padding), the forms in which the host service `programs` takes and
// gives a file. An artifact is written (`toJSON`) as `{ id: { owner, name },
// media_type, data: { bytes: content } | { files: [[path, content], …] },
// entry, extension }`, the item `out.table(name).push(artifact)` takes on a
// table declared with `StandardTable.ArtifactSet`, the content passed on as
// given.
//
// A stage that declares a read of such a table gets its files with
// `cx.get(name).projection`: `{ artifacts: [artifact, …], violations: [text,
// …] }`, each artifact a plain object of seven fields, `{ id, media_type,
// entry, data, fixed_path, refs, extension }` (the form above with the two
// fields `toJSON` leaves out: the file's fixed path or `null`, and the
// artifacts it refers to), each content `{ text }` or `{ base64 }`, with one
// more field, `path`, where the host writes the file (or `null`). An artifact
// read can be pushed as it is: a push ignores `path`.

// @hand export ArtifactId
// An artifact's identity: `new ArtifactId(owner, name)`, as Rust's
// `ArtifactId::new`.
export class ArtifactId {
  constructor(owner, name) { this.owner = owner; this.name = name; }
  toJSON() { return { owner: this.owner, name: this.name }; }
}
// @hand export Artifact
// One file, or a folder of files, of an output: `Artifact.bytes(id,
// media_type, content)` or `Artifact.folder(id, files)`, then the builder
// methods `with_extension(extension)` and `as_entry()`, as Rust's.
export class Artifact {
  constructor(id, media_type, data) {
    this.id = id;
    this.media_type = media_type;
    this.data = data;
    this.entry = false;
    this.extension = null;
  }
  static bytes(id, media_type, content) { return new Artifact(id, media_type, { bytes: content }); }
  static folder(id, files) { return new Artifact(id, "", { files }); }
  with_extension(extension) { this.extension = extension; return this; }
  as_entry() { this.entry = true; return this; }
  toJSON() { return { id: this.id, media_type: this.media_type, data: this.data, entry: this.entry, extension: this.extension }; }
}
