#![cfg(test)]

use super::{assert_error_message, open_fixture_at, HtmlBrowserViewport, VIEWPORT};
use crate::options::{LegacyOperationPlan, OperationPlan};
use std::{fs, path::Path};

const CLICK_POSITION: (f32, f32) = (50.0, 40.0);

fn legacy_plan(state_path: &Path, expected_sha256: &str) -> LegacyOperationPlan {
    LegacyOperationPlan {
        fixture_sha256: "legacy-fixture".to_owned(),
        operation: "click".to_owned(),
        x: CLICK_POSITION.0,
        y: CLICK_POSITION.1,
        expected_dom_state_path: state_path.to_path_buf(),
        expected_dom_state_sha256: expected_sha256.to_owned(),
    }
}

fn legacy_click_target_html(changes_style: bool) -> String {
    let click_handler = if changes_style {
        " onclick=\"this.style.background='#00f'\""
    } else {
        ""
    };
    /* WHY: 非focus要素を使い、クリックによる既定のfocus描画を画素不変の対照へ混入させない。 */
    format!("<html><head><style>body{{margin:0}}#target{{position:absolute;left:10px;top:10px;width:100px;height:60px;background:#f00}}</style></head><body><div id=target{click_handler}>go</div></body></html>")
}

fn legacy_operation_result(
    markup: &str,
    expected_sha256: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let origin = "https://example.test/legacy-click.html";
    let viewport = HtmlBrowserViewport::new(VIEWPORT.0, VIEWPORT.1, VIEWPORT.2)?;
    let mut session = open_fixture_at(markup, origin, viewport)?;
    let state_dir = tempfile::tempdir()?;
    let state_path = state_dir.path().join("expected-state.json");
    fs::write(&state_path, br#"{"expanded":true}"#)?;
    let plan = legacy_plan(&state_path, expected_sha256);
    let result = super::measure_post_operation(
        &mut session,
        &OperationPlan::Legacy(plan),
        markup.as_bytes(),
        origin,
        viewport,
        false,
    );
    session.close()?;
    result
}

#[test]
fn legacy_click_with_changed_pixels_rejects_wrong_state_sha(
) -> Result<(), Box<dyn std::error::Error>> {
    let html = legacy_click_target_html(true);
    let result = legacy_operation_result(html.as_str(), &"0".repeat(64));
    assert_error_message(
        result,
        "expected DOM state evidence SHA does not match its file",
        "wrong state SHA was accepted",
    )
}

#[test]
fn legacy_click_records_advanced_generation_for_changed_pixels(
) -> Result<(), Box<dyn std::error::Error>> {
    let html = legacy_click_target_html(true);
    let result = legacy_operation_result(
        html.as_str(),
        &crate::provenance::hex_digest(br#"{"expanded":true}"#),
    )?;
    assert!(result["generation_after"].as_u64() > result["generation_before"].as_u64());
    assert_ne!(result["pixel_sha_before"], result["pixel_sha_after"]);
    assert_eq!(result["visual_change_observed"], true);
    Ok(())
}

#[test]
fn legacy_no_op_click_rejects_unchanged_pixels() -> Result<(), Box<dyn std::error::Error>> {
    let html = legacy_click_target_html(false);
    let result = legacy_operation_result(html.as_str(), "unused");
    assert_error_message(
        result,
        "post-operation did not change frame pixels",
        "no-op click changed frame pixels",
    )
}
