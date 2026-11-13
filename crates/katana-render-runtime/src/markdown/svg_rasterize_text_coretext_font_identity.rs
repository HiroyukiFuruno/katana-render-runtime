use objc2_core_foundation::{CFRetained, CFType, CFURL};
use objc2_core_text::{CTFont, kCTFontURLAttribute};
use resvg::usvg::fontdb::{Database, ID, Source};
use rustybuzz::ttf_parser::{self, name_id};

const MAX_POSTSCRIPT_NAME_BYTES: usize = 63;

pub(super) fn matches(database: &Database, id: ID, font: &CTFont) -> bool {
    let Some(face) = database.face(id) else {
        return false;
    };
    let path = match &face.source {
        Source::File(path) | Source::SharedFile(path, _) => path,
        Source::Binary(_) => return false,
    };
    let before = super::super::font_source_generation_uncached(database, id);
    if !before.durable_reusable() || !same_file(font, path) {
        return false;
    }
    let same_face = if let Some(cached) =
        super::super::super::fallback_memo::physical_face_lookup(database, id, &before)
    {
        cached
    } else {
        database
            .with_face_data(id, |bytes, index| {
                unique_postscript_index(bytes, &face.post_script_name) == Some(index)
            })
            .unwrap_or(false)
    };
    let after = super::super::font_source_generation_uncached(database, id);
    if before != after {
        return false;
    }
    super::super::super::fallback_memo::physical_face_store(database, id, after, same_face);
    same_face
}

fn same_file(font: &CTFont, path: &std::path::Path) -> bool {
    /* WHY: CoreText descriptor が指す file と DB source の物理 path を照合する。 */
    let descriptor = unsafe { font.font_descriptor() };
    let attribute = unsafe { descriptor.attribute(kCTFontURLAttribute) };
    let Some(coretext_path) = file_path_from_url_attribute(attribute) else {
        return false;
    };
    match (
        std::fs::canonicalize(path),
        std::fs::canonicalize(coretext_path),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn file_path_from_url_attribute(
    attribute: Option<CFRetained<CFType>>,
) -> Option<std::path::PathBuf> {
    attribute?.downcast::<CFURL>().ok()?.to_file_path()
}

fn unique_postscript_index(bytes: &[u8], name: &str) -> Option<u32> {
    let count = ttf_parser::fonts_in_collection(bytes).unwrap_or(1);
    let mut matched = None;
    for index in 0..count {
        let face = ttf_parser::Face::parse(bytes, index).ok()?;
        let names = face
            .names()
            .into_iter()
            .filter(|item| item.name_id == name_id::POST_SCRIPT_NAME)
            .map(postscript_name)
            .collect::<Option<Vec<_>>>()?;
        if names.is_empty() {
            return None;
        }
        if names.iter().any(|candidate| candidate == name) {
            if matched.is_some() {
                return None;
            }
            matched = Some(index);
        }
    }
    matched
}

fn postscript_name(item: ttf_parser::name::Name<'_>) -> Option<String> {
    let name = if let Some(name) = item.to_string() {
        name
    } else if item.platform_id == ttf_parser::PlatformId::Macintosh
        && item.encoding_id == 0
        && item.name.is_ascii()
    {
        String::from_utf8_lossy(item.name).into_owned()
    } else {
        return None;
    };
    if name.is_empty()
        || name.len() > MAX_POSTSCRIPT_NAME_BYTES
        || !name
            .bytes()
            .all(|byte| (b'!'..=b'~').contains(&byte) && !b"[](){}<>/%".contains(&byte))
    {
        return None;
    }
    Some(name)
}

#[cfg(test)]
#[path = "svg_rasterize_text_coretext_font_identity_tests.rs"]
mod tests;

#[cfg(test)]
fn mac_roman_name(name: &[u8]) -> ttf_parser::name::Name<'_> {
    ttf_parser::name::Name {
        platform_id: ttf_parser::PlatformId::Macintosh,
        encoding_id: 0,
        language_id: 0,
        name_id: name_id::POST_SCRIPT_NAME,
        name,
    }
}
