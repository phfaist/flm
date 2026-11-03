//! The extension `phfpack`: notation and quantity macros for quantum
//! information, written in Rust against the guest client `flm_ext::guest`.
//!
//! The extension holds one package, the package `_` (the package a `use`
//! entry `phfpack` gives), with one feature, `phfqit`. The feature defines
//! macros for bras and kets, norms, operators, gates, groups and entropies;
//! each macro is a *user definition* (a macro defined by substitution: a name,
//! an argument syntax and a template), and every use of one is replaced by its
//! template before any output is written.
//!
//! The `flm` program carries the extension compiled in (the engine `native`);
//! `extensions/rust/build-wasm.sh` builds the same crate as a WebAssembly
//! module (the engine `wasm`). A document uses it by listing `phfpack` under
//! `use` in its front matter:
//!
//! ```yaml
//! ---
//! flm:
//!   use: [phfpack]
//! ---
//! ```
//!
//! # What an author may write
//!
//! The macros are written in formulas, `\(\ket{\psi}\)`. Each is replaced by its
//! template in the document's tree before any output is written: the HTML
//! output keeps the formula as LaTeX text, `\lvert{\psi}\rangle`, and the
//! recomposed LaTeX output holds the same text. Each built-in macro except the
//! backtick form (below) is declared for both running text and formulas.
//! In each of the two, the macro is available when its template parses there.
//! A macro whose template is valid in running text, such as `\Ident` or
//! `\bit{0}`, therefore works there too. Any other macro, `\ket{x}` among them,
//! is an Error where it is used in running text; the rest of the document is
//! still rendered.
//!
//! **Delimited macros.** Their arguments are written between delimiters drawn
//! at the size the use asks for:
//!
//! | macro | gives |
//! |---|---|
//! | `\abs{x}` | `\lvert x \rvert` |
//! | `\norm{x}` | `\lVert x \rVert` |
//! | `\avg{x}` | `\langle x \rangle` |
//! | `\ket{x}`, `\bra{x}` | `\lvert x \rangle`, `\langle x \rvert` |
//! | `\braket{x}{y}` | `\langle x \vert y \rangle` |
//! | `\ketbra{x}{y}`, `\proj{x}` | `\lvert x \rangle \langle y \rvert`, the same with `x` twice |
//! | `\matrixel{x}{A}{y}`, `\dmatrixel{x}{A}` | `\langle x \vert A \vert y \rangle`, the same with `x` twice |
//! | `\innerprod{x}{y}` | `\langle x, y \rangle` |
//! | `\oket`, `\obra`, `\obraket`, `\oketbra`, `\oproj`, `\omatrixel`, `\odmatrixel` | the same with double angle brackets `\llangle`, `\rrangle` |
//! | `\intervalc{a}{b}`, `\intervalo`, `\intervalco`, `\intervaloc` | `[a, b]`, `]a, b[`, `[a, b[`, `]a, b]` |
//!
//! **Sizes.** A delimited macro takes its size in one of three ways: a size
//! command after a backtick (``\abs`\Big{x}``), a size command in brackets
//! (`\abs[\Big]{x}`), or a star (`\abs*{x}`, also `\abs[*]{x}` and
//! ``\abs`*{x}``), which draws the delimiters as tall as the content. The size
//! commands are `\big`, `\Big`, `\bigg` and `\Bigg`; a use gives at most one
//! size. Every delimiter of the macro, the bar of `\braket` included, is drawn
//! at the size asked for. Sizes apply in formulas only: a delimited macro used
//! in running text (possible when its template parses there) writes its
//! delimiters at their natural size, and a use in running text that asks for a
//! size is an Error.
//!
//! **Simple macros.** `\Hs` (`\mathscr{H}`), `\Ident` (the character `𝟙`),
//! `\bit{0}` (a bit in a monospace font), `\bitstring{0110}` (bits in a
//! monospace font, with a line above and below), `\gate{H}` and `\AND`, `\XOR`,
//! `\CNOT`, `\NOT`, `\NOOP` (gates in small capitals), the groups and algebras
//! `\uu(n)`, `\UU(n)`, `\su(n)`, `\SU(n)`, `\so(n)`, `\SO(n)`, `\slalg(n)`,
//! `\SL(n)`, `\GL(n)` and `\SN(n)` (each written with its argument in
//! parentheses; `\SN(n)` gives `\mathrm{S}_{n}`), and the operators `\tr`, `\supp`, `\rank`, `\linspan`,
//! `\spec`, `\diag`, `\Re`, `\Im`, `\poly`.
//!
//! **The backtick form**, in formulas only: a backtick, an optional size (a
//! size command or a star, written right after the backtick), and a group in
//! braces, brackets, parentheses or angle brackets write the group between its
//! own delimiters at that size. `` `\Big(a+b) `` gives `\Bigl (a+b\Bigr )`,
//! `` `*{x} `` gives `\mathopen{}\left \{x\right \}\mathclose{}`, and `` `<x> ``
//! gives `\langle x\rangle`. A size that is not one of the four size commands
//! or the star (`` `\huge(x) ``) is not read as a size, and the group that must
//! follow the backtick is reported missing. In running text a backtick is a
//! quotation mark, as it is without the extension.
//!
//! **Quantity macros**: entropies and other measures of information. Each is
//! written with optional parts that appear in the output only when given. A
//! quantity macro that takes a size takes it after a backtick only (a bracket
//! after the macro is its first optional part, not a size); `\ee` and
//! `\emptysystem` take none. In the table below, `[state]`, `[epsilon]` and
//! `[cond]` are optional arguments. A subscript or superscript that would hold
//! nothing is left out, and the parenthesis is drawn at the size asked for. The
//! parenthesis, with the state's subscript after it, is left out when every part
//! it holds is blank: `\Hmaxf{}` is the symbol alone, `H_{\mathrm{max}}`, and
//! `\DD{}{}` is `D`.
//!
//! | macro | arguments | gives |
//! |---|---|---|
//! | `\HH`, `\Hmin`, `\Hzero`, `\Hmaxf` | size, `[state]`, `[epsilon]`, `{target}`, `[cond]` | `H_{sub}^{epsilon}(target \,\vert\, cond)_{state}`, *sub* being nothing, `\mathrm{min}`, `\mathrm{max},0` or `\mathrm{max}`: ``\Hmin`\Big[\rho][\epsilon]{X}[Y]`` |
//! | `\Hfn` | size, `(arg)` | `H(arg)`: ``\Hfn`\big(p_1\rho_1 + p_2\rho_2)`` |
//! | `\II` | size, `[state]`, `[epsilon]`, `{A}`, `{B}`, `[cond]` | `I^{epsilon}(A \,:\, B \,\vert\, cond)_{state}`: `\II[\rho]{A}{C}[B]` |
//! | `\DD` | `_{sub}` and `^{sup}` in either order, each optional; size; `{rho}`; `{sigma}` | `D_{sub}^{sup}(rho \,\Vert\, sigma)`: `\DD_{\mathrm{Rob}}^{\epsilon}{\rho}{\sigma}` |
//! | `\Dmax`, `\Dminz`, `\Dminf`, `\Dr`, `\DHyp`, `\Dhyp` | `[epsilon]`, size, `{rho}`, `{sigma}` | `D_{sub}^{epsilon}(rho \,\Vert\, sigma)`, *sub* being `\mathrm{max}`, `\mathrm{min,0}`, `\mathrm{min}`, `\mathrm{r}`, `\mathrm{H}` or `\mathrm{h}`; `\DHyp` and `\Dhyp` write `\eta` when no epsilon is given: ``\Dmax[\epsilon]`\big{\rho}{\sigma}`` |
//! | `\IdentProc` | size, `[A]`, `[A']`, `{arg}` | `\operatorname{id}_{A \to A'}(arg)`, the parenthesis only around an argument that is not empty: `\IdentProc[A][A']{\rho}` |
//! | `\ee` | `^{x}`, optional | `e^{x}`: `\ee^{-i\theta}` |
//! | `\emptysystem` | none | `\emptyset`, a trivial system |
//!
//! The bar before a conditioning system, and the double bar of a relative
//! entropy, are drawn at the size of the parenthesis. For an entropy of its own
//! (another symbol or subscript), a document declares a macro with the same
//! arguments and a template of its own (next section).
//!
//! # Declaring macros
//!
//! The feature's settings declare more macros under the key
//! `macro_definitions`. A document writes them in its front matter under
//! `flm`, `features`, `phfqit`, `macro_definitions`:
//!
//! ```yaml
//! ---
//! flm:
//!   use: [phfpack]
//!   features:
//!     phfqit:
//!       macro_definitions:
//!         opnorm:
//!           delimited_args: ['{']
//!           content: '#[\lVert]{DelimLeft}{#1}#[\rVert]{DelimRight}_\infty'
//!       spaceKets:
//!         Bar: '\mkern 2mu '
//! ---
//! ```
//!
//! This example declares the macro `\opnorm`, a delimited macro with one
//! argument (`\opnorm{A}`, ``\opnorm`\Big{A}``, `\opnorm*{A}`), and changes the
//! spacing around the bar of `\braket`, `\matrixel` and `\dmatrixel`. A
//! declaration is one of three forms:
//!
//! - a text: the template of a macro that takes no argument;
//! - a map with `delimited_args`: a delimited macro like the built-in ones, which
//!   takes the three size forms before the arguments listed;
//! - a map with `args`: any list of arguments, in which `SizeArgBacktick`,
//!   `SizeArgStar` and `SizeArgOptArg` are the three ways of giving a size (a
//!   size after a backtick, a star, a size in brackets).
//!
//! The arguments are written as the `args` of a user definition of the
//! `userdefs` feature: one text whose characters are argument codes (`'[{'`),
//! or a list whose entries are each an argument code (`'{'`, `'r()'`) or an
//! argument's map. An argument's map gives the argument's syntax either as a
//! code (`code: 'r()'`) or as a structured syntax, a standard kind of
//! argument with its parameters (`syntax: delimited-group` with `delimiters:
//! ['(', ')']`), beside the argument's `name`, `mode`, `layout`,
//! `opaque_wrapper`, `scoped`, `docs` and `role`. A declaration written as a
//! map also gives the macro's `argument_number_offset`, the `defaults` of its
//! optional arguments and its `docs`; its `content` is one template, or a map
//! with a template for running text (`text`) and one for formulas (`math`).
//! The schema that `flm schema --of front-matter` prints for a document that
//! uses the extension describes every key.
//!
//! A declared macro with the name of a built-in macro of `phfqit` replaces it. A
//! declared macro with the name of a command another feature defines in the same
//! parsing mode, such as `\emph`, makes the registry build fail. A declaration
//! that cannot be read (both `delimited_args` and `args` given, a code that is
//! not an argument code, an argument's map that gives both a code and a
//! structured syntax) is reported when the settings are read, where the
//! document wrote it, with the path of the value at fault
//! (`phfqit.macro_definitions.abs.args`).
//!
//! A template is written in the template language of user definitions, FLM
//! text with placeholders: `#1`, `#2` and `#{name}` are the arguments. The
//! feature adds four placeholders of its own, whose content the feature
//! computes at each use:
//!
//! - `#[\lVert]{DelimLeft}`, `#[\vert]{DelimMiddle}` and `#[\rVert]{DelimRight}`
//!   write the delimiter in brackets, on the left, in the middle or on the
//!   right, at the size the use asked for;
//! - `#[spaceKets.Bar]{config}` writes a named value of the settings (below).
//!
//! **Named values.** A named value is a text held in the feature's settings
//! under a key and an entry name, such as `spaceKets.Bar`, which a template
//! inserts with `#[key.entry]{config}`. The keys are `spaceKets` and
//! `spaceOKets`, each with the entries `Bar` and `RLAngle` (the spacing around the bar of `\braket` and
//! between the angle brackets of `\ketbra`, and of their double-bracket
//! versions), and `space`, with `BeforeComma` and `AfterComma` (around the
//! comma of `\innerprod`). Each key is combined entry by entry: a document that
//! sets one entry keeps the default of every other. A use of a macro whose
//! template reads a named value that the settings do not hold is an Error at
//! the use (`phfpack.phfqit.unknown-value`), and the use is replaced by its
//! source text.
//!
//! # The package and its requirements
//!
//! The package `_` uses the standard package, `stdcore`. The feature `phfqit`
//! requires the `userdefs` feature of the standard package, whose stage
//! replaces the macros by their templates: a registry that holds `phfqit`
//! without `userdefs` fails to build. The package states the feature order
//! `math-mathjax < phfqit`: where the feature `math-mathjax` (the extension that
//! processes mathematics with MathJax) is loaded, the macros of `phfqit` that
//! MathJax also defines, such as `\Im` and `\Re`, take `phfqit`'s definitions.
//! Where `math-mathjax` is not loaded, the order says nothing.
//!
//! # The items of the guest client the extension uses
//!
//! The extension shows how a feature written in Rust against the guest client
//! declares user definitions with a hook of its own, computes placeholders at
//! each use, and declares settings as data. Each step uses items of
//! `flm_ext::guest`. The macro `extension!` and the types `Extension`,
//! `Package` and `Feature` are the guest client's own: they declare the
//! extension's packages and features. The items under the modules `core`,
//! `lang`, `stdcore` and `techy` each mirror an item of the library
//! `flm-core`, except `ExpandCx::definition`, which hands a hook the
//! definition registered with the use's element and has no counterpart there:
//!
//! - **The package and the feature.** The function given to the macro
//!   `flm_ext::guest::extension!` declares the package `_` with
//!   `Extension::package`: a `Package` that uses `stdcore`, whose `flm` block
//!   states the feature order, and that holds the feature `phfqit`, a `Feature`
//!   with its title, its documentation, its settings, its configuration reader
//!   and its `register` function.
//! - **Settings as data.** `Feature::settings` declares the configuration with
//!   a JSON schema (the schema of the crate's own settings types, generated by
//!   `schemars`), the four keys merged entry by entry with their default
//!   entries, and the templates and named values marked as FLM markup.
//!   `Feature::read` declares the configuration reader, which reads the
//!   configuration written into the settings types key by key (each macro
//!   declaration with `serde`) and converts each declaration, when the host
//!   reads a document's settings, and answers the value at fault as a
//!   `core::settings::ReadError` (its path below the feature's name, and the
//!   condition it violates) or the configuration unchanged. `register`
//!   receives that configuration as a JSON value, reads it the same way, and
//!   converts each declaration into the guest's `Definition`.
//!   The settings types hold each argument as the guest's
//!   `lang::mint::ArgDef`, which `serde` reads and writes in the data form of
//!   a user definition's arguments (an argument code, or an argument's map
//!   with a code or a structured syntax), and whose JSON schema (`schemars`)
//!   is that form's, listing the argument codes and the standard kinds; they
//!   hold the arguments of `delimited_args` as the guest's
//!   `lang::mint::ArgListData`, read, written and described in the data form
//!   of a user definition's argument list (one text of codes, or a list).
//! - **Elements and lowering rules.** For each macro, `register` builds the
//!   guest's `lang::rewrites::userdefs::Definition` (the arguments, the
//!   template, the parsing modes, the default values, the argument number
//!   offset, and the four computed placeholders, declared with
//!   `Definition::extra_placeholder`), names its element with
//!   `stdcore::features::userdefs::element_name_under` under the prefix
//!   `phfpack.phfqit`, declares the element with `FeatureBuilder::element`,
//!   passing the declaration that `UserDefinition::element_def_with` builds
//!   with the feature's own expansion hook (an implementation of the guest's
//!   trait `Expand`), and declares the lowering rule that
//!   `stdcore::features::userdefs::expand_in_tree_rule_for` builds with
//!   `FeatureBuilder::lowering_rule`. The rule requires the capability
//!   `flm.userdefs.always-expanded` of the `userdefs` feature, so every plan
//!   that lowers one of the macros runs the `userdefs` stage that replaces it
//!   by its template; this requirement is the feature's only dependency on
//!   another feature. The extension interface compiles every definition's
//!   templates once the language is built.
//! - **The size slots.** Each way of giving a size is an argument of one of
//!   the library's standard kinds, declared by its code: the embellishment
//!   argument whose one marker is the backtick (``e{`}``), a star (`*`), an
//!   optional argument in brackets (`[`), and, in the backtick form, one
//!   optional marker argument per size command (`t{\Bigg}`, `t{\Big}`,
//!   `t{\bigg}`, `t{\big}`) before the star. At a use, the hook reads which
//!   slot is provided and the size written there from the use's node
//!   (`techy::node::Node::argument_content_nodes`, `children`,
//!   `span_content`).
//! - **Placeholders computed by the feature.** At each use, the expansion hook
//!   finds the use's definition (`ExpandCx::definition`) and its variant for
//!   the use's parsing mode (`UserDefinition::variant_for_use`), lists the
//!   placeholders that the variant's template and default values write
//!   (`CompiledTemplate::placeholders`, `ModeVariant::defaults`), compiles each
//!   delimiter at the use's size and each named value in the use's mode
//!   (`compile_template_with`, against the expansion's language,
//!   `Expander::lang`, and the feature's `TemplateLanguage`), and gives the
//!   compiled texts to the placeholders with `ExtraValues`, laid over the
//!   bindings of the use's arguments (`DefinitionBindings::new`). It then
//!   instantiates the template with `Expander::instantiate`.
//! - **Diagnostics.** A use that cannot be expanded is reported with
//!   `Expander::recover`, which reports a `techy::error::Diagnostic` and
//!   replaces the use by its source text: under `phfpack.phfqit.bad-size` for
//!   a size that is not one, under `phfpack.phfqit.unknown-value` for a named
//!   value the settings do not hold, and under the library's own identifier,
//!   `flm.userdefs.bad-definition`, for a macro that has no valid template in
//!   the use's parsing mode.
//! - **The registry digest.** `register` passes the settings' bytes to
//!   `FeatureBuilder::digest_input`, so that two registries built from
//!   different declared macros or named values have different digests.

mod feature;
mod macros;
mod settings;
mod sizing;
mod tables;

use flm_ext::guest::{Extension, Feature, Package};
use serde_json::json;

flm_ext::guest::extension!(define);

/// Declares the extension's one package, `_`: it uses the standard package,
/// states the feature order `math-mathjax < phfqit`, and holds the feature
/// `phfqit`.
fn define(ext: &mut Extension) {
    ext.package(
        "_",
        Package::new().uses(["stdcore"]).flm(json!({ "feature_order": ["math-mathjax < phfqit"] })).feature(
            feature::FEATURE,
            Feature::new()
                .title("Quantum-information notation")
                .docs(
                    "Notation macros for quantum information: delimited macros (absolute values, norms, \
                     averages, bras and kets, inner products, intervals) whose delimiters are drawn at \
                     the size a use asks for; operators, gates and groups; quantity macros (entropies \
                     and other measures of information); the backtick form, which writes a group \
                     between its own delimiters at a size in formulas; and the macros declared in the \
                     feature's settings. Every macro is replaced by its template in the document's \
                     tree by the `userdefs` feature's `flm.userdefs.expand-always` stage, for every \
                     output, including a recomposed LaTeX output that keeps the other user \
                     definitions as LaTeX commands.",
                )
                .settings(feature::settings())
                .read(settings::read)
                .register(feature::register),
        ),
    );
}
