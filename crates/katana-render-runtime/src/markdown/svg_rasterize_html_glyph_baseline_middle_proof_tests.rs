use super::{
    FONT_FAMILY, LINE_ADVANCE, MULTILINE, MULTILINE_CHUNK_COUNT, THREE_LINE_CHUNK_COUNT,
    THREE_LINES, fixture, source, text,
};
use crate::markdown::svg_rasterize::{
    GeneratedClipProvenance,
    provenance::{self, AuthoredTextContext},
};

#[test]
fn additional_lane_rejects_nonzero_first_dy_with_otherwise_valid_lines() -> Result<(), String> {
    let shifted = MULTILINE.replacen("dy='0'", "dy='1'", 1);
    for (body, first_dy, expected) in [(MULTILINE, 0.0, true), (shifted.as_str(), 1.0, false)] {
        let tree = fixture(body, "")?;
        let text = text(&tree)?;
        assert_eq!(text.chunks().len(), MULTILINE_CHUNK_COUNT);
        assert_eq!(text.dy().first(), Some(&first_dy));
        assert!(text.dy().contains(&LINE_ADVANCE));
        assert_eq!(super::additional_middle_layout_supported(text), expected);
    }
    Ok(())
}

#[test]
fn multiline_proof_rejects_unsupported_stylesheet() -> Result<(), String> {
    let styled = source(MULTILINE, "", FONT_FAMILY).replace(
        "</svg>",
        "<style>@media screen { text {font-size:14px} }</style></svg>",
    );
    assert_rejected_proof(MULTILINE, &styled)
}

#[test]
fn multiline_proof_rejects_whitespace_only_source() -> Result<(), String> {
    let candidate = source(
        "<tspan x='0' dy='0'> </tspan><tspan x='0' dy='16.8'>  </tspan>",
        "",
        FONT_FAMILY,
    );
    assert_rejected_proof(MULTILINE, &candidate)
}

#[test]
fn multiline_proof_rejects_authored_text_between_tspans() -> Result<(), String> {
    let mixed = MULTILINE.replacen("</tspan><tspan", "</tspan>outside<tspan", 1);
    for (body, accepted) in [(MULTILINE, true), (mixed.as_str(), false)] {
        let tree = fixture(body, "")?;
        let proof = context(&source(body, "", FONT_FAMILY))?;
        assert_eq!(proof.matches_additional(text(&tree)?), accepted);
    }
    Ok(())
}

#[test]
fn multiline_layout_rejects_nonzero_position_of_hidden_trailing_text() -> Result<(), String> {
    for (hidden_dy, accepted) in [(0.0, true), (1.0, false)] {
        let body = format!("{MULTILINE}<tspan display='none' dy='{hidden_dy}'>E</tspan>");
        let tree = fixture(&body, "")?;
        let text = text(&tree)?;
        assert_eq!(text.chunks().len(), MULTILINE_CHUNK_COUNT);
        let visible = text
            .chunks()
            .iter()
            .map(|chunk| chunk.text())
            .collect::<String>();
        assert_eq!(visible, "ABCD");
        assert_eq!(text.dy().len(), visible.chars().count() + 1);
        assert_eq!(text.dy().last(), Some(&hidden_dy));
        assert_eq!(super::additional_middle_layout_supported(text), accepted);
    }
    Ok(())
}

#[test]
fn additional_proof_rejects_complex_hierarchy_and_descendant_style() -> Result<(), String> {
    for body in [
        "<tspan x='0' dy='0'><tspan>AB</tspan></tspan><tspan x='0' dy='16.8'>CD</tspan>",
        "<tspan x='0' dy='0'>AB</tspan><tspan x='0' dy='16.8' font-size='14'>CD</tspan>",
        "<tspan x='0' dy='0'>AB</tspan><tspan x='0' dy='16.8' style='font-size:14px'>CD</tspan>",
    ] {
        let candidate = source(body, "", FONT_FAMILY);
        assert_rejected_proof(MULTILINE, &candidate)?;
    }
    let candidate = source(MULTILINE, "", FONT_FAMILY)
        .replace("</svg>", "<style>tspan {font-size:14px}</style></svg>");
    assert_rejected_proof(MULTILINE, &candidate)
}

#[test]
fn multiline_proof_rejects_duplicate_use_and_chunk_mismatch() -> Result<(), String> {
    let original = source(MULTILINE, "", FONT_FAMILY);
    for candidate in [
        original.replace("</svg>", "<text>ABCD</text></svg>"),
        original.replace("</svg>", "<use href='#run'/></svg>"),
        original.replace("16.8", "20"),
        original
            .replace(">AB</tspan>", ">A</tspan>")
            .replace(">CD</tspan>", ">BCD</tspan>"),
    ] {
        assert_rejected_proof(MULTILINE, &candidate)?;
    }
    Ok(())
}

#[test]
fn multiline_proof_rejects_text_and_origin_mismatch() -> Result<(), String> {
    let original = source(MULTILINE, "", FONT_FAMILY);
    for candidate in [
        original.replace(">AB</tspan>", ">XY</tspan>"),
        original.replace("y='20'", "y='21'"),
    ] {
        assert_rejected_proof(MULTILINE, &candidate)?;
    }
    Ok(())
}

#[test]
fn multiline_proof_rejects_the_same_text_split_into_a_different_number_of_lines()
-> Result<(), String> {
    let two_lines = MULTILINE.replace(">CD</tspan>", ">CDEF</tspan>");
    let original = fixture(&two_lines, "")?;
    let reshaped = fixture(THREE_LINES, "")?;
    let original_text = text(&original)?;
    let reshaped_text = text(&reshaped)?;
    assert_eq!(original_text.chunks().len(), MULTILINE_CHUNK_COUNT);
    assert_eq!(reshaped_text.chunks().len(), THREE_LINE_CHUNK_COUNT);
    let proof = context(&source(&two_lines, "", FONT_FAMILY))?;
    assert!(proof.matches_additional(original_text));
    assert!(!proof.matches_additional(reshaped_text));
    Ok(())
}

fn assert_rejected_proof(body: &str, candidate: &str) -> Result<(), String> {
    let tree = fixture(body, "")?;
    let text = text(&tree)?;
    let context = context(candidate)?;
    assert!(!context.matches_additional(text), "{candidate}");
    assert!(
        super::super::super::policy::uniform_policy_with_context(
            text,
            false,
            context.matches_additional(text)
        )
        .is_none()
    );
    Ok(())
}

fn context(source: &str) -> Result<AuthoredTextContext, String> {
    provenance::authored_svg_context(source, &GeneratedClipProvenance::default(), false)
        .map(|context| context.text)
        .map_err(|error| error.to_string())
}

#[test]
fn multiline_proof_rejects_zero_second_line_advance() -> Result<(), String> {
    let tree = fixture(MULTILINE, "")?;
    let text = text(&tree)?;
    assert_eq!(text.chunks().len(), MULTILINE_CHUNK_COUNT);
    assert!(text.dy().contains(&LINE_ADVANCE));
    let original = source(MULTILINE, "", FONT_FAMILY);
    assert!(context(&original)?.matches_additional(text));
    let changed = original.replace("dy='16.8'", "dy='0'");
    assert_ne!(changed, original);
    assert_rejected_proof(MULTILINE, &changed)
}
