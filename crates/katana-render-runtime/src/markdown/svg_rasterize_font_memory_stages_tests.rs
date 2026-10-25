use super::{current_process_rss_kib, html_font_memory_snapshot};
use resvg::usvg::fontdb::Database;
use serde_json::{Value, json};
use skia_safe::{Data, FontMgr};
use std::time::Instant;

#[path = "svg_rasterize_font_memory_stage_font.rs"]
mod physical_font;
use physical_font::{
    PhysicalFont, assert_typeface_source, draw_first_glyph, emit_source_stage,
    selected_physical_font,
};

const FAMILY: &str = "Trebuchet MS";
const SAMPLE: &str = "AV";

#[test]
#[cfg(target_os = "macos")]
#[ignore = "staged font initialization RSS/time diagnostic"]
fn macos_font_initialization_stages_report_time_and_rss() -> Result<(), Box<dyn std::error::Error>>
{
    if std::env::var_os("KRR_TEST_FONT_STAGES_CHILD").is_none() {
        return run_stage_probe_in_child();
    }
    let process_started = Instant::now();
    emit_stage(
        "process-start",
        process_started.elapsed().as_nanos(),
        json!({}),
    );
    let database = acquire_font_database();
    profile_selected_font(&database)
}

fn acquire_font_database() -> std::sync::Arc<Database> {
    let database_started = Instant::now();
    let database = super::super::super::font::html_font_db_for_text(FAMILY, SAMPLE);
    let database_elapsed_ns = database_started.elapsed().as_nanos();
    emit_stage(
        "font-database",
        database_elapsed_ns,
        json!({"total_faces": database.len()}),
    );
    let snapshot_started = Instant::now();
    let memory = html_font_memory_snapshot(&database);
    emit_stage(
        "font-database-memory-snapshot",
        snapshot_started.elapsed().as_nanos(),
        json!({
            "owned_font_bytes": memory.owned_font_bytes,
            "system_font_file_faces": memory.system_font_file_faces,
            "total_faces": memory.total_faces,
        }),
    );
    database
}

fn profile_selected_font(database: &Database) -> Result<(), Box<dyn std::error::Error>> {
    let source_started = Instant::now();
    let source = selected_physical_font(database)?;
    emit_source_stage(&source, source_started.elapsed().as_nanos());
    let (_manager, typeface) = load_selected_typeface(&source)?;
    let proof_started = Instant::now();
    assert_typeface_source(&typeface, &source)?;
    emit_stage(
        "physical-source-proof",
        proof_started.elapsed().as_nanos(),
        json!({
            "typeface_unique_id": typeface.unique_id(),
            "postscript_name": typeface.post_script_name(),
        }),
    );
    let draw = draw_first_glyph(&typeface)?;
    emit_stage(
        "first-glyph-draw",
        draw.elapsed_ns,
        json!({"glyph_ids": draw.glyph_ids, "pixel_sha256": draw.pixel_sha256}),
    );
    Ok(())
}

fn load_selected_typeface(
    source: &PhysicalFont,
) -> Result<(FontMgr, skia_safe::Typeface), Box<dyn std::error::Error>> {
    let manager_started = Instant::now();
    let manager = FontMgr::default();
    emit_stage(
        "skia-font-manager-default",
        manager_started.elapsed().as_nanos(),
        json!({}),
    );
    let data_started = Instant::now();
    let data = Data::new_copy(&source.bytes);
    emit_stage(
        "skia-data-copy",
        data_started.elapsed().as_nanos(),
        json!({"bytes": source.bytes.len()}),
    );
    let typeface = create_physical_typeface(&manager, data, source.index)?;
    Ok((manager, typeface))
}

fn create_physical_typeface(
    manager: &FontMgr,
    data: Data,
    index: u32,
) -> Result<skia_safe::Typeface, Box<dyn std::error::Error>> {
    let typeface_started = Instant::now();
    let typeface = manager
        .new_from_data(data, Some(index))
        .ok_or("Skia could not load selected physical font")?;
    emit_stage(
        "skia-new-from-data",
        typeface_started.elapsed().as_nanos(),
        json!({
            "unique_id": typeface.unique_id(),
            "postscript_name": typeface.post_script_name(),
        }),
    );
    Ok(typeface)
}

fn emit_stage(stage: &str, elapsed_ns: u128, details: Value) {
    eprintln!(
        "{}",
        json!({"probe":"html-font-stage-rss", "stage":stage, "elapsed_ns":elapsed_ns, "rss_kib":current_process_rss_kib(), "details":details})
    );
}

fn run_stage_probe_in_child() -> Result<(), Box<dyn std::error::Error>> {
    let output = crate::system::ProcessService::create_command(std::env::current_exe()?)
        .args([
            "markdown::svg_rasterize::tests::font_memory_tests::stages::macos_font_initialization_stages_report_time_and_rss",
            "--exact", "--ignored", "--nocapture", "--test-threads=1",
        ])
        .env("KRR_TEST_FONT_STAGES_CHILD", "1")
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "font stage child failed: {stdout}\n{stderr}"
    );
    assert_single_test_result(&stdout, &stderr);
    eprintln!("{stdout}");
    eprintln!("{stderr}");
    Ok(())
}

fn assert_single_test_result(stdout: &str, stderr: &str) {
    let records = stdout
        .lines()
        .chain(stderr.lines())
        .filter(|line| line.starts_with("test "))
        .collect::<Vec<_>>();
    assert_eq!(
        records.len(),
        1,
        "font stage child selected unexpected test records"
    );
    assert!(
        records[0].contains("macos_font_initialization_stages_report_time_and_rss")
            && records[0].ends_with(" ... ok"),
        "font stage diagnostic case did not pass: {}",
        records[0]
    );
}
