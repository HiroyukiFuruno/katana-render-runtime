use super::types::SvgRasterizeError;
use std::collections::HashSet;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_composite_authored_text_context.rs"]
mod authored_text_context;
#[cfg(target_os = "macos")]
pub(super) use authored_text_context::AuthoredTextContext;

#[derive(Clone, Default)]
pub(crate) struct GeneratedClipProvenance {
    pub(crate) clips: Vec<GeneratedClip>,
}

#[derive(Clone)]
pub(crate) struct GeneratedClip {
    pub(crate) id: String,
    pub(crate) shape: String,
}

#[derive(Default)]
pub(super) struct AuthoredClipInfo {
    pub(super) ids: HashSet<String>,
    pub(super) generated_ids: HashSet<String>,
    pub(super) has_object_bounding_box: bool,
}

#[cfg(target_os = "macos")]
pub(super) struct AuthoredSvgContext {
    pub(super) clips: AuthoredClipInfo,
    pub(super) text: AuthoredTextContext,
}

pub(super) fn authored_clip_ids(
    svg: &str,
    generated: &GeneratedClipProvenance,
) -> Result<AuthoredClipInfo, SvgRasterizeError> {
    let document = resvg::usvg::roxmltree::Document::parse(svg)
        .map_err(|error| SvgRasterizeError::ParseFailed(error.to_string()))?;
    Ok(authored_clip_info(&document, generated))
}

#[cfg(target_os = "macos")]
pub(super) fn authored_svg_context(
    svg: &str,
    generated: &GeneratedClipProvenance,
    needs_authored_clip_ids: bool,
) -> Result<AuthoredSvgContext, SvgRasterizeError> {
    let document = resvg::usvg::roxmltree::Document::parse(svg)
        .map_err(|error| SvgRasterizeError::ParseFailed(error.to_string()))?;
    Ok(AuthoredSvgContext {
        clips: if needs_authored_clip_ids {
            authored_clip_info(&document, generated)
        } else {
            AuthoredClipInfo::default()
        },
        text: authored_text_context::collect(&document),
    })
}

fn authored_clip_info(
    document: &resvg::usvg::roxmltree::Document<'_>,
    generated: &GeneratedClipProvenance,
) -> AuthoredClipInfo {
    let clips = document
        .descendants()
        .filter(|node| node.has_tag_name("clipPath"))
        .collect::<Vec<_>>();
    let ids = clip_ids(&clips);
    let generated_ids = generated_clip_ids(document, &clips, generated);
    AuthoredClipInfo {
        ids,
        generated_ids,
        has_object_bounding_box: has_object_bounding_box(&clips),
    }
}

fn clip_ids(clips: &[resvg::usvg::roxmltree::Node<'_, '_>]) -> HashSet<String> {
    clips
        .iter()
        .filter_map(|node| node.attribute("id"))
        .map(str::to_owned)
        .collect()
}

fn generated_clip_ids(
    document: &resvg::usvg::roxmltree::Document<'_>,
    clips: &[resvg::usvg::roxmltree::Node<'_, '_>],
    generated: &GeneratedClipProvenance,
) -> HashSet<String> {
    generated
        .clips
        .iter()
        .filter(|clip| {
            clips
                .iter()
                .filter(|node| node.attribute("id") == Some(clip.id.as_str()))
                .count()
                == 1
        })
        .filter(|clip| clip_shape_matches(document, clip))
        .map(|clip| clip.id.clone())
        .collect()
}

fn has_object_bounding_box(clips: &[resvg::usvg::roxmltree::Node<'_, '_>]) -> bool {
    clips
        .iter()
        .any(|node| node.attribute("clipPathUnits") == Some("objectBoundingBox"))
}

fn clip_shape_matches(
    document: &resvg::usvg::roxmltree::Document<'_>,
    generated: &GeneratedClip,
) -> bool {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("clipPath") && node.attribute("id") == Some(generated.id.as_str())
        })
        .is_some_and(|node| {
            let children = node
                .children()
                .filter(|child| child.is_element())
                .collect::<Vec<_>>();
            children.len() == 1 && document.input_text()[children[0].range()] == generated.shape
        })
}

#[cfg(test)]
mod tests {
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
}

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_composite_authored_text_context_tests.rs"]
mod authored_text_context_tests;
