//! The formula renderer's script definition, `flm.html.mathjax`: what this
//! backend ships so that a page can typeset the mathematics it carries, and
//! what a host that does not trust the authors of that mathematics has to do
//! for itself.
//!
//! The **formula renderer** is MathJax, a script that runs in the reader's
//! browser, reads LaTeX out of the page and typesets it. This backend does not
//! translate passthrough mathematics (a formula whose source the
//! [`math-passthrough`](crate::math_passthrough) feature
//! carries unread): it writes the author's LaTeX into the page and lets
//! that script do the rest ([the emitter](super::emit)). The definition
//! [`mathjax_script`] is how a page gets the script: its **configuration**
//! first, then the **address** its program is loaded from ([`MATHJAX_LOADER`]),
//! in that order, because the program reads the configuration when it runs. A
//! host decides how both parts arrive, through the render option
//! [`definitions`](super::HtmlRenderOptions::definitions).
//!
//! # What the shipped configuration says
//!
//! It holds what this backend's own output needs, and nothing that restricts
//! what an author may write inside a formula.
//!
//! - It loads the extension that lets a formula carry HTML of the page
//!   inside it (`[tex]/texhtml`), adds that extension's package to the
//!   packages that apply, and switches its reading on (`allowTexHTML`). That
//!   mechanism is how content inside a formula (the argument of a text
//!   command, and the invisible point a reference leads to) reaches the
//!   reader.
//! - It declares the delimiters this backend writes — `\(…\)`, `\[…\]` and
//!   environments (`processEnvironments`) — and no others, so a stray `$` in a
//!   formula is not read as a delimiter.
//! - It tells the renderer to set the words of a formula in the page's own
//!   text font rather than in its mathematical font (`mtextInheritFont`). The
//!   words a document writes inside a formula are page text and this backend
//!   writes them into the page itself; the words the renderer sets for itself
//!   — the text of an equation's tag — then look like them.
//! - It turns the renderer's own equation numbering and cross-reference
//!   handling off (`tags: 'none'`, `processRefs: false`): a document's numbers
//!   and references are this library's to resolve.
//! - It typesets only inside elements carrying the class of a formula
//!   (`startup.elements`), so a backslash in code and a `\(` in
//!   prose are left alone.
//!
//! After the configuration object, the same part carries one short listener for
//! clicks on the page. MathJax takes the address off every link inside its
//! output, keeping it in a `data-mjx-href` attribute, and follows only the
//! links it made itself; a reference written in text inside a formula is a
//! link this backend wrote, so without the listener a click on it
//! would do nothing. The listener follows such a link — a plain primary click,
//! no modifier key held, on one of this backend's links inside a typeset
//! formula — and leaves every other click on the page alone. Reaching such a
//! link from the keyboard is not repaired by it: MathJax makes the whole
//! content of a formula unfocusable when it attaches the description it speaks
//! to a screen reader.
//!
//! The same part keeps the formulas typeset while a script of the page brings
//! it up to date in place, under the two conventions for a page updated in
//! place that the rendering guide states
//! ([`REGION_UPDATED_EVENT`](super::page::REGION_UPDATED_EVENT),
//! [`ORIGINAL_HTML_ATTRIBUTE`](super::page::ORIGINAL_HTML_ATTRIBUTE)). Just
//! before the renderer typesets a formula, it records the HTML inside the
//! formula's element in that attribute, so that an update leaves the typeset
//! formula alone while its HTML stays the same; and when a region of the page
//! was updated, it typesets the formulas of that region that carry no record —
//! the new ones and those whose HTML changed. A host that replaces this part
//! with a configuration of its own and updates its pages in place keeps these
//! lines.
//!
//! # Mathematics is not inspected
//!
//! **FLM does not look inside a mathematics region**, by design and for now.
//! It keeps no list of allowed symbols, it does not check whether a document
//! defines a command there, and it hands the characters the author typed to
//! the renderer unchanged. The intended solution is a mathematics parser
//! of FLM's own, which does not exist yet; until it does, an author who writes
//! mathematics is trusted with what the renderer makes of it.
//!
//! That is a statement about the *content* of a formula. The emitter still
//! escapes its own `<tex-html>` mechanism in what it writes inside a formula
//! ([the emitter](super::emit)).
//!
//! # Rendering mathematics written by authors the host does not trust
//!
//! A host that renders mathematics written by authors it does not trust
//! replaces the shipped configuration with the hardened configuration below,
//! through the [`definitions`](super::HtmlRenderOptions::definitions) option.
//! This section is the whole of what such a host needs.
//!
//! This library does not make such mathematics safe by itself, for three
//! reasons. Mathematics is not inspected, as above. The `<tex-html>` mechanism
//! is on, because it is how content inside a formula reaches the page, and
//! MathJax's own documentation warns that it must not be used "in settings
//! where your readers can enter expressions that are shown to other users",
//! since the HTML inside a formula is not filtered in any way. And a document
//! that *defines a macro* inside mathematics can build that tag out of parts
//! that never appear in the source, which no rewriting of the source can catch.
//! No locked configuration is shipped either, because a library that does not
//! inspect mathematics cannot promise that the result is safe. This holds for
//! the mathematics that
//! [`math-passthrough`](crate::math_passthrough) writes,
//! which is what this script typesets.
//!
//! Every fact below was checked against **MathJax 4.1.3** — the packages, the
//! defaults and the filter's behavior are that version's. A host should check
//! them again against the version it ships.
//!
//! ## What reaches the page when nothing is added
//!
//! Under the packages MathJax loads by default (`base`, `ams`, `newcommand`,
//! `textmacros`, `noundefined`, `require`, `autoload`, `configmacros`), a
//! formula can put this into the page, command by command:
//!
//! | what a document writes inside a formula | what arrives in the page | what closes it |
//! |---|---|---|
//! | `\href{javascript:…}{x}` | an `<a href>` carrying that address | emptying the `html` autoload entry; the safe component's address filter |
//! | `\cssId{…}{x}` | an `id`, which can shadow an identifier this backend wrote | the same, or the safe component's `cssIDs` |
//! | `\class{…}{x}` | a class, which pulls in the host page's own styling | the same, or the safe component's `classes` |
//! | `\style{background: url(…)}{x}` | a `style` whose declaration makes the browser fetch a file | the same, or the safe component's `styles` |
//! | `\data{foo}{bar}{x}` | a `data-foo` attribute | the same, or the safe component's `dataPattern` |
//! | `\require{html}…` | all of the above again, regardless of what was removed | removing the `require` package |
//! | `\bbox[background: url(…)]{x}` | a `style` that fetches a file | the safe component's `styles` |
//! | `\mmlToken{mi}[href=…][id=…][class=…][style=…]{x}` | an `<a href>`, an `id`, a class and a fetching `style` | **only** the safe component |
//!
//! `\mmlToken` decides the shape of the recipe: it is defined in the `base`
//! package, which MathJax does not let a configuration remove, so
//! no list of removed packages closes it. The safe component is what does.
//!
//! ## The recipe
//!
//! **The safe component.** `ui/safe` is a component MathJax ships that walks
//! the mathematics it has built and filters the attributes a document can
//! reach from inside a formula:
//! addresses, class names, element identifiers and style declarations. Four
//! settings say how far it goes, and the values below are the ones this
//! library's own measurements back:
//!
//! - `URLs: 'safe'` — keeps `http:`, `https:` and `file:` and refuses
//!   `javascript:` and `data:`.
//! - `classes: 'none'`, `cssIDs: 'none'` — drops a class or an identifier
//!   written from inside a formula altogether. No FLM construct needs an
//!   author to set either.
//! - `styles: 'none'` — drops every style declaration written from inside a
//!   formula. MathJax's milder `styles: 'safe'` keeps the declarations on a
//!   list of its own, and **that is not enough**: `cursor` is on that list and
//!   `http:` is a safe address, so `\bbox[cursor: url(http://…)]{x}` survives
//!   it and makes the reader's browser fetch a file the document named.
//!
//! With `'none'`, the *style argument* of `\bbox` and of `\enclose` has no
//! effect, and nothing reports it: `\bbox[border: 2px solid red]{x}` is set
//! with no border. Their color forms keep working —
//! `\bbox[yellow]{x}`, `\color{red}`, `\enclose{circle}[mathbackground="yellow"]`
//! — because a color is set as an attribute of the mathematics rather than as
//! a style declaration.
//!
//! **Three commands still put a style declaration into the page, and none of
//! them makes the browser fetch anything.**
//! `\enclose{…}[mathbackground="url(…)"]`, `\colorbox{url(…)}` and
//! `\color{url(…)}` each put `background-color: url(…)` or `color: url(…)`
//! into the page. Those two properties take a color and not an image, so the
//! declaration is invalid and a browser discards it: nothing is fetched. A
//! second declaration cannot be added beside the color either —
//! `\colorbox{red; cursor: url(http://…)}{x}` is refused with
//! `Invalid color value`.
//!
//! **The safe component does not filter the HTML a `<tex-html>` carries.** That
//! HTML is not part of the mathematics the component walks, which is why this
//! backend's own identifiers, classes and `data-flm-*` attributes survive it
//! untouched — and equally why it does not stop the macros defined inside
//! mathematics (below). The two
//! must be closed separately.
//!
//! **The packages.** Remove `html` (it is what gives `\href`, `\class`,
//! `\style`, `\cssId` and `\data`) **and** empty its autoload entry
//! (`autoload: {html: []}`): removing it from the package list alone changes
//! nothing, because autoloading — MathJax's habit of fetching a package the
//! first time one of its commands is used — puts it straight back. Remove
//! `require` too, or `\require{html}` loads it back by hand. Without
//! `require`, a document cannot use the packages MathJax ships but neither
//! loads by default nor autoloads, which are reachable only through
//! `\require`:
//! `physics` is one, measured — under this recipe
//! `\require{physics}\dv{f}{x}` comes out as the command names in red instead
//! of a derivative. A host that wants such a package lists it in the package
//! list of its own configuration instead.
//!
//! **Macros defined inside mathematics.** Stopping them takes two settings
//! together: remove the
//! `newcommand` package **and** empty its autoload entry
//! (`autoload: {newcommand: []}`). The first alone changes nothing, for the
//! same autoloading reason. Together they remove `\newcommand`,
//! `\renewcommand`, `\newenvironment`, `\renewenvironment`, `\def` and `\let`
//! from mathematics: an everyday `\newcommand{\Reals}{\mathbb{R}}` in a
//! formula then comes out as the command name in red. Because authors lose
//! these commands, the choice belongs to the host and not to this library.
//!
//! ## The hardened configuration, whole
//!
//! ```js
//! /* flm.html.mathjax */
//! window.MathJax = {
//!   loader: {load: ['[tex]/texhtml', 'ui/safe']},
//!   tex: {
//!     packages: {'[+]': ['texhtml'], '[-]': ['html', 'require', 'newcommand']},
//!     autoload: {html: [], newcommand: []},
//!     allowTexHTML: true,
//!     inlineMath: [['\\(', '\\)']],
//!     displayMath: [['\\[', '\\]']],
//!     processEnvironments: true,
//!     processRefs: false,
//!     tags: 'none'
//!   },
//!   chtml: {mtextInheritFont: true},
//!   options: {
//!     safeOptions: {
//!       allow: {URLs: 'safe', classes: 'none', cssIDs: 'none', styles: 'none'}
//!     }
//!   },
//!   startup: {elements: ['.flm-pres-math--passthrough']}
//! };
//! document.addEventListener('click', function (event) {
//!   if (event.defaultPrevented || event.button !== 0 ||
//!       event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) { return; }
//!   var from = event.target;
//!   var link = from && from.closest &&
//!     from.closest('mjx-container mjx-html a.flm-pres-link[data-mjx-href]');
//!   if (!link) { return; }
//!   event.preventDefault();
//!   window.location.href = link.getAttribute('data-mjx-href');
//! });
//! ```
//!
//! The class in `startup.elements` is the class of the presentation element
//! that holds passthrough mathematics
//! ([`class_of_element`](super::names::class_of_element) of
//! `flm.pres.math-passthrough`), and the class in the listener's selector is the
//! class of the link element (the same function of `flm.pres.link`); a host
//! that copies this block copies both classes with it.
//!
//! The listener comes after the object here; a host writing its own
//! configuration copies it unchanged if it wants a link inside the text of a
//! formula to be followed when a reader clicks it.
//!
//! ## How a host delivers it
//!
//! An override replaces the *whole* content of the definition, so a host
//! writes both parts: its configuration, and then the address the program is
//! loaded from.
//!
//! ```rust
//! use std::collections::BTreeMap;
//!
//! use flm_core::core::registry::DefContent;
//! use flm_core::model::names::Name;
//! use flm_core::stdcore::backends::html::{
//!     DefinitionOverride, HtmlRenderOptions, MATHJAX_LOADER,
//! };
//!
//! // The host's own configuration, hardened as above (abbreviated here).
//! let hardened = "window.MathJax = {/* … */};";
//!
//! let mut definitions = BTreeMap::new();
//! definitions.insert(
//!     Name::from("flm.html.mathjax"),
//!     DefinitionOverride::Parts(vec![
//!         DefContent::Inline(String::from(hardened)),
//!         DefContent::Url(String::from(MATHJAX_LOADER)),
//!     ]),
//! );
//!
//! let options = HtmlRenderOptions { definitions, ..HtmlRenderOptions::default() };
//! assert_eq!(options.definitions.len(), 1);
//! ```

use crate::core::registry::{DefContent, ScriptDef};
use crate::model::names::Name;

use super::conventions::SCRIPT_MATHJAX;

/// The address the formula renderer's program is loaded from by default:
/// MathJax version 4, from the content delivery network its own documentation
/// gives, pinned to the major version so that corrections arrive without an
/// edit here.
///
/// A host that will not load from a network replaces it through the
/// [`definitions`](super::HtmlRenderOptions::definitions) option; this library
/// never fetches the address itself.
pub const MATHJAX_LOADER: &str = "https://cdn.jsdelivr.net/npm/mathjax@4/tex-chtml.js";

/// The configuration of the formula renderer, read from `mathjax-config.js`:
/// the first part of the script
/// definition [`mathjax_script`] registers.
///
/// See the [module documentation](self) for what it says, for what this
/// library does and does not promise about mathematics, and for the
/// configuration a host writes instead when it does not trust the authors of
/// the mathematics it renders.
const MATHJAX_CONFIGURATION: &str = include_str!("mathjax-config.js");

/// Returns the script definition of the formula renderer, `flm.html.mathjax`: its
/// configuration and then the address its program is loaded from, in that
/// order, because the program reads the configuration when it runs.
///
/// The emitter requires this definition at the first passthrough formula it
/// writes in a tree, so a document with no passthrough mathematics loads
/// nothing.
///
/// What the configuration says, and what a host that renders mathematics
/// written by authors it does not trust has to write instead, is the [module
/// documentation](self).
pub fn mathjax_script() -> ScriptDef {
    ScriptDef {
        id: Name::from(SCRIPT_MATHJAX),
        content: vec![
            DefContent::Inline(String::from(MATHJAX_CONFIGURATION)),
            DefContent::Url(String::from(MATHJAX_LOADER)),
        ],
        docs: String::from(
            "the formula renderer: its configuration, then the address its program is loaded \
             from",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::html::page::{ORIGINAL_HTML_ATTRIBUTE, REGION_UPDATED_EVENT};

    #[test]
    fn the_configuration_follows_the_contract_for_a_page_updated_in_place() {
        assert!(MATHJAX_CONFIGURATION.contains(&format!("'{REGION_UPDATED_EVENT}'")));
        assert!(MATHJAX_CONFIGURATION.contains(&format!("'{ORIGINAL_HTML_ATTRIBUTE}'")));
        assert!(!MATHJAX_CONFIGURATION.to_ascii_lowercase().contains("</script"));
    }
}
