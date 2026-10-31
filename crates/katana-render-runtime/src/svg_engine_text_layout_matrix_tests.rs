use crate::svg_engine::usvg::{self, Node, Options, Tree};
use std::sync::Arc;

#[path = "svg_engine_text_layout_position_tests.rs"]
mod position_tests;
#[path = "svg_engine_text_layout_spacing_tests.rs"]
mod spacing_tests;
#[path = "svg_engine_text_layout_variation_tests.rs"]
mod variation_tests;

const FONT: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
const TOLERANCE: f32 = 0.001;

fn parse(attributes: &str, content: &str) -> Result<Tree, String> {
    let mut database = usvg::fontdb::Database::new();
    database.load_font_data(FONT.to_vec());
    let options = Options {
        fontdb: Arc::new(database),
        ..Options::default()
    };
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='200' height='100'><text id='sample' x='20' y='50' font-family='Noto Sans' font-size='20' {attributes}>{content}</text></svg>"
    );
    Tree::from_str(&svg, &options).map_err(|error| error.to_string())
}

fn text(tree: &Tree) -> Result<&usvg::Text, String> {
    match tree.node_by_id("sample") {
        Some(Node::Text(text)) => Ok(text),
        _ => Err("text missing".to_owned()),
    }
}

fn near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < TOLERANCE,
        "{actual} != {expected}"
    );
}
