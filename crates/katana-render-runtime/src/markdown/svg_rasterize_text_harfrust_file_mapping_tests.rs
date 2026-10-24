use fontique::{SourceCache, SourceId, SourceInfo, SourceKind};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc, time::Instant};

const SAMPLE_TEXTS: [&str; 3] = ["AV", "office", "漢字"];
const SAMPLE_SIZES: [f32; 3] = [12.0, 16.0, 21.0];
const PAIR_COUNT: usize = 25;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf")
}

pub(super) fn helvetica_path() -> PathBuf {
    PathBuf::from("/System/Library/Fonts/Helvetica.ttc")
}

pub(super) fn database_for_index(
    path: &std::path::Path,
    expected_index: u32,
) -> Result<(resvg::usvg::fontdb::Database, resvg::usvg::fontdb::ID), String> {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_source(resvg::usvg::fontdb::Source::File(path.to_path_buf()));
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    let face = database
        .faces()
        .find(|face| {
            face.index == expected_index
                && database
                    .face_source(face.id)
                    .is_some_and(|(source, _)| match source {
                        resvg::usvg::fontdb::Source::File(source) => source
                            .canonicalize()
                            .is_ok_and(|candidate| candidate == canonical),
                        _ => false,
                    })
        })
        .ok_or("requested font face index missing")?
        .id;
    Ok((database, face))
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

pub(super) fn source_sha(path: &std::path::Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[derive(Clone)]
struct TimedArm {
    elapsed_ns: u128,
    index: u32,
    widths: [f32; SAMPLE_TEXTS.len()],
}

fn shape_widths(font: &harfrust::font::Font) -> Result<[f32; SAMPLE_TEXTS.len()], String> {
    let mut widths = [0.0; SAMPLE_TEXTS.len()];
    for (slot, (size, text)) in SAMPLE_SIZES.into_iter().zip(SAMPLE_TEXTS).enumerate() {
        widths[slot] = super::shape_width(font, size, text)?;
    }
    Ok(widths)
}

fn timed_lazy(path: &std::path::Path, expected_index: u32) -> Result<TimedArm, String> {
    let (database, face) = database_for_index(path, expected_index)?;
    let index = database
        .face(face)
        .ok_or("file face metadata missing")?
        .index;
    let generation = super::super::super::super::font::font_source_generation(&database, face);
    let key = super::super::entry::CacheKey::File {
        path: path.to_path_buf(),
        generation: generation.clone(),
        face_index: index,
    };
    let start = Instant::now();
    let lazy = super::super::entry::lazy_file_font(&database, face, path, index, key, generation)
        .map_err(|error| error.to_string())?
        .ok_or("lazy file font missing")?;
    let widths = shape_widths(&lazy)?;
    Ok(TimedArm {
        elapsed_ns: start.elapsed().as_nanos(),
        index,
        widths,
    })
}

fn timed_mapped(path: &std::path::Path, index: u32) -> Result<TimedArm, String> {
    let start = Instant::now();
    let mapped = mapped_font(path, index)?;
    let widths = shape_widths(&mapped)?;
    Ok(TimedArm {
        elapsed_ns: start.elapsed().as_nanos(),
        index,
        widths,
    })
}

fn median(samples: &[u128]) -> u128 {
    let mut ordered = samples.to_vec();
    ordered.sort_unstable();
    ordered[ordered.len() / 2]
}

fn run_pair(
    path: &std::path::Path,
    index: u32,
    lazy_first: bool,
) -> Result<(TimedArm, TimedArm), String> {
    let (lazy, mapped) = if lazy_first {
        (timed_lazy(path, index)?, timed_mapped(path, index)?)
    } else {
        let mapped = timed_mapped(path, index)?;
        let lazy = timed_lazy(path, index)?;
        (lazy, mapped)
    };
    assert_eq!(lazy.index, index);
    assert_eq!(mapped.index, index);
    assert_eq!(lazy.widths, mapped.widths);
    Ok((lazy, mapped))
}

struct RepeatedSamples {
    orders: Vec<&'static str>,
    lazy_ns: Vec<u128>,
    mapped_ns: Vec<u128>,
}

fn collect_repeated_samples(path: &std::path::Path, index: u32) -> Result<RepeatedSamples, String> {
    let mut samples = RepeatedSamples {
        orders: Vec::with_capacity(PAIR_COUNT),
        lazy_ns: Vec::with_capacity(PAIR_COUNT),
        mapped_ns: Vec::with_capacity(PAIR_COUNT),
    };
    for pair in 0..PAIR_COUNT {
        let lazy_first = pair % 2 == 0;
        let (lazy, mapped) = run_pair(path, index, lazy_first)?;
        samples.lazy_ns.push(lazy.elapsed_ns);
        samples.mapped_ns.push(mapped.elapsed_ns);
        samples.orders.push(if lazy_first {
            "lazy-first"
        } else {
            "mapped-first"
        });
    }
    Ok(samples)
}

fn print_repeated_observation(
    label: &str,
    index: u32,
    before_sha: &str,
    after_sha: &str,
    orders: Vec<&str>,
    lazy_ns: Vec<u128>,
    mapped_ns: Vec<u128>,
) {
    let lazy_median = median(&lazy_ns);
    let mapped_median = median(&mapped_ns);
    println!(
        "{}",
        serde_json::json!({
            "fixture": label,
            "pairs": PAIR_COUNT,
            "faceIndex": index,
            "sourceSha256Before": before_sha,
            "sourceSha256After": after_sha,
            "pairOrders": orders,
            "lazyCreateAndShapeNs": lazy_ns,
            "fontiqueMappedCreateAndShapeNs": mapped_ns,
            "lazyCreateAndShapeMedianNs": lazy_median,
            "fontiqueMappedCreateAndShapeMedianNs": mapped_median,
            "metricsEqual": true,
            "rssMeasurement": false,
            "performanceAcceptance": false,
        })
    );
}

pub(super) fn run_repeated_probe(
    label: &str,
    path: &std::path::Path,
    index: u32,
    expected_sha: &str,
) -> Result<(), String> {
    let samples = collect_repeated_samples(path, index)?;
    let after_sha = source_sha(path)?;
    assert_eq!(after_sha, expected_sha);
    print_repeated_observation(
        label,
        index,
        expected_sha,
        &after_sha,
        samples.orders,
        samples.lazy_ns,
        samples.mapped_ns,
    );
    Ok(())
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

#[test]
#[ignore = "diagnostic timing comparison; run explicitly under the existing macOS test recipe"]
fn fontique_mmap_and_lazy_file_repeat_creation_and_shape_probe() -> Result<(), String> {
    let path = fixture_path();
    let expected_sha = source_sha(&path)?;
    run_repeated_probe("NotoSans-Regular.ttf", &path, 0, &expected_sha)
}

#[path = "svg_rasterize_text_harfrust_file_mapping_system_tests.rs"]
mod system_tests;
