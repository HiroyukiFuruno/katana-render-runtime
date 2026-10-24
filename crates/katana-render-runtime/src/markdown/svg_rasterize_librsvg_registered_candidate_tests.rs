use std::sync::Arc;

use glib::prelude::*;
use pangocairo::FontMap;
use resvg::usvg::{Options, Tree};
use tempfile::TempDir;

use super::{DefaultMapGuard, make_font_map, register_face};

#[test]
fn registers_noto_sans_and_loads_matching_checksum() -> Result<(), String> {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(super::super::super::font_file_fixture::FONT_BYTES.to_vec());
    let id = database.faces().next().ok_or("Noto Sans face missing")?.id;
    let map = make_font_map().map_err(|error| format!("{error:?}"))?;
    let temp = TempDir::new().map_err(|error| error.to_string())?;
    register_face(&database, id, &map, temp.path()).map_err(|error| format!("{error:?}"))
}

#[test]
fn default_font_map_guard_restores_after_normal_and_error_paths() -> Result<(), String> {
    let original = FontMap::default();
    with_replacement(Ok(())).map_err(str::to_string)?;
    if FontMap::default().as_ptr() != original.as_ptr() {
        return Err("normal guard path did not restore default map".to_string());
    }
    let original = FontMap::default();
    let failed = with_replacement(Err::<(), _>("registration failure"));
    if FontMap::default().as_ptr() != original.as_ptr() {
        return Err("error guard path did not restore default map".to_string());
    }
    assert_eq!(failed, Err("registration failure"));
    Ok(())
}

fn with_replacement<T>(result: Result<T, &str>) -> Result<T, &str> {
    let previous = FontMap::default()
        .dynamic_cast::<FontMap>()
        .map_err(|_| "default map is not a Cairo map")?;
    let replacement = make_font_map().map_err(|_| "FT map unavailable")?;
    FontMap::set_default(Some(&replacement));
    let _guard = DefaultMapGuard { previous };
    result
}

#[test]
fn registered_librsvg_text_produces_pixels() -> Result<(), String> {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(super::super::super::font_file_fixture::FONT_BYTES.to_vec());
    let options = Options {
        fontdb: Arc::new(database),
        ..Options::default()
    };
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="24"><text x="1" y="18" font-family="Noto Sans">A</text></svg>"#;
    let tree = Tree::from_str(svg, &options).map_err(|error| error.to_string())?;
    let image = super::render(svg, &tree, 1.0).map_err(|error| format!("{error:?}"))?;
    if !image.rgba.chunks_exact(4).any(|pixel| pixel[3] != 0) {
        return Err("registered text produced no alpha pixels".to_string());
    }
    Ok(())
}
