use super::super::font_file_fixture::FONT_BYTES;
use super::{
    Tag, repack_face,
    verification_support::{change_name_table, font_with_only, truncated_name_font},
    verify_face,
};

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
