use super::super::font_file_fixture::FONT_BYTES;
use super::{
    ReadFontRef, TableProvider, Tag, repack_face, validate_metrics,
    verification_support::{
        change_name_table, font_with_changed_table_byte, font_with_only, truncated_name_font,
    },
    verify_face,
};

const UNITS_PER_EM_LOW_BYTE: usize = 19;
const NUM_GLYPHS_LOW_BYTE: usize = 5;
const METRIC_CASES: [([u8; 4], usize, &str); 2] = [
    (
        *b"head",
        UNITS_PER_EM_LOW_BYTE,
        "repacked face units-per-em changed",
    ),
    (
        *b"maxp",
        NUM_GLYPHS_LOW_BYTE,
        "repacked face glyph count changed",
    ),
];

#[test]
fn verification_rejects_invalid_source_output_and_changed_table() -> Result<(), String> {
    let valid = repack_face(FONT_BYTES, 0)?;
    verify_face(FONT_BYTES, 0, &valid)?;
    let changed = change_name_table(&valid)?;
    let truncated = &valid[..1];
    let cases = [
        (
            FONT_BYTES,
            u32::MAX,
            valid.as_slice(),
            "source TTC face verification failed",
        ),
        (
            FONT_BYTES,
            0,
            truncated,
            "repacked SFNT verification failed",
        ),
        (FONT_BYTES, 0, changed.as_slice(), "repacked table"),
    ];
    for (source, index, output, expected) in cases {
        let Err(error) = verify_face(source, index, output) else {
            return Err("invalid face verification unexpectedly succeeded".to_owned());
        };
        assert_verification_error(&error, expected);
    }
    Ok(())
}

fn assert_verification_error(error: &str, expected: &str) {
    assert!(
        error.contains(expected),
        "unexpected verification error: {error}"
    );
    if expected == "repacked table" {
        assert!(error.contains("changed"), "unexpected table error: {error}");
    }
}

#[test]
fn fixture_mutator_rejects_invalid_font_missing_name_and_bad_offset() -> Result<(), String> {
    let missing = font_with_only(Tag::new(b"head"))?;
    let truncated = truncated_name_font()?;
    assert!(change_name_table(&[0]).is_err());
    assert!(font_with_only(Tag::new(b"zzzz")).is_err());
    for (font, expected) in [
        (missing.as_slice(), "name table missing"),
        (truncated.as_slice(), "name table offset invalid"),
    ] {
        let Err(error) = change_name_table(font) else {
            return Err("invalid fixture unexpectedly succeeded".to_owned());
        };
        assert_eq!(error, expected);
    }
    Ok(())
}

#[test]
fn verification_rejects_changed_table_set_and_missing_metric_tables() -> Result<(), String> {
    let head_only = font_with_only(Tag::new(b"head"))?;
    let maxp_only = font_with_only(Tag::new(b"maxp"))?;
    assert_eq!(
        verify_face(FONT_BYTES, 0, &head_only),
        Err("repacked face table tag set changed".into())
    );
    for (font, expected) in [
        (head_only.as_slice(), "source maxp table missing:"),
        (maxp_only.as_slice(), "source head table missing:"),
    ] {
        let error = verify_face(font, 0, font)
            .err()
            .ok_or("incomplete font unexpectedly accepted")?;
        assert!(error.starts_with(expected), "unexpected error: {error}");
    }
    Ok(())
}

#[test]
fn metric_guard_rejects_changed_units_and_glyph_count() -> Result<(), String> {
    let source = ReadFontRef::from_index(FONT_BYTES, 0).map_err(|error| format!("{error:?}"))?;
    for (tag, offset, expected) in METRIC_CASES {
        let tag = Tag::new(&tag);
        let changed = font_with_changed_table_byte(FONT_BYTES, tag, offset)?;
        let output = ReadFontRef::from_index(&changed, 0).map_err(|error| format!("{error:?}"))?;
        assert_metric_changed(&source, &output, tag)?;
        assert_eq!(validate_metrics(&source, &output), Err(expected.into()));
        let error = verify_face(FONT_BYTES, 0, &changed)
            .err()
            .ok_or("changed table unexpectedly accepted")?;
        assert_verification_error(&error, "repacked table");
    }
    Ok(())
}

#[test]
fn table_byte_mutator_rejects_invalid_inputs() {
    assert!(font_with_changed_table_byte(&[0], Tag::new(b"head"), 0).is_err());
    for (tag, offset, expected) in [
        (Tag::new(b"zzzz"), 0, "source table missing"),
        (Tag::new(b"head"), usize::MAX, "table byte missing"),
    ] {
        assert_eq!(
            font_with_changed_table_byte(FONT_BYTES, tag, offset)
                .err()
                .as_deref(),
            Some(expected)
        );
    }
}

fn assert_metric_changed(
    source: &ReadFontRef<'_>,
    output: &ReadFontRef<'_>,
    tag: Tag,
) -> Result<(), String> {
    if tag == Tag::new(b"head") {
        let before = source.head().map_err(|error| format!("{error:?}"))?;
        let after = output.head().map_err(|error| format!("{error:?}"))?;
        assert_ne!(before.units_per_em(), after.units_per_em());
    } else {
        let before = source.maxp().map_err(|error| format!("{error:?}"))?;
        let after = output.maxp().map_err(|error| format!("{error:?}"))?;
        assert_ne!(before.num_glyphs(), after.num_glyphs());
    }
    Ok(())
}
