use super::super::super::super::super::font_file_fixture::{self, FontFileFixture};
use super::super::super::proof::CreationProof;
use resvg::usvg::fontdb::Database;
use std::{path::Path, sync::Arc};

const FONT_BYTES: &[u8] = font_file_fixture::FONT_BYTES;

pub(super) type TransientDatabaseFixture = (Arc<Database>, Arc<CreationProof>, FontFileFixture);

pub(super) fn binary_database() -> Arc<Database> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    Arc::new(database)
}

pub(super) fn transient_database(root: &Path) -> Result<TransientDatabaseFixture, String> {
    let mut fixture = font_file_fixture::create(root.join("font.ttf"), FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let path_text = fixture
        .path
        .to_str()
        .ok_or("fixture font path is not UTF-8")?;
    let mut proof = CreationProof::default();
    proof.load(&mut fixture.database, &[path_text]);
    proof.capture_prefix(&fixture.database);
    let original = Arc::new(fixture.database.clone());
    let (shared, shared_proof) = Arc::new(proof)
        .transient(&original)
        .ok_or("transient shared database was unavailable")?;
    Ok((shared, shared_proof, fixture))
}
