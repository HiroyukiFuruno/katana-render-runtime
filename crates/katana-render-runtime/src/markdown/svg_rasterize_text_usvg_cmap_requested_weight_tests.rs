use resvg::usvg::fontdb::{Database, ID, Weight};
use resvg::usvg::{self, Node};
use std::sync::Arc;

const NORMAL_WEIGHT: u16 = 400;
const MEDIUM_WEIGHT: u16 = 500;
const SEMIBOLD_WEIGHT: u16 = 600;
const BOLD_WEIGHT: u16 = 700;
const WEIGHTS: [u16; 4] = [NORMAL_WEIGHT, MEDIUM_WEIGHT, SEMIBOLD_WEIGHT, BOLD_WEIGHT];
const FALLBACK_GLYPH: char = 'あ';

type ExpectedFaces = [(u16, ID); WEIGHTS.len()];

#[test]
fn mixed_tspans_keep_requested_fallback_weight_in_both_orders() -> Result<(), String> {
    let (database, faces) = registered_weight_faces()?;
    for weights in [
        [SEMIBOLD_WEIGHT, BOLD_WEIGHT],
        [BOLD_WEIGHT, SEMIBOLD_WEIGHT],
        [NORMAL_WEIGHT, MEDIUM_WEIGHT],
        [MEDIUM_WEIGHT, NORMAL_WEIGHT],
    ] {
        let tree = parse_weight_fixture(Arc::clone(&database), weights)?;
        let Some(Node::Text(text)) = tree.node_by_id("x") else {
            return Err("text missing".into());
        };
        assert_uncached_requested_selection(&tree, text, weights, &faces)?;
        assert_layouted_selection(text, weights, &faces)?;
    }
    Ok(())
}

fn parse_weight_fixture(database: Arc<Database>, weights: [u16; 2]) -> Result<usvg::Tree, String> {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><text id='x' x='0' y='20' font-family='Noto Sans' font-size='16'><tspan font-weight='{}'>{FALLBACK_GLYPH}</tspan><tspan font-weight='{}'>{FALLBACK_GLYPH}</tspan></text></svg>",
        weights[0], weights[1]
    );
    parse_svg(&svg, database)
}

fn registered_weight_faces() -> Result<(Arc<Database>, ExpectedFaces), String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/fonts/han-kerning/NotoSansCJKjp-han14.otf"
    );
    let mut source = Database::new();
    source
        .load_font_file(path)
        .map_err(|error| error.to_string())?;
    let fallback = source.faces().next().ok_or("fixture fallback missing")?;
    let mut database = Database::new();
    database.load_font_data(super::super::tests::FONT_BYTES.to_vec());
    let faces = WEIGHTS.map(|weight| {
        let mut face = fallback.clone();
        face.weight = Weight(weight);
        (weight, database.push_face_info(face))
    });
    Ok((Arc::new(database), faces))
}

#[test]
fn japanese_system_ui_keeps_the_normal_and_bold_interface_faces() -> Result<(), String> {
    let svg = "<svg xmlns='http://www.w3.org/2000/svg'><text id='ui-normal' x='0' y='50' font-family='system-ui' font-size='16' font-weight='400'>A注</text><text id='ui-bold' x='0' y='80' font-family='system-ui' font-size='16' font-weight='700'>A注</text></svg>";
    let database = super::super::super::super::font::html_font_db_for_markup(svg);
    assert!(database.faces().any(|face| {
        face.families
            .iter()
            .any(|(family, _)| super::super::super::super::font::is_internal_family(family))
    }));
    let tree = parse_svg(svg, database)?;
    assert_interface_faces(&tree)
}

fn parse_svg(svg: &str, database: Arc<Database>) -> Result<usvg::Tree, String> {
    let mut options = usvg::Options {
        fontdb: database,
        ..usvg::Options::default()
    };
    super::super::install(&mut options);
    super::super::super::fallback_memo::with_validated_tree_parse(|| {
        usvg::Tree::from_str(svg, &options)
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

fn expected_face(faces: &ExpectedFaces, weight: u16) -> Result<ID, String> {
    faces
        .iter()
        .find(|(registered, _)| *registered == weight)
        .map(|(_, id)| *id)
        .ok_or_else(|| format!("unregistered fixture weight {weight}"))
}

fn assert_layouted_selection(
    text: &usvg::Text,
    weights: [u16; 2],
    faces: &ExpectedFaces,
) -> Result<(), String> {
    let spans = text.layouted();
    assert_eq!(spans.len(), 2);
    for (span, weight) in spans.iter().zip(weights) {
        let glyph = span.positioned_glyphs.first().ok_or("glyph missing")?;
        assert_eq!(
            glyph.font,
            expected_face(faces, weight)?,
            "requested weight {weight}"
        );
    }
    Ok(())
}

fn assert_uncached_requested_selection(
    tree: &usvg::Tree,
    text: &usvg::Text,
    weights: [u16; 2],
    faces: &ExpectedFaces,
) -> Result<(), String> {
    let source = text.chunks().first().ok_or("chunk missing")?;
    let mut database = tree.fontdb().clone();
    let selector = super::html_styled_selector();
    let primary = super::html_font_selector();
    for (span, weight) in source.spans().iter().zip(weights) {
        let base = primary(span.font(), &mut database).ok_or("primary missing")?;
        let chosen = selector(span.font(), FALLBACK_GLYPH, &[base], &mut database)
            .ok_or("uncached fallback missing")?;
        let face = database.face(chosen).ok_or("uncached face missing")?;
        assert_eq!(
            chosen,
            expected_face(faces, weight)?,
            "uncached weight {weight}"
        );
        assert!(matches!(face.source, usvg::fontdb::Source::File(_)));
        assert!(!super::super::super::font::memo_usable());
        let retry = selector(span.font(), FALLBACK_GLYPH, &[base, chosen], &mut database);
        assert!(
            retry.is_some_and(|face| face != base && face != chosen),
            "{retry:?}"
        );
    }
    Ok(())
}
