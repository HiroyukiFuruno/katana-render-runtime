use std::{env, fs, io, io::Read, path::PathBuf};

const ARCHIVE: &[u8] = include_bytes!("src/markdown/generated/zenuml-runtime-assets.bin.br");
const OUTPUT_FILENAME: &str = "mermaid-zenuml.min.js";
const BROTLI_BUFFER_SIZE: usize = 4096;

mod archive_index {
    include!("src/markdown/generated/zenuml-runtime-assets-index.rs");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/markdown/generated/zenuml-runtime-assets.bin.br");
    println!("cargo:rerun-if-changed=src/markdown/generated/zenuml-runtime-assets-index.rs");
    if PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/svg-engine/upstream")
        .is_dir()
    {
        println!("cargo:rerun-if-changed=../../tests/fixtures/svg-engine/upstream");
    }

    let mut decompressor = brotli_decompressor::Decompressor::new(ARCHIVE, BROTLI_BUFFER_SIZE);
    let mut archive = Vec::new();
    decompressor.read_to_end(&mut archive)?;
    validate_archive_length(&archive)?;

    let end = archive_index::MERMAID_ZENUML_ASSET_OFFSET
        .checked_add(archive_index::MERMAID_ZENUML_ASSET_LENGTH)
        .ok_or_else(|| io::Error::other("Mermaid ZenUML asset offset overflow"))?;
    let bytes = archive
        .get(archive_index::MERMAID_ZENUML_ASSET_OFFSET..end)
        .ok_or_else(|| io::Error::other("Mermaid ZenUML asset range is out of bounds"))?;
    let source = std::str::from_utf8(bytes)?;
    let output_dir = env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("OUT_DIR is not set"))?;
    fs::write(output_dir.join(OUTPUT_FILENAME), source)?;
    Ok(())
}

fn validate_archive_length(archive: &[u8]) -> io::Result<()> {
    if archive.len() == archive_index::ZENUML_RUNTIME_ASSETS_UNCOMPRESSED_LENGTH {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "ZenUML runtime asset archive length mismatch: expected {}, got {}",
        archive_index::ZENUML_RUNTIME_ASSETS_UNCOMPRESSED_LENGTH,
        archive.len()
    )))
}
