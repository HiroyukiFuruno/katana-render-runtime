use std::str::FromStr;

use super::support::GeometryValue;
use svgtypes::Length;

pub(super) fn parse_geometry_value(value: &str) -> Option<GeometryValue> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("auto") {
        return Some(GeometryValue::Auto);
    }
    let Ok(length) = Length::from_str(value) else {
        return unsupported_css_value(value).then_some(GeometryValue::Unsupported);
    };
    if !length.number.is_finite() || length.number < 0.0 {
        return None;
    }
    let value = if length.number == 0.0 && value.starts_with('-') {
        value.strip_prefix('-').unwrap_or(value)
    } else {
        value
    };
    Some(GeometryValue::Length(value.to_owned()))
}

fn unsupported_css_value(value: &str) -> bool {
    let mut parser = cssparser::Parser::new(value);
    match parser.next() {
        Ok(cssparser::Token::Function(name)) if unsupported_function(name.as_ref()) => {
            parser
                .parse_nested_block(|nested| {
                    let mut contains_tokens = false;
                    while nested.next().is_ok() {
                        contains_tokens = true;
                    }
                    Ok::<_, cssparser::ParseError<()>>(contains_tokens)
                })
                .is_ok_and(|contains_tokens| contains_tokens)
                && parser.expect_exhausted().is_ok()
        }
        Ok(cssparser::Token::Ident(name)) if unsupported_keyword(name.as_ref()) => {
            parser.expect_exhausted().is_ok()
        }
        _ => false,
    }
}

fn unsupported_function(name: &str) -> bool {
    [
        "var", "calc", "min", "max", "clamp", "env", "attr", "round", "mod", "rem", "sin", "cos",
        "tan", "asin", "acos", "atan", "atan2", "pow", "sqrt", "hypot", "log", "exp",
    ]
    .iter()
    .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

fn unsupported_keyword(name: &str) -> bool {
    ["inherit", "initial", "unset", "revert", "revert-layer"]
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
}
