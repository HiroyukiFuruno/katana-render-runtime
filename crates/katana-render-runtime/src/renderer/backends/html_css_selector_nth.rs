use super::CssCompoundSelector;
use super::CssNthExpression;

pub(super) fn parse_nth_child<'a>(
    compound: &mut CssCompoundSelector,
    source: &'a str,
) -> Option<&'a str> {
    let rest = source.strip_prefix(":nth-child(")?;
    let end = rest.find(')')?;
    if compound.nth_child.is_some() {
        return None;
    }
    compound.nth_child = Some(CssNthExpression::parse(&rest[..end])?);
    Some(&rest[end + 1..])
}

impl CssNthExpression {
    pub(super) fn parse(source: &str) -> Option<Self> {
        let normalized = source
            .chars()
            .filter(|character| !character.is_ascii_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();
        match normalized.as_str() {
            "odd" => return Some(Self { step: 2, offset: 1 }),
            "even" => return Some(Self { step: 2, offset: 0 }),
            _ => {}
        }
        let Some((step, offset)) = normalized.split_once('n') else {
            return normalized
                .parse::<i32>()
                .ok()
                .map(|offset| Self { step: 0, offset });
        };
        let step = match step {
            "" | "+" => 1,
            "-" => -1,
            value => value.parse().ok()?,
        };
        let offset = if offset.is_empty() {
            0
        } else {
            offset.parse().ok()?
        };
        Some(Self { step, offset })
    }
}
