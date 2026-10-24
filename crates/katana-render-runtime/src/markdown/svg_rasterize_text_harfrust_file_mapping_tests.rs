use fontique::{SourceCache, SourceId, SourceInfo, SourceKind};
use std::{path::PathBuf, sync::Arc, time::Instant};

const SAMPLE_TEXTS: [&str; 3] = ["AV", "office", "漢字"];
const SAMPLE_SIZES: [f32; 3] = [12.0, 16.0, 21.0];

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf")
}

fn mapped_font(path: &std::path::Path, index: u32) -> Result<harfrust::font::Font, String> {
    let source = SourceInfo::new(SourceId::new(), SourceKind::Path(Arc::from(path)));
    let mut cache = SourceCache::default();
    let blob = cache.get(&source).ok_or("Fontique mmap source missing")?;
    harfrust::font::Font::new(blob.into_raw_parts().0, index)
        .ok_or("Fontique mapped source could not create HarfRust font".to_owned())
}

fn assert_same_widths(
    lazy: &harfrust::font::Font,
    mapped: &harfrust::font::Font,
) -> Result<(), String> {
    for (size, text) in SAMPLE_SIZES.into_iter().zip(SAMPLE_TEXTS) {
        let left = super::shape_width(lazy, size, text)?;
        let right = super::shape_width(mapped, size, text)?;
        assert!(
            (left - right).abs() < 0.001,
            "mapped width changed for {text}"
        );
    }
    Ok(())
}

fn print_mapping_observation(
    path: &std::path::Path,
    index: u32,
    durable: bool,
    lazy_ns: u128,
    mapped_ns: u128,
    retained_bytes: usize,
) -> Result<(), String> {
    println!(
        "{}",
        serde_json::json!({
            "fixture": "NotoSans-Regular.ttf",
            "faceIndex": index,
            "sourceGenerationDurable": durable,
            "lazyFileCreateNs": lazy_ns,
            "fontiqueMappedCreateNs": mapped_ns,
            "lazyRetainedBytes": retained_bytes,
            "fontiqueMappedBytes": std::fs::metadata(path).map_err(|e| e.to_string())?.len(),
            "metricsEqual": true,
            "performanceAcceptance": false,
        })
    );
    Ok(())
}

fn run_mapping_probe(
    path: &std::path::Path,
    database: &resvg::usvg::fontdb::Database,
    face: resvg::usvg::fontdb::ID,
    index: u32,
) -> Result<(), String> {
    let generation = super::font_source_generation(database, face);
    let lazy_start = Instant::now();
    let lazy = super::font_for_face(database, face).ok_or("lazy file font missing")?;
    let lazy_ns = lazy_start.elapsed().as_nanos();
    let mapped_start = Instant::now();
    let mapped = mapped_font(path, index)?;
    let mapped_ns = mapped_start.elapsed().as_nanos();

    assert_same_widths(&lazy, &mapped)?;
    assert_eq!(index, 0);
    assert!(generation.durable_reusable());
    print_mapping_observation(
        path,
        index,
        generation.durable_reusable(),
        lazy_ns,
        mapped_ns,
        lazy.retained_source_bytes,
    )
}

#[test]
#[ignore = "diagnostic comparison; run explicitly under the existing macOS test recipe"]
fn fontique_mmap_matches_lazy_file_metrics() -> Result<(), String> {
    let path = fixture_path();
    let (database, face) = super::file_database(&path)?;
    let index = database
        .face(face)
        .ok_or("file face metadata missing")?
        .index;
    run_mapping_probe(&path, &database, face, index)
}
