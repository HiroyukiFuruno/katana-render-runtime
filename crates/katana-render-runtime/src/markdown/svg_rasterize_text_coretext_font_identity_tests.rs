use super::{
    CTFont, Database, Source, mac_roman_name, matches, name_id, postscript_name, same_file,
    ttf_parser, unique_postscript_index,
};
use core_foundation::base::TCFType;
use core_graphics::{data_provider::CGDataProvider, font::CGFont};
use core_text::font::new_from_CGFont;
use objc2_core_foundation::CFString;
use std::sync::Arc;

const FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const SFNT_HEADER_BYTES: usize = 12;
const FONT_SIZE: f64 = 16.0;
const HELVETICA_FAMILY: &str = "Helvetica";

fn helvetica_coretext_font() -> objc2_core_foundation::CFRetained<CTFont> {
    /* WHY: 各source対照で同じCoreText要求を再利用し、差をfontdb faceだけに限定する。 */
    let name = CFString::from_str(HELVETICA_FAMILY);
    unsafe { CTFont::with_name(&name, FONT_SIZE, std::ptr::null()) }
}

fn normal_file_face(database: &Database, family: &str) -> Option<resvg::usvg::fontdb::ID> {
    database
        .faces()
        .find(|face| {
            face.weight == resvg::usvg::fontdb::Weight::NORMAL
                && face.style == resvg::usvg::fontdb::Style::Normal
                && face.families.iter().any(|(name, _)| name == family)
                && matches!(&face.source, Source::File(_) | Source::SharedFile(_, _))
        })
        .map(|face| face.id)
}

#[test]
fn mac_roman_postscript_names_require_ascii() {
    let record = mac_roman_name(b"Helvetica-Bold");
    assert_eq!(postscript_name(record).as_deref(), Some("Helvetica-Bold"));
    let maximum_length_name = postscript_name(mac_roman_name(&[b'A'; 63]));
    assert_eq!(maximum_length_name.as_ref().map(String::len), Some(63));
    assert!(maximum_length_name.is_some_and(|name| name.bytes().all(|byte| byte == b'A')));
    assert_eq!(postscript_name(mac_roman_name(&[b'A'; 64])), None);
    for invalid in [
        mac_roman_name(b"\xff"),
        ttf_parser::name::Name {
            encoding_id: 1,
            ..record
        },
        mac_roman_name(b"bad name"),
        mac_roman_name(b"bad/name"),
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
    let collection = super::super::super::tests::duplicate_sfnt_as_two_face_ttc(FONT)?;
    assert_eq!(ttf_parser::fonts_in_collection(&collection), Some(2));
    assert!(ttf_parser::Face::parse(&collection, 1).is_ok());
    assert_eq!(unique_postscript_index(&collection, &name), None);
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
fn bundled_font_without_postscript_records_has_no_unique_identity() -> Result<(), String> {
    let expected_name = "NotoSans-Regular";
    assert_eq!(unique_postscript_index(FONT, expected_name), Some(0));
    let mut font = FONT.to_vec();
    let table_count = u16::from_be_bytes([font[4], font[5]]) as usize;
    let name_offset = (0..table_count)
        .map(|index| SFNT_HEADER_BYTES + index * 16)
        .find(|&offset| &font[offset..offset + 4] == b"name")
        .ok_or("name table missing")?;
    let offset = u32::from_be_bytes(
        font[name_offset + 8..name_offset + 12]
            .try_into()
            .map_err(|_| "invalid name table offset")?,
    ) as usize;
    let count = u16::from_be_bytes([font[offset + 2], font[offset + 3]]) as usize;
    let mut removed = 0;
    for index in 0..count {
        let name_id_offset = offset + 6 + index * 12 + 6;
        if u16::from_be_bytes([font[name_id_offset], font[name_id_offset + 1]])
            == name_id::POST_SCRIPT_NAME
        {
            font[name_id_offset..name_id_offset + 2].copy_from_slice(&0u16.to_be_bytes());
            removed += 1;
        }
    }
    assert!(removed > 0);
    assert!(ttf_parser::Face::parse(&font, 0).is_ok());
    assert_eq!(unique_postscript_index(&font, expected_name), None);
    Ok(())
}

#[test]
fn binary_and_removed_font_faces_cannot_match_coretext() -> Result<(), String> {
    let font = helvetica_coretext_font();
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

#[test]
fn current_helvetica_file_face_matches_coretext() -> Result<(), String> {
    let database =
        crate::markdown::svg_rasterize::font::html_font_db_for_text(HELVETICA_FAMILY, "A");
    let id = normal_file_face(&database, HELVETICA_FAMILY)
        .ok_or("normal Helvetica file face missing")?;
    let font = helvetica_coretext_font();
    assert!(matches(&database, id, &font));
    Ok(())
}

#[test]
fn matching_file_font_respects_database_memo_registration() -> Result<(), String> {
    use super::super::super::super::fallback_memo as memo;
    let database = helvetica_file_database()?;
    let id = normal_file_face(&database, HELVETICA_FAMILY).ok_or("Helvetica face missing")?;
    let font = helvetica_coretext_font();
    memo::with_validated_tree_parse(|| {
        let generation = super::super::super::font_source_generation_uncached(&database, id);
        assert!(super::super::super::memo_usable());
        assert!(generation.durable_reusable());
        assert!(matches(&database, id, &font));
        assert_eq!(memo::physical_face_lookup(&database, id, &generation), None);
        let mut options = resvg::usvg::Options::default();
        memo::install(&mut options);
        let mut selected = Arc::clone(&database);
        (options.font_resolver.select_fallback)('魚', &[id], &mut selected);
        assert!(Arc::ptr_eq(&database, &selected));
        assert!(matches(&database, id, &font));
        assert_eq!(
            memo::physical_face_lookup(&database, id, &generation),
            Some(true)
        );
    });
    Ok(())
}

fn helvetica_file_database() -> Result<Arc<Database>, String> {
    let original =
        crate::markdown::svg_rasterize::font::html_font_db_for_text(HELVETICA_FAMILY, "A");
    let original_id = normal_file_face(&original, HELVETICA_FAMILY).ok_or("Helvetica missing")?;
    let Some(Source::File(path) | Source::SharedFile(path, _)) =
        original.face_source(original_id).map(|source| source.0)
    else {
        return Err("Helvetica file missing".into());
    };
    let mut database = Database::new();
    database.load_font_source(Source::File(path));
    Ok(Arc::new(database))
}

#[test]
fn moved_font_symlink_stops_matching_the_old_coretext_file_path()
-> Result<(), Box<dyn std::error::Error>> {
    let database =
        crate::markdown::svg_rasterize::font::html_font_db_for_text(HELVETICA_FAMILY, "A");
    let id =
        normal_file_face(&database, HELVETICA_FAMILY).ok_or("normal Helvetica face missing")?;
    let face = database.face(id).ok_or("selected Helvetica face missing")?;
    let (Source::File(path) | Source::SharedFile(path, _)) = &face.source else {
        return Err("Helvetica fixture has no file path".into());
    };
    let directory = tempfile::tempdir()?;
    let before = directory.path().join("linked-font");
    let after = directory.path().join("moved-font");
    let font = helvetica_coretext_font();
    std::os::unix::fs::symlink(path, &before)?;
    assert!(same_file(&font, &before));
    std::fs::rename(&before, &after)?;
    assert!(!same_file(&font, &before));
    assert!(same_file(&font, &after));
    Ok(())
}

#[test]
fn coretext_font_from_memory_has_no_file_identity() -> Result<(), String> {
    let provider = CGDataProvider::from_buffer(Arc::new(FONT.to_vec()));
    let graphics_font =
        CGFont::from_data_provider(provider).map_err(|_| "CGFont creation failed")?;
    let font = new_from_CGFont(&graphics_font, FONT_SIZE);
    /* WHY: 両クレートが参照する同一のCTFontを、所有者の生存中だけ借用する。 */
    let borrowed = unsafe { &*font.as_concrete_TypeRef().cast::<CTFont>() };
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
    assert!(path.is_file());
    assert!(!same_file(borrowed, &path));
    Ok(())
}

#[test]
fn coretext_match_rejects_a_font_replaced_during_face_read() -> Result<(), String> {
    use std::sync::atomic::Ordering;

    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let (database, id, source) = replaceable_helvetica(directory.path())?;
    let font = helvetica_coretext_font();
    assert!(matches(&database, id, &font));
    source.armed.store(true, Ordering::Relaxed);
    assert!(!matches(&database, id, &font));
    assert!(!source.replacement.exists());
    assert!(!same_file(&font, &source.path));
    Ok(())
}

use crate::markdown::svg_rasterize::text_shaping::cache::generation_tests::during_generation_tests::ReplacedWhileReading;

type ReplaceableFace = (Database, resvg::usvg::fontdb::ID, Arc<ReplacedWhileReading>);

fn replaceable_helvetica(directory: &std::path::Path) -> Result<ReplaceableFace, String> {
    let original = helvetica_file_database()?;
    let id = normal_file_face(&original, HELVETICA_FAMILY).ok_or("Helvetica missing")?;
    let mut face = original.face(id).ok_or("Helvetica face missing")?.clone();
    let Source::File(path) = &face.source else {
        return Err("expected a physical Helvetica source".into());
    };
    let source = Arc::new(ReplacedWhileReading {
        path: directory.join("Helvetica.ttc"),
        replacement: directory.join("replacement.ttf"),
        armed: Default::default(),
        bytes: std::fs::read(path).map_err(|error| error.to_string())?,
    });
    std::os::unix::fs::symlink(path, &source.path).map_err(|error| error.to_string())?;
    std::fs::write(&source.replacement, FONT).map_err(|error| error.to_string())?;
    let data = Arc::clone(&source);
    face.source = Source::SharedFile(source.path.clone(), data);
    let mut database = Database::new();
    let id = database.push_face_info(face);
    Ok((database, id, source))
}

#[test]
fn bundled_noto_file_face_does_not_match_helvetica_coretext() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("NotoSans-Regular.ttf");
    std::fs::write(&path, FONT).map_err(|error| error.to_string())?;
    let mut database = Database::new();
    database.load_font_source(Source::File(path));
    let face = database.faces().next().ok_or("Noto file face missing")?;
    assert_eq!(face.weight, resvg::usvg::fontdb::Weight::NORMAL);
    assert!(face.families.iter().any(|(name, _)| name == "Noto Sans"));
    assert!(matches!(&face.source, Source::File(_)));
    let font = helvetica_coretext_font();
    assert!(!matches(&database, face.id, &font));
    Ok(())
}
