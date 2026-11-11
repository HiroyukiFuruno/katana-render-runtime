use super::super::layout;
use super::{TITLE, prepare, text, tree};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const DISABLE_RUNS: &str = "KRR_HTML_FEATURE_DISABLE_FALLBACK_RUNS";

#[test]
fn start_only_diagnostic_preserves_start_and_rejects_other_anchors() -> TestResult {
    run_diagnostic(
        "anchor_tests::mixed_fallback_runs_preserve_start_middle_and_end_anchor_positions",
        "KRR_HTML_FEATURE_START_ONLY_FALLBACK_RUNS",
    )
}

#[test]
fn float_advance_diagnostic_preserves_unrounded_chunk_positions() -> TestResult {
    run_diagnostic(
        "multi_chunk_tests::multiple_chunks_keep_independent_origins_anchors_and_source_shaping",
        "KRR_HTML_FEATURE_FLOAT_FALLBACK_ADVANCES",
    )
}

#[test]
fn disable_diagnostic_skips_an_eligible_fallback_run() -> TestResult {
    let tree = tree(TITLE, "")?;
    let text = text(&tree)?;
    let prepared = prepare(&tree, text)?;
    let layouts = layout(tree.fontdb(), text, &prepared)?;
    if std::env::var(DISABLE_RUNS).as_deref() == Ok("1") {
        assert!(layouts.is_none());
        return Ok(());
    }
    assert!(layouts.is_some());
    run_diagnostic(
        "diagnostic_tests::disable_diagnostic_skips_an_eligible_fallback_run",
        DISABLE_RUNS,
    )
}

fn run_diagnostic(case: &str, flag: &str) -> TestResult {
    let (_, module) = module_path!()
        .split_once("::")
        .ok_or("test crate missing")?;
    let (parent, _) = module.rsplit_once("::").ok_or("test module missing")?;
    let name = format!("{parent}::{case}");
    let output = crate::system::ProcessService::create_command(std::env::current_exe()?)
        .args([name.as_str(), "--exact", "--color=never"])
        .env(flag, "1")
        .output()?;
    let stdout = std::str::from_utf8(&output.stdout)?;
    let stderr = std::str::from_utf8(&output.stderr)?;
    assert!(output.status.success(), "{name} failed: {stdout}\n{stderr}");
    let passed = format!("test {name} ... ok");
    assert_eq!(stdout.lines().filter(|line| *line == passed).count(), 1);
    Ok(())
}
