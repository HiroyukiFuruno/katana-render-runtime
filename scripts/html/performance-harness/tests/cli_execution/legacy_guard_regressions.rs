use super::{cli, sha, Inputs, TempDir};
use serde_json::json;
use std::{fs, path::Path};

const CLICK_X: f32 = 50.0;
const CLICK_Y: f32 = 40.0;
const STATE_BYTES: &[u8] = br#"{"scope":"unit-test-only","state":"click-guard-regression"}"#;

fn legacy_markup(changes_pixels: bool) -> String {
    let handler = if changes_pixels {
        " onclick=\"this.style.background='#00f'\""
    } else {
        ""
    };
    /* WHY: divは既定focus描画がなく、no-op clickの画素差を混ぜない。 */
    format!("<html><head><style>body{{margin:0}}#target{{position:absolute;left:10px;top:10px;width:100px;height:60px;background:#f00}}</style></head><body><div id=target{handler}>go</div></body></html>")
}

fn legacy_inputs(
    temp: &TempDir,
    changes_pixels: bool,
    expected_state_sha: &str,
) -> Result<Inputs, Box<dyn std::error::Error>> {
    let fixture = temp.0.join("legacy-click.html");
    let html = legacy_markup(changes_pixels);
    fs::write(&fixture, html.as_bytes())?;
    let fixture_sha = sha(html.as_bytes());
    let evidence = temp.0.join("unit-only-state.json");
    fs::write(&evidence, STATE_BYTES)?;
    let snapshot = temp.0.join("unit-only-source-snapshot.json");
    fs::write(
        &snapshot,
        br#"{"scope":"unit-test-only; not release evidence"}"#,
    )?;
    let plan = temp.0.join("legacy-operation-plan.json");
    write_legacy_plan(&plan, &fixture_sha, &evidence, expected_state_sha)?;
    Ok(Inputs {
        fixture,
        plan,
        evidence,
        snapshot,
        fixture_sha,
        evidence_bytes: STATE_BYTES.to_vec(),
    })
}

fn write_legacy_plan(
    path: &Path,
    fixture_sha: &str,
    evidence: &Path,
    expected_state_sha: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = json!({
        "fixture_sha256": fixture_sha,
        "operation": "click",
        "x": CLICK_X,
        "y": CLICK_Y,
        "expected_dom_state_path": evidence,
        "expected_dom_state_sha256": expected_state_sha,
    });
    fs::write(path, serde_json::to_vec(&plan)?)?;
    Ok(())
}

fn assert_legacy_guard_rejected(
    temp: &TempDir,
    changes_pixels: bool,
    expected_state_sha: &str,
    guard_message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let inputs = legacy_inputs(temp, changes_pixels, expected_state_sha)?;
    let output = temp.0.join("must-not-exist.json");
    let result = cli(&inputs, &output)?;
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("worker 0 failed"), "{stderr}");
    assert!(stderr.contains(guard_message), "{stderr}");
    assert!(!output.exists());
    Ok(())
}

#[test]
fn cli_rejects_legacy_no_op_click_without_report() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::create()?;
    assert_legacy_guard_rejected(
        &temp,
        false,
        "unused",
        "post-operation did not change frame pixels",
    )
}

#[test]
fn cli_rejects_legacy_wrong_state_sha_without_report() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::create()?;
    assert_legacy_guard_rejected(
        &temp,
        true,
        &"0".repeat(64),
        "expected DOM state evidence SHA does not match its file",
    )
}
