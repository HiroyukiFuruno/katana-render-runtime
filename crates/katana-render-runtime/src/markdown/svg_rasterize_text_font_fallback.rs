#[cfg(target_os = "macos")]
use super::INTERFACE_BOLD_WEIGHT;
use super::{FallbackAttributes, fallback_attributes, scan_fallback_faces, usvg};

#[cfg(not(target_os = "macos"))]
pub(super) fn matching_fallback_nonmacos(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested_weight: u16,
    requested_italic: bool,
    probe: impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> (Option<usvg::fontdb::ID>, bool) {
    if database.face(base_face_id).is_none() {
        return (None, false);
    }
    matching_fallback_regular(
        database,
        base_face_id,
        character,
        fallback_attributes(requested_weight, requested_italic),
        false,
        probe,
    )
}

#[cfg(target_os = "macos")]
pub(super) fn matching_fallback_macos(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested_weight: u16,
    requested_italic: bool,
    mut probe: impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> (Option<usvg::fontdb::ID>, bool) {
    let Some(base_face) = database.face(base_face_id) else {
        return (None, false);
    };
    let alias_probe = system_ui_bold_alias(
        database,
        base_face,
        character,
        requested_weight,
        requested_italic,
        &mut probe,
    );
    finish_macos_fallback(
        database,
        base_face_id,
        character,
        requested_weight,
        requested_italic,
        alias_probe,
        probe,
    )
}

#[cfg(target_os = "macos")]
fn finish_macos_fallback(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested_weight: u16,
    requested_italic: bool,
    alias_probe: AliasProbe,
    probe: impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> (Option<usvg::fontdb::ID>, bool) {
    let interface_face_is_required = match alias_probe {
        AliasProbe::Selected(id) => return (Some(id), true),
        AliasProbe::Unavailable => return (None, false),
        AliasProbe::NoGlyph => true,
        AliasProbe::NotNeeded => false,
    };
    matching_fallback_regular(
        database,
        base_face_id,
        character,
        fallback_attributes(requested_weight, requested_italic),
        interface_face_is_required,
        probe,
    )
}

pub(super) fn matching_fallback_regular(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    attributes: FallbackAttributes,
    interface_face_is_required: bool,
    mut probe: impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> (Option<usvg::fontdb::ID>, bool) {
    #[cfg(not(target_os = "macos"))]
    let _ = interface_face_is_required;
    scan_fallback_faces(
        database,
        base_face_id,
        character,
        attributes,
        |id, glyph| {
            #[cfg(target_os = "macos")]
            if interface_face_is_required
                && database
                    .face(id)
                    .is_some_and(super::super::super::font::is_interface_face)
            {
                return Some(false);
            }
            probe(id, glyph)
        },
    )
}

#[cfg(target_os = "macos")]
pub(super) enum AliasProbe {
    NotNeeded,
    Selected(usvg::fontdb::ID),
    NoGlyph,
    Unavailable,
}

#[cfg(target_os = "macos")]
pub(super) fn system_ui_bold_alias(
    database: &usvg::fontdb::Database,
    base_face: &usvg::fontdb::FaceInfo,
    character: char,
    requested_weight: u16,
    requested_italic: bool,
    probe: &mut impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> AliasProbe {
    let required = requested_interface_fallback(base_face, requested_weight, requested_italic)
        && super::super::super::font::has_native_interface_candidate(database);
    if !required {
        return AliasProbe::NotNeeded;
    }
    let Some(id) = super::super::super::font::bold_alias_id(database) else {
        return AliasProbe::Unavailable;
    };
    match probe(id, character) {
        Some(true) => AliasProbe::Selected(id),
        Some(false) => AliasProbe::NoGlyph,
        None => AliasProbe::Unavailable,
    }
}

#[cfg(target_os = "macos")]
fn requested_interface_fallback(
    base_face: &usvg::fontdb::FaceInfo,
    requested_weight: u16,
    requested_italic: bool,
) -> bool {
    requested_weight == INTERFACE_BOLD_WEIGHT
        && !requested_italic
        && super::super::super::font::is_system_ui_primary(base_face)
}
