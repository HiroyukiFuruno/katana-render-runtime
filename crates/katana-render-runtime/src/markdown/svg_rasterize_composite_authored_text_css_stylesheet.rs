use cssparser::{
    AtRuleParser, BasicParseErrorKind, CowRcStr, ParseError, Parser, ParserState,
    QualifiedRuleParser, StyleSheetParser, Token,
};

pub(in crate::markdown::svg_rasterize::provenance::authored_text_context) fn sensitive_selectors<
    'i,
>(
    source: &'i str,
) -> Option<Vec<simplecss::Selector<'i>>> {
    let mut input = Parser::new(source);
    let mut audit = StylesheetAudit::default();
    for rule in StyleSheetParser::new(&mut input, &mut audit) {
        if rule.is_err() {
            return None;
        }
    }
    Some(audit.sensitive_selectors)
}

#[derive(Default)]
struct StylesheetAudit<'i> {
    sensitive_selectors: Vec<simplecss::Selector<'i>>,
}

impl<'i> QualifiedRuleParser<'i> for StylesheetAudit<'i> {
    type Prelude = Vec<&'i str>;
    type QualifiedRule = ();
    type Error = ();

    fn parse_prelude(&mut self, input: &mut Parser<'i>) -> Result<Self::Prelude, ParseError<()>> {
        selector_sources(input)
    }

    fn parse_block(
        &mut self,
        selectors: Self::Prelude,
        _: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<(), ParseError<()>> {
        let declarations = super::audit::declaration_block_audit(input)?;
        if declarations.sensitive_function {
            return Err(ParseError::unexpected_token());
        }
        if declarations.sensitive {
            self.collect_sensitive_selectors(selectors)?;
        }
        Ok(())
    }
}

impl<'i> StylesheetAudit<'i> {
    fn collect_sensitive_selectors(
        &mut self,
        selectors: Vec<&'i str>,
    ) -> Result<(), ParseError<()>> {
        for source in selectors {
            let selector =
                super::selector_from_source(source).ok_or_else(ParseError::unexpected_token)?;
            self.sensitive_selectors.push(selector);
        }
        Ok(())
    }
}

impl<'i> AtRuleParser<'i> for StylesheetAudit<'i> {
    type Prelude = ();
    type AtRule = ();
    type Error = ();

    fn parse_prelude(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
    ) -> Result<(), ParseError<()>> {
        if !name.eq_ignore_ascii_case("keyframes") {
            return Err(ParseError::from_basic_kind(
                BasicParseErrorKind::AtRuleInvalid,
            ));
        }
        input.expect_ident()?;
        input.expect_exhausted()?;
        Ok(())
    }

    fn parse_block(
        &mut self,
        _: (),
        _: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<(), ParseError<()>> {
        let mut audit = KeyframeRuleAudit;
        for rule in StyleSheetParser::new(input, &mut audit) {
            rule.map_err(|(error, _, _)| error)?;
        }
        Ok(())
    }

    fn rule_without_block(&mut self, _: (), _: &ParserState) -> Result<(), ()> {
        Err(())
    }
}

fn selector_sources<'i>(input: &mut Parser<'i>) -> Result<Vec<&'i str>, ParseError<()>> {
    input.parse_comma_separated(|selector| {
        let start = selector.position();
        while !selector.is_exhausted() {
            if matches!(selector.next()?, Token::SquareBracketBlock) {
                selector.parse_nested_block(consume_selector_attribute)?;
            }
        }
        Ok(selector.slice_from(start).trim())
    })
}

fn consume_selector_attribute(input: &mut Parser<'_>) -> Result<(), ParseError<()>> {
    while !input.is_exhausted() {
        input.next()?;
    }
    Ok(())
}

struct KeyframeRuleAudit;

impl<'i> QualifiedRuleParser<'i> for KeyframeRuleAudit {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = ();

    fn parse_prelude(&mut self, input: &mut Parser<'i>) -> Result<(), ParseError<()>> {
        if input.is_exhausted() {
            return Err(ParseError::unexpected_token());
        }
        while !input.is_exhausted() {
            input.next()?;
        }
        Ok(())
    }

    fn parse_block(
        &mut self,
        _: (),
        _: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<(), ParseError<()>> {
        let declarations = super::audit::declaration_block_audit(input)?;
        if declarations.sensitive || declarations.sensitive_function {
            Err(ParseError::unexpected_token())
        } else {
            Ok(())
        }
    }
}

impl<'i> AtRuleParser<'i> for KeyframeRuleAudit {
    type Prelude = ();
    type AtRule = ();
    type Error = ();
}
