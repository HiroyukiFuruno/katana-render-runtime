use super::{Font, cached_face_line_metrics, metrics_from_font, static_library_metrics};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use read_fonts::FontRef;
use resvg::usvg::fontdb::{Database, Source};
use std::sync::Arc;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const SFNT_TAG_BYTE_LENGTH: usize = 4;
const SIZES: [f32; 6] = [2.5, 7.5, 12.25, 12.5, 15.0, 22.5];
type TestResult = Result<(), String>;

fn oracle(data: &[u8], size: f32) -> Option<(f32, f32)> {
    let face = rustybuzz::ttf_parser::Face::parse(data, 0).ok()?;
    let scale = size / f32::from(face.units_per_em());
    let leading = (f32::from(face.line_gap()) * scale).round();
    let before = (leading / 2.0).floor();
    Some((
        (f32::from(face.ascender()) * scale).round() + before,
        (-f32::from(face.descender()) * scale).round() + leading - before,
    ))
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
        set_word(&mut bytes, b"OS/2", 62, 1 << 7)?;
        set_word(&mut bytes, b"OS/2", 72, gap as u16)?;
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
    for field in [4, 6] {
        let mut bytes = FONT_BYTES.to_vec();
        set_word(&mut bytes, b"OS/2", 62, 0)?;
        set_word(&mut bytes, b"hhea", field, 0)?;
        let font = font(&bytes)?;
        assert!(static_library_metrics(&font, 15.0).is_none());
        assert_bits(metrics_from_font(&font, 15.0), oracle(&bytes, 15.0));
    }
    let mut bytes = FONT_BYTES.to_vec();
    set_word(&mut bytes, b"OS/2", 0, 3)?;
    set_word(&mut bytes, b"OS/2", 62, 1 << 7)?;
    let font = font(&bytes)?;
    assert!(static_library_metrics(&font, 15.0).is_none());
    assert_bits(metrics_from_font(&font, 15.0), oracle(&bytes, 15.0));
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
