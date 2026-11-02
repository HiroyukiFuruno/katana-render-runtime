use super::{
    CTFont, Database, Source, mac_roman_name, matches, name_id, postscript_name, ttf_parser,
    unique_postscript_index,
};
use objc2_core_foundation::CFString;

const FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const SFNT_HEADER_BYTES: usize = 12;
const FONT_SIZE: f64 = 16.0;

#[test]
fn mac_roman_postscript_names_require_ascii() {
    let record = mac_roman_name(b"Helvetica-Bold");
    assert_eq!(postscript_name(record).as_deref(), Some("Helvetica-Bold"));
    for invalid in [
        mac_roman_name(b"\xff"),
        ttf_parser::name::Name {
            encoding_id: 1,
            ..record
        },
        mac_roman_name(b"bad name"),
    ] {
        assert_eq!(postscript_name(invalid), None);
    }
}

#[test]
fn bundled_font_postscript_identity_rejects_missing_name_and_truncated_font() -> Result<(), String>
{
    let face = ttf_parser::Face::parse(FONT, 0).map_err(|error| format!("{error:?}"))?;
    let name = face
        .names()
        .into_iter()
        .filter(|record| record.name_id == name_id::POST_SCRIPT_NAME)
        .find_map(postscript_name)
        .ok_or("bundled font has no PostScript name")?;
    assert_eq!(unique_postscript_index(FONT, &name), Some(0));
    assert_eq!(
        unique_postscript_index(FONT, "Definitely-Not-The-Bundled-Font"),
        None
    );
    assert_eq!(
        unique_postscript_index(&FONT[..SFNT_HEADER_BYTES], &name),
        None
    );
    Ok(())
}

#[test]
fn binary_and_removed_font_faces_cannot_match_coretext() -> Result<(), String> {
    let name = CFString::from_str("Helvetica");
    let font = unsafe { CTFont::with_name(&name, FONT_SIZE, std::ptr::null()) };
    let mut database = Database::new();
    database.load_font_data(FONT.to_vec());
    let id = database.faces().next().ok_or("bundled font missing")?.id;
    assert!(matches!(
        database.face(id).map(|face| &face.source),
        Some(Source::Binary(_))
    ));
    assert!(!matches(&database, id, &font));
    database.remove_face(id);
    assert!(database.face(id).is_none());
    assert!(!matches(&database, id, &font));
    Ok(())
}
