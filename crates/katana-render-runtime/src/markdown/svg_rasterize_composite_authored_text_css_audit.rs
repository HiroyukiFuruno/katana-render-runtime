use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, Token,
};

pub(in crate::markdown::svg_rasterize::provenance::authored_text_context) fn inline_style_is_safe(
    source: &str,
) -> bool {
    let mut input = Parser::new(source);
    declaration_block_audit(&mut input)
        .is_ok_and(|audit| !audit.sensitive && !audit.sensitive_function)
}

pub(super) fn declaration_block_audit(
    input: &mut Parser<'_>,
) -> Result<DeclarationAudit, ParseError<()>> {
    let mut audit = DeclarationAudit::default();
    for declaration in RuleBodyParser::new(input, &mut audit) {
        declaration.map_err(|(error, _, _)| error)?;
    }
    Ok(audit)
}

fn contains_function(input: &mut Parser<'_>) -> Result<bool, ParseError<()>> {
    let mut found = false;
    while !input.is_exhausted() {
        match input.next()?.clone() {
            Token::Function(_) => {
                input.parse_nested_block(contains_function)?;
                found = true;
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                found |= input.parse_nested_block(contains_function)?;
            }
            _ => {}
        }
    }
    Ok(found)
}

#[derive(Default)]
pub(super) struct DeclarationAudit {
    pub(super) sensitive: bool,
    pub(super) sensitive_function: bool,
}

impl<'i> DeclarationParser<'i> for DeclarationAudit {
    type Declaration = ();
    type Error = ();

    fn parse_value(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
        _: &ParserState,
    ) -> Result<Self::Declaration, ParseError<Self::Error>> {
        let sensitive = super::is_sensitive_property(&name);
        let has_function = contains_function(input)?;
        self.sensitive |= sensitive;
        self.sensitive_function |= sensitive && has_function;
        Ok(())
    }
}

impl<'i> QualifiedRuleParser<'i> for DeclarationAudit {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = ();
}

impl<'i> AtRuleParser<'i> for DeclarationAudit {
    type Prelude = ();
    type AtRule = ();
    type Error = ();
}

impl<'i> RuleBodyItemParser<'i, (), ()> for DeclarationAudit {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}
