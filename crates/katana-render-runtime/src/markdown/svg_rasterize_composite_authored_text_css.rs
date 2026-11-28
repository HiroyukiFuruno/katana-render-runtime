#[path = "svg_rasterize_composite_authored_text_css_audit.rs"]
mod audit;
#[path = "svg_rasterize_composite_authored_text_css_stylesheet.rs"]
mod stylesheet;
pub(super) use audit::inline_style_is_safe;
pub(super) use stylesheet::sensitive_selectors;

const LAYOUT_SENSITIVE_PROPERTIES: &[&str] = &[
    "alignment-baseline",
    "baseline-shift",
    "dominant-baseline",
    "letter-spacing",
    "line-height",
    "text-anchor",
    "text-length",
    "textlength",
    "transform",
    "translate",
    "rotate",
    "scale",
    "text-orientation",
    "text-rendering",
    "text-transform",
    "vertical-align",
    "white-space",
    "word-spacing",
    "writing-mode",
    "direction",
    "unicode-bidi",
    "text-indent",
    "x",
    "y",
    "dx",
    "dy",
];

pub(super) fn is_sensitive_property(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "all"
        || name == "font"
        || name.starts_with("font-")
        || is_layout_sensitive_property(&name)
}

fn is_layout_sensitive_property(name: &str) -> bool {
    LAYOUT_SENSITIVE_PROPERTIES.contains(&name)
}

pub(super) fn selector_from_source(source: &str) -> Option<simplecss::Selector<'_>> {
    if has_unsupported_top_level_token(source) {
        return None;
    }
    let selector = simplecss::Selector::parse(source)?;
    Some(selector)
}

fn has_unsupported_top_level_token(source: &str) -> bool {
    /* WHY: simplecssがselector prefixを返すdelimiterをCSS tokenで検出し、quoted attribute内容はblockとして避ける。 */
    let mut input = cssparser::Parser::new(source);
    while !input.is_exhausted() {
        let Ok(token) = input.next() else { return true };
        if matches!(
            token,
            cssparser::Token::Colon
                | cssparser::Token::Comma
                | cssparser::Token::CurlyBracketBlock
                | cssparser::Token::Delim('/')
                | cssparser::Token::Delim('|')
                | cssparser::Token::Function(_)
                | cssparser::Token::ParenthesisBlock
                | cssparser::Token::Semicolon
        ) {
            return true;
        }
    }
    false
}
