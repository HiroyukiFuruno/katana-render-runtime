use super::super::font::{FontSourceGeneration, font_source_generation};
use resvg::usvg::fontdb::{Database, ID, Source};
use std::sync::Arc;

pub(super) fn has_char(database: &Arc<Database>, face_id: ID, character: char) -> Result<bool, ()> {
    let Some((Source::File(_), face_index)) = database.face_source(face_id) else {
        return stock_probe(database, face_id, character);
    };
    if !super::super::font::memo_usable() {
        return Err(());
    }
    probe_file(database, face_id, face_index, character)
}

fn probe_file(
    database: &Arc<Database>,
    face_id: ID,
    face_index: u32,
    character: char,
) -> Result<bool, ()> {
    let generation = font_source_generation(database, face_id);
    if !generation.is_file() || !generation.reusable() {
        super::storage::remove_face(database, face_id, face_index);
        return Err(());
    }
    if let Some(cached) = super::storage::lookup(database, face_id, face_index, &generation)? {
        return cached_result(database, face_id, face_index, cached, character);
    }
    load_file_mapping(database, face_id, face_index, &generation, character)
}

fn cached_result(
    database: &Arc<Database>,
    face_id: ID,
    face_index: u32,
    cached: super::storage::CachedCmap,
    character: char,
) -> Result<bool, ()> {
    match super::sfnt::has_character(&cached.cmap_sfnt, cached.mapping, character) {
        Some(supported) => Ok(supported),
        None => {
            super::storage::remove_face(database, face_id, face_index);
            Err(())
        }
    }
}

fn load_file_mapping(
    database: &Arc<Database>,
    face_id: ID,
    face_index: u32,
    generation: &FontSourceGeneration,
    character: char,
) -> Result<bool, ()> {
    let loaded = read_file_mapping(database, face_id, generation)?;
    let supported =
        super::sfnt::has_character(&loaded.cmap_sfnt, loaded.mapping, character).ok_or(())?;
    if !super::storage::insert(
        database,
        face_id,
        face_index,
        generation.clone(),
        loaded.cmap_sfnt,
        loaded.mapping,
    ) {
        return Err(());
    }
    Ok(supported)
}

fn read_file_mapping(
    database: &Arc<Database>,
    face_id: ID,
    generation: &FontSourceGeneration,
) -> Result<super::sfnt::CachedMapping, ()> {
    validate_file_generation(database, face_id, generation)?;
    let loaded = database
        .with_face_data(face_id, super::sfnt::read_mapping)
        .flatten()
        .ok_or(())?;
    validate_file_generation(database, face_id, generation)?;
    Ok(loaded)
}

fn validate_file_generation(
    database: &Database,
    face_id: ID,
    generation: &FontSourceGeneration,
) -> Result<(), ()> {
    let after = super::super::font::font_source_generation_uncached(database, face_id);
    if after != *generation {
        return Err(());
    }
    Ok(())
}

fn stock_probe(database: &Arc<Database>, face_id: ID, character: char) -> Result<bool, ()> {
    Ok(database
        .with_face_data(face_id, |data, index| {
            let font = skrifa::FontRef::from_index(data, index).ok()?;
            Some(
                skrifa::charmap::Charmap::new(&font)
                    .map(character)
                    .is_some(),
            )
        })
        .flatten()
        .unwrap_or(false))
}

#[cfg(test)]
#[path = "svg_rasterize_text_usvg_cmap_probe_tests.rs"]
mod tests;
