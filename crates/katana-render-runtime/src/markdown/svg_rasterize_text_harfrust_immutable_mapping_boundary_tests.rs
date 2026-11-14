use super as mapping;
use super::super::super::super::font::FontSourceGeneration;
use super::super::FontFaceCacheScope;
use super::super::entry::load_file_font;
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use resvg::usvg::fontdb::{Database, ID, Source};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

const SYSTEM_FONT: &str = "/System/Library/Fonts/Helvetica.ttc";
const OVERSIZED_SOURCE_BYTES: u64 = 64 * 1024 * 1024 + 1;

struct SystemFont {
    database: Database,
    face: ID,
    path: PathBuf,
    index: u32,
    length: u64,
}

impl SystemFont {
    fn cache_key(&self, generation: &FontSourceGeneration) -> mapping::CacheKey {
        mapping::CacheKey::File {
            path: self.path.clone(),
            generation: generation.clone(),
            face_index: self.index,
        }
    }

    fn load(
        &self,
        length: u64,
        generation: FontSourceGeneration,
    ) -> Result<Option<mapping::FontHandle>, String> {
        mapping::try_load(
            &self.database,
            self.face,
            &self.path,
            self.index,
            self.cache_key(&generation),
            generation,
            length,
        )
    }
}

#[test]
fn immutable_mapping_error_prevents_fallback_to_lazy_file() -> Result<(), String> {
    let source = system_font()?;
    let _scope = FontFaceCacheScope::enter();
    let generation = mapping::font_source_generation_uncached(&source.database, source.face);
    let load = |length| {
        load_file_font(
            &source.database,
            source.face,
            &source.path,
            source.index,
            source.cache_key(&generation),
            generation.clone(),
            length,
        )
    };
    assert!(load(source.length + 1).is_none());
    let valid = load(source.length).ok_or("current system font mapping missing")?;
    assert!(valid.clean(&source.database, source.face));
    Ok(())
}

#[test]
fn immutable_mapping_checks_the_actual_system_font_length() -> Result<(), String> {
    let source = system_font()?;
    let _scope = FontFaceCacheScope::enter();
    let generation = mapping::font_source_generation_uncached(&source.database, source.face);
    let loaded = source
        .load(source.length, generation.clone())?
        .ok_or("system mapping missing")?;
    assert!(loaded.clean(&source.database, source.face));
    assert_eq!(loaded.retained_source_bytes, source.length as usize);
    let changed_length = source.load(source.length + 1, generation);
    assert!(changed_length.is_err_and(|error| error.contains("mapping length changed")));
    Ok(())
}

#[test]
fn immutable_mapping_rejects_an_unavailable_source_generation() -> Result<(), String> {
    let source = system_font()?;
    let _scope = FontFaceCacheScope::enter();
    let unavailable = mapping::font_source_generation_uncached(&Database::new(), source.face);
    let current = mapping::font_source_generation_uncached(&source.database, source.face);
    assert_ne!(unavailable, current);
    let result = source.load(source.length, unavailable);
    assert!(result.is_err_and(|error| error.contains("font generation changed")));
    Ok(())
}

#[test]
fn immutable_mapping_declines_mismatched_path_and_index() -> Result<(), String> {
    let source = system_font()?;
    for (path, index) in [
        (
            source.path.with_file_name("different-font.ttf"),
            source.index,
        ),
        (source.path.clone(), source.index + 1),
    ] {
        assert!(!mapping::eligible_source(
            &source.database,
            source.face,
            &path,
            index,
            source.length,
        )?);
    }
    Ok(())
}

#[test]
fn immutable_mapping_declines_binary_and_removed_sources() -> Result<(), String> {
    let mut source = system_font()?;
    let mut binary = Database::new();
    let faces = binary.load_font_source(Source::Binary(Arc::new(FONT_BYTES.to_vec())));
    let face = *faces.first().ok_or("binary fixture face is missing")?;
    assert!(!mapping::eligible_source(
        &binary,
        face,
        &source.path,
        source.index,
        source.length
    )?);
    source.database.remove_face(source.face);
    assert!(!mapping::eligible_source(
        &source.database,
        source.face,
        &source.path,
        source.index,
        source.length,
    )?);
    Ok(())
}

#[test]
fn immutable_mapping_rejects_oversize_alias_and_absent_system_paths() -> Result<(), String> {
    let source = system_font()?;
    assert!(!mapping::eligible_path(
        &source.path,
        OVERSIZED_SOURCE_BYTES
    )?);
    let alias = Path::new("/System/Library/Fonts/../Fonts/Helvetica.ttc");
    assert!(!mapping::eligible_path(alias, source.length)?);
    let missing = PathBuf::from(format!(
        "/System/Library/Fonts/krr-immutable-absent-{}.ttf",
        std::process::id(),
    ));
    assert!(!missing.exists());
    assert!(mapping::eligible_path(&missing, source.length).is_err());
    Ok(())
}

#[test]
fn filesystem_error_conversion_preserves_io_and_nix_display_messages() {
    let io_error = std::io::Error::new(std::io::ErrorKind::InvalidInput, "fixture I/O failure");
    assert_eq!(
        mapping::filesystem_error_to_string(io_error),
        "fixture I/O failure"
    );

    let nix_error = nix::errno::Errno::EINVAL;
    assert_eq!(
        mapping::filesystem_error_to_string(nix_error),
        "EINVAL: Invalid argument"
    );
}

fn system_font() -> Result<SystemFont, String> {
    let path = Path::new(SYSTEM_FONT)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut database = Database::new();
    database.load_font_source(Source::File(path.clone()));
    let (face, index) = database
        .faces()
        .find(|face| face.index == 0)
        .map(|face| (face.id, face.index))
        .ok_or("system font fixture face is missing")?;
    let length = std::fs::metadata(&path)
        .map_err(|error| error.to_string())?
        .len();
    assert!(mapping::eligible_source(
        &database, face, &path, index, length
    )?);
    Ok(SystemFont {
        database,
        face,
        path,
        index,
        length,
    })
}
