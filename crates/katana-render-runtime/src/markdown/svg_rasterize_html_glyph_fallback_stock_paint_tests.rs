use super::{TITLE, prepare, text, tree};
use sha2::{Digest, Sha256};

/* WHY: R4381 drawTextBlobs[21] は stock Chrome の独立した描画値。
SKP SHA256: 5b9c113079c821dbb5996756f82e1bd875838591fc0996af6328612c299ded2a */
const ORIGIN_X: f32 = f32::from_bits(0x4357fc00);
const ORIGIN_Y: f32 = 19.984375;
const IDS: [u16; 37] = [
    928, 1007, 955, 660, 969, 959, 964, 972, 1007, 937, 1007, 940, 947, 949, 962, 988, 3, 36, 51,
    44, 926, 979, 998, 941, 660, 947, 995, 1007, 887, 943, 1007, 985, 660, 969, 1007, 964, 2596,
];
const X_BITS: [u32; 37] = [
    0x00000000, 0x417fd700, 0x41ffd700, 0x423fe140, 0x427fd700, 0x429fe660, 0x42bfe140, 0x42dfdc20,
    0x42ffd700, 0x430fe8f0, 0x431fe660, 0x432fe3d0, 0x433fe140, 0x434f1204, 0x435f0f74, 0x436e1749,
    0x437e14b9, 0x438172fa, 0x43862a38, 0x438a9f82, 0x438cd926, 0x4394d7de, 0x439cd696, 0x43a4d54e,
    0x43acd406, 0x43b4d2be, 0x43bcd176, 0x43c4d02e, 0x43cccee6, 0x43d4cd9e, 0x43dccc56, 0x43e4cb0e,
    0x43ec8c5f, 0x43f48b17, 0x43fc89cf, 0x44024444, 0x440643a0,
];
const FACES: [(&str, &str); 3] = [
    (
        "HiraginoSans-W3",
        "ff92033a300f55aff2f45f57479a560af2c5467973b425d3f724f870af07f83e",
    ),
    (
        "TrebuchetMS",
        "ff1d49fde63aabd7f4fc7cd0a80ac7cdb1b9540058988fcbb64d7c23639a7386",
    ),
    (
        "HiraginoSans-W3",
        "ff92033a300f55aff2f45f57479a560af2c5467973b425d3f724f870af07f83e",
    ),
];

#[test]
#[ignore = "diagnostic only; stock picture depends on exact macOS font files"]
fn start_fallback_glyph_paint_matches_stock_picture() -> Result<(), String> {
    let tree = tree(TITLE, "text-anchor='start'")?;
    let spans = prepare(&tree, text(&tree)?)?;
    assert_eq!(
        spans.iter().map(|s| s.glyph_ids.len()).collect::<Vec<_>>(),
        [16, 4, 17]
    );
    verify_faces(&tree, &spans)?;
    let mut ids = Vec::new();
    let mut actual = Vec::new();
    for span in &spans {
        let alignment = span
            .fallback_alignment
            .as_ref()
            .ok_or("fallback alignment missing")?;
        ids.extend_from_slice(&span.glyph_ids);
        for glyph in &span.glyphs {
            let t =
                super::super::super::paint::normalized_transform(span, glyph, alignment.css_scale);
            actual.push((t.tx, t.ty));
        }
    }
    assert_eq!(ids, IDS);
    let expected: Vec<_> = X_BITS
        .iter()
        .map(|bits| (ORIGIN_X + f32::from_bits(*bits), ORIGIN_Y))
        .collect();
    assert_eq!(actual, expected);
    Ok(())
}

fn verify_faces(tree: &resvg::usvg::Tree, spans: &[super::PreparedSpan]) -> Result<(), String> {
    for (span, (name, hash)) in spans.iter().zip(FACES) {
        let face = tree.fontdb().face(span.font).ok_or("font face missing")?;
        assert_eq!(face.post_script_name, name);
        let (index, digest) = tree
            .fontdb()
            .with_face_data(span.font, |bytes, index| {
                let digest = Sha256::digest(bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                (index, digest)
            })
            .ok_or("font face data missing")?;
        assert_eq!(index, 0);
        assert_eq!(digest, hash);
    }
    Ok(())
}
