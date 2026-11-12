use resvg::usvg::{self, Node};

const NORMAL_WEIGHT: u16 = 400;
const MEDIUM_WEIGHT: u16 = 500;
const SEMIBOLD_WEIGHT: u16 = 600;
const BOLD_WEIGHT: u16 = 700;

#[test]
fn mixed_tspans_keep_requested_fallback_weight_in_both_orders() -> Result<(), String> {
    for weights in [
        [SEMIBOLD_WEIGHT, BOLD_WEIGHT],
        [BOLD_WEIGHT, SEMIBOLD_WEIGHT],
        [NORMAL_WEIGHT, MEDIUM_WEIGHT],
        [MEDIUM_WEIGHT, NORMAL_WEIGHT],
    ] {
        let tree = parse_weight_fixture(weights)?;
        assert_interface_faces(&tree)?;
        let Some(Node::Text(text)) = tree.node_by_id("x") else {
            return Err("text missing".into());
        };
        assert_uncached_requested_selection(&tree, text, weights)?;
        assert_layouted_selection(&tree, text, weights)?;
    }
    Ok(())
}

fn parse_weight_fixture(weights: [u16; 2]) -> Result<usvg::Tree, String> {
    let html = format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><text id='x' x='0' y='20' font-family='Helvetica' font-size='16'><tspan font-weight='{}'>注</tspan><tspan font-weight='{}'>注</tspan></text><text id='ui-normal' x='0' y='50' font-family='system-ui' font-size='16' font-weight='400'>A注</text><text id='ui-bold' x='0' y='80' font-family='system-ui' font-size='16' font-weight='700'>A注</text></svg>",
        weights[0], weights[1]
    );
    let database = super::super::super::super::font::html_font_db_for_markup(&html);
    assert!(database.faces().any(|face| {
        face.families
            .iter()
            .any(|(family, _)| super::super::super::super::font::is_internal_family(family))
    }));
    let mut options = usvg::Options {
        fontdb: database,
        ..usvg::Options::default()
    };
    super::super::install(&mut options);
    super::super::super::fallback_memo::with_validated_tree_parse(|| {
        usvg::Tree::from_str(&html, &options)
    })
    .map_err(|error| error.to_string())
}

fn assert_interface_faces(tree: &usvg::Tree) -> Result<(), String> {
    for (id, weight, expected) in [
        ("ui-normal", NORMAL_WEIGHT, ".HiraKakuInterface-W4"),
        ("ui-bold", BOLD_WEIGHT, ".HiraKakuInterface-W6"),
    ] {
        assert_interface_face(tree, id, weight, expected)?;
    }
    Ok(())
}

fn assert_interface_face(
    tree: &usvg::Tree,
    id: &str,
    weight: u16,
    expected: &str,
) -> Result<(), String> {
    let Some(Node::Text(text)) = tree.node_by_id(id) else {
        return Err(format!("{id} missing"));
    };
    let source = text
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .ok_or("interface source span missing")?;
    assert_eq!(source.font().weight(), weight);
    assert!(
        matches!(source.font().families().first(), Some(usvg::FontFamily::Named(name)) if name == "system-ui")
    );
    let glyph = text
        .layouted()
        .last()
        .and_then(|span| span.positioned_glyphs.last())
        .ok_or("interface glyph missing")?;
    let face = tree
        .fontdb()
        .face(glyph.font)
        .ok_or("interface face missing")?;
    assert_eq!(face.post_script_name, expected, "{id}");
    Ok(())
}

fn expected_face(weight: u16) -> Result<&'static str, String> {
    match weight {
        SEMIBOLD_WEIGHT => Ok("HiraginoSans-W5"),
        BOLD_WEIGHT => Ok("HiraginoSans-W6"),
        NORMAL_WEIGHT => Ok("HiraginoSans-W3"),
        MEDIUM_WEIGHT => Ok("HiraginoSans-W4"),
        _ => Err("invalid fixture weight".into()),
    }
}

fn assert_layouted_selection(
    tree: &usvg::Tree,
    text: &usvg::Text,
    weights: [u16; 2],
) -> Result<(), String> {
    let spans = text.layouted();
    assert_eq!(spans.len(), 2);
    for (span, weight) in spans.iter().zip(weights) {
        let glyph = span.positioned_glyphs.first().ok_or("glyph missing")?;
        let face = tree.fontdb().face(glyph.font).ok_or("face missing")?;
        assert_eq!(
            face.post_script_name,
            expected_face(weight)?,
            "requested weight {weight}"
        );
    }
    Ok(())
}

fn assert_uncached_requested_selection(
    tree: &usvg::Tree,
    text: &usvg::Text,
    weights: [u16; 2],
) -> Result<(), String> {
    let source = text.chunks().first().ok_or("chunk missing")?;
    let mut database = tree.fontdb().clone();
    let selector = super::html_styled_selector();
    let primary = super::html_font_selector();
    for (span, weight) in source.spans().iter().zip(weights) {
        let base = primary(span.font(), &mut database).ok_or("primary missing")?;
        let chosen = selector(span.font(), '注', &[base], &mut database)
            .ok_or("uncached fallback missing")?;
        let face = database.face(chosen).ok_or("uncached face missing")?;
        assert_eq!(
            face.post_script_name,
            expected_face(weight)?,
            "uncached weight {weight}"
        );
        assert!(matches!(face.source, usvg::fontdb::Source::File(_)));
        assert!(!super::super::super::font::memo_usable());
        let retry = selector(span.font(), '注', &[base, chosen], &mut database);
        assert!(
            retry.is_some_and(|face| face != base && face != chosen),
            "{retry:?}"
        );
    }
    Ok(())
}
