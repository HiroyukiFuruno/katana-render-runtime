use super::usvg;
use super::{
    LINEAR_GRADIENT, RADIAL_GRADIENT, STATIC_FONT, assert_render_roundtrip, assert_svg_semantic,
    fixture_font_database, fixture_path, glyph_svg, parse_fixture_svg, read_and_verify_manifest,
    unicode_for_glyph, verify_manifest_file,
};
use skrifa::{
    FontRef, GlyphId, MetadataProvider,
    color::ColorGlyphFormat,
    instance::{LocationRef, Size},
};
use std::{fs, sync::Arc};

#[path = "svg_engine_colr_radial_tests.rs"]
mod radial_tests;
type PaintCase = (u32, &'static str);
type ClipBoxBounds = (f32, f32, f32, f32);

#[derive(Clone, Copy)]
struct ExpectedClipBox {
    glyph_id: u32,
    bounds: ClipBoxBounds,
}

const PAINT_CASES: [PaintCase; 17] = [
    (90, LINEAR_GRADIENT),
    (91, LINEAR_GRADIENT),
    (92, LINEAR_GRADIENT),
    (94, RADIAL_GRADIENT),
    (95, RADIAL_GRADIENT),
    (96, RADIAL_GRADIENT),
    (97, RADIAL_GRADIENT),
    (98, RADIAL_GRADIENT),
    (154, "fill=\"#000000\""),
    (133, "mix-blend-mode:screen"),
    (134, "mix-blend-mode:overlay"),
    (135, "mix-blend-mode:darken"),
    (136, "mix-blend-mode:lighten"),
    (137, "mix-blend-mode:color-dodge"),
    (138, "mix-blend-mode:color-burn"),
    (139, "mix-blend-mode:hard-light"),
    (140, "mix-blend-mode:soft-light"),
];

const MORE_COMPOSITE_CASES: [PaintCase; 7] = [
    (141, "mix-blend-mode:difference"),
    (142, "mix-blend-mode:exclusion"),
    (143, "mix-blend-mode:multiply"),
    (144, "mix-blend-mode:hue"),
    (145, "mix-blend-mode:saturation"),
    (146, "mix-blend-mode:color"),
    (147, "mix-blend-mode:luminosity"),
];

const EXPECTED_CLIP_BOXES: [ExpectedClipBox; 5] = [
    ExpectedClipBox {
        glyph_id: 156,
        bounds: (0.0, 500.0, 500.0, 1000.0),
    },
    ExpectedClipBox {
        glyph_id: 157,
        bounds: (0.0, 0.0, 500.0, 500.0),
    },
    ExpectedClipBox {
        glyph_id: 158,
        bounds: (500.0, 0.0, 1000.0, 500.0),
    },
    ExpectedClipBox {
        glyph_id: 159,
        bounds: (500.0, 500.0, 1000.0, 1000.0),
    },
    ExpectedClipBox {
        glyph_id: 160,
        bounds: (250.0, 250.0, 750.0, 750.0),
    },
];
const NO_COLRV1_CLIP_BOX_GLYPHS: [u32; 5] = [161, 162, 163, 164, 165];

#[test]
fn colrv1_gradient_and_composite_glyphs_roundtrip() -> Result<(), String> {
    verify_cases(PAINT_CASES.into_iter().chain(MORE_COMPOSITE_CASES))
}

#[test]
fn color_font_clip_metadata_and_writer_pixels_survive_roundtrip() -> Result<(), String> {
    verify_static_font_manifest()?;
    verify_clip_box_metadata()?;
    for expected in EXPECTED_CLIP_BOXES {
        verify_standard_case("glyph-clip-box", expected.glyph_id, "clipPath")?;
    }
    for glyph_id in NO_COLRV1_CLIP_BOX_GLYPHS {
        verify_unclipped_glyph(glyph_id)?;
    }
    Ok(())
}

fn verify_cases(cases: impl Iterator<Item = PaintCase>) -> Result<(), String> {
    verify_static_font_manifest()?;
    for (glyph_id, semantic) in cases {
        let case = format!("glyph-{glyph_id}");
        verify_standard_case(&case, glyph_id, semantic)?;
    }
    Ok(())
}

fn verify_static_font_manifest() -> Result<(), String> {
    let manifest = read_and_verify_manifest()?;
    verify_manifest_file(&manifest, STATIC_FONT)
}

fn verify_clip_box_metadata() -> Result<(), String> {
    let bytes = fs::read(fixture_path(STATIC_FONT)).map_err(|error| error.to_string())?;
    let font = FontRef::new(&bytes).map_err(|error| error.to_string())?;
    for expected in EXPECTED_CLIP_BOXES {
        let actual = clip_box_bounds(&font, expected.glyph_id)
            .ok_or_else(|| format!("glyph {} has no COLR clip box", expected.glyph_id))?;
        if actual != expected.bounds {
            return Err(format!(
                "glyph {} clip box metadata mismatch",
                expected.glyph_id
            ));
        }
    }
    for glyph_id in NO_COLRV1_CLIP_BOX_GLYPHS {
        if clip_box_bounds(&font, glyph_id).is_some() {
            return Err(format!("glyph {glyph_id} unexpectedly has a COLR clip box"));
        }
    }
    Ok(())
}

fn clip_box_bounds(font: &FontRef<'_>, glyph_id: u32) -> Option<ClipBoxBounds> {
    font.color_glyphs()
        .get_with_format(GlyphId::new(glyph_id), ColorGlyphFormat::ColrV1)
        .and_then(|glyph| glyph.bounding_box(LocationRef::default(), Size::unscaled()))
        .map(|bounds| (bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max))
}

fn verify_standard_case(case: &str, glyph_id: u32, semantic: &str) -> Result<(), String> {
    let (tree, database) = fixture_tree(glyph_id, case)?;
    let serialized = tree.to_string(&usvg::WriteOptions::default());
    assert_svg_semantic(&serialized, semantic, false, case)?;
    let reparsed = parse_fixture_svg(&serialized, database, case)?;
    let again = reparsed.to_string(&usvg::WriteOptions::default());
    assert_svg_semantic(&again, semantic, false, case)?;
    assert_render_roundtrip(&tree, &reparsed, case)
}

fn verify_unclipped_glyph(glyph_id: u32) -> Result<(), String> {
    let case = format!("glyph-{glyph_id}-without-clip-box");
    let (tree, database) = fixture_tree(glyph_id, &case)?;
    let serialized = tree.to_string(&usvg::WriteOptions::default());
    if !serialized.contains("<path") {
        return Err(format!("{case} did not produce vector paths"));
    }
    let reparsed = parse_fixture_svg(&serialized, database, &case)?;
    assert_render_roundtrip(&tree, &reparsed, &case)
}

fn fixture_tree(
    glyph_id: u32,
    case: &str,
) -> Result<(usvg::Tree, Arc<usvg::fontdb::Database>), String> {
    let path = fixture_path(STATIC_FONT);
    let bytes = fs::read(&path).map_err(|error| error.to_string())?;
    let character = unicode_for_glyph(&bytes, glyph_id)?;
    let database = fixture_font_database(&path)?;
    let svg = glyph_svg(&database, character, case)?;
    let tree = parse_fixture_svg(&svg, database.clone(), case)?;
    Ok((tree, database))
}
