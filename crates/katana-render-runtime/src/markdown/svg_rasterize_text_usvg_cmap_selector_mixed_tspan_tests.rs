#[cfg(target_os = "macos")]
use resvg::usvg;

#[cfg(target_os = "macos")]
const LATIN_FAMILY: &str = "Trebuchet MS";
#[cfg(target_os = "macos")]
const EXPLICIT_CJK_FAMILY: &str = "Hiragino Kaku Gothic ProN";
#[cfg(target_os = "macos")]
const FALLBACK_CJK_FAMILY: &str = "Hiragino Sans";
#[cfg(target_os = "macos")]
const MIXED_TEXT: &str = "ABC動物";
#[cfg(target_os = "macos")]
const EXPLICIT_GLYPH_COUNT: usize = 3;
#[cfg(target_os = "macos")]
const FALLBACK_LATIN_GLYPH_COUNT: usize = 3;
#[cfg(target_os = "macos")]
const FALLBACK_CJK_START_INDEX: usize = FALLBACK_LATIN_GLYPH_COUNT;

#[cfg(target_os = "macos")]
#[test]
fn same_chunk_mixed_tspans_preserve_faces_at_weight_400() -> Result<(), String> {
    assert_explicit_tspan_faces(400)
}

#[cfg(target_os = "macos")]
#[test]
fn same_chunk_mixed_tspans_preserve_faces_at_weight_700() -> Result<(), String> {
    assert_explicit_tspan_faces(700)
}

#[cfg(target_os = "macos")]
fn assert_explicit_tspan_faces(weight: u16) -> Result<(), String> {
    let tree = explicit_tspan_tree(weight)?;
    let text = mixed_text(&tree)?;
    if text.chunks().len() != 1 || text.layouted().len() != 2 {
        return Err("explicit tspans did not remain one chunk with two spans".to_string());
    }
    validate_explicit_glyphs(&tree, text)
}

#[cfg(target_os = "macos")]
fn explicit_tspan_tree(weight: u16) -> Result<usvg::Tree, String> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="30"><text x="0" y="20" font-size="16" font-weight="{weight}"><tspan font-family="{LATIN_FAMILY}">A</tspan><tspan font-family="{EXPLICIT_CJK_FAMILY}">日本</tspan></text></svg>"#
    );
    let database = super::super::super::html_font_db_for_text(
        &format!("{LATIN_FAMILY}, {EXPLICIT_CJK_FAMILY}"),
        "A日本",
    );
    let mut options = usvg::Options {
        fontdb: database,
        ..usvg::Options::default()
    };
    options.font_resolver.select_font = super::html_font_selector();
    options.font_resolver.select_fallback = Box::new(|_, _, _| None);
    usvg::Tree::from_str(&svg, &options).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn validate_explicit_glyphs(tree: &usvg::Tree, text: &usvg::Text) -> Result<(), String> {
    let glyphs = positioned_glyphs(text);
    if glyphs.len() != EXPLICIT_GLYPH_COUNT
        || glyphs[1].font != glyphs[2].font
        || glyphs[0].font == glyphs[1].font
    {
        return Err("explicit mixed glyph faces changed".to_string());
    }
    assert_glyph_face(tree, glyphs[0].font, LATIN_FAMILY)?;
    assert_glyph_face(tree, glyphs[1].font, EXPLICIT_CJK_FAMILY)
}

#[cfg(target_os = "macos")]
#[test]
fn same_chunk_mixed_latin_and_cjk_preserves_fallback_faces() -> Result<(), String> {
    let tree = fallback_chunk_tree()?;
    let text = mixed_text(&tree)?;
    if text.chunks().len() != 1 {
        return Err(format!(
            "expected one shaping chunk, found {}",
            text.chunks().len()
        ));
    }
    validate_fallback_glyphs(&tree, text)
}

#[cfg(target_os = "macos")]
fn fallback_chunk_tree() -> Result<usvg::Tree, String> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="30"><text x="0" y="20" font-family="{LATIN_FAMILY}" font-size="16">{MIXED_TEXT}</text></svg>"#
    );
    let database = super::super::super::html_font_db_for_text(
        &format!("{LATIN_FAMILY}, {FALLBACK_CJK_FAMILY}"),
        MIXED_TEXT,
    );
    let mut options = usvg::Options {
        fontdb: database,
        ..usvg::Options::default()
    };
    options.font_resolver.select_font = super::html_font_selector();
    options.font_resolver.select_fallback = super::html_selector();
    usvg::Tree::from_str(&svg, &options).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn validate_fallback_glyphs(tree: &usvg::Tree, text: &usvg::Text) -> Result<(), String> {
    let glyphs = positioned_glyphs(text);
    if glyphs.len() != MIXED_TEXT.chars().count() {
        return Err("mixed fallback glyph count changed".to_string());
    }
    let latin = glyphs[0].font;
    let cjk = glyphs[FALLBACK_CJK_START_INDEX].font;
    if glyphs[..FALLBACK_LATIN_GLYPH_COUNT]
        .iter()
        .any(|glyph| glyph.font != latin)
        || glyphs[FALLBACK_CJK_START_INDEX..]
            .iter()
            .any(|glyph| glyph.font != cjk)
        || latin == cjk
    {
        return Err("single chunk replaced the Latin run with its fallback face".to_string());
    }
    assert_glyph_face(tree, latin, LATIN_FAMILY)?;
    assert_glyph_face(tree, cjk, FALLBACK_CJK_FAMILY)
}

#[cfg(target_os = "macos")]
fn positioned_glyphs(text: &usvg::Text) -> Vec<&usvg::layout::PositionedGlyph> {
    text.layouted()
        .iter()
        .flat_map(|span| span.positioned_glyphs.iter())
        .collect()
}

#[cfg(target_os = "macos")]
fn mixed_text(tree: &usvg::Tree) -> Result<&usvg::Text, String> {
    let texts = tree
        .root()
        .children()
        .iter()
        .filter_map(|node| match node {
            usvg::Node::Text(text) => Some(text.as_ref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if texts.len() != 1 {
        return Err(format!("expected one text node, found {}", texts.len()));
    }
    texts
        .into_iter()
        .next()
        .ok_or_else(|| "text missing".to_string())
}

#[cfg(target_os = "macos")]
fn assert_glyph_face(
    tree: &usvg::Tree,
    font: resvg::usvg::fontdb::ID,
    expected: &str,
) -> Result<(), String> {
    let face = tree.fontdb().face(font).ok_or("glyph face missing")?;
    if face.families.iter().any(|(name, _)| name == expected) {
        Ok(())
    } else {
        Err(format!(
            "glyph selected an unexpected face: expected {expected}"
        ))
    }
}
