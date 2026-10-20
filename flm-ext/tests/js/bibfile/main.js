// tests/js/bibfile/main.js: a citation provider reading the entries of the
// cited keys from a JSON file beside the document (`bib.json`, or the file
// the configuration `file` names), through the host service `files`; a
// service slot `bibfile.notes` the package `_` declares and the package
// `extra` fills, read by a document stage that reports each note; and the
// workflow `noted`, which runs that stage.
import { StageDecl, Effect, Scope } from "flm:core/stage";
import { Arity } from "flm:core/services";
import { PROVIDERS_SLOT } from "flm:stdcore/features/citations";

const NOTES = "bibfile.notes";

const SETTINGS = {
  config: {
    docs: "Where the entries are read from.",
    schema: { type: "object", properties: { file: { type: "string", description: "The JSON file beside the document." } } },
  },
};

// The entry of one record: the author, the title emphasized, the year.
function pieces(record) {
  return [
    { text: `${record.author}, ` },
    { emphasized: [{ text: record.title }] },
    { text: ` (${record.year})` },
  ];
}

// The provider: every key in one call. A key the file does not hold is "not
// mine" (`null`); two spellings of one key are one work (the identity is the
// key in lower case).
function provider(file) {
  return {
    name: "bibfile.json",
    answer_all(requests, cx) {
      const text = cx.host("files").read_text("document", file);
      if (text === null)
        return requests.map(() => ({ error: `the file \`${file}\` beside the document must hold the bibliography entries` }));
      const records = JSON.parse(text);
      return requests.map(({ key }) => {
        const record = records[key.toLowerCase()];
        if (record === undefined) return null;
        return { entry: cx.build_entry(pieces(record)), identity: key.toLowerCase() };
      });
    },
  };
}

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    host: ["files"],
    flm: { workflows: { noted: { extends: "text", demands: ["flm.validated", NOTES], docs: "The text output, with the notes reported." } } },
    features: {
      bibfile: {
        title: "Bibliography file",
        settings: SETTINGS,
        register(b, settings) {
          b.peer_slot(PROVIDERS_SLOT, provider(settings.file ?? "bib.json"));
          b.service_slot(NOTES, Arity.OrderedMany, "notes other packages add, each `{ note }`");
          b.table(NOTES, "the notes, one item per contribution");
          b.stage({
            decl: () => new StageDecl("bibfile.notes", Effect.Compute, Scope.Document).writes(NOTES),
            run(cx, out) {
              for (const { note } of cx.services(NOTES)) {
                out.table(NOTES).push({ note });
                out.diags({ id: "bibfile.note", severity: "note", message: `a package notes: ${note}` });
              }
            },
          });
        },
      },
    },
  });
  ext.package("extra", {
    uses: [".#_"],
    features: {
      extra: {
        title: "Extra notes",
        register(b) {
          b.contribute_to(NOTES, { note: "the extra package was used" });
        },
      },
    },
  });
}
