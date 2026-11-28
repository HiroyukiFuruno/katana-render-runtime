use super::super::{MermaidJsRuntimeOps, RuntimeAsset};
use crate::markdown::color_preset::DiagramColorPreset;

const JOURNEY_SOURCE: &str = "journey\n    title Diagram preview\n    section Edit\n      Write Markdown: 5: User\n      Check diagram: 4: User, KatanA\n    section Export\n      Export to HTML: 3: KatanA";

#[test]
fn journey_svg_text_fallback_inherits_the_html_label_typography() -> Result<(), String> {
    let asset = RuntimeAsset::mermaid();
    let rendered = asset
        .materialize_at(asset.materialized_path())
        .and_then(|bundle| {
            MermaidJsRuntimeOps::render(JOURNEY_SOURCE, &bundle, DiagramColorPreset::dark())
        });
    let svg = rendered.as_ref().map_err(|error| error.to_owned())?;

    assert_journey_fallbacks_use_inherited_font(svg)?;
    assert_journey_labels_are_retained(svg);
    Ok(())
}

fn assert_journey_fallbacks_use_inherited_font(svg: &str) -> Result<(), String> {
    assert!(!svg.contains("<foreignObject"), "{svg}");
    assert!(svg.contains("font-family:\"trebuchet ms\",verdana,arial,sans-serif;font-size:16px"));
    for text in svg.split("<text").skip(1) {
        let (tag, _) = text.split_once('>').ok_or("Journey text tag is unclosed")?;
        if !tag.contains("class=\"task") && !tag.contains("class=\"journey-section") {
            continue;
        }
        assert!(tag.contains("text-anchor: middle"), "{tag}");
        assert!(tag.contains("dominant-baseline=\"central\""), "{tag}");
        assert!(!tag.contains("Open Sans"), "{tag}");
        assert!(!tag.contains("font-size:"), "{tag}");
        assert!(!tag.contains("font-family:"), "{tag}");
    }
    Ok(())
}

fn assert_journey_labels_are_retained(svg: &str) {
    for label in [
        "Edit",
        "Write Markdown",
        "Check diagram",
        "Export",
        "Export to HTML",
    ] {
        assert!(
            svg.contains(&format!(">{label}</tspan>")),
            "missing {label:?}"
        );
    }
}
