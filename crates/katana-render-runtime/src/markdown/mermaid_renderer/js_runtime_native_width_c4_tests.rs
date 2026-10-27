use super::helpers::render_fixture_with_font_audit;
use super::oracle::{assert_bridge_widths_match_rust, call_text, rounded_width};
use crate::markdown::svg_rasterize::{SvgRasterizeOps, SvgTextRunMetrics};
use serde_json::Value;

const C4_SOURCE: &str = r#"C4Dynamic
title Runtime width probe
Container(spa, "Single Page Application", "javascript and react")
Container(api, "API Application", "Java and Spring Boot")
Rel(spa, api, "Uses", "JSON/HTTPS")"#;
const C4_CONTEXT_FIXTURE_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/08-02-c4-context-full.md");
const C4_LABEL_PROBE_FAMILY: &str = "\"Open Sans\",sans-serif";
const C4_LABEL_PROBE_SIZE: f32 = 14.0;
const C4_LABEL_PROBE_WEIGHT: u16 = 700;

#[test]
fn nonclass_c4_uses_native_advance_for_svg_text() -> Result<(), String> {
    for (source, expected) in [
        (C4_SOURCE, "Application"),
        (
            super::mermaid_fixture_source(C4_CONTEXT_FIXTURE_MARKDOWN)?,
            "Banking Customer A",
        ),
    ] {
        let (result, font_audit) = render_fixture_with_font_audit(source)?;
        println!(
            "c4_native_width_observation={}",
            serde_json::json!({
                "label": expected,
                "calls": &result.calls,
                "boxes": &result.boxes,
                "labelWidths": c4_label_width_probe(expected)?,
                "fontAudit": font_audit
            })
        );
        assert_c4_label_tokens(&result.calls, expected);
        assert_bridge_widths_match_rust(&result.calls)?;
    }
    Ok(())
}

fn assert_c4_label_tokens(calls: &[Value], label: &str) {
    for expected_word in label.split_whitespace() {
        assert!(
            calls.iter().any(|call| {
                call_text(call)
                    .split_whitespace()
                    .any(|word| word == expected_word)
            }),
            "actual C4 SVG text did not measure complete word {expected_word:?}: {calls:?}"
        );
    }
}

fn c4_label_width_probe(label: &str) -> Result<Value, String> {
    let whole = measure_c4_label(label)?;
    let parts = c4_part_advances(label)?;
    let prefix = label.rsplit_once(' ').map_or(label, |(prefix, _)| prefix);
    Ok(serde_json::json!({
        "family": C4_LABEL_PROBE_FAMILY,
        "size": C4_LABEL_PROBE_SIZE,
        "weight": C4_LABEL_PROBE_WEIGHT,
        "wholeAdvance": whole.advance,
        "wholeInkBounds": whole.ink_bounds,
        "prefixAdvance": measure_c4_label(prefix)?.advance,
        "partAdvances": parts,
        "sumAdvances": parts.iter().copied().map(f64::from).sum::<f64>(),
        "sumLayoutAdvances": parts.iter().copied().map(rounded_width).sum::<f64>()
    }))
}

fn c4_part_advances(label: &str) -> Result<Vec<f32>, String> {
    label
        .split_whitespace()
        .enumerate()
        .map(|(index, word)| {
            let text = if index == 0 {
                word.to_owned()
            } else {
                format!(" {word}")
            };
            measure_c4_label(&text).map(|metrics| metrics.advance)
        })
        .collect()
}

fn measure_c4_label(text: &str) -> Result<SvgTextRunMetrics, String> {
    SvgRasterizeOps::measure_svg_text_metrics(
        text,
        C4_LABEL_PROBE_FAMILY,
        C4_LABEL_PROBE_SIZE,
        C4_LABEL_PROBE_WEIGHT,
        false,
    )
    .map_err(|error| format!("C4 width observation failed: {error:?}"))
}
