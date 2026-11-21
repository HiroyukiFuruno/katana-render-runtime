use super::{current_process_rss_kib, html_font_memory_snapshot};
use resvg::usvg::fontdb::{Database, Source};
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
fn physical_font_stage_helpers_verify_and_draw_selected_source()
-> Result<(), Box<dyn std::error::Error>> {
    /* WHY: ignored診断を実行しない通常laneでも、物理source照合と実描画の退行を検知する。 */
    let database = acquire_font_database();
    profile_selected_font(&database)
}

#[test]
fn physical_font_stage_rejects_a_database_without_the_selected_family() {
    assert!(selected_physical_font(&Database::new()).is_err());
}

#[test]
fn font_memory_probe_rejects_a_real_face_without_sample_glyphs()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes =
        include_bytes!("../../../../tests/fixtures/fonts/han-kerning/NotoSansCJKjp-han14.otf");
    let manager = FontMgr::new();
    let typeface = manager
        .new_from_data(Data::new_copy(bytes), Some(0))
        .ok_or("Skia could not load the CJK subset")?;
    assert_ne!(typeface.unichar_to_glyph('あ' as i32), 0);
    let error = draw_first_glyph(&typeface)
        .err()
        .ok_or("font without sample glyphs was accepted")?;
    assert_eq!(
        error.to_string(),
        "selected physical face did not resolve the sample glyphs"
    );
    Ok(())
}

#[test]
fn font_memory_probe_rejects_mapped_glyphs_without_outline_pixels()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::markdown::svg_rasterize::text_shaping::test_font_data::font_with_empty_table;
    use rustybuzz::ttf_parser::Tag;

    let original = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
    let font = font_with_empty_table(original, Tag::from_bytes(b"glyf"))?;
    let typeface = FontMgr::new()
        .new_from_data(Data::new_copy(&font), Some(0))
        .ok_or("Skia could not load the font with empty outlines")?;
    assert!(
        SAMPLE
            .chars()
            .all(|character| typeface.unichar_to_glyph(character as i32) != 0)
    );
    let error = draw_first_glyph(&typeface)
        .err()
        .ok_or("font with empty outlines was accepted")?;
    assert_eq!(
        error.to_string(),
        "first glyph draw produced an empty pixel buffer"
    );
    Ok(())
}

#[test]
fn selected_font_rejects_binary_backed_real_face() -> Result<(), Box<dyn std::error::Error>> {
    let source = selected_physical_font(&acquire_font_database())?;
    let mut database = Database::new();
    let loaded = database.load_font_source(Source::Binary(std::sync::Arc::new(source.bytes)));
    assert!(!loaded.is_empty());
    let error = selected_physical_font(&database)
        .err()
        .ok_or("binary source was accepted")?;
    assert_eq!(error.to_string(), "selected face is not file-backed");
    Ok(())
}

#[test]
fn selected_typeface_rejects_mismatched_source_bytes_and_index()
-> Result<(), Box<dyn std::error::Error>> {
    let source = selected_physical_font(&acquire_font_database())?;
    let (_manager, typeface) = load_selected_typeface(&source)?;
    assert_typeface_source(&typeface, &source)?;
    let mut altered = PhysicalFont {
        path: source.path.clone(),
        index: source.index,
        bytes: source.bytes.clone(),
    };
    altered.bytes.push(0);
    assert_eq!(
        assert_typeface_source(&typeface, &altered)
            .err()
            .map(|error| error.to_string()),
        Some("loaded typeface differs from the selected physical source".to_owned())
    );
    altered.bytes = source.bytes;
    altered.index = source.index.checked_add(1).ok_or("font index overflow")?;
    assert_eq!(
        assert_typeface_source(&typeface, &altered)
            .err()
            .map(|error| error.to_string()),
        Some("loaded typeface differs from the selected physical source".to_owned())
    );
    Ok(())
}

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
