use std::ops::Range;

use cssparser::{
    AtRuleParser, CowRcStr, ParseError, Parser, ParserState, QualifiedRuleParser, StyleSheetParser,
};

use super::{consume_nested, parse_declaration_block};

pub(super) fn parse(source: &str) -> Option<Vec<Range<usize>>> {
    let mut input = Parser::new(source);
    let mut edits = Vec::new();
    parse_block(&mut input, &mut edits).ok()?;
    Some(edits)
}

fn parse_block(
    input: &mut Parser<'_>,
    edits: &mut Vec<Range<usize>>,
) -> Result<(), ParseError<()>> {
    let mut parser = FilterStylesheetParser { edits };
    let has_error = StyleSheetParser::new(input, &mut parser).any(|result| result.is_err());
    if has_error {
        Err(ParseError::unexpected_token())
    } else {
        Ok(())
    }
}

struct FilterStylesheetParser<'a> {
    edits: &'a mut Vec<Range<usize>>,
}

impl<'i> QualifiedRuleParser<'i> for FilterStylesheetParser<'_> {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = ();

    fn parse_prelude(
        &mut self,
        input: &mut Parser<'i>,
    ) -> Result<Self::Prelude, ParseError<Self::Error>> {
        consume_nested(input)?;
        Ok(())
    }

    fn parse_block(
        &mut self,
        _prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Self::QualifiedRule, ParseError<Self::Error>> {
        parse_declaration_block(input, self.edits)?;
        Ok(())
    }
}

impl<'i> AtRuleParser<'i> for FilterStylesheetParser<'_> {
    type Prelude = ();
    type AtRule = ();
    type Error = ();

    fn parse_prelude(
        &mut self,
        _name: CowRcStr<'i>,
        input: &mut Parser<'i>,
    ) -> Result<Self::Prelude, ParseError<Self::Error>> {
        consume_nested(input)
    }

    fn parse_block(
        &mut self,
        _prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Self::AtRule, ParseError<Self::Error>> {
        parse_block(input, self.edits)?;
        Ok(())
    }
}
