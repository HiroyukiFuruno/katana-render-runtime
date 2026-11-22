use objc2_core_foundation::{CFRetained, CFType, CFURL};
use objc2_core_text::{CTFont, CTFontTableOptions, kCTFontTableName, kCTFontURLAttribute};
use resvg::usvg::fontdb::{Database, ID, Source};
use rustybuzz::ttf_parser::{self, name_id};
use std::{io::Read, path::PathBuf};

const MAX_POSTSCRIPT_NAME_BYTES: usize = 63;
const MAX_FONT_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_FONT_TABLE_BYTES: usize = 1024 * 1024;
const MAX_COLLECTION_FACES: u32 = 128;

#[derive(Clone)]
pub(in crate::markdown::svg_rasterize) struct ActualCoreTextFontMetadata {
    pub(in crate::markdown::svg_rasterize) canonical_path: PathBuf,
    pub(in crate::markdown::svg_rasterize) post_script_name: String,
}

pub(in crate::markdown::svg_rasterize) struct ActualCoreTextFace {
    pub(in crate::markdown::svg_rasterize) canonical_path: PathBuf,
    pub(in crate::markdown::svg_rasterize) face_index: u32,
    pub(in crate::markdown::svg_rasterize) post_script_name: String,
}

pub(in crate::markdown::svg_rasterize) fn actual_font_metadata(
    font: &CTFont,
) -> Option<ActualCoreTextFontMetadata> {
    Some(ActualCoreTextFontMetadata {
        canonical_path: canonical_file_path(font)?,
        post_script_name: coretext_postscript_name(font)?,
    })
}

pub(in crate::markdown::svg_rasterize) fn resolve_actual_face(
    font: &CTFont,
) -> Option<ActualCoreTextFace> {
    let metadata = actual_font_metadata(font)?;
    let bytes = read_font_bytes(&metadata.canonical_path)?;
    let face_index = unique_postscript_index(&bytes, &metadata.post_script_name)
        .or_else(|| unique_name_table_index(&bytes, font))?;
    Some(ActualCoreTextFace {
        canonical_path: metadata.canonical_path,
        face_index,
        post_script_name: metadata.post_script_name,
    })
}

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
    match (std::fs::canonicalize(path), canonical_file_path(font)) {
        (Ok(left), Some(right)) => left == right,
        _ => false,
    }
}

fn canonical_file_path(font: &CTFont) -> Option<PathBuf> {
    let descriptor = unsafe { font.font_descriptor() };
    let attribute = unsafe { descriptor.attribute(kCTFontURLAttribute) };
    let path = file_path_from_url_attribute(attribute)?;
    std::fs::canonicalize(path).ok()
}

fn file_path_from_url_attribute(attribute: Option<CFRetained<CFType>>) -> Option<PathBuf> {
    attribute?.downcast::<CFURL>().ok()?.to_file_path()
}

fn read_font_bytes(path: &std::path::Path) -> Option<Vec<u8>> {
    let length = usize::try_from(std::fs::metadata(path).ok()?.len()).ok()?;
    (length <= MAX_FONT_FILE_BYTES).then_some(())?;
    let mut file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::with_capacity(length);
    file.by_ref()
        .take((MAX_FONT_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= MAX_FONT_FILE_BYTES).then_some(bytes)
}

fn unique_postscript_index(bytes: &[u8], name: &str) -> Option<u32> {
    let count = collection_face_count(bytes)?;
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

fn unique_name_table_index(bytes: &[u8], font: &CTFont) -> Option<u32> {
    let table = unsafe { font.table(kCTFontTableName, CTFontTableOptions::empty()) }?;
    (table.len() <= MAX_FONT_TABLE_BYTES).then_some(())?;
    let table = table.to_vec();
    let mut matched = None;
    for index in 0..collection_face_count(bytes)? {
        let face = ttf_parser::Face::parse(bytes, index).ok()?;
        if face.raw_face().table(ttf_parser::Tag::from_bytes(b"name")) != Some(table.as_slice()) {
            continue;
        }
        if matched.replace(index).is_some() {
            return None;
        }
    }
    matched
}

fn collection_face_count(bytes: &[u8]) -> Option<u32> {
    let count = ttf_parser::fonts_in_collection(bytes).unwrap_or(1);
    (count <= MAX_COLLECTION_FACES).then_some(count)
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
    validated_postscript_name(name)
}

fn coretext_postscript_name(font: &CTFont) -> Option<String> {
    validated_postscript_name(unsafe { font.post_script_name() }.to_string())
}

fn validated_postscript_name(name: String) -> Option<String> {
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
