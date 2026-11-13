use resvg::usvg::fontdb::{Database, FaceInfo, ID, Language, Source, Style};
use std::path::{Path, PathBuf};

pub(in crate::markdown::svg_rasterize) const INTERNAL_INTERFACE_BOLD_FAMILY: &str =
    "__krr_internal_system_ui_interface_bold_700";
const INTERFACE_BOLD_POSTSCRIPT: &str = ".HiraKakuInterface-W6";
const INTERFACE_SAMPLE: &str = "装飾面";
const CORETEXT_INTERFACE_POINT_SIZE: f64 = 12.0;

pub(in crate::markdown::svg_rasterize) fn is_internal_family(family: &str) -> bool {
    family.eq_ignore_ascii_case(INTERNAL_INTERFACE_BOLD_FAMILY)
}

pub(in crate::markdown::svg_rasterize) fn register_interface_bold(
    database: &mut Database,
) -> Option<ID> {
    if !has_native_interface_candidate(database) {
        return None;
    }
    register_coretext_identity(
        database,
        coretext_interface_identity(objc2_core_text::CTFontUIFontType::EmphasizedSystem)?,
    )
}

pub(super) fn register_coretext_identity(
    database: &mut Database,
    (postscript, path, bold): (String, PathBuf, bool),
) -> Option<ID> {
    if postscript != INTERFACE_BOLD_POSTSCRIPT || !bold || !native_system_font_path(&path) {
        return None;
    }
    let path = super::canonical_font_source_path(&path).ok()?;
    let matches = matching_native_faces(database, &path, &postscript);
    if matches.len() != 1 {
        return None;
    }
    let candidate = matches[0].to_owned();
    drop(matches);
    register_unique_alias(database, &candidate)
}

pub(in crate::markdown::svg_rasterize) fn bold_alias_id(database: &Database) -> Option<ID> {
    let aliases = database
        .faces()
        .filter(|face| {
            face.families
                .iter()
                .any(|(name, _)| is_internal_family(name))
        })
        .collect::<Vec<_>>();
    let [face] = aliases.as_slice() else {
        return None;
    };
    (face.post_script_name == INTERFACE_BOLD_POSTSCRIPT
        && face.style == Style::Normal
        && interface_family(face)
        && native_face_source_path(&face.source).is_some())
    .then_some(face.id)
}

pub(in crate::markdown::svg_rasterize) fn is_system_ui_primary(face: &FaceInfo) -> bool {
    face.post_script_name
        .trim_start_matches('.')
        .starts_with("SFNS")
        && face
            .families
            .iter()
            .any(|(name, _)| matches!(name.to_ascii_lowercase().as_str(), ".sf ns" | "system font"))
}

pub(in crate::markdown::svg_rasterize) fn has_native_interface_candidate(
    database: &Database,
) -> bool {
    bold_alias_id(database).is_some()
        || database.faces().any(|face| {
            face.post_script_name == INTERFACE_BOLD_POSTSCRIPT
                && native_face_source_path(&face.source).is_some()
                && interface_family(face)
        })
}

pub(in crate::markdown::svg_rasterize) fn is_interface_face(face: &FaceInfo) -> bool {
    face.post_script_name.starts_with(".HiraKakuInterface-W") && interface_family(face)
}

fn matching_native_faces<'a>(
    database: &'a Database,
    path: &Path,
    postscript: &str,
) -> Vec<&'a FaceInfo> {
    database
        .faces()
        .filter(|face| {
            face.post_script_name == postscript
                && face.style == Style::Normal
                && interface_family(face)
                && native_face_source_path(&face.source)
                    .and_then(|source| super::canonical_font_source_path(source).ok())
                    .is_some_and(|source| source == path)
        })
        .collect()
}

fn register_unique_alias(database: &mut Database, candidate: &FaceInfo) -> Option<ID> {
    let aliases = database
        .faces()
        .filter(|face| {
            face.families
                .iter()
                .any(|(name, _)| is_internal_family(name))
        })
        .map(|face| face.id)
        .collect::<Vec<_>>();
    if aliases.iter().any(|id| *id != candidate.id) || aliases.len() > 1 {
        return None;
    }
    if let Some(alias) = aliases.first() {
        return Some(*alias);
    }
    let mut face = candidate.clone();
    face.families.push((
        INTERNAL_INTERFACE_BOLD_FAMILY.to_string(),
        Language::English_UnitedStates,
    ));
    database.remove_face(candidate.id);
    Some(database.push_face_info(face))
}

fn interface_family(face: &FaceInfo) -> bool {
    face.families.iter().any(|(name, _)| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "hiragino sans" | ".hiragino kaku gothic interface"
        )
    })
}

fn native_face_source_path(source: &Source) -> Option<&Path> {
    match source {
        Source::File(path) | Source::SharedFile(path, _) if native_system_font_path(path) => {
            Some(path)
        }
        _ => None,
    }
}

fn native_system_font_path(path: &Path) -> bool {
    path.to_string_lossy().starts_with("/System/Library/Fonts/")
}

pub(super) fn coretext_interface_identity(
    kind: objc2_core_text::CTFontUIFontType,
) -> Option<(String, PathBuf, bool)> {
    use objc2_core_foundation::{CFRange, CFString};
    use objc2_core_text::CTFont;

    let language = CFString::from_str("ja");
    /* WHY: CoreTextへ公開API指定のUI font種別・日本語language・定数point sizeを渡す。 */
    let base = unsafe {
        CTFont::new_ui_font_for_language(kind, CORETEXT_INTERFACE_POINT_SIZE, Some(&language))
    }?;
    let sample = CFString::from_str(INTERFACE_SAMPLE);
    let range = CFRange {
        location: 0,
        length: sample.length(),
    };
    /* WHY: rangeはsample自身のCFString.length()から得たUTF-16全範囲。 */
    let font = unsafe { base.for_string(&sample, range) };
    coretext_face_identity(&font)
}

pub(super) fn coretext_face_identity(
    font: &objc2_core_text::CTFont,
) -> Option<(String, PathBuf, bool)> {
    use objc2_core_foundation::{CFString, CFURL};
    use objc2_core_text::{CTFontSymbolicTraits, kCTFontNameAttribute, kCTFontURLAttribute};

    /* WHY: CoreText fontから公開descriptorを取得し、その属性を検査する。 */
    let descriptor = unsafe { font.font_descriptor() };
    /* WHY: name属性のCFTypeをCFStringへ型検証してから読む。 */
    let postscript = unsafe { descriptor.attribute(kCTFontNameAttribute) }?
        .downcast::<CFString>()
        .ok()?
        .to_string();
    /* WHY: URL属性のCFTypeをCFURLへ型検証してからfile pathへ変換する。 */
    let path = unsafe { descriptor.attribute(kCTFontURLAttribute) }?
        .downcast::<CFURL>()
        .ok()?
        .to_file_path()?;
    /* WHY: この生存中CTFontのsymbolic traitsだけを読み取る。 */
    let bold = unsafe { font.symbolic_traits() }.contains(CTFontSymbolicTraits::BoldTrait);
    Some((postscript, path, bold))
}
