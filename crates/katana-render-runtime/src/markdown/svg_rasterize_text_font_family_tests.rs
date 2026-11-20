use super::{PlatformFontStrategy, fontdb_family_for_database, usvg::fontdb::Family};
use crate::markdown::svg_rasterize::font::bundled_font_db;
use std::{collections::BTreeSet, path::PathBuf};

struct FontFamilyFixture;

impl PlatformFontStrategy for FontFamilyFixture {
    fn installed_paths(&self, _: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
        BTreeSet::new()
    }

    fn installed_system_ui_family(&self) -> Option<&'static str> {
        Some("Noto Sans")
    }

    fn compatible_named_family(&self, _: &str) -> Option<&'static str> {
        Some("Noto Sans")
    }
}

#[test]
fn database_family_selection_resolves_the_platform_alias_and_compatible_face() {
    let database = bundled_font_db();
    let platform = FontFamilyFixture;
    for name in ["system-ui", "Missing platform font"] {
        assert_eq!(
            fontdb_family_for_database(&database, name, &platform),
            Family::Name("Noto Sans"),
        );
    }
}
