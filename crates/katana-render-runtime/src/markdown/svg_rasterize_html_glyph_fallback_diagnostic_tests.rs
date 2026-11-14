use super::super::layout;
use super::{TITLE, prepare, text, tree};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const DISABLE_RUNS: &str = "KRR_HTML_FEATURE_DISABLE_FALLBACK_RUNS";
const RESVG_TEXT: &str = "KRR_HTML_FEATURE_RESVG_TEXT_RASTER";
const RESVG_PROBE_WIDTH: u32 = 100;
const RESVG_PROBE_HEIGHT: u32 = 40;
const RESVG_PROBE_CONTROL_ROWS: usize = 4;
const RGBA_CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;

#[test]
fn resvg_text_diagnostic_matches_normal_svg_rendering() -> TestResult {
    if std::env::var(RESVG_TEXT).as_deref() != Ok("1") {
        return run_diagnostic(
            "diagnostic_tests::resvg_text_diagnostic_matches_normal_svg_rendering",
            RESVG_TEXT,
        );
    }
    assert_resvg_text_matches_normal()
}

fn assert_resvg_text_matches_normal() -> TestResult {
    use crate::markdown::svg_rasterize::{GeneratedClipProvenance, SvgRasterizeOps, font};
    use resvg::usvg::{Options, Tree};

    let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='40'><rect width='4' height='4'/><text x='10' y='28' font-family='Noto Sans' font-size='20'>AV</text></svg>";
    let options = Options {
        fontdb: font::bundled_font_db(),
        ..Options::default()
    };
    let tree = Tree::from_str(svg, &options)?;
    let provenance = GeneratedClipProvenance::default();
    let html = SvgRasterizeOps::render_html_tree_with_provenance(&tree, 1.0, svg, &provenance)?;
    let normal = SvgRasterizeOps::render_tree_with_provenance(&tree, 1.0, svg, &provenance)?;
    assert_eq!(
        (html.width, html.height),
        (RESVG_PROBE_WIDTH, RESVG_PROBE_HEIGHT)
    );
    assert_eq!(
        (html.width, html.height, &html.rgba),
        (normal.width, normal.height, &normal.rgba)
    );
    assert!(probe_has_glyph_pixels(&html.rgba));
    Ok(())
}

fn probe_has_glyph_pixels(rgba: &[u8]) -> bool {
    rgba.chunks_exact(RGBA_CHANNELS)
        .skip(RESVG_PROBE_CONTROL_ROWS * RESVG_PROBE_WIDTH as usize)
        .any(|pixel| pixel[ALPHA_CHANNEL] > 0)
}

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
