use rstest::rstest;

use super::super::{assert_render_roundtrip, usvg};
use super::{STATIC_FONT, fixture_tree, read_and_verify_manifest, verify_manifest_file};

const GLYPH_ID: u32 = 96;
const EXPECTED_FOCAL_RADIUS: f32 = 100.0;
const FIXTURE_CASE: &str = "glyph-96-radial-roundtrip";

#[rstest]
#[case("four-digit", true)]
#[case("standard-eight-digit", false)]
fn colrv1_radial_glyph_preserves_focal_radius_and_pixels(
    #[case] precision: &str,
    #[case] low_precision: bool,
) -> Result<(), String> {
    let manifest = read_and_verify_manifest()?;
    verify_manifest_file(&manifest, STATIC_FONT)?;
    let (original, database) = fixture_tree(GLYPH_ID, FIXTURE_CASE)?;
    let source_fr = focal_radius(&original)?;
    assert_eq!(source_fr, EXPECTED_FOCAL_RADIUS, "fixture focal radius");
    let serialized = serialize_variant(&original, low_precision);
    let reparsed = super::parse_fixture_svg(&serialized, database, precision)?;
    assert_eq!(focal_radius(&reparsed)?, source_fr, "writer focal radius");
    assert_render_roundtrip(&original, &reparsed, precision)
}

fn serialize_variant(tree: &usvg::Tree, low_precision: bool) -> String {
    let options = if low_precision {
        super::super::super::writer_options(FIXTURE_CASE)
    } else {
        usvg::WriteOptions::default()
    };
    tree.to_string(&options)
}

fn focal_radius(tree: &usvg::Tree) -> Result<f32, String> {
    tree.radial_gradients()
        .first()
        .map(|gradient| gradient.fr().get())
        .ok_or_else(|| "glyph-96 fixture has no radial gradient".to_owned())
}
