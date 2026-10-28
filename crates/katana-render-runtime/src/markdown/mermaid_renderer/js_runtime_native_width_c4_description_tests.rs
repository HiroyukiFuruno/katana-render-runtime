use super::helpers::{render_fixture_targets_with_font_audit, render_fixture_with_font_audit};
use serde_json::Value;

const FIXTURE: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/08-02-c4-context-full.md");
const FIRST_ROW: &[&str] = &["A", " customer", " of", " the", " bank,", " with"];
const SECOND_ROW: &[&str] = &["A", " system", " of", " the", " bank,", " with"];
const CONTAINER: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/08-03-c4-container.md");
const JAPANESE: &str = include_str!("../../../../../tests/fixtures/mermaid/ja/08-02-c4.md");

#[test]
fn c4_container_and_japanese_boundary_record_fixture_observations() -> Result<(), String> {
    for (fixture, targets) in [
        (
            CONTAINER,
            &[("row", "Banking Customer"), ("g", "Banking Customer")][..],
        ),
        (
            JAPANESE,
            &[
                ("row", "銀行のシステム。"),
                ("g", "バンキングシステムFキュー"),
            ][..],
        ),
    ] {
        observe_c4_fixture(fixture, targets)?;
    }
    Ok(())
}

fn observe_c4_fixture(fixture: &str, targets: &[(&str, &str)]) -> Result<(), String> {
    let source = super::mermaid_fixture_source(fixture)?;
    let (result, audit) = render_fixture_targets_with_font_audit(source, targets)?;
    assert!(!result.svg.is_empty());
    assert!(
        !result.c4_description_rows.is_empty(),
        "target row was not observed"
    );
    assert!(!result.boxes.is_empty(), "target boxes were not observed");
    println!(
        "c4_fixture_observation={}",
        serde_json::json!({
            "targets": targets, "svg": result.svg, "rows": result.c4_description_rows,
            "boxes": result.boxes, "fontAudit": audit
        })
    );
    Ok(())
}

#[test]
fn c4_description_rows_record_real_bbox_and_span_advances() -> Result<(), String> {
    let source = super::mermaid_fixture_source(FIXTURE)?;
    let (result, audit) = render_fixture_with_font_audit(source)?;
    println!(
        "c4_description_width_observation={}",
        serde_json::json!({"rows": &result.c4_description_rows, "fontAudit": &audit})
    );
    for expected in [FIRST_ROW, SECOND_ROW] {
        assert_description_row(&result.c4_description_rows, expected)?;
    }
    #[cfg(target_os = "macos")]
    assert!(
        audit["faces"]
            .as_array()
            .is_some_and(|faces| !faces.is_empty())
    );
    Ok(())
}

fn assert_description_row(observations: &[Value], expected: &[&str]) -> Result<(), String> {
    let text = expected.join("");
    let row = observations
        .iter()
        .find(|row| row["text"] == text)
        .ok_or("description row missing")?;
    assert_eq!(
        row["grouped"], true,
        "C4 description was not grouped: {row}"
    );
    let parts = row["parts"].as_array().ok_or("description parts missing")?;
    let actual: Vec<_> = parts.iter().map(|part| part["text"].as_str()).collect();
    assert_eq!(
        actual,
        expected.iter().map(|part| Some(*part)).collect::<Vec<_>>()
    );
    for part in parts {
        assert_description_part(part)?;
    }
    assert!(
        row["bbox"]["width"]
            .as_f64()
            .is_some_and(|width| width > 0.0)
    );
    if !row["wholeAdvance"].is_null() {
        assert!(row["wholeAdvance"].as_f64().is_some_and(f64::is_finite));
    }
    Ok(())
}

fn assert_description_part(part: &Value) -> Result<(), String> {
    assert_eq!(part["family"], "\"Open Sans\",sans-serif");
    assert!((part["fontSize"].as_f64().ok_or("font size missing")? - 11.48).abs() < 0.0001);
    assert_eq!(part["weight"], 400);
    for key in ["x", "advance", "sourceWidth"] {
        assert!(
            part[key].as_f64().is_some_and(f64::is_finite),
            "{key}: {part}"
        );
    }
    Ok(())
}
