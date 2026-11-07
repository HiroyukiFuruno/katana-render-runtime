#[cfg(target_os = "macos")]
use super::{AliasProbe, platform_fallback, system_ui_bold_alias};
use super::{FallbackAttributes, matching_fallback_regular, usvg};

pub(in crate::markdown::svg_rasterize::text_shaping) fn matching_fallback_with_request(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested: &usvg::Font,
    mut probe: impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> (Option<usvg::fontdb::ID>, bool) {
    let attributes = requested_fallback_attributes(requested);
    let require_non_interface =
        match requested_preflight(database, base_face_id, character, attributes, &mut probe) {
            Ok(value) => value,
            Err(result) => return result,
        };
    matching_requested_regular(
        database,
        base_face_id,
        character,
        attributes,
        require_non_interface,
        |id, glyph| probe_requested_face(database, id, glyph, &mut probe),
    )
}

fn matching_requested_regular(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    attributes: FallbackAttributes,
    require_non_interface: bool,
    mut probe: impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> (Option<usvg::fontdb::ID>, bool) {
    #[cfg(target_os = "macos")]
    let cacheable = {
        let (preferred, cacheable) =
            platform_fallback::select(database, base_face_id, character, attributes, &mut probe);
        if preferred.is_some() {
            return (preferred, cacheable);
        }
        cacheable
    };
    #[cfg(not(target_os = "macos"))]
    let cacheable = true;
    let (selected, regular_cacheable) = matching_fallback_regular(
        database,
        base_face_id,
        character,
        attributes,
        require_non_interface,
        &mut probe,
    );
    (selected, cacheable && regular_cacheable)
}

fn probe_requested_face(
    database: &usvg::fontdb::Database,
    id: usvg::fontdb::ID,
    glyph: char,
    probe: &mut impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> Option<bool> {
    #[cfg(target_os = "macos")]
    if database.face(id).is_some_and(|face| {
        face.families
            .iter()
            .any(|(family, _)| super::super::super::font::is_internal_family(family))
    }) {
        return Some(false);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = database;
    probe(id, glyph)
}

pub(super) fn requested_preflight(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    attributes: FallbackAttributes,
    probe: &mut impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> Result<bool, (Option<usvg::fontdb::ID>, bool)> {
    let Some(base_face) = database.face(base_face_id) else {
        return Err((None, false));
    };
    #[cfg(target_os = "macos")]
    return requested_alias_outcome(database, base_face, character, attributes, probe);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (base_face, character, attributes, probe);
        Ok(false)
    }
}

#[cfg(target_os = "macos")]
fn requested_alias_outcome(
    database: &usvg::fontdb::Database,
    base_face: &usvg::fontdb::FaceInfo,
    character: char,
    attributes: FallbackAttributes,
    probe: &mut impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> Result<bool, (Option<usvg::fontdb::ID>, bool)> {
    match requested_alias(database, base_face, character, attributes, probe) {
        AliasProbe::Selected(id) => Err((Some(id), true)),
        AliasProbe::Unavailable => Err((None, false)),
        AliasProbe::NoGlyph => Ok(true),
        AliasProbe::NotNeeded => Ok(false),
    }
}

#[cfg(target_os = "macos")]
fn requested_alias(
    database: &usvg::fontdb::Database,
    base_face: &usvg::fontdb::FaceInfo,
    character: char,
    attributes: FallbackAttributes,
    probe: &mut impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> AliasProbe {
    system_ui_bold_alias(
        database,
        base_face,
        character,
        attributes.weight,
        attributes.style != usvg::fontdb::Style::Normal
            || attributes.stretch != usvg::fontdb::Stretch::Normal,
        probe,
    )
}

fn requested_fallback_attributes(requested: &usvg::Font) -> FallbackAttributes {
    FallbackAttributes {
        weight: requested.weight(),
        style: usvg::fontdb::Style::from(requested.style()),
        stretch: usvg::fontdb::Stretch::from(requested.stretch()),
    }
}
