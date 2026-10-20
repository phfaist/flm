//! Every documentation text that `flm info` shows is FLM that parses without a problem.
//!
//! The environment is the program's default one: the environment of a document that
//! specifies nothing, prepared as `flm info` prepares it without `-d`. For every page of
//! its tree ([`Tree::walk`]), the test takes the documentation texts the page embeds as FLM,
//! as `flm info` takes them before it renders a page (the list the library's generator of
//! the pages gives, [`texts`]), and parses each with the registry that renders the pages
//! (the package `std` and the extension `flmdoc`, prepared once over an empty text) through
//! the function `flm info` uses ([`flm_cli::text_problems`]). The test fails with the text
//! and the path of its page on any problem: `flm info` would show the text as plain text,
//! with a warning.
#![cfg(feature = "bundled-extensions")]

use std::collections::BTreeSet;
use std::sync::Arc;

use flm_core::core::registry::PackageFactory;
use flm_info::Environment;
use flm_info::pages::{Tree, texts};
use flm_main::files::DiskFiles;
use flm_main::report;
use flm_main::session::{Input, Request, prepare};

/// The compiled packages that the program carries besides the host's own, as its `main`
/// lists them.
fn compiled_packages() -> Vec<PackageFactory> {
    vec![
        #[cfg(feature = "phfpack")]
        flm_phfpack::factory().unwrap(),
    ]
}

#[test]
fn every_documentation_text_parses() {
    let packages = compiled_packages();
    let mut request = Request::new(Input::Text(String::new()));
    request.default_math = flm_cli::default_math();
    let env = Environment::of_request(request, Arc::new(DiskFiles), &packages).expect("the default environment");
    let tree = Tree::new(&env, None);
    let mut embedded: Vec<(String, String)> = Vec::new();
    let mut pages = 0;
    for page in tree.walk() {
        pages += 1;
        embedded.extend(texts(&page).into_iter().map(|text| (page.path.clone(), text.text)));
    }
    assert!(pages > 500, "{pages} pages");

    // The registry that renders the pages: `std` and `flmdoc`, as a page's front matter asks.
    let mut request = Request::new(Input::Text(String::from("---\nflm:\n  use: [flmdoc]\n  math: false\n---\n")));
    request.profile = Some(String::from("text"));
    let rendering = prepare(request, Arc::new(DiskFiles), &packages).expect("the rendering environment");
    let lang = rendering.registry().lang();

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut failures: Vec<String> = Vec::new();
    for (path, text) in &embedded {
        if !seen.insert(text.as_str()) {
            continue;
        }
        let problems = flm_cli::text_problems(lang, text);
        if !problems.is_empty() {
            let problems: Vec<String> = problems.iter().map(report::diagnostic).collect();
            failures.push(format!("page `{path}`: {text:?}\n{}", problems.join("\n")));
        }
    }
    assert!(seen.len() > 300, "{} distinct texts", seen.len());
    assert!(failures.is_empty(), "{} texts do not parse:\n\n{}", failures.len(), failures.join("\n\n"));
}
