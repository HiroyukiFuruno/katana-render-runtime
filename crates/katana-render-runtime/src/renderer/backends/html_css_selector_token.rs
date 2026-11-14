use super::super::CssCombinator;

pub(super) enum SelectorToken {
    Compound(String),
    Combinator(CssCombinator),
}

#[derive(Default)]
struct SelectorTokenizer {
    tokens: Vec<SelectorToken>,
    current: String,
    bracket_depth: u8,
    parenthesis_depth: u8,
    quote: Option<char>,
    pending_descendant: bool,
}

pub(super) fn selector_tokens(selector: &str) -> Option<Vec<SelectorToken>> {
    if selector.is_empty() {
        return None;
    }
    let mut tokenizer = SelectorTokenizer::default();
    for character in selector.chars() {
        tokenizer.consume(character)?;
    }
    tokenizer.finish()
}

impl SelectorTokenizer {
    fn consume(&mut self, character: char) -> Option<()> {
        if self.consume_quoted(character) {
            return Some(());
        }
        self.consume_unquoted(character)
    }

    fn consume_unquoted(&mut self, character: char) -> Option<()> {
        if self.unsupported_sibling_combinator(character) {
            return None;
        }
        match character {
            '\'' | '"' if self.bracket_depth > 0 => self.start_quote(character),
            '[' => self.open_attribute()?,
            ']' => self.close_attribute()?,
            '(' if self.bracket_depth == 0 => self.open_parenthesis()?,
            ')' if self.bracket_depth == 0 => self.close_parenthesis()?,
            '>' if self.outside_group() => self.push_child()?,
            value if value.is_whitespace() && self.outside_group() => self.push_whitespace(),
            value => self.push_value(value),
        }
        Some(())
    }

    fn unsupported_sibling_combinator(&self, character: char) -> bool {
        self.outside_group() && matches!(character, '+' | '~')
    }

    fn outside_group(&self) -> bool {
        self.bracket_depth == 0 && self.parenthesis_depth == 0
    }

    fn consume_quoted(&mut self, character: char) -> bool {
        let Some(quote) = self.quote else {
            return false;
        };
        self.current.push(character);
        if quote == character {
            self.quote = None;
        }
        true
    }

    fn start_quote(&mut self, character: char) {
        self.quote = Some(character);
        self.current.push(character);
    }

    fn open_attribute(&mut self) -> Option<()> {
        self.bracket_depth = self.bracket_depth.checked_add(1)?;
        self.current.push('[');
        Some(())
    }

    fn close_attribute(&mut self) -> Option<()> {
        self.bracket_depth = self.bracket_depth.checked_sub(1)?;
        self.current.push(']');
        Some(())
    }

    fn open_parenthesis(&mut self) -> Option<()> {
        self.parenthesis_depth = self.parenthesis_depth.checked_add(1)?;
        self.current.push('(');
        Some(())
    }

    fn close_parenthesis(&mut self) -> Option<()> {
        self.parenthesis_depth = self.parenthesis_depth.checked_sub(1)?;
        self.current.push(')');
        Some(())
    }

    fn push_child(&mut self) -> Option<()> {
        self.push_compound();
        if !matches!(self.tokens.last(), Some(SelectorToken::Compound(_))) {
            return None;
        }
        self.tokens
            .push(SelectorToken::Combinator(CssCombinator::Child));
        self.pending_descendant = false;
        Some(())
    }

    fn push_whitespace(&mut self) {
        self.push_compound();
        self.pending_descendant = matches!(self.tokens.last(), Some(SelectorToken::Compound(_)));
    }

    fn push_value(&mut self, value: char) {
        if self.pending_descendant {
            self.tokens
                .push(SelectorToken::Combinator(CssCombinator::Descendant));
            self.pending_descendant = false;
        }
        self.current.push(value);
    }

    fn push_compound(&mut self) {
        if !self.current.is_empty() {
            self.tokens
                .push(SelectorToken::Compound(std::mem::take(&mut self.current)));
        }
    }

    fn finish(mut self) -> Option<Vec<SelectorToken>> {
        if self.bracket_depth != 0 || self.parenthesis_depth != 0 || self.quote.is_some() {
            return None;
        }
        self.push_compound();
        matches!(self.tokens.last(), Some(SelectorToken::Compound(_))).then_some(self.tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::CssSelector;

    #[test]
    fn selectors_reject_unmatched_brackets_and_parentheses() {
        for source in ["div]", "div[", "span)", "span:nth-child("] {
            assert!(CssSelector::parse(source).is_none(), "{source}");
        }
    }

    #[test]
    fn selectors_require_a_right_hand_side_after_child_combinators() {
        for source in ["main >", "main > > p"] {
            assert!(CssSelector::parse(source).is_none(), "{source}");
        }
        assert!(CssSelector::parse("main > p").is_some());
    }
}
