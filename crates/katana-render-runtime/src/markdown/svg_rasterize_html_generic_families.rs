use super::super::super::configure_generic_families;
use resvg::usvg;

pub(super) fn configure_html_generic_families(database: &mut usvg::fontdb::Database) {
    configure_generic_families(database);
    configure_html_sans_serif_family(database);
    configure_html_serif_family(database);
}

fn configure_html_sans_serif_family(database: &mut usvg::fontdb::Database) {
    let family = configured_html_sans_serif_family();
    if has_family(database, family) {
        database.set_sans_serif_family(family);
        return;
    }
    for family in ["Arial", "Liberation Sans"] {
        if has_family(database, family) {
            database.set_sans_serif_family(family);
            break;
        }
    }
}

pub(in super::super) fn configured_html_sans_serif_family() -> &'static str {
    /* WHY: macOSのHTML sans-serifはChromeの実選択に合わせる。 */
    #[cfg(target_os = "macos")]
    {
        "Helvetica"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "Arial"
    }
}

fn configure_html_serif_family(database: &mut usvg::fontdb::Database) {
    for family in ["Times", "Times New Roman", "Liberation Serif"] {
        if has_family(database, family) {
            database.set_serif_family(family);
            break;
        }
    }
}

fn has_family(database: &usvg::fontdb::Database, family: &str) -> bool {
    database
        .query(&usvg::fontdb::Query {
            families: &[usvg::fontdb::Family::Name(family)],
            ..usvg::fontdb::Query::default()
        })
        .is_some()
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::configure_html_sans_serif_family;
    use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
    use resvg::usvg::fontdb::{Database, Family, Language};

    #[test]
    fn html_sans_serif_prefers_the_configured_family_when_it_is_available() -> Result<(), String> {
        let mut source = Database::new();
        source.load_font_data(FONT_BYTES.to_vec());
        let mut face = source
            .faces()
            .next()
            .ok_or("bundled sans face missing")?
            .clone();
        face.families = vec![("Arial".to_owned(), Language::English_UnitedStates)];
        let mut database = Database::new();
        database.push_face_info(face);

        configure_html_sans_serif_family(&mut database);

        assert_eq!(database.family_name(&Family::SansSerif), "Arial");
        Ok(())
    }
}
