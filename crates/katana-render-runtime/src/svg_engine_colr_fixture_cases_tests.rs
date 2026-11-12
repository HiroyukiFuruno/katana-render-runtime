use super::super::{LINEAR_GRADIENT, RADIAL_GRADIENT};
use super::{
    clip_box_bounds, fixture_tree, verify_standard_case, verify_static_font_manifest,
    verify_unclipped_glyph, with_static_font,
};

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

#[test]
fn colr_fixture_rejects_glyph_without_unicode_mapping() -> Result<(), String> {
    const MAPPED_GLYPH_ID: u32 = 96;
    verify_static_font_manifest()?;
    let (valid, _) = fixture_tree(MAPPED_GLYPH_ID, "mapped-glyph-control")?;
    assert!(valid.size().width() > 0.0 && valid.size().height() > 0.0);
    let Err(error) = verify_standard_case("unmapped-glyph", u32::MAX, RADIAL_GRADIENT) else {
        return Err("unmapped glyph unexpectedly passed the COLR roundtrip".to_owned());
    };
    assert_eq!(
        error,
        format!("COLR glyph {} has no Unicode mapping", u32::MAX)
    );
    assert_eq!(
        verify_unclipped_glyph(u32::MAX),
        Err(format!("COLR glyph {} has no Unicode mapping", u32::MAX))
    );
    Ok(())
}

#[test]
fn colr_fixture_rejects_mapped_glyph_without_vector_paths() {
    /* WHY: 同梱フォントのU+FE001はUnicodeに対応するが、輪郭とCOLRの描画を持たない。 */
    const MAPPED_PATHLESS_GLYPH_ID: u32 = 1;
    assert_eq!(
        verify_unclipped_glyph(MAPPED_PATHLESS_GLYPH_ID),
        Err(format!(
            "glyph-{MAPPED_PATHLESS_GLYPH_ID}-without-clip-box did not produce vector paths"
        ))
    );
}

#[test]
fn colr_roundtrip_rejects_a_missing_paint_semantic() -> Result<(), String> {
    const GLYPH: u32 = 96;
    const CASE: &str = "missing-colr-semantic";
    const MISSING_SEMANTIC: &str = "unsupported-fixture-paint";
    verify_static_font_manifest()?;
    verify_standard_case(CASE, GLYPH, RADIAL_GRADIENT)?;
    assert_eq!(
        verify_standard_case(CASE, GLYPH, MISSING_SEMANTIC).err(),
        Some(format!(
            "{CASE} lost COLR paint semantic {MISSING_SEMANTIC}"
        ))
    );
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

fn verify_clip_box_metadata() -> Result<(), String> {
    with_static_font(|font| {
        for expected in EXPECTED_CLIP_BOXES {
            let actual = clip_box_bounds(font, expected.glyph_id)
                .ok_or_else(|| format!("glyph {} has no COLR clip box", expected.glyph_id))?;
            if actual != expected.bounds {
                return Err(format!(
                    "glyph {} clip box metadata mismatch",
                    expected.glyph_id
                ));
            }
        }
        for glyph_id in NO_COLRV1_CLIP_BOX_GLYPHS {
            if clip_box_bounds(font, glyph_id).is_some() {
                return Err(format!("glyph {glyph_id} unexpectedly has a COLR clip box"));
            }
        }
        Ok(())
    })
}
