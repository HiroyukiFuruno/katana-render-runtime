use std::ops::Range;

use cssparser::{
    AtRuleParser, CowRcStr, ParseError, Parser, ParserState, QualifiedRuleParser, StyleSheetParser,
};

use super::selector;
use super::selector::consume_nested;

const MAX_STYLE_BYTES: usize = 48 * 1024;
const MAX_REWRITTEN_STYLE_BYTES: usize = 96 * 1024;
const MAX_RULE_EDITS: usize = 256;

pub(super) fn rewrite(source: &str, ids: &[String], max_output_bytes: usize) -> Option<String> {
    let max_output_bytes = max_output_bytes.min(MAX_REWRITTEN_STYLE_BYTES);
    if source.len() > MAX_STYLE_BYTES || source.len() > max_output_bytes {
        return None;
    }
    let mut input = Parser::new(source);
    let mut parser = MarkerStylesheetParser {
        source,
        ids,
        max_output_bytes,
        edits: Vec::new(),
    };
    if StyleSheetParser::new(&mut input, &mut parser).any(|result| result.is_err()) {
        return None;
    }
    apply_edits(source, parser.edits, max_output_bytes)
}

fn apply_edits(source: &str, mut edits: Vec<Edit>, max_output_bytes: usize) -> Option<String> {
    edits.sort_by_key(|edit| edit.range.start);
    if !valid_edits(source, &edits) {
        return None;
    }
    let output_size = edits.iter().fold(source.len(), |size, edit| {
        size - edit.range.len() + edit.replacement.len()
    });
    if output_size > max_output_bytes {
        return None;
    }
    let mut output = source.to_owned();
    for edit in edits.into_iter().rev() {
        output.replace_range(edit.range, &edit.replacement);
    }
    Some(output)
}

fn valid_edits(source: &str, edits: &[Edit]) -> bool {
    edits
        .iter()
        .all(|edit| edit.range.start < edit.range.end && source.get(edit.range.clone()).is_some())
        && edits
            .windows(2)
            .all(|pair| pair[0].range.end <= pair[1].range.start)
}

struct Edit {
    range: Range<usize>,
    replacement: String,
}

struct MarkerStylesheetParser<'a> {
    source: &'a str,
    ids: &'a [String],
    max_output_bytes: usize,
    edits: Vec<Edit>,
}

impl<'i> QualifiedRuleParser<'i> for MarkerStylesheetParser<'_> {
    type Prelude = Option<Edit>;
    type QualifiedRule = ();
    type Error = ();

    fn parse_prelude(
        &mut self,
        input: &mut Parser<'i>,
    ) -> Result<Self::Prelude, ParseError<Self::Error>> {
        let start = input.position().byte_index();
        consume_nested(input)?;
        let end = input.position().byte_index();
        let Some(prelude) = self.source.get(start..end) else {
            return Err(ParseError::unexpected_token());
        };
        if !prelude.as_bytes().contains(&b'$') && !prelude.as_bytes().contains(&b'\\') {
            return Ok(None);
        }
        let Some(max_output_bytes) = remaining_output_budget(
            self.source.len(),
            &self.edits,
            end - start,
            self.max_output_bytes,
        ) else {
            return Err(ParseError::unexpected_token());
        };
        Ok(
            selector::rewrite_selector_list(prelude, self.ids, max_output_bytes).map(
                |replacement| Edit {
                    range: start..end,
                    replacement,
                },
            ),
        )
    }

    fn parse_block(
        &mut self,
        edit: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Self::QualifiedRule, ParseError<Self::Error>> {
        if let Some(edit) = edit {
            if !can_add_edit(self.source.len(), &self.edits, &edit, self.max_output_bytes) {
                return Err(ParseError::unexpected_token());
            }
            self.edits.push(edit);
        }
        consume_nested(input)?;
        Ok(())
    }
}

fn can_add_edit(source_len: usize, edits: &[Edit], next: &Edit, max_output_bytes: usize) -> bool {
    if edits.len() >= MAX_RULE_EDITS {
        return false;
    }
    let current_size = edits.iter().fold(source_len, |size, edit| {
        size - edit.range.len() + edit.replacement.len()
    });
    let next_size = current_size - next.range.len() + next.replacement.len();
    next_size <= max_output_bytes
}

fn remaining_output_budget(
    source_len: usize,
    edits: &[Edit],
    range_len: usize,
    max_output_bytes: usize,
) -> Option<usize> {
    let current_size = edits.iter().fold(source_len, |size, edit| {
        size - edit.range.len() + edit.replacement.len()
    });
    max_output_bytes
        .checked_sub(current_size)?
        .checked_add(range_len)
}

impl<'i> AtRuleParser<'i> for MarkerStylesheetParser<'_> {
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
        consume_nested(input)
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::{Edit, MarkerStylesheetParser, apply_edits, rewrite};
    use cssparser::{Parser, QualifiedRuleParser};

    #[test]
    fn stylesheet_budget_rejects_source_before_rewriting() {
        assert!(rewrite("g{fill:red}", &[], 0).is_none());
    }

    #[test]
    fn stylesheet_edits_reject_overlap_and_oversized_output() {
        let overlapping = vec![
            Edit {
                range: 0..2,
                replacement: "x".to_owned(),
            },
            Edit {
                range: 1..3,
                replacement: "y".to_owned(),
            },
        ];
        assert!(apply_edits("abc", overlapping, usize::MAX).is_none());
        let oversized = vec![Edit {
            range: 0..1,
            replacement: "xx".to_owned(),
        }];
        assert!(apply_edits("a", oversized, 1).is_none());
        let valid = vec![Edit {
            range: 0..1,
            replacement: "x".to_owned(),
        }];
        assert_eq!(apply_edits("a", valid, 1), Some("x".to_owned()));
    }

    #[test]
    fn stylesheet_prelude_rejects_unrelated_source_and_exhausted_budget() {
        let source = "[id$='end']";
        for backing in ["", source] {
            let mut input = Parser::new(source);
            let mut parser = MarkerStylesheetParser {
                source: backing,
                ids: &[],
                max_output_bytes: 0,
                edits: Vec::new(),
            };
            assert!(parser.parse_prelude(&mut input).is_err());
            assert!(parser.edits.is_empty());
        }
    }
}
