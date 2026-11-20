use super::{fontdb_family_for_database, usvg::fontdb::Family};
use crate::markdown::svg_rasterize::font::bundled_font_db;

#[test]
fn database_family_selection_resolves_the_platform_alias_and_compatible_face() {
    let database = bundled_font_db();
    for (name, system_ui, compatible) in [
        ("system-ui", Some("Noto Sans"), None),
        ("Missing platform font", None, Some("Noto Sans")),
    ] {
        assert_eq!(
            fontdb_family_for_database(&database, name, system_ui, compatible),
            Family::Name("Noto Sans"),
        );
    }
}
