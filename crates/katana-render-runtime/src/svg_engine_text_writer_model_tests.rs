use super::{AlignmentBaseline, BaselineShift, DominantBaseline, FontStretch, TextSpan};
use crate::svg_engine::usvg::{self, Group, Node, Options, Tree, WriteOptions};
use std::sync::Arc;

const FONT: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40"><text id="sample" x="1" y="25" font-family="Noto Sans" font-size="16">Text</text></svg>"#;

fn fixture() -> Result<Tree, String> {
    let mut database = usvg::fontdb::Database::new();
    database.load_font_data(FONT.to_vec());
    let options = Options {
        fontdb: Arc::new(database),
        ..Options::default()
    };
    Tree::from_str(SVG, &options).map_err(|error| error.to_string())
}

fn first_span(group: &mut Group) -> Option<&mut TextSpan> {
    group.children.iter_mut().find_map(|node| match node {
        Node::Text(text) => text.chunks.first_mut()?.spans.first_mut(),
        Node::Group(group) => first_span(group),
        _ => None,
    })
}

fn serialized(tree: &Tree) -> String {
    tree.to_string(&WriteOptions {
        preserve_text: true,
        ..WriteOptions::default()
    })
}

fn assert_tspan_attribute(svg: &str, name: &str, expected: &str) -> Result<(), String> {
    let doc = usvg::roxmltree::Document::parse(svg).map_err(|error| error.to_string())?;
    assert!(
        doc.descendants()
            .any(|node| node.has_tag_name("tspan") && node.attribute(name) == Some(expected)),
        "missing {name}={expected}"
    );
    Ok(())
}

#[test]
fn writer_serializes_valid_dominant_baseline_model_values() -> Result<(), String> {
    let original = fixture()?;
    for (value, expected) in [
        (DominantBaseline::UseScript, "use-script"),
        (DominantBaseline::NoChange, "no-change"),
        (DominantBaseline::ResetSize, "reset-size"),
        (DominantBaseline::Hanging, "hanging"),
    ] {
        let mut tree = original.clone();
        first_span(&mut tree.root)
            .ok_or("text span missing")?
            .dominant_baseline = value;
        assert_tspan_attribute(&serialized(&tree), "dominant-baseline", expected)?;
    }
    Ok(())
}

#[test]
fn writer_serializes_alignment_and_font_stretch_model_values() -> Result<(), String> {
    let original = fixture()?;
    for (value, expected) in [
        (AlignmentBaseline::BeforeEdge, "before-edge"),
        (AlignmentBaseline::Hanging, "hanging"),
        (AlignmentBaseline::Middle, "middle"),
    ] {
        let mut tree = original.clone();
        first_span(&mut tree.root)
            .ok_or("text span missing")?
            .alignment_baseline = value;
        assert_tspan_attribute(&serialized(&tree), "alignment-baseline", expected)?;
    }
    let mut tree = original.clone();
    first_span(&mut tree.root)
        .ok_or("text span missing")?
        .font
        .stretch = FontStretch::ExtraExpanded;
    assert_tspan_attribute(&serialized(&tree), "font-stretch", "extra-expanded")
}

#[test]
fn writer_keeps_mixed_baseline_shift_nesting() -> Result<(), String> {
    let mut tree = fixture()?;
    first_span(&mut tree.root)
        .ok_or("text span missing")?
        .baseline_shift = vec![
        BaselineShift::Baseline,
        BaselineShift::Number(2.0),
        BaselineShift::Subscript,
    ];
    let svg = serialized(&tree);
    let doc = usvg::roxmltree::Document::parse(&svg).map_err(|error| error.to_string())?;
    let number = doc
        .descendants()
        .find(|node| node.attribute("baseline-shift") == Some("2"))
        .ok_or("number shift missing")?;
    let parent = number.parent().ok_or("baseline shift parent missing")?;
    assert!(parent.has_tag_name("tspan"));
    assert_eq!(parent.attribute("baseline-shift"), None);
    assert!(
        number
            .children()
            .any(|node| node.attribute("baseline-shift") == Some("sub"))
    );
    Ok(())
}
