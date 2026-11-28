use super::super::FontShape;
use super::{Arc, SelectedFace};
use crate::markdown::svg_rasterize::font_file_fixture::{self, FontFileFixture};
use resvg::usvg::FontVariation;
use std::error::Error;

pub(super) const FONT_SIZE: f32 = 18.0;
pub(super) const REGULAR_WEIGHT: u16 = 400;
pub(super) const INTERMEDIATE_WEIGHT: u16 = 401;
const CONDENSED_WIDTH: f32 = 75.0;
const ROBOTO_FLEX: &[u8] = include_bytes!(
    "../../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/RobotoFlex.subset.ttf"
);

pub(super) fn roboto_flex(
    directory: &tempfile::TempDir,
) -> Result<FontFileFixture, Box<dyn Error>> {
    font_file_fixture::create(directory.path().join("RobotoFlex.ttf"), ROBOTO_FLEX)
}

pub(super) fn variable_face(fixture: &FontFileFixture) -> SelectedFace {
    SelectedFace {
        database: Arc::new(fixture.database.clone()),
        id: fixture.face_id,
        alias: "Roboto Flex".into(),
        variations: vec![FontVariation::new(*b"wdth", CONDENSED_WIDTH)],
    }
}

pub(super) fn duplicate_width_face(fixture: &FontFileFixture) -> SelectedFace {
    let mut face = variable_face(fixture);
    face.variations.push(FontVariation::new(*b"wdth", 100.0));
    face
}

pub(super) fn regular_shape() -> FontShape {
    FontShape::new(FONT_SIZE, REGULAR_WEIGHT, false)
}
