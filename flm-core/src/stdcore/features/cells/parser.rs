//! The parser of a cell's styles argument ([`CellStylesParser`]): an
//! optional group delimited by `<` and `>`, holding document content.
//!
//! `\cell<H l>{…}` and `\celldata<H, l>{…}` write their style tokens in angle
//! brackets. The styles argument is an ordinary argument declaration that
//! carries this parser, which reads the group as the language reads an
//! optional `[…]` group: the opening `<` may follow whitespace or a comment,
//! a `>` inside a `{…}` group belongs to that group, an argument that is not
//! written is absent, and an opening `<` without its `>` gets the language's
//! ordinary report of an unclosed group. The group holds content (the tokens
//! as characters, a value in braces as a group), which the table's reader
//! ([`read_grid`](super::read_grid)) reads at lowering.

use std::fmt;
use std::sync::Arc;

use techy::core::constructs::{
    ArgumentParser, ConstructParserResult, OptionalGroupArgumentParser, ParseContext, ParsedArgumentNodes,
};
use techy::core::specs::ArgumentSpec;
use techy::core::token::GroupRule;

use crate::lang::mint::{DefinitionShape, FlmArgumentParser};
use crate::lang::{Flm, FlmGroupType};

/// The identity of the parser in the registry digest, `flm.cells.styles`.
pub const IDENTITY: &str = "flm.cells.styles";

/// The parser of the styles argument of `\cell` and `\celldata`: an optional
/// `<…>` group of document content, built on techy's optional group parser
/// with the delimiters `<` and `>`. See the [module documentation](self).
///
/// What it states through [`FlmArgumentParser`]: the argument holds content;
/// its identity is [`IDENTITY`]; it has no shape in a recomposed user
/// definition, since no user definition declares such an argument; its
/// description is "an optional `<…>` group of style tokens". An argument
/// that is not written is absent.
pub struct CellStylesParser {
    angles: OptionalGroupArgumentParser<Flm>,
}

impl CellStylesParser {
    /// Creates the parser.
    pub fn new() -> CellStylesParser {
        let rule = Arc::new(GroupRule {
            group_type: FlmGroupType::Content,
            open: String::from("<"),
            close: String::from(">"),
        });
        CellStylesParser { angles: OptionalGroupArgumentParser::new(rule) }
    }
}

impl Default for CellStylesParser {
    fn default() -> CellStylesParser {
        CellStylesParser::new()
    }
}

impl fmt::Debug for CellStylesParser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CellStylesParser")
    }
}

impl ArgumentParser<Flm> for CellStylesParser {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        self.angles.parse_argument(cx, spec)
    }
}

impl FlmArgumentParser for CellStylesParser {
    fn holds_content(&self) -> bool {
        true
    }

    fn identity(&self) -> Vec<u8> {
        IDENTITY.as_bytes().to_vec()
    }

    fn definition_shape(&self) -> DefinitionShape {
        DefinitionShape::NoShape { token: "cell-styles" }
    }

    fn description(&self) -> String {
        String::from("an optional `<…>` group of style tokens")
    }
}
