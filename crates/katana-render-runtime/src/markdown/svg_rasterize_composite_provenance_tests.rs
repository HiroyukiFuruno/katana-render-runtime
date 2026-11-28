#[cfg(target_os = "macos")]
use super::authored_svg_context;
use super::{GeneratedClip, GeneratedClipProvenance, authored_clip_ids};

fn provenance(shape: &str) -> GeneratedClipProvenance {
    GeneratedClipProvenance {
        clips: vec![GeneratedClip {
            id: "krr-clip-1".to_string(),
            shape: shape.to_string(),
        }],
    }
}

#[test]
fn accepts_only_the_single_matching_generated_clip() -> Result<(), String> {
    let svg = r#"<svg><defs><clipPath id="krr-clip-1"><rect width="1" height="1"/></clipPath></defs></svg>"#;
    let info = authored_clip_ids(svg, &provenance(r#"<rect width="1" height="1"/>"#))
        .map_err(|error| error.to_string())?;
    assert!(info.generated_ids.contains("krr-clip-1"));
    Ok(())
}

#[test]
fn rejects_duplicate_or_mismatched_author_clip() -> Result<(), String> {
    let duplicate = r#"<svg><defs><clipPath id="krr-clip-1"><rect width="1" height="1"/></clipPath><clipPath id="krr-clip-1"><circle r="1"/></clipPath></defs></svg>"#;
    let multiple_children = r#"<svg><defs><clipPath id="krr-clip-1"><rect width="1" height="1"/><rect width="2" height="2"/></clipPath></defs></svg>"#;
    let mismatch =
        r#"<svg><defs><clipPath id="krr-clip-1"><circle r="1"/></clipPath></defs></svg>"#;
    for svg in [duplicate, multiple_children, mismatch] {
        let info = authored_clip_ids(svg, &provenance(r#"<rect width="1" height="1"/>"#))
            .map_err(|error| error.to_string())?;
        assert!(!info.generated_ids.contains("krr-clip-1"));
        assert!(info.ids.contains("krr-clip-1"));
    }
    let empty = authored_clip_ids(mismatch, &GeneratedClipProvenance::default())
        .map_err(|error| error.to_string())?;
    assert!(empty.generated_ids.is_empty());
    Ok(())
}

#[test]
#[cfg(target_os = "macos")]
fn authored_text_context_preserves_unneeded_clip_default() -> Result<(), String> {
    let svg = r#"<svg><defs><clipPath id="authored" clipPathUnits="objectBoundingBox"><rect width="1" height="1"/></clipPath></defs></svg>"#;
    let context = authored_svg_context(svg, &GeneratedClipProvenance::default(), false)
        .map_err(|error| error.to_string())?;
    assert!(context.clips.ids.is_empty());
    assert!(context.clips.generated_ids.is_empty());
    assert!(!context.clips.has_object_bounding_box);
    Ok(())
}
