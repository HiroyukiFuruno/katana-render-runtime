use super::{
    TypefaceFontProvider, load_typeface, preflight_families, register_family_aliases, used_face_ids,
};
use resvg::usvg;
use std::{collections::BTreeSet, sync::Arc};

#[test]
fn missing_registered_family_is_rejected_before_null_fallback() {
    let provider = TypefaceFontProvider::new();
    let families = BTreeSet::from(["Open Sans".to_string()]);
    assert!(preflight_families(&families, &provider).is_err());
    assert!(preflight_families(&BTreeSet::new(), &provider).is_ok());
}

#[test]
fn trailing_generic_does_not_replace_skia_first_family_contract() -> Result<(), String> {
    let mut database = usvg::fontdb::Database::new();
    database.load_font_data(super::super::font_file_fixture::FONT_BYTES.to_vec());
    let options = usvg::Options {
        fontdb: Arc::new(database),
        ..Default::default()
    };
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40"><text x="1" y="20" font-family="Noto Sans, sans-serif">AV</text></svg>"#;
    let tree = usvg::Tree::from_str(svg, &options).map_err(|error| error.to_string())?;
    let (ids, families) = used_face_ids(&tree).map_err(|error| format!("{error:?}"))?;
    assert_eq!(ids.len(), 1);
    assert_eq!(families, BTreeSet::from(["Noto Sans".to_string()]));
    Ok(())
}

#[test]
fn lowercase_family_alias_resolves_registered_physical_face() -> Result<(), String> {
    let mut database = usvg::fontdb::Database::new();
    database.load_font_data(super::super::font_file_fixture::FONT_BYTES.to_vec());
    let id = database.faces().next().ok_or("bundled font missing")?.id;
    let system = skia_safe::FontMgr::default();
    let (typeface, _) =
        load_typeface(&database, id, &system).map_err(|error| format!("{error:?}"))?;
    let mut provider = TypefaceFontProvider::new();
    let families = BTreeSet::from(["noto sans".to_string()]);
    assert!(preflight_families(&families, &provider).is_err());
    register_family_aliases(&mut provider, &typeface, "Noto Sans");
    preflight_families(&families, &provider).map_err(|error| format!("{error:?}"))
}
