//! The text options a host sets on the standard package
//! (`flm_core::stdcore::StdcoreConfig::blocks`), reaching a document's printed
//! text: every family of typographic substitution on, every family off, and
//! the ligature families alone.

mod common;

use flm_core::lang::rewrites::blocks::BlocksConfig;
use flm_core::stdcore::StdcoreConfig;

use common::spine::text_at;
use common::stdcore::registry;

/// Two paragraphs holding a lone straight `'`, a straight `"` pair, the TeX
/// dash and the ellipsis: the spellings the position test decides.
const STRAIGHT: &str = "Hello 'world'. Does this \"work\" or does it ``function?''\n\n\
                        It does---doesn't it? Maybe...\n";

/// The same two paragraphs with the TeX quote ligatures written out, which the
/// ligature family resolves without the position test.
const LIGATURES: &str = "Hello `world'. Does this \"work\" or does it ``function?''\n\n\
                         It does---doesn't it? Maybe...\n";

/// The text `source` prints as under `blocks`, never wrapped.
fn text(blocks: BlocksConfig, source: &str) -> String {
    text_at(&registry(StdcoreConfig::new().blocks(blocks)), source, None)
}

// Ported from Python FLM's test/test_flmenvironment.py::TestFLMEnvironment::test_chars_options_auto.
#[test]
fn every_family_on_resolves_the_quotes_the_dash_and_the_ellipsis() {
    assert_eq!(
        text(BlocksConfig::new(), STRAIGHT),
        "Hello ‘world’. Does this “work” or does it “function?”\n\n\
         It does—doesn’t it? Maybe…\n"
    );
}

// Ported from Python FLM's test/test_flmenvironment.py::TestFLMEnvironment::test_chars_options_no_auto.
#[test]
fn every_family_off_leaves_the_source_spellings_as_written() {
    assert_eq!(
        text(BlocksConfig::no_typography(), STRAIGHT),
        "Hello 'world'. Does this \"work\" or does it ``function?''\n\n\
         It does---doesn't it? Maybe...\n"
    );
}

// Ported from Python FLM's test/test_flmenvironment.py::TestFLMEnvironment::test_chars_options_ligonly.
#[test]
fn the_ligature_families_alone_leave_the_straight_double_quotes_straight() {
    let ligatures =
        BlocksConfig { auto_unicode_quotes: false, ..BlocksConfig::new() };
    assert_eq!(
        text(ligatures, LIGATURES),
        "Hello ‘world’. Does this \"work\" or does it “function?”\n\n\
         It does—doesn’t it? Maybe…\n"
    );
}
