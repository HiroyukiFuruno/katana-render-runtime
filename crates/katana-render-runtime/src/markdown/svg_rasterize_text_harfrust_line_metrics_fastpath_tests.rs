use super::{
    Font, cached_face_line_metrics, diagnosed_cached_face_line_metrics, metrics_from_font,
    sorted_blob_directory, static_library_metrics,
};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use read_fonts::FontRef;
use resvg::usvg::fontdb::{Database, ID, Source};
use std::sync::Arc;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const SFNT_TAG_BYTE_LENGTH: usize = 4;
const OS2_FS_SELECTION_OFFSET: usize = 62;
const OS2_TYPO_GAP_OFFSET: usize = 72;
const HHEA_ASCENDER_OFFSET: usize = 4;
const HHEA_DESCENDER_OFFSET: usize = 6;
const HHEA_LINE_GAP_OFFSET: usize = 8;
const SIZES: [f32; 6] = [2.5, 7.5, 12.25, 12.5, 15.0, 22.5];
const MIN_DESCENDER_MAGNITUDE: f32 = 32768.0;
type TestResult = Result<(), String>;
type MetricsReader = fn(&Database, ID, f32) -> Result<Option<(f32, f32)>, ()>;

fn oracle(data: &[u8], size: f32) -> Option<(f32, f32)> {
    super::tests::legacy_metrics(data, 0, size)
}

fn assert_bits(actual: Option<(f32, f32)>, expected: Option<(f32, f32)>) {
    assert_eq!(
        actual.map(|(a, d)| (a.to_bits(), d.to_bits())),
        expected.map(|(a, d)| (a.to_bits(), d.to_bits()))
    );
}

fn table_offset(data: &[u8], tag: &[u8; SFNT_TAG_BYTE_LENGTH]) -> Result<usize, String> {
    let face = FontRef::new(data).map_err(|error| format!("table fixture: {error:?}"))?;
    face.table_directory()
        .table_records()
        .iter()
        .find(|record| record.tag().to_be_bytes() == *tag)
        .map(|record| record.offset() as usize)
        .ok_or_else(|| "real fixture table absent".to_string())
}

fn set_word(
    data: &mut [u8],
    tag: &[u8; SFNT_TAG_BYTE_LENGTH],
    field: usize,
    value: u16,
) -> TestResult {
    let start = table_offset(data, tag)? + field;
    let destination = data
        .get_mut(start..start + 2)
        .ok_or("fixture word absent")?;
    destination.copy_from_slice(&value.to_be_bytes());
    Ok(())
}

fn font(data: &[u8]) -> Result<Font, String> {
    Font::new(data.to_vec(), 0).ok_or_else(|| "real library font absent".to_string())
}

fn database(source: Source) -> Result<Database, String> {
    let mut parsed = Database::new();
    parsed.load_font_data(FONT_BYTES.to_vec());
    let mut face = parsed
        .faces()
        .next()
        .ok_or("real source face absent")?
        .clone();
    face.source = source;
    let mut database = Database::new();
    database.push_face_info(face);
    Ok(database)
}

#[test]
fn line_metrics_require_a_render_scope_for_both_readers() -> TestResult {
    let database = database(Source::Binary(Arc::new(FONT_BYTES.to_vec())))?;
    let id = database.faces().next().ok_or("source face absent")?.id;
    let readers: [MetricsReader; 2] =
        [cached_face_line_metrics, diagnosed_cached_face_line_metrics];
    for read in readers {
        for size in SIZES {
            assert_eq!(read(&database, id, size), Ok(None));
            let expected = oracle(FONT_BYTES, size);
            assert!(expected.is_some());
            SvgRasterizeOps::with_html_font_render_scope(|| -> TestResult {
                let actual = read(&database, id, size).map_err(|()| "source commit rejected")?;
                assert_bits(actual, expected);
                Ok(())
            })?;
        }
    }
    Ok(())
}

#[test]
fn static_library_metrics_match_full_face_bits_for_real_sources() -> TestResult {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
    for source in [
        Source::File(path.clone()),
        Source::Binary(Arc::new(FONT_BYTES.to_vec())),
        Source::SharedFile(path, Arc::new(FONT_BYTES.to_vec())),
    ] {
        let database = database(source)?;
        let id = database.faces().next().ok_or("source face absent")?.id;
        SvgRasterizeOps::with_html_font_render_scope(|| -> TestResult {
            for size in SIZES {
                let expected = oracle(FONT_BYTES, size);
                assert!(expected.is_some());
                assert_bits(static_library_metrics(&font(FONT_BYTES)?, size), expected);
                let actual = cached_face_line_metrics(&database, id, size)
                    .map_err(|()| "source commit rejected")?;
                assert_bits(actual, expected);
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn static_library_metrics_preserve_fractional_rounding_and_signed_leading() -> TestResult {
    for gap in [200_i16, -200_i16] {
        let mut bytes = FONT_BYTES.to_vec();
        set_word(&mut bytes, b"OS/2", OS2_FS_SELECTION_OFFSET, 1 << 7)?;
        set_word(&mut bytes, b"OS/2", OS2_TYPO_GAP_OFFSET, gap as u16)?;
        let font = font(&bytes)?;
        for size in SIZES {
            let expected = oracle(&bytes, size);
            assert!(expected.is_some());
            assert_bits(static_library_metrics(&font, size), expected);
        }
    }
    Ok(())
}

#[test]
fn one_sided_zero_and_old_os2_selection_keep_legacy_results() -> TestResult {
    for field in [HHEA_ASCENDER_OFFSET, HHEA_DESCENDER_OFFSET] {
        let mut bytes = FONT_BYTES.to_vec();
        set_word(&mut bytes, b"OS/2", OS2_FS_SELECTION_OFFSET, 0)?;
        set_word(&mut bytes, b"hhea", field, 0)?;
        let font = font(&bytes)?;
        assert!(static_library_metrics(&font, 15.0).is_none());
        assert_bits(metrics_from_font(&font, 15.0), oracle(&bytes, 15.0));
    }
    let mut bytes = FONT_BYTES.to_vec();
    set_word(&mut bytes, b"OS/2", 0, 3)?;
    set_word(&mut bytes, b"OS/2", OS2_FS_SELECTION_OFFSET, 1 << 7)?;
    let font = font(&bytes)?;
    assert!(static_library_metrics(&font, 15.0).is_none());
    assert_bits(metrics_from_font(&font, 15.0), oracle(&bytes, 15.0));
    Ok(())
}

#[test]
fn extreme_hhea_descender_preserves_legacy_line_metrics() -> TestResult {
    for descender in [i16::MIN, i16::MIN + 1] {
        let bytes = extreme_descender_font(descender)?;
        let font = font(&bytes)?;
        let line = font.metrics().h_line().ok_or("line metrics missing")?;
        assert_eq!(line.descender.to_f32(), f32::from(descender));
        let legacy = rustybuzz::ttf_parser::Face::parse(&bytes, 0)
            .map_err(|error| format!("legacy face: {error:?}"))?;
        assert_eq!(legacy.descender(), descender);
        for size in SIZES {
            let expected = oracle(&bytes, size);
            assert!(expected.is_some());
            if descender == i16::MIN {
                assert_eq!(static_library_metrics(&font, size), None);
            } else {
                assert_bits(static_library_metrics(&font, size), expected);
            }
            assert_bits(metrics_from_font(&font, size), expected);
        }
    }
    Ok(())
}

#[test]
fn minimum_descender_at_units_per_em_has_exact_positive_magnitude() -> TestResult {
    let mut bytes = extreme_descender_font(i16::MIN)?;
    set_word(&mut bytes, b"hhea", HHEA_LINE_GAP_OFFSET, 0)?;
    let face = rustybuzz::ttf_parser::Face::parse(&bytes, 0)
        .map_err(|error| format!("legacy face: {error:?}"))?;
    assert_eq!(face.descender(), i16::MIN);
    assert_eq!(face.line_gap(), 0);
    let size = f32::from(face.units_per_em());
    let (_, descent) = metrics_from_font(&font(&bytes)?, size).ok_or("line metrics missing")?;
    assert_eq!(descent, MIN_DESCENDER_MAGNITUDE);
    Ok(())
}

fn extreme_descender_font(descender: i16) -> Result<Vec<u8>, String> {
    let mut bytes = FONT_BYTES.to_vec();
    set_word(&mut bytes, b"OS/2", OS2_FS_SELECTION_OFFSET, 0)?;
    set_word(
        &mut bytes,
        b"hhea",
        HHEA_DESCENDER_OFFSET,
        u16::from_be_bytes(descender.to_be_bytes()),
    )?;
    Ok(bytes)
}

#[test]
fn blob_directory_rejects_missing_and_invalid_face_references() -> TestResult {
    let font = font(FONT_BYTES)?;
    assert!(matches!(font.source(), harfrust::font::Source::Blob(_)));
    let original = database(Source::Binary(Arc::new(FONT_BYTES.to_vec())))?;
    let mut face = original.faces().next().ok_or("source face absent")?.clone();
    assert!(sorted_blob_directory(&original, face.id, &font));
    assert!(!sorted_blob_directory(&Database::new(), face.id, &font));
    face.index = 1;
    let mut mismatched = Database::new();
    mismatched.push_face_info(face);
    let id = mismatched
        .faces()
        .next()
        .ok_or("mismatched face absent")?
        .id;
    assert!(!sorted_blob_directory(&mismatched, id, &font));
    Ok(())
}

#[test]
fn invalid_required_tables_keep_the_full_parser_rejection() -> TestResult {
    for (tag, field, value) in [
        (*b"head", 18, 0),
        (*b"head", 50, 2),
        (*b"maxp", 4, 0),
        (*b"maxp", 0, u16::MAX),
    ] {
        let mut bytes = FONT_BYTES.to_vec();
        set_word(&mut bytes, &tag, field, value)?;
        assert!(oracle(&bytes, 15.0).is_none());
        if let Some(font) = Font::new(bytes, 0) {
            assert!(static_library_metrics(&font, 15.0).is_none());
            assert!(metrics_from_font(&font, 15.0).is_none());
        }
    }
    Ok(())
}

fn recursive_bytes() -> Result<Vec<u8>, String> {
    use sha2::{Digest, Sha256};
    let path = std::path::PathBuf::from(
        std::env::var_os("KRR_RECURSIVE_PROBE_SFNT_PATH").ok_or("public SFNT path missing")?,
    );
    assert!(path.is_absolute());
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    let sha = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        sha,
        "f0b8414e5c903579bfd9dbca230a49f9063d92c6d622c6700f121a50a61e06ba"
    );
    Ok(bytes)
}

#[test]
#[ignore = "diagnostic only; pinned public Recursive SFNT required"]
fn variable_face_keeps_direct_none_and_full_parser_fallback() -> TestResult {
    let bytes = recursive_bytes()?;
    assert!(
        rustybuzz::ttf_parser::Face::parse(&bytes, 0)
            .map_err(|e| format!("{e:?}"))?
            .is_variable()
    );
    let mut database = Database::new();
    database.load_font_data(bytes.clone());
    let id = database.faces().next().ok_or("variable face absent")?.id;
    SvgRasterizeOps::with_html_font_render_scope(|| {
        for size in SIZES {
            assert_eq!(cached_face_line_metrics(&database, id, size), Ok(None));
            assert!(metrics_from_font(&font(&bytes)?, size).is_none());
            let fallback = database
                .with_face_data(id, |data, _| oracle(data, size))
                .flatten();
            assert!(fallback.is_some());
            assert_bits(fallback, oracle(&bytes, size));
        }
        Ok(())
    })
}

#[path = "svg_rasterize_text_harfrust_line_metrics_rejection_tests.rs"]
mod rejection_tests;
