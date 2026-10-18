use std::{path::Path, sync::Arc};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;
type FreshWriter = fn(&Path, &[u8]) -> ProbeResult<()>;

#[test]
fn shared_file_source_records_real_path_and_face_index() -> ProbeResult<()> {
    let directory = super::probe_test_support::TempProbeDirectory::create()?;
    let path = directory.file("shared-noto.ttf");
    std::fs::write(&path, FONT_BYTES)?;
    let path = path.canonicalize()?;
    let source =
        resvg::usvg::fontdb::Source::SharedFile(path.clone(), Arc::new(FONT_BYTES.to_vec()));
    let (face, database) = load_shared_noto(source)?;
    assert_shared_source_record(&face.source, &path)?;
    let index = database
        .with_face_data(face.id, |_, index| index)
        .ok_or("face data missing")?;
    assert_eq!(index, face.index);
    assert_eq!(std::fs::read(path)?, FONT_BYTES);
    Ok(())
}

fn load_shared_noto(
    source: resvg::usvg::fontdb::Source,
) -> ProbeResult<(resvg::usvg::fontdb::FaceInfo, resvg::usvg::fontdb::Database)> {
    let mut database = resvg::usvg::fontdb::Database::new();
    let ids = database.load_font_source(source);
    assert_eq!(ids.len(), 1);
    let face = database
        .face(ids[0])
        .ok_or("shared Noto face missing")?
        .clone();
    assert_eq!(face.index, 0);
    Ok((face, database))
}

fn assert_shared_source_record(
    source: &resvg::usvg::fontdb::Source,
    path: &Path,
) -> ProbeResult<()> {
    let record = super::bbox_probe::source_record(source)?;
    assert_eq!(record["kind"], "SharedFile");
    assert_eq!(record["path"].as_str(), path.to_str());
    assert_eq!(record["len"].as_u64(), Some(FONT_BYTES.len() as u64));
    Ok(())
}

#[test]
fn bbox_write_fresh_checks_output_contract() -> ProbeResult<()> {
    assert_write_contract(super::bbox_probe::write_fresh)
}

#[test]
fn html_png_write_fresh_checks_output_contract() -> ProbeResult<()> {
    assert_write_contract(super::html_png_probe::write_fresh)
}

fn assert_write_contract(writer: FreshWriter) -> ProbeResult<()> {
    let directory = super::probe_test_support::TempProbeDirectory::create()?;
    assert_fresh_write(writer, &directory)?;
    assert_occupied_write_is_rejected(writer, &directory)?;
    assert_outside_tmp_write_is_rejected(writer)?;
    Ok(())
}

fn assert_fresh_write(
    writer: FreshWriter,
    directory: &super::probe_test_support::TempProbeDirectory,
) -> ProbeResult<()> {
    let path = directory.file("fresh-output.bin");
    writer(&path, b"fresh bytes")?;
    assert_eq!(std::fs::read(path)?, b"fresh bytes");
    Ok(())
}

fn assert_occupied_write_is_rejected(
    writer: FreshWriter,
    directory: &super::probe_test_support::TempProbeDirectory,
) -> ProbeResult<()> {
    let path = directory.file("occupied-output.bin");
    std::fs::write(&path, b"original bytes")?;
    assert!(writer(&path, b"replacement bytes").is_err());
    assert_eq!(std::fs::read(path)?, b"original bytes");
    Ok(())
}

fn assert_outside_tmp_write_is_rejected(writer: FreshWriter) -> ProbeResult<()> {
    let external_parent = std::env::temp_dir().canonicalize()?;
    let workspace_tmp = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace root missing")?
        .join("tmp")
        .canonicalize()?;
    assert!(!external_parent.starts_with(workspace_tmp));
    let path = external_parent.join(format!("krr-probe-outside-{}.bin", std::process::id()));
    assert!(!path.exists(), "unique external output path is occupied");
    let result = writer(&path, b"must not be written");
    let created = path.exists();
    if created {
        std::fs::remove_file(&path)?;
    }
    assert!(result.is_err());
    assert!(!created, "rejected write created an external file");
    Ok(())
}
