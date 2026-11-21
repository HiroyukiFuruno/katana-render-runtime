use super::{fontdb_family_for_database, matching_font_face_with_platform, usvg::fontdb::Family};
use crate::markdown::svg_rasterize::font::{bundled_font_db, platform::PlatformFontStrategy};
use resvg::usvg::fontdb::{Database, ID};
use std::{collections::BTreeSet, ops::ControlFlow, path::PathBuf};

struct OverridePlatform(ControlFlow<Option<ID>>);

impl PlatformFontStrategy for OverridePlatform {
    fn installed_paths(&self, _: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
        BTreeSet::new()
    }

    fn face_override(
        &self,
        _: &Database,
        _: &str,
        _: &[String],
        _: u16,
        _: bool,
    ) -> ControlFlow<Option<ID>> {
        self.0
    }
}

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

#[test]
fn platform_override_can_select_or_reject_before_database_query() {
    let database = bundled_font_db();
    let selected = database.faces().next().map(|face| face.id);
    assert_eq!(
        matching_font_face_with_platform(
            &database,
            "ignored",
            400,
            false,
            &OverridePlatform(ControlFlow::Break(selected))
        ),
        selected
    );
    assert_eq!(
        matching_font_face_with_platform(
            &database,
            "ignored",
            400,
            false,
            &OverridePlatform(ControlFlow::Break(None)),
        ),
        None
    );
}

#[test]
fn platform_continue_falls_through_to_the_requested_bundled_family() {
    let database = bundled_font_db();
    let selected = matching_font_face_with_platform(
        &database,
        "Noto Sans",
        400,
        false,
        &OverridePlatform(ControlFlow::Continue(())),
    );
    let face = selected.and_then(|id| database.face(id));
    assert!(face.is_some_and(|face| {
        face.families
            .iter()
            .any(|(family, _)| family.eq_ignore_ascii_case("Noto Sans"))
    }));
}
