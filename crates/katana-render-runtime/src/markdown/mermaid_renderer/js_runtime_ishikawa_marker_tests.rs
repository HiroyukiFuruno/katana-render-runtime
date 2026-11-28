#[cfg(target_os = "macos")]
use super::reference_score_tests;
#[cfg(target_os = "macos")]
use crate::markdown::color_preset::DiagramColorPreset;
#[cfg(target_os = "macos")]
use resvg::usvg::roxmltree::Document;

#[cfg(target_os = "macos")]
const CASES: [(&str, usize); 4] = [
    (
        include_str!(
            "../../../../../tests/fixtures/mermaid/en/13-01-ishikawa-diagram-3-categories.md"
        ),
        9,
    ),
    (
        include_str!(
            "../../../../../tests/fixtures/mermaid/en/13-02-ishikawa-diagram-4-categories.md"
        ),
        18,
    ),
    (
        include_str!("../../../../../tests/fixtures/mermaid/ja/13-01-3.md"),
        9,
    ),
    (
        include_str!("../../../../../tests/fixtures/mermaid/ja/13-02-4.md"),
        18,
    ),
];

#[cfg(target_os = "macos")]
#[test]
fn real_ishikawa_markers_survive_native_dom_serialization() -> Result<(), String> {
    for (markdown, expected_count) in CASES {
        let source = reference_score_tests::mermaid_fixture_source(markdown)?;
        let (svg, observations) = reference_score_tests::render_with_native_font_observations(
            &source,
            DiagramColorPreset::dark(),
        )?;
        assert_ishikawa_marker_references(&svg, expected_count)
            .map_err(|error| format!("{error}; native font observations: {observations}"))?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_ishikawa_marker_references(svg: &str, expected_count: usize) -> Result<(), String> {
    let document = Document::parse(svg).map_err(|error| error.to_string())?;
    let expected = ishikawa_marker_url(&document)?;
    let count = count_ishikawa_lines(&document, &expected)?;
    (count == expected_count)
        .then_some(())
        .ok_or_else(|| format!("ishikawa marker count {count} != {expected_count}"))
}

#[cfg(target_os = "macos")]
fn ishikawa_marker_url(document: &Document<'_>) -> Result<String, String> {
    let marker_id = document
        .descendants()
        .find(|node| node.has_tag_name("marker"))
        .and_then(|node| node.attribute("id"))
        .ok_or("ishikawa marker definition missing")?;
    Ok(format!("url(#{marker_id})"))
}

#[cfg(target_os = "macos")]
fn count_ishikawa_lines(document: &Document<'_>, expected: &str) -> Result<usize, String> {
    let mut count = 0;
    for line in document.descendants().filter(|node| {
        node.has_tag_name("line")
            && node.attribute("class").is_some_and(|class| {
                class
                    .split_whitespace()
                    .any(|name| matches!(name, "ishikawa-branch" | "ishikawa-sub-branch"))
            })
    }) {
        if line.attribute("marker-start") != Some(expected) {
            return Err(format!(
                "ishikawa marker-start mismatch for {}",
                line.attribute("class").unwrap_or("unknown")
            ));
        }
        count += 1;
    }
    Ok(count)
}
