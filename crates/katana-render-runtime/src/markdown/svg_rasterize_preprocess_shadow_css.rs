use std::ops::Range;

use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, Token,
};

#[path = "svg_rasterize_preprocess_shadow_stylesheet.rs"]
mod stylesheet;
#[path = "svg_rasterize_preprocess_shadow_value.rs"]
mod value;
use value::{apply_rgba_edits, scan_filter_value};

pub(super) fn normalize_inline_style(source: &str) -> Option<String> {
    let edits = parse_declarations(source)?;
    apply_rgba_edits(source, edits)
}

pub(super) fn normalize_stylesheet(source: &str) -> Option<String> {
    let edits = stylesheet::parse(source)?;
    apply_rgba_edits(source, edits)
}

fn parse_declarations(source: &str) -> Option<Vec<Range<usize>>> {
    let mut input = Parser::new(source);
    let mut edits = Vec::new();
    parse_declaration_block(&mut input, &mut edits).ok()?;
    Some(edits)
}

pub(super) fn parse_declaration_block(
    input: &mut Parser<'_>,
    edits: &mut Vec<Range<usize>>,
) -> Result<(), ParseError<()>> {
    let mut parser = FilterDeclarationParser { edits };
    let has_error = RuleBodyParser::new(input, &mut parser).any(|result| result.is_err());
    if has_error {
        Err(ParseError::unexpected_token())
    } else {
        Ok(())
    }
}

struct FilterDeclarationParser<'a> {
    edits: &'a mut Vec<Range<usize>>,
}

impl<'i> DeclarationParser<'i> for FilterDeclarationParser<'_> {
    type Declaration = ();
    type Error = ();

    fn parse_value(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
        _declaration_start: &ParserState,
    ) -> Result<Self::Declaration, ParseError<Self::Error>> {
        if name.eq_ignore_ascii_case("filter") {
            scan_filter_value(input, false, self.edits)?;
        } else {
            consume_nested(input)?;
        }
        Ok(())
    }
}

impl AtRuleParser<'_> for FilterDeclarationParser<'_> {
    type Prelude = ();
    type AtRule = ();
    type Error = ();
}

impl QualifiedRuleParser<'_> for FilterDeclarationParser<'_> {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = ();
}

impl RuleBodyItemParser<'_, (), ()> for FilterDeclarationParser<'_> {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

pub(super) fn consume_nested(input: &mut Parser<'_>) -> Result<(), ParseError<()>> {
    while !input.is_exhausted() {
        let token = input.next_including_whitespace_and_comments()?.clone();
        if is_nested_token(&token) {
            input.parse_nested_block(consume_nested)?;
        }
    }
    Ok(())
}

fn is_nested_token(token: &Token<'_>) -> bool {
    matches!(
        token,
        Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock
    )
}
