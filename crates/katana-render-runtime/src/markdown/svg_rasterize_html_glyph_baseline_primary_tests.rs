use super::{has_known_missing_base_table, usvg};
use read_fonts::{FontRef, TableProvider, types::Tag};
use rstest::rstest;
use std::sync::Arc;

const NOTO: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const CJK: &[u8] =
    include_bytes!("../../../../tests/fixtures/fonts/han-kerning/NotoSansCJKjp-han14.otf");

#[rstest]
#[case::noto_without_base(NOTO, false)]
#[case::cjk_with_base(CJK, true)]
fn known_missing_base_matches_real_font_table(
    #[case] bytes: &[u8],
    #[case] expected_base: bool,
) -> Result<(), String> {
    let font = FontRef::new(bytes).map_err(|error| format!("font parse failed: {error:?}"))?;
    let base_present = font.data_for_tag(Tag::new(b"BASE")).is_some();
    assert_eq!(base_present, expected_base);
    let mut database = usvg::fontdb::Database::new();
    let ids = database.load_font_source(usvg::fontdb::Source::Binary(Arc::new(bytes.to_vec())));
    let [id] = ids.as_slice() else {
        return Err("expected one font face".to_owned());
    };
    assert_eq!(has_known_missing_base_table(&database, *id), !base_present);
    Ok(())
}

#[test]
fn unknown_face_is_not_treated_as_known_missing_base() {
    let database = usvg::fontdb::Database::new();
    let unknown = usvg::fontdb::ID::dummy();
    assert!(database.face(unknown).is_none());
    assert!(!has_known_missing_base_table(&database, unknown));
}
